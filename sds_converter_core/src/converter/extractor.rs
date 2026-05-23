use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::error::SdsError;

/// Default maximum characters to send to the LLM — consistent with ConvertConfig::default().
const DEFAULT_MAX_LLM_CHARS: usize = 80_000;

const MAX_BINARY_INPUT_BYTES: u64 = 500 * 1024 * 1024; // 500 MB for binary formats
const MAX_TEXT_INPUT_BYTES: u64 = 100 * 1024 * 1024; // 100 MB for text formats

/// Character count below which we assume a PDF is image-only and attempt OCR.
const OCR_FALLBACK_THRESHOLD: usize = 200;

pub enum InputFormat {
    Pdf,
    Docx,
    Txt,
    Xlsx,
    Html,
    Url,
}

pub fn detect_format(path: &Path) -> Result<InputFormat, SdsError> {
    detect_format_str(
        path.to_str()
            .ok_or_else(|| SdsError::UnsupportedFormat("(invalid path)".to_string()))?,
    )
}

/// Detect input format from a file path or URL string.
pub fn detect_format_str(input: &str) -> Result<InputFormat, SdsError> {
    if input.starts_with("http://") || input.starts_with("https://") {
        return Ok(InputFormat::Url);
    }
    let ext = std::path::Path::new(input)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());
    match ext.as_deref() {
        Some("pdf") => Ok(InputFormat::Pdf),
        Some("docx") => Ok(InputFormat::Docx),
        Some("txt") => Ok(InputFormat::Txt),
        Some("xlsx") | Some("xls") | Some("xlsm") => Ok(InputFormat::Xlsx),
        Some("html") | Some("htm") => Ok(InputFormat::Html),
        Some(e) => Err(SdsError::UnsupportedFormat(e.to_string())),
        None => Err(SdsError::UnsupportedFormat("(no extension)".to_string())),
    }
}

pub async fn extract_text(path: &Path) -> Result<String, SdsError> {
    extract_text_limited(path, DEFAULT_MAX_LLM_CHARS).await
}

/// Extract text from a URL (fetches HTML and strips tags).
pub async fn extract_text_from_url(url: &str) -> Result<String, SdsError> {
    extract_text_from_url_limited(url, DEFAULT_MAX_LLM_CHARS).await
}

/// Like [`extract_text_from_url`] but truncates to `max_chars` after cleaning.
pub async fn extract_text_from_url_limited(url: &str, max_chars: usize) -> Result<String, SdsError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| SdsError::Extract(e.to_string()))?;
    let html = client.get(url)
        .send()
        .await
        .map_err(|e| SdsError::Extract(format!("HTTP GET failed: {e}")))?
        .text()
        .await
        .map_err(|e| SdsError::Extract(format!("response body failed: {e}")))?;
    let raw = extract_text_from_html_str(&html);
    Ok(clean_extracted_text(&raw, max_chars))
}

