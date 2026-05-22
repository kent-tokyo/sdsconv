//! LLM-based bidirectional conversion between Safety Data Sheet (SDS) documents and
//! the Japanese MHLW SDS Data Exchange Format v1.0 (JIS Z 7253 / GHS).
//!
//! # Quick start
//!
//! ```no_run
//! use sds_converter_core::{
//!     AnthropicBackend, LlmConfig,
//!     convert_to_json, ConvertConfig, Language,
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
//!     let (sds, warnings) =
//!         convert_to_json(std::path::Path::new("input.pdf"), &backend, &config).await?;
//!     for w in &warnings { eprintln!("WARN: {w}"); }
//!     std::fs::write("output.json", serde_json::to_string_pretty(&sds)?)?;
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
pub mod error;
pub mod language;
pub mod schema;

pub use converter::{
    convert_from_json, convert_from_template, convert_to_json, fill_template, openai_compat_url,
    AnthropicBackend, LlmBackend, LlmConfig, ConvertConfig, OpenAiCompatBackend,
};
pub use converter::extractor::{extract_text, extract_text_limited};
pub use converter::validator::validate;
pub use error::SdsError;
pub use language::Language;
pub use schema::SdsRoot;
