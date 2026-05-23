pub mod extractor;
pub mod generator;
pub mod html;
pub mod llm;
pub mod pdf;
pub mod template;
pub mod validator;

use std::path::Path;

use crate::error::SdsError;
use crate::language::Language;
use crate::schema::SdsRoot;

pub use extractor::InputFormat;
pub use generator::generate_docx;
pub use pdf::generate_pdf;
pub use llm::{
    openai_compat_url, AnthropicBackend, AnyBackend, build_any_backend,
    extract_sds_from_pdf_vision, LlmBackend, LlmConfig, OpenAiCompatBackend,
};
pub use template::fill_template;

/// Configuration for document conversion.
#[derive(Debug, Clone)]
pub struct ConvertConfig {
    /// Language hint for the source document. `None` = auto-detect.
    pub source_language: Option<Language>,
    /// Language used for section headings in the generated document.
    pub output_language: Language,
    /// Maximum characters of extracted text sent to the LLM (quality control).
    /// Defaults to 80,000 to capture all 16 SDS sections including transport/regulatory.
    /// CLI overrides this via `--quality` (low=15k, medium=30k, high=60k).
    pub max_chars: usize,
}

impl Default for ConvertConfig {
    fn default() -> Self {
        Self {
            source_language: None,
            output_language: Language::default(),
            max_chars: 80_000,
        }
    }
}

/// Extract text from a PDF or DOCX file and convert it to [`SdsRoot`] via LLM.
///
/// Returns `(SdsRoot, Vec<String>)` where the `Vec` contains any warnings:
/// sections skipped due to schema mismatch, and structural validation issues.
pub async fn convert_to_json<B: LlmBackend + Sync>(
    input_path: &Path,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, Vec<String>), SdsError> {
    let text = extractor::extract_text_limited(input_path, config.max_chars).await?;
    if text.trim().is_empty() {
        return Err(SdsError::Extract(
            "No text extracted — document may be image-only or empty".into(),
        ));
    }
    let (sds, mut warnings) =
        llm::extract_sds_from_text(backend, &text, config.source_language).await?;
    let validation_warnings = validator::validate(&sds);
    warnings.extend(validation_warnings);
    Ok((sds, warnings))
}

/// Convert raw document bytes to [`SdsRoot`] via LLM.
///
/// Writes the bytes to a temporary file (preserving the original extension for format
/// detection), then delegates to [`convert_to_json`].  The temp file is deleted on return.
///
/// This is the primary entry point for web / API callers that receive file bytes directly.
pub async fn convert_bytes_to_json<B: LlmBackend + Sync>(
    data: &[u8],
    filename: &str,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, Vec<String>), SdsError> {
    let suffix = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_ascii_lowercase()))
        .unwrap_or_default();

    // Write to a named temp file so format detection (by extension) works.
    let data_owned = data.to_vec();
    let tmp = tokio::task::spawn_blocking(move || {
        use std::io::Write as _;
        let mut f = tempfile::Builder::new()
            .suffix(&suffix)
            .tempfile()
            .map_err(|e| SdsError::Extract(format!("tempfile create: {e}")))?;
        f.write_all(&data_owned)
            .map_err(|e| SdsError::Extract(format!("tempfile write: {e}")))?;
        f.flush()
            .map_err(|e| SdsError::Extract(format!("tempfile flush: {e}")))?;
        // Convert to TempPath to close the write handle (needed on Windows
        // to avoid sharing-violation errors when the extractor opens the file).
        Ok::<_, SdsError>(f.into_temp_path())
    })
    .await
    .map_err(|e| SdsError::Extract(format!("spawn_blocking panicked: {e}")))??;

    convert_to_json(tmp.as_ref(), backend, config).await
    // `tmp` (TempPath) is dropped here — the temp file is deleted automatically.
}

/// Convert an [`SdsRoot`] to a `.docx` file using the built-in layout.
pub fn convert_from_json(
    sds: &SdsRoot,
    output_path: &Path,
    config: &ConvertConfig,
) -> Result<(), SdsError> {
    generate_docx(sds, output_path, config.output_language)
}

/// Fetch an HTML page from `url`, extract its text, and convert it to [`SdsRoot`] via LLM.
pub async fn convert_url_to_json<B: LlmBackend + Sync>(
    url: &str,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, Vec<String>), SdsError> {
    let text = extractor::extract_text_from_url_limited(url, config.max_chars).await?;
    if text.trim().is_empty() {
        return Err(SdsError::Extract(
            "No text extracted from URL — page may be empty or JavaScript-rendered".into(),
        ));
    }
    let (sds, mut warnings) =
        llm::extract_sds_from_text(backend, &text, config.source_language).await?;
    let validation_warnings = validator::validate(&sds);
    warnings.extend(validation_warnings);
    Ok((sds, warnings))
}

/// Read a PDF file and extract SDS data using Anthropic's native PDF document API.
///
/// This bypasses text extraction entirely — the raw PDF bytes are base64-encoded and passed
/// directly to the model as a document content block. Use this as a fallback when
/// [`convert_to_json`] fails with [`crate::error::SdsError::ImageOnlyPdf`] (i.e. the PDF is
/// image-only and tesseract is not installed).
///
/// Size limit: 32 MB. Requires an Anthropic API key and a `claude-*` model.
pub async fn convert_pdf_to_json_vision(
    input_path: &Path,
    api_key: &str,
    llm_config: &LlmConfig,
    config: &ConvertConfig,
) -> Result<(SdsRoot, Vec<String>), SdsError> {
    let bytes = std::fs::read(input_path)
        .map_err(|e| SdsError::Extract(format!("reading PDF: {e}")))?;
    let (sds, mut warnings) =
        llm::extract_sds_from_pdf_vision(api_key, llm_config, &bytes, config.source_language)
            .await?;
    let validation_warnings = validator::validate(&sds);
    warnings.extend(validation_warnings);
    Ok((sds, warnings))
}

/// Fill a Word template (`.docx`) with data from an [`SdsRoot`].
///
/// Placeholders in the template use `{{FieldName}}` syntax where `FieldName` is
/// the leaf key from the MHLW JSON schema (e.g. `{{TradeNameJP}}`,
/// `{{CompanyName}}`). Full dot-path keys are also accepted for disambiguation
/// (e.g. `{{Identification.SupplierInformation.CompanyName}}`).
pub fn convert_from_template(
    sds: &SdsRoot,
    template_path: &Path,
    output_path: &Path,
) -> Result<(), SdsError> {
    fill_template(sds, template_path, output_path)
}
