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

/// Detect the language of a document by extracting a small text sample and running heuristics.
///
/// Uses the first 5 000 characters — sufficient for language detection without a full extraction.
/// Returns an error only if the file cannot be read or has an unsupported format.
pub async fn detect_language_from_file(path: &Path) -> Result<crate::language::Language, SdsError> {
    let sample = extract_text_limited(path, 5_000).await.unwrap_or_default();
    Ok(crate::language::detect_language(&sample))
}

/// Detect the language of an HTML page fetched from a URL.
pub async fn detect_language_from_url(url: &str) -> Result<crate::language::Language, SdsError> {
    let sample = extract_text_from_url_limited(url, 5_000).await.unwrap_or_default();
    Ok(crate::language::detect_language(&sample))
}

/// Extract text from a URL (fetches HTML and strips tags).
pub async fn extract_text_from_url(url: &str) -> Result<String, SdsError> {
    extract_text_from_url_limited(url, DEFAULT_MAX_LLM_CHARS).await
}

/// Returns `true` for private, loopback, link-local, and metadata IP addresses/hostnames.
fn is_private_host(host: &str) -> bool {
    use std::net::IpAddr;
    // Block numeric IPs that are private/loopback/metadata
    if let Ok(ip) = host.parse::<IpAddr>() {
        return match ip {
            IpAddr::V4(v4) => {
                v4.is_loopback()        // 127.x.x.x
                || v4.is_private()      // 10.x, 172.16-31.x, 192.168.x
                || v4.is_link_local()   // 169.254.x.x (AWS metadata)
                || v4.is_unspecified()  // 0.0.0.0
                || v4.is_broadcast()
            }
            IpAddr::V6(v6) => v6.is_loopback() || v6.is_unspecified(),
        };
    }
    // Block well-known metadata hostnames
    matches!(host,
        "localhost" | "metadata.google.internal" | "instance-data"
    )
}

/// Like [`extract_text_from_url`] but truncates to `max_chars` after cleaning.
pub async fn extract_text_from_url_limited(url: &str, max_chars: usize) -> Result<String, SdsError> {
    // SSRF guard: reject private/loopback/metadata addresses before making a request.
    let parsed = reqwest::Url::parse(url)
        .map_err(|e| SdsError::Extract(format!("Invalid URL: {e}")))?;
    let host = parsed
        .host_str()
        .ok_or_else(|| SdsError::Extract("URL has no host".into()))?;
    if is_private_host(host) {
        return Err(SdsError::Extract(
            "URL points to a private/reserved address".into(),
        ));
    }

    const MAX_BODY_BYTES: usize = 50 * 1024 * 1024; // 50 MB

    let response = shared_http_client()
        .get(url)
        .send()
        .await
        .map_err(|e| SdsError::Extract(format!("HTTP GET failed: {e}")))?;

    // Reject responses whose Content-Length header exceeds the limit.
    if let Some(content_length) = response.content_length() {
        if content_length > MAX_BODY_BYTES as u64 {
            return Err(SdsError::Extract(format!(
                "URL response too large ({} bytes, limit 50 MB)", content_length
            )));
        }
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| SdsError::Extract(format!("response body failed: {e}")))?;
    if bytes.len() > MAX_BODY_BYTES {
        return Err(SdsError::Extract(format!(
            "URL response too large ({} bytes, limit 50 MB)", bytes.len()
        )));
    }
    let html = String::from_utf8_lossy(&bytes).into_owned();
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
                // ② pdftotext fallback — handles CID/Shift-JIS fonts that pdf-extract
                //   cannot decode (it panics or returns garbled bytes).
                //   pdftotext is part of poppler-utils; silently skipped if not installed.
                let path_pt = path_b.clone();
                let pt_text = tokio::task::spawn_blocking(move || pdftotext_fallback(&path_pt))
                    .await
                    .unwrap_or(None);

                if let Some(pt) = pt_text {
                    if pt.trim().chars().count() >= OCR_FALLBACK_THRESHOLD {
                        tracing::debug!(
                            chars = pt.trim().chars().count(),
                            "pdftotext fallback succeeded"
                        );
                        return Ok(clean_extracted_text(&pt, max_chars));
                    }
                }

                // ③ Sparse text: likely a scanned PDF — attempt OCR fallback.
                let ocr = tokio::task::spawn_blocking(move || ocr_pdf_with_tesseract(&path_b))
                    .await
                    .unwrap_or_else(|e| Err(SdsError::Extract(e.to_string())));

                match ocr {
                    Ok(text) if !text.trim().is_empty() => text,
                    Err(e) => {
                        // Tesseract is unavailable — signal that vision OCR may help.
                        return Err(SdsError::ImageOnlyPdf(e.to_string()));
                    }
                    Ok(_) => raw, // tesseract ran but produced nothing; keep sparse text
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
// pdftotext fallback (poppler)
// ---------------------------------------------------------------------------

/// Extract text from a PDF using the `pdftotext` CLI (part of poppler-utils).
///
/// This is used as a middle tier when `pdf-extract` fails or returns sparse text
/// on PDFs that use CID fonts (e.g. Shift-JIS encoded Japanese PDFs).  The `-utf8`
/// flag instructs pdftotext to always output UTF-8, handling the encoding conversion
/// that `pdf-extract` cannot perform.
///
/// Returns `None` if:
/// - `pdftotext` is not installed (silently ignored; caller falls through to OCR)
/// - the command exits with a non-zero status
/// - the output is empty or whitespace-only
fn pdftotext_fallback(path: &Path) -> Option<String> {
    let path_str = path.to_str()?;
    let out = std::process::Command::new("pdftotext")
        .args(["-utf8", path_str, "-"]) // "-" writes to stdout
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    if text.trim().is_empty() { None } else { Some(text) }
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

    // Pass 3 — truncate to max_chars (counted in Unicode scalar values, not bytes)
    // so that Japanese/CJK text is not cut at 1/3 of the intended character limit.
    if out.chars().count() > max_chars {
        let byte_offset = out
            .char_indices()
            .nth(max_chars)
            .map(|(i, _)| i)
            .unwrap_or(out.len());
        out.truncate(byte_offset);
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

/// Returns lazily-initialised, shared CSS selectors for HTML extraction.
fn html_selectors() -> (&'static scraper::Selector, &'static scraper::Selector, &'static scraper::Selector) {
    use scraper::Selector;
    use std::sync::OnceLock;
    static ROW:  OnceLock<Selector> = OnceLock::new();
    static CELL: OnceLock<Selector> = OnceLock::new();
    static BODY: OnceLock<Selector> = OnceLock::new();
    (
        ROW.get_or_init(||  Selector::parse("tr").expect("static CSS selector is valid")),
        CELL.get_or_init(|| Selector::parse("td, th").expect("static CSS selector is valid")),
        BODY.get_or_init(|| Selector::parse("body").expect("static CSS selector is valid")),
    )
}

/// Returns a long-lived shared `reqwest::Client` for URL fetches.
/// Avoids creating a new client (and TLS context) on every call.
fn shared_http_client() -> &'static reqwest::Client {
    use std::sync::OnceLock;
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .build()
            .expect("failed to build shared HTTP client")
    })
}

/// Extract visible text from an HTML string, skipping script/style/nav elements.
/// Table cells are tab-separated; rows are newline-separated.
pub fn extract_text_from_html_str(html: &str) -> String {
    use scraper::Html;

    let document = Html::parse_document(html);
    let (row_sel, cell_sel, body_sel) = html_selectors();

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