/// Like [`extract_text`] but truncates to `max_chars` after cleaning.
pub async fn extract_text_limited(path: &Path, max_chars: usize) -> Result<String, SdsError> {
    let input_format = detect_format(path)?;
    let size_limit = match &input_format {
        InputFormat::Txt | InputFormat::Html => MAX_TEXT_INPUT_BYTES,
        _ => MAX_BINARY_INPUT_BYTES,
    };
    let file_size = std::fs::metadata(path)
        .map_err(|e| SdsError::Extract(format!("file stat failed: {e}")))?
        .len();
    if file_size > size_limit {
        return Err(SdsError::Extract(format!(
            "input file too large ({} bytes, limit {} MB)",
            file_size,
            size_limit / 1024 / 1024
        )));
    }
    let raw = match input_format {
        InputFormat::Pdf => {
            let path_a = path.to_path_buf();
            let path_b = path.to_path_buf();

            // Try text-based extraction first.
            let raw = tokio::task::spawn_blocking(move || {
                pdf_extract::extract_text(&path_a).map_err(|e| SdsError::Extract(e.to_string()))
            })
            .await
            .unwrap_or_else(|e| Err(SdsError::Extract(e.to_string())))
            .unwrap_or_default(); // treat extraction failure as empty → triggers OCR

            if raw.trim().chars().count() >= OCR_FALLBACK_THRESHOLD {
                raw
            } else {
                // Sparse text: likely a scanned PDF — attempt OCR fallback.
                let ocr = tokio::task::spawn_blocking(move || ocr_pdf_with_tesseract(&path_b))
                    .await
                    .unwrap_or_else(|e| Err(SdsError::Extract(e.to_string())));

                match ocr {
                    Ok(text) if !text.trim().is_empty() => text,
                    Err(e) if raw.trim().is_empty() => {
                        // Nothing at all — surface the OCR error so the user
                        // knows they need to install tesseract / poppler.
                        return Err(e);
                    }
                    Err(e) => {
                        tracing::info!("OCR fallback unavailable ({e}); using sparse PDF text");
                        raw
                    }
                    Ok(_) => raw, // OCR returned empty, keep original sparse text
                }
            }
        }
        InputFormat::Docx => {
            let path = path.to_path_buf();
            tokio::task::spawn_blocking(move || extract_text_from_docx(&path))
                .await
                .unwrap_or_else(|e| Err(SdsError::Extract(e.to_string())))?
        }
        InputFormat::Txt => {
            let path = path.to_path_buf();
            tokio::task::spawn_blocking(move || {
                std::fs::read_to_string(&path).map_err(|e| SdsError::Extract(e.to_string()))
            })
            .await
            .unwrap_or_else(|e| Err(SdsError::Extract(e.to_string())))?
        }
        InputFormat::Xlsx => {
            let path = path.to_path_buf();
            tokio::task::spawn_blocking(move || extract_text_from_xlsx(&path))
                .await
                .unwrap_or_else(|e| Err(SdsError::Extract(e.to_string())))?
        }
        InputFormat::Html => {
            let path = path.to_path_buf();
            tokio::task::spawn_blocking(move || {
                let html = std::fs::read_to_string(&path)
                    .map_err(|e| SdsError::Extract(e.to_string()))?;
                Ok(extract_text_from_html_str(&html))
            })
            .await
            .unwrap_or_else(|e| Err(SdsError::Extract(e.to_string())))?
        }
        InputFormat::Url => {
            return Err(SdsError::Extract(
                "Use extract_text_from_url() for URL inputs".to_string(),
            ));
        }
    };
    Ok(clean_extracted_text(&raw, max_chars))
}

// ---------------------------------------------------------------------------
// OCR fallback (pdftoppm + tesseract CLI)
// ---------------------------------------------------------------------------

/// Convert every page of a PDF to PNG with pdftoppm, then OCR with tesseract.
///
/// Returns `Err` with an install hint if either tool is absent.
/// Returns `Ok("")` only when tesseract ran but produced no text.
fn ocr_pdf_with_tesseract(pdf_path: &Path) -> Result<String, SdsError> {
    use std::path::PathBuf;

    let tmp = tempfile::tempdir()
        .map_err(|e| SdsError::Extract(format!("OCR tmpdir: {e}")))?;

    let page_prefix = tmp.path().join("page");

    // Step 1 — rasterise PDF pages to PNG at 300 dpi.
    let status = std::process::Command::new("pdftoppm")
        .args([
            "-r", "300",
            "-png",
            pdf_path.to_str().unwrap_or(""),
            page_prefix.to_str().unwrap_or(""),
        ])
        .status()
        .map_err(|e| SdsError::Extract(format!(
            "pdftoppm not found ({e}). \
             Install poppler: `brew install poppler` / `apt install poppler-utils` / \
             https://github.com/oschwartz10612/poppler-windows/releases"
        )))?;

    if !status.success() {
        return Err(SdsError::Extract(format!("pdftoppm exited with {status}")));
    }

    // Step 2 — collect PNG files in page order.
    let mut pngs: Vec<PathBuf> = std::fs::read_dir(tmp.path())
        .map_err(|e| SdsError::Extract(e.to_string()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| e.eq_ignore_ascii_case("png"))
                .unwrap_or(false)
        })
        .collect();
    pngs.sort();

    if pngs.is_empty() {
        return Err(SdsError::Extract("pdftoppm produced no images".to_string()));
    }

    // Step 3 — OCR each page and concatenate.
    let ocr_stem = tmp.path().join("ocr");
    let mut combined = String::new();

    for png in &pngs {
        // Try jpn+eng first (common for Japanese SDS); fall back to eng-only.
        let ok = try_tesseract(png, &ocr_stem, "jpn+eng")
            .or_else(|_| try_tesseract(png, &ocr_stem, "eng"))
            .is_ok();

        if ok {
            let txt = ocr_stem.with_extension("txt");
            if let Ok(page_text) = std::fs::read_to_string(&txt) {
                combined.push_str(&page_text);
                combined.push('\n');
            }
        }
    }

    // tmp dir is cleaned up on drop.
    Ok(combined)
}

