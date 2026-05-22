use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

use crate::error::SdsError;
use crate::schema::SdsRoot;

/// Fill a Word (.docx) template by replacing `{{FieldName}}` placeholders with
/// values from an [`SdsRoot`]. Placeholders use the leaf field name (e.g.
/// `{{TradeNameJP}}`, `{{CompanyName}}`). Full dot-path keys are also supported
/// (e.g. `{{Identification.SupplierInformation.CompanyName}}`).
///
/// If the same leaf name appears in multiple sections the first non-empty value
/// is used; use the full path form to target a specific occurrence.
pub fn fill_template(
    sds: &SdsRoot,
    template_path: &Path,
    output_path: &Path,
) -> Result<(), SdsError> {
    let values = flatten_sds(sds);

    let template_bytes = std::fs::read(template_path)
        .map_err(|e| SdsError::Extract(format!("template open failed: {e}")))?;

    let filled = fill_docx_bytes(&template_bytes, &values)?;

    std::fs::write(output_path, &filled)
        .map_err(|e| SdsError::Extract(format!("output write failed: {e}")))?;

    Ok(())
}

// ── Internal helpers ─────────────────────────────────────────────────────────

/// Build a flat `key → value` map from SdsRoot.
/// Each leaf string/number/bool gets two entries:
///   - short key  (`TradeNameJP`)
///   - full path  (`Identification.TradeProductIdentity.TradeNameJP`)
/// Vec<String> values are joined with `\n`.
pub(crate) fn flatten_sds(sds: &SdsRoot) -> HashMap<String, String> {
    let v = serde_json::to_value(sds).unwrap_or_default();
    let mut map = HashMap::new();
    flatten_value("", &v, &mut map);
    map
}

fn flatten_value(prefix: &str, value: &serde_json::Value, map: &mut HashMap<String, String>) {
    match value {
        serde_json::Value::Object(obj) => {
            for (k, v) in obj {
                let full = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                match v {
                    serde_json::Value::String(s) if !s.is_empty() => {
                        map.entry(k.clone()).or_insert_with(|| s.clone());
                        map.insert(full, s.clone());
                    }
                    serde_json::Value::Number(n) => {
                        let s = n.to_string();
                        map.entry(k.clone()).or_insert_with(|| s.clone());
                        map.insert(full, s);
                    }
                    serde_json::Value::Bool(b) => {
                        let s = b.to_string();
                        map.entry(k.clone()).or_insert_with(|| s.clone());
                        map.insert(full, s);
                    }
                    serde_json::Value::Array(arr) => {
                        let s: Vec<&str> = arr.iter().filter_map(|v| v.as_str()).collect();
                        if !s.is_empty() {
                            let joined = s.join("\n");
                            map.entry(k.clone()).or_insert_with(|| joined.clone());
                            map.insert(full, joined);
                        }
                    }
                    _ => flatten_value(&full, v, map),
                }
            }
        }
        _ => {}
    }
}

/// Read template .docx bytes, fill placeholders, return filled .docx bytes.
fn fill_docx_bytes(
    template: &[u8],
    values: &HashMap<String, String>,
) -> Result<Vec<u8>, SdsError> {
    use std::io::Cursor;
    use zip::{ZipArchive, ZipWriter, write::SimpleFileOptions};

    let mut archive = ZipArchive::new(Cursor::new(template))
        .map_err(|e| SdsError::Extract(format!("template ZIP read failed: {e}")))?;

    // Collect all entry names first.
    let names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).map(|e| e.name().to_string()))
        .collect::<Result<_, _>>()
        .map_err(|e| SdsError::Extract(format!("ZIP index failed: {e}")))?;

    let mut out = Vec::new();
    let mut writer = ZipWriter::new(Cursor::new(&mut out));

    for name in &names {
        let mut entry = archive
            .by_name(name)
            .map_err(|e| SdsError::Extract(format!("ZIP entry '{name}' read failed: {e}")))?;

        let compression = entry.compression();
        let options = SimpleFileOptions::default().compression_method(compression);

        writer
            .start_file(name, options)
            .map_err(|e| SdsError::Extract(format!("ZIP write '{name}' failed: {e}")))?;

        // Substitute placeholders in XML parts that may contain them.
        if is_content_xml(name) {
            let mut xml = String::new();
            entry
                .read_to_string(&mut xml)
                .map_err(|e| SdsError::Extract(format!("read '{name}' failed: {e}")))?;
            let filled = apply_substitutions(&xml, values);
            writer
                .write_all(filled.as_bytes())
                .map_err(|e| SdsError::Extract(format!("ZIP write '{name}' failed: {e}")))?;
        } else {
            let mut buf = Vec::new();
            entry
                .read_to_end(&mut buf)
                .map_err(|e| SdsError::Extract(format!("read '{name}' failed: {e}")))?;
            writer
                .write_all(&buf)
                .map_err(|e| SdsError::Extract(format!("ZIP write '{name}' failed: {e}")))?;
        }
    }

    writer
        .finish()
        .map_err(|e| SdsError::Extract(format!("ZIP finalize failed: {e}")))?;

    Ok(out)
}

