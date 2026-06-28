use std::path::Path;
use std::sync::OnceLock;

use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use serde_json::Value;

use sdsconv_core::{
    build_any_backend, convert_bytes_to_json_with_report, convert_to_json_with_report,
    convert_url_to_json, enrich_composition, extract_text_limited, prune_empty_fields, validate,
    ConvertConfig, Language, LlmConfig, SourceCountry, SdsRoot,
};
use sdsconv_core::converter::CorrectionConfig;

// ---------------------------------------------------------------------------
// Shared Tokio runtime
// ---------------------------------------------------------------------------

static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();

fn rt() -> &'static tokio::runtime::Runtime {
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime")
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn parse_lang(lang: Option<&str>) -> Option<Language> {
    match lang? {
        "ja"    => Some(Language::Japanese),
        "en"    => Some(Language::English),
        "zh-cn" => Some(Language::ChineseSimplified),
        "zh-tw" => Some(Language::ChineseTraditional),
        _       => None,
    }
}

fn parse_country(country: Option<&str>) -> Option<SourceCountry> {
    match country? {
        "jp" => Some(SourceCountry::Japan),
        "cn" => Some(SourceCountry::China),
        "tw" => Some(SourceCountry::Taiwan),
        "kr" => Some(SourceCountry::Korea),
        _    => None,
    }
}

fn make_config(
    lang: Option<&str>,
    country: Option<&str>,
    max_chars: usize,
    correct: bool,
) -> ConvertConfig {
    ConvertConfig {
        source_language: parse_lang(lang),
        source_country: parse_country(country),
        output_language: Language::Japanese,
        max_chars,
        correction: if correct { Some(CorrectionConfig::default()) } else { None },
    }
}

/// Parse warnings from `validate()` into structured JSON objects.
/// Formats recognised:
///   [HIGH][S2-GHS-INCOMPLETE] Section 2: ...
///   [SourceVerify] Section 1 (...): ...
///   Section 1 (Identification): ...  (structural)
fn parse_warnings(warnings: Vec<String>) -> Value {
    use serde_json::json;
    let items: Vec<Value> = warnings
        .into_iter()
        .map(|w| {
            // [LEVEL][RULE] message
            if let Some(rest) = w.strip_prefix('[') {
                if let Some(level_end) = rest.find(']') {
                    let level = &rest[..level_end];
                    let after_level = &rest[level_end + 1..];
                    if let Some(rule_part) = after_level.strip_prefix('[') {
                        if let Some(rule_end) = rule_part.find(']') {
                            let rule = &rule_part[..rule_end];
                            let message = rule_part[rule_end + 2..].trim().to_string();
                            return json!({"level": level, "rule": rule, "message": message});
                        }
                    }
                    // [SourceVerify] ...
                    let message = after_level.trim_start_matches(']').trim().to_string();
                    return json!({"level": "WARN", "rule": level, "message": message});
                }
            }
            // plain structural warning
            json!({"level": "WARN", "rule": "STRUCTURAL", "message": w})
        })
        .collect();
    Value::Array(items)
}

// ---------------------------------------------------------------------------
// Exposed Python functions
// ---------------------------------------------------------------------------

/// Extract raw text from a PDF/DOCX/XLSX/HTML/TXT file.
#[pyfunction]
#[pyo3(signature = (path, max_chars = 80_000))]
fn extract_text(path: &str, max_chars: usize) -> PyResult<String> {
    rt().block_on(async {
        extract_text_limited(Path::new(path), max_chars)
            .await
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))
    })
}

/// Convert a file to MHLW standard JSON.
/// Returns (sds_json, report_json) as JSON strings.
#[pyfunction]
#[pyo3(signature = (
    path,
    backend    = "anthropic",
    api_key    = "",
    model      = "claude-haiku-4-5-20251001",
    lang       = None,
    country    = None,
    max_chars  = 80_000,
    max_tokens = 16_384,
    correct    = false,
    enrich     = false,
))]
#[allow(clippy::too_many_arguments)]
fn to_json_with_report(
    path:       &str,
    backend:    &str,
    api_key:    &str,
    model:      &str,
    lang:       Option<&str>,
    country:    Option<&str>,
    max_chars:  usize,
    max_tokens: u32,
    correct:    bool,
    enrich:     bool,
) -> PyResult<(String, String)> {
    let llm_cfg = LlmConfig { model: model.to_string(), max_tokens };
    let be = build_any_backend(backend, api_key.to_string(), llm_cfg);
    let config = make_config(lang, country, max_chars, correct);
    let path_buf = Path::new(path).to_path_buf();

    rt().block_on(async move {
        let (sds, report) = convert_to_json_with_report(&path_buf, &be, &config)
            .await
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        if enrich {
            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap();
            enrich_composition(&sds, &client).await;
        }

        let pruned = prune_empty_fields(
            serde_json::to_value(&sds).map_err(|e| PyRuntimeError::new_err(e.to_string()))?,
        );
        let sds_json    = serde_json::to_string_pretty(&pruned).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let report_json = serde_json::to_string_pretty(&report).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let _ = sds; // sds is moved into pruned above via to_value
        Ok((sds_json, report_json))
    })
}