fn try_tesseract(input: &Path, output_stem: &Path, lang: &str) -> Result<(), SdsError> {
    let status = std::process::Command::new("tesseract")
        .arg(input.to_str().unwrap_or(""))
        .arg(output_stem.to_str().unwrap_or(""))
        .args(["-l", lang])
        .status()
        .map_err(|e| SdsError::Extract(format!(
            "tesseract not found ({e}). \
             Install: `brew install tesseract tesseract-lang` / \
             `apt install tesseract-ocr tesseract-ocr-jpn` / \
             https://github.com/UB-Mannheim/tesseract/wiki"
        )))?;

    if !status.success() {
        return Err(SdsError::Extract(format!(
            "tesseract exited with {status} (lang={lang}; \
             ensure the language pack is installed)"
        )));
    }
    Ok(())
}

/// Clean and condense raw extracted text before sending to the LLM.
///
/// Three passes:
///   1. Remove separator lines, collapse blank runs, strip control chars.
///   2. Deduplicate repeated short lines (PDF page headers/footers).
///   3. Truncate to `max_chars` at a UTF-8 char boundary.
pub fn clean_extracted_text(text: &str, max_chars: usize) -> String {
    // Pass 1 — noise removal
    let mut out = String::with_capacity(text.len().min(max_chars + 1024));
    let mut blank_run = 0usize;

    for line in text.lines() {
        let trimmed = line.trim();

        // Drop control characters and zero-width spaces but keep CJK / Latin content
        let trimmed: String = trimmed
            .chars()
            .filter(|&c| c >= ' ' || c == '\t')
            .collect();
        let trimmed = trimmed.trim();

        // Drop lines that are purely visual separators (─━=─-*•· etc.)
        if !trimmed.is_empty()
            && trimmed.chars().all(|c| {
                matches!(c,
                    '-' | '=' | '_' | '*' | '─' | '━' | '╌' | '╍'
                    | '┄' | '┅' | '┈' | '┉' | '╴' | '╶' | '╸'
                    | '·' | '•' | '~' | '/' | '\\' | '|' | '+' | '#'
                )
            })
            && trimmed.chars().count() >= 3
        {
            continue;
        }

        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(trimmed);
            out.push('\n');
        }
    }

    // Pass 2 — deduplicate repeated short lines (page headers / footers)
    // Any line ≤ 80 chars appearing 3+ times is treated as a repeated header/footer.
    {
        let mut freq: HashMap<String, usize> = HashMap::new();
        for line in out.lines() {
            if line.len() <= 80 {
                *freq.entry(line.to_string()).or_default() += 1;
            }
        }
        let mut first_seen: HashSet<String> = HashSet::new();
        let mut deduped = String::with_capacity(out.len());
        for line in out.lines() {
            let count = freq.get(line).copied().unwrap_or(1);
            if line.len() <= 80 && count >= 3 {
                if first_seen.insert(line.to_string()) {
                    deduped.push_str(line);
                    deduped.push('\n');
                }
            } else {
                deduped.push_str(line);
                deduped.push('\n');
            }
        }
        out = deduped;
    }

    // Pass 3 — truncate to max_chars at a valid UTF-8 char boundary
    if out.len() > max_chars {
        let mut at = max_chars;
        while at > 0 && !out.is_char_boundary(at) {
            at -= 1;
        }
        out.truncate(at);
        out.push_str("\n[テキスト省略]\n");
    }

    out
}

pub fn extract_text_from_docx(path: &Path) -> Result<String, SdsError> {
    let docx = docx_rust::DocxFile::from_file(path)
        .map_err(|e| SdsError::Docx(format!("open failed: {e:?}")))?;
    let docx = docx
        .parse()
        .map_err(|e| SdsError::Docx(format!("parse failed: {e:?}")))?;
    Ok(docx.document.body.text())
}