/// Returns true for XML parts that may contain user-authored text with placeholders.
fn is_content_xml(name: &str) -> bool {
    matches!(
        name,
        "word/document.xml"
            | "word/header1.xml"
            | "word/header2.xml"
            | "word/header3.xml"
            | "word/footer1.xml"
            | "word/footer2.xml"
            | "word/footer3.xml"
    ) || (name.starts_with("word/header") && name.ends_with(".xml"))
        || (name.starts_with("word/footer") && name.ends_with(".xml"))
}

/// Normalize split placeholders then replace `{{key}}` with values.
fn apply_substitutions(xml: &str, values: &HashMap<String, String>) -> String {
    let normalized = normalize_split_runs(xml);
    let mut result = normalized;
    for (key, value) in values {
        let placeholder = format!("{{{{{key}}}}}");
        if result.contains(&placeholder) {
            result = result.replace(&placeholder, &escape_xml(value));
        }
    }
    result
}

/// Remove XML run-boundary tags that appear *inside* a `{{...}}` placeholder.
///
/// Word sometimes splits a typed word across multiple `<w:r>` runs (especially
/// after spell-check or autocorrect). This state-machine pass merges such splits
/// so that `{{Trade</w:t></w:r><w:r><w:t>NameJP}}` becomes `{{TradeNameJP}}`.
fn normalize_split_runs(xml: &str) -> String {
    let mut out = String::with_capacity(xml.len());
    let bytes = xml.as_bytes();
    let n = bytes.len();
    let mut i = 0;

    while i < n {
        // Detect `{{` — start of a placeholder.
        if i + 1 < n && bytes[i] == b'{' && bytes[i + 1] == b'{' {
            out.push('{');
            out.push('{');
            i += 2;

            // Collect placeholder text, skipping any XML tags until `}}`.
            loop {
                if i >= n {
                    break;
                }
                if i + 1 < n && bytes[i] == b'}' && bytes[i + 1] == b'}' {
                    out.push('}');
                    out.push('}');
                    i += 2;
                    break;
                }
                if bytes[i] == b'<' {
                    // Skip XML tag (including attributes).
                    while i < n && bytes[i] != b'>' {
                        i += 1;
                    }
                    if i < n {
                        i += 1; // consume '>'
                    }
                } else {
                    out.push(bytes[i] as char);
                    i += 1;
                }
            }
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_no_split() {
        let xml = "<w:t>{{TradeNameJP}}</w:t>";
        assert_eq!(normalize_split_runs(xml), xml);
    }

    #[test]
    fn normalize_split_across_runs() {
        let xml = "<w:t>{{Trade</w:t></w:r><w:r><w:t>NameJP}}</w:t>";
        assert_eq!(normalize_split_runs(xml), "<w:t>{{TradeNameJP}}</w:t>");
    }

    #[test]
    fn normalize_split_with_rpr() {
        let xml = "<w:t>{{Co</w:t></w:r><w:r><w:rPr><w:b/></w:rPr><w:t>mpanyName}}</w:t>";
        assert_eq!(normalize_split_runs(xml), "<w:t>{{CompanyName}}</w:t>");
    }

    #[test]
    fn apply_substitution_basic() {
        let xml = "<w:t>{{CompanyName}}</w:t>";
        let mut values = HashMap::new();
        values.insert("CompanyName".to_string(), "ACME Corp".to_string());
        assert_eq!(apply_substitutions(xml, &values), "<w:t>ACME Corp</w:t>");
    }

    #[test]
    fn apply_substitution_xml_escape() {
        let xml = "<w:t>{{CompanyName}}</w:t>";
        let mut values = HashMap::new();
        values.insert("CompanyName".to_string(), "A & B <Ltd>".to_string());
        assert_eq!(
            apply_substitutions(xml, &values),
            "<w:t>A &amp; B &lt;Ltd&gt;</w:t>"
        );
    }

    #[test]
    fn flatten_sds_partial() {
        use crate::schema::{SdsRoot, Datasheet};
        let sds = SdsRoot {
            datasheet: Some(Datasheet {
                issue_date: Some("2024-01-01".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let map = flatten_sds(&sds);
        assert_eq!(map.get("IssueDate").map(String::as_str), Some("2024-01-01"));
        assert!(map.contains_key("Datasheet.IssueDate"));
    }
}
