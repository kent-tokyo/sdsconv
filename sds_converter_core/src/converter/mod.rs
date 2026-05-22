pub mod extractor;
pub mod generator;
pub mod llm;
pub mod template;
pub mod validator;

use std::path::Path;

use crate::error::SdsError;
use crate::language::Language;
use crate::schema::SdsRoot;

pub use extractor::InputFormat;
pub use generator::generate_docx;
pub use llm::{openai_compat_url, AnthropicBackend, LlmBackend, LlmConfig, OpenAiCompatBackend};
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

/// Convert an [`SdsRoot`] to a `.docx` file using the built-in layout.
pub fn convert_from_json(
    sds: &SdsRoot,
    output_path: &Path,
    config: &ConvertConfig,
) -> Result<(), SdsError> {
    generate_docx(sds, output_path, config.output_language)
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
