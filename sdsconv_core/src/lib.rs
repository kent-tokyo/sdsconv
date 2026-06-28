//! LLM-based bidirectional conversion between Safety Data Sheet (SDS) documents and
//! the Japanese MHLW SDS Data Exchange Format v1.0 (JIS Z 7253 / GHS).
//!
//! # Quick start
//!
//! ```no_run
//! use sdsconv_core::{
//!     AnthropicBackend, LlmConfig,
//!     convert_to_json, convert_to_json_with_report, ConvertConfig, Language,
//! };
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let backend = AnthropicBackend::new(
//!         std::env::var("ANTHROPIC_API_KEY")?,
//!         LlmConfig::default(),
//!     );
//!     let config = ConvertConfig {
//!         source_language: Some(Language::Japanese),
//!         output_language: Language::Japanese,
//!         ..Default::default()
//!     };
//!     // `convert_to_json_with_report` returns structured metadata (language, sections, notes).
//!     let (sds, report) =
//!         convert_to_json_with_report(std::path::Path::new("input.pdf"), &backend, &config).await?;
//!     for w in &report.warnings { eprintln!("WARN: {w}"); }
//!     eprintln!("Populated sections: {:?}", report.populated_sections);
//!     eprintln!("Standardization notes: {:?}", report.standardization_notes);
//!     std::fs::write("output.json", serde_json::to_string_pretty(&sds)?)?;
//!     std::fs::write("output_report.json", serde_json::to_string_pretty(&report)?)?;
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! - **SDS → JSON**: PDF/DOCX/XLSX/TXT → MHLW standard JSON via LLM (parallel extraction,
//!   automatic retry, JSON repair).
//! - **JSON → DOCX**: Generates a JIS Z 7253-compliant Word document with localized headings.
//! - **Multilingual**: `ja` / `en` / `zh-CN` / `zh-TW` source documents and output headings.
//! - **Pluggable LLM**: Ships with [`AnthropicBackend`] and [`OpenAiCompatBackend`].
//!   Implement [`converter::LlmBackend`] to bring your own.

pub mod converter;
pub mod country;
pub mod enrichment;
pub mod error;
pub mod ghs_codes;
pub mod language;
pub mod schema;

pub use converter::{
    AnthropicBackend, AnyBackend, ConvertConfig, ConversionReport, LlmBackend, LlmConfig,
    OpenAiCompatBackend, build_any_backend,
    convert_bytes_to_json, convert_bytes_to_json_with_report,
    convert_from_json, convert_from_template,
    convert_pdf_to_json_vision, convert_to_json, convert_to_json_with_report,
    convert_url_to_json,
    extract_sds_from_pdf_vision, fill_template, openai_compat_url,
    prune_empty_fields,
};
pub use converter::extractor::{
    detect_format_str, detect_language_from_file, detect_language_from_url,
    extract_text, extract_text_from_url, extract_text_limited,
};
pub use converter::validator::validate;
pub use enrichment::{CasInfo, CasWarning, enrich_composition, lookup_cas};
pub use error::SdsError;
pub use ghs_codes::{h_code_description, is_valid_h_code, is_valid_p_code, p_code_description};
pub use country::SourceCountry;
pub use language::{detect_language, Language};
pub use schema::SdsRoot;