/// Convert raw bytes to MHLW standard JSON (for API / in-memory use).
/// Returns (sds_json, report_json).
#[pyfunction]
#[pyo3(signature = (
    data,
    filename,
    backend    = "anthropic",
    api_key    = "",
    model      = "claude-haiku-4-5-20251001",
    lang       = None,
    country    = None,
    max_chars  = 80_000,
    max_tokens = 16_384,
    correct    = false,
))]
#[allow(clippy::too_many_arguments)]
fn to_json_bytes_with_report(
    data:       &[u8],
    filename:   &str,
    backend:    &str,
    api_key:    &str,
    model:      &str,
    lang:       Option<&str>,
    country:    Option<&str>,
    max_chars:  usize,
    max_tokens: u32,
    correct:    bool,
) -> PyResult<(String, String)> {
    let llm_cfg = LlmConfig { model: model.to_string(), max_tokens };
    let be = build_any_backend(backend, api_key.to_string(), llm_cfg);
    let config = make_config(lang, country, max_chars, correct);
    let data = data.to_vec();
    let filename = filename.to_string();

    rt().block_on(async move {
        let (sds, report) = convert_bytes_to_json_with_report(&data, &filename, &be, &config)
            .await
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let pruned   = prune_empty_fields(serde_json::to_value(&sds).map_err(|e| PyRuntimeError::new_err(e.to_string()))?);
        let sds_json    = serde_json::to_string_pretty(&pruned).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let report_json = serde_json::to_string_pretty(&report).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok((sds_json, report_json))
    })
}

/// Fetch an SDS from a URL and convert to MHLW standard JSON.
/// Returns (sds_json, report_json).
#[pyfunction]
#[pyo3(signature = (
    url,
    backend    = "anthropic",
    api_key    = "",
    model      = "claude-haiku-4-5-20251001",
    lang       = None,
    country    = None,
    max_chars  = 80_000,
    max_tokens = 16_384,
    correct    = false,
))]
#[allow(clippy::too_many_arguments)]
fn to_json_url_with_report(
    url:        &str,
    backend:    &str,
    api_key:    &str,
    model:      &str,
    lang:       Option<&str>,
    country:    Option<&str>,
    max_chars:  usize,
    max_tokens: u32,
    correct:    bool,
) -> PyResult<(String, String)> {
    let llm_cfg = LlmConfig { model: model.to_string(), max_tokens };
    let be = build_any_backend(backend, api_key.to_string(), llm_cfg);
    let config = make_config(lang, country, max_chars, correct);
    let url = url.to_string();

    rt().block_on(async move {
        let (sds, warnings) = convert_url_to_json(&url, &be, &config)
            .await
            .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;

        let pruned   = prune_empty_fields(serde_json::to_value(&sds).map_err(|e| PyRuntimeError::new_err(e.to_string()))?);
        let sds_json    = serde_json::to_string_pretty(&pruned).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        let report_json = serde_json::to_string_pretty(&warnings).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        Ok((sds_json, report_json))
    })
}

/// Validate a MHLW standard JSON (as a JSON string or Python dict serialised to string).
/// Returns a JSON array of finding objects: [{"level": "HIGH", "rule": "...", "message": "..."}, ...]
#[pyfunction]
fn validate_json(json_text: &str) -> PyResult<String> {
    let sds: SdsRoot = serde_json::from_str(json_text)
        .map_err(|e| PyValueError::new_err(format!("invalid JSON: {e}")))?;
    let warnings = validate(&sds);
    let findings = parse_warnings(warnings);
    serde_json::to_string_pretty(&findings).map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

// ---------------------------------------------------------------------------
// Module definition
// ---------------------------------------------------------------------------

#[pymodule]
#[pyo3(name = "_sdsconv")]
fn sdsconv_module(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(extract_text, m)?)?;
    m.add_function(wrap_pyfunction!(to_json_with_report, m)?)?;
    m.add_function(wrap_pyfunction!(to_json_bytes_with_report, m)?)?;
    m.add_function(wrap_pyfunction!(to_json_url_with_report, m)?)?;
    m.add_function(wrap_pyfunction!(validate_json, m)?)?;
    Ok(())
}