pub fn extract_text_from_xlsx(path: &Path) -> Result<String, SdsError> {
    use calamine::{open_workbook_auto, Reader};
    let mut wb = open_workbook_auto(path)
        .map_err(|e| SdsError::Extract(format!("xlsx open failed: {e}")))?;
    let mut out = String::new();
    for sheet_name in wb.sheet_names().to_owned() {
        if let Ok(range) = wb.worksheet_range(&sheet_name) {
            for row in range.rows() {
                let cells: Vec<String> = row
                    .iter()
                    .map(|c| c.to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !cells.is_empty() {
                    out.push_str(&cells.join("\t"));
                    out.push('\n');
                }
            }
        }
    }
    Ok(out)
}

/// Extract visible text from an HTML string, skipping script/style/nav elements.
/// Table cells are tab-separated; rows are newline-separated.
pub fn extract_text_from_html_str(html: &str) -> String {
    use scraper::{Html, Selector};

    let document = Html::parse_document(html);
    let row_sel = Selector::parse("tr").unwrap();
    let cell_sel = Selector::parse("td, th").unwrap();
    let body_sel = Selector::parse("body").unwrap();

    let body = match document.select(&body_sel).next() {
        Some(b) => b,
        None => return String::new(),
    };

    let mut out = String::new();

    for node in body.children() {
        collect_node_text(
            scraper::ElementRef::wrap(node),
            &row_sel,
            &cell_sel,
            &mut out,
        );
    }

    out
}

fn collect_node_text(
    node: Option<scraper::ElementRef<'_>>,
    row_sel: &scraper::Selector,
    cell_sel: &scraper::Selector,
    out: &mut String,
) {
    let Some(el) = node else { return };
    let tag = el.value().name();

    if tag == "table" {
        for row in el.select(row_sel) {
            let cells: Vec<String> = row
                .select(cell_sel)
                .map(|c| c.text().collect::<String>().trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if !cells.is_empty() {
                out.push_str(&cells.join("\t"));
                out.push('\n');
            }
        }
        return;
    }

    // Skip noise elements
    if matches!(tag, "script" | "style" | "nav" | "header" | "footer" | "noscript") {
        return;
    }

    // For block-like elements emit a newline before and after.
    let is_block = matches!(
        tag,
        "p" | "div" | "section" | "article" | "li" | "dt" | "dd"
            | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
            | "br" | "hr" | "blockquote" | "pre"
    );

    if is_block && !out.ends_with('\n') {
        out.push('\n');
    }

    for child in el.children() {
        if let Some(text) = child.value().as_text() {
            let t = text.trim();
            if !t.is_empty() {
                out.push_str(t);
                out.push(' ');
            }
        } else if let Some(child_el) = scraper::ElementRef::wrap(child) {
            collect_node_text(Some(child_el), row_sel, cell_sel, out);
        }
    }

    if is_block && !out.ends_with('\n') {
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn separator_lines_are_dropped() {
        let input = "Section 1\n---\nContent\n===\nMore content\n";
        let result = clean_extracted_text(input, 1000);
        assert!(!result.contains("---"));
        assert!(!result.contains("==="));
        assert!(result.contains("Section 1"));
        assert!(result.contains("Content"));
    }

    #[test]
    fn multiple_blank_lines_collapse_to_one() {
        let input = "Line A\n\n\n\nLine B\n";
        let result = clean_extracted_text(input, 1000);
        // Should have at most one blank line between A and B
        assert!(!result.contains("\n\n\n"));
        assert!(result.contains("Line A"));
        assert!(result.contains("Line B"));
    }

    #[test]
    fn cjk_content_passes_through() {
        let input = "第1節 化学品の名称\n製品名：テスト化学物質\n";
        let result = clean_extracted_text(input, 1000);
        assert!(result.contains("第1節"));
        assert!(result.contains("テスト化学物質"));
    }

    #[test]
    fn truncation_lands_on_utf8_boundary() {
        let input: String = "あ".repeat(100);
        let result = clean_extracted_text(&input, 10);
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
    }

    #[test]
    fn repeated_header_lines_deduplicated() {
        let header = "Company Inc. SDS";
        let mut input = String::new();
        for i in 0..10 {
            input.push_str(header);
            input.push('\n');
            input.push_str(&format!("Section {i} content\n"));
        }
        let result = clean_extracted_text(&input, 10_000);
        let count = result.matches(header).count();
        assert_eq!(count, 1, "header appeared {count} times, expected 1");
    }

    #[test]
    fn short_non_repeated_lines_kept() {
        let input = "Line A\nLine B\nLine C\n";
        let result = clean_extracted_text(input, 1000);
        assert!(result.contains("Line A"));
        assert!(result.contains("Line B"));
        assert!(result.contains("Line C"));
    }
}
