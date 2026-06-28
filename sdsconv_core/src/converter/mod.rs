pub mod compliance;
pub mod corrector;
pub mod extractor;
pub mod generator;
pub mod html;
pub mod llm;
pub mod pdf;
pub mod template;
pub mod validator;

use std::path::Path;

// ---------------------------------------------------------------------------
// MHLW §3.3: prune fields with no valid value
// ---------------------------------------------------------------------------

/// Recursively remove null, empty-string, empty-array, and empty-object fields
/// from a JSON value tree.
///
/// Per MHLW SDS data exchange format §3.3, fields with no valid value must be
/// omitted entirely rather than serialised as `""` or `null`.
pub fn prune_empty_fields(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Object(map) => {
            let pruned: serde_json::Map<_, _> = map
                .into_iter()
                .filter_map(|(k, v)| {
                    let pv = prune_empty_fields(v);
                    match &pv {
                        Value::Null => None,
                        Value::String(s) if s.trim().is_empty() => None,
                        Value::Array(a)  if a.is_empty() => None,
                        Value::Object(o) if o.is_empty() => None,
                        _ => Some((k, pv)),
                    }
                })
                .collect();
            Value::Object(pruned)
        }
        Value::Array(arr) => {
            let pruned: Vec<_> = arr
                .into_iter()
                .map(prune_empty_fields)
                .filter(|v| match v {
                    Value::Null => false,
                    Value::String(s) if s.trim().is_empty() => false,
                    Value::Array(a)  if a.is_empty() => false,
                    Value::Object(o) if o.is_empty() => false,
                    _ => true,
                })
                .collect();
            Value::Array(pruned)
        }
        other => other,
    }
}

use serde::{Deserialize, Serialize};

use crate::country::SourceCountry;
use crate::error::SdsError;
use crate::language::Language;
use crate::schema::SdsRoot;

// ---------------------------------------------------------------------------
// ConversionReport
// ---------------------------------------------------------------------------

/// Structured report produced alongside every `to-json` conversion.
///
/// Describes what was extracted, what was missing, and any normalisations
/// applied to meet the JIS Z 7253 / MHLW standard.
///
/// The report is `serde`-serialisable so it can be written as JSON by the CLI
/// and consumed as a structured value by web-app / API callers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ConversionReport {
    /// BCP-47 tag of the detected or user-specified source language
    /// (e.g. `"ja"`, `"en"`, `"zh-CN"`, `"zh-TW"`).
    pub source_language: String,
    /// `true` if the language was inferred from text content rather than
    /// supplied explicitly via `--lang` / `ConvertConfig::source_language`.
    pub language_auto_detected: bool,
    /// MHLW section keys that contain at least one extracted value.
    pub populated_sections: Vec<String>,
    /// MHLW section keys for which no data was found.
    pub empty_sections: Vec<String>,
    /// Notes explaining how source values were normalised to conform to the
    /// current JIS Z 7253 / MHLW standard.
    ///
    /// Example: section heading terminology updated in JIS Z 7253:2019.
    pub standardization_notes: Vec<String>,
    /// Validation warnings emitted by the MHLW schema validator.
    pub warnings: Vec<String>,
    /// Country-specific compliance gap report, present when a source country is known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compliance_diff: Option<compliance::ComplianceDiffReport>,
}

impl ConversionReport {
    /// Build a `ConversionReport` from a completed [`SdsRoot`] and metadata.
    pub fn from_sds(
        sds: &SdsRoot,
        source_language: Language,
        language_auto_detected: bool,
        warnings: Vec<String>,
    ) -> Self {
        let mut populated = Vec::new();
        let mut empty = Vec::new();

        macro_rules! check_opt {
            ($key:literal, $field:expr) => {
                if $field.is_some() {
                    populated.push($key.to_string());
                } else {
                    empty.push($key.to_string());
                }
            };
        }
        macro_rules! check_vec {
            ($key:literal, $field:expr) => {
                if $field.as_ref().map_or(false, |v: &Vec<_>| !v.is_empty()) {
                    populated.push($key.to_string());
                } else {
                    empty.push($key.to_string());
                }
            };
        }

        check_opt!("Identification",                      sds.identification);
        check_opt!("HazardIdentification",                sds.hazard_identification);
        check_opt!("Composition",                         sds.composition);
        check_opt!("FirstAidMeasures",                    sds.first_aid_measures);
        check_opt!("FireFightingMeasures",                sds.fire_fighting_measures);
        check_opt!("AccidentalReleaseMeasures",           sds.accidental_release_measures);
        check_opt!("HandlingAndStorage",                  sds.handling_and_storage);
        check_opt!("ExposureControlPersonalProtection",   sds.exposure_control_personal_protection);
        check_opt!("PhysicalChemicalProperties",          sds.physical_chemical_properties);
        check_opt!("StabilityReactivity",                 sds.stability_reactivity);
        check_vec!("ToxicologicalInformation",            sds.toxicological_information);
        check_vec!("EcologicalInformation",               sds.ecological_information);
        check_opt!("DisposalConsiderations",              sds.disposal_considerations);
        check_opt!("TransportInformation",                sds.transport_information);
        check_opt!("RegulatoryInformation",               sds.regulatory_information);
        check_opt!("OtherInformation",                    sds.other_information);

        let standardization_notes = match source_language {
            Language::Japanese => vec![
                "Section headings follow JIS Z 7253:2019. \
                 Section 1 uses '化学品及び会社情報' (JIS Z 7253:2019); \
                 older source documents that use '製品及び会社情報' are normalised automatically."
                    .to_string(),
            ],
            Language::English => vec![
                "Section headings follow GHS Rev.10 / ISO 11014.".to_string(),
            ],
            Language::ChineseSimplified => vec![
                "Section headings follow GB/T 16483-2012.".to_string(),
            ],
            Language::ChineseTraditional => vec![
                "Section headings follow CNS 15030.".to_string(),
            ],
        };

        Self {
            source_language: source_language.bcp47().to_string(),
            language_auto_detected,
            populated_sections: populated,
            empty_sections: empty,
            standardization_notes,
            warnings,
            compliance_diff: None, // filled by convert_to_json_with_report if country is known
        }
    }
}

pub use compliance::{ComplianceDiffReport, ComplianceGap, generate_compliance_diff};
pub use corrector::{CorrectionConfig, CorrectionResult};
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
    /// Country/regulatory-region of the source SDS. `None` = inferred from language.
    ///
    /// When set, country-specific extraction rules are injected into the LLM prompt
    /// (e.g. China requires 24-hour emergency contact), country-specific validation
    /// checks are applied, and a compliance-gap report is generated alongside the JSON.
    pub source_country: Option<SourceCountry>,
    /// Language used for section headings in the generated document.
    pub output_language: Language,
    /// Maximum characters of extracted text sent to the LLM (quality control).
    /// Defaults to 80,000 to capture all 16 SDS sections including transport/regulatory.
    /// CLI overrides this via `--quality` (low=15k, medium=30k, high=60k).
    pub max_chars: usize,
    /// Opt-in validation-driven correction pass.
    ///
    /// When `Some`, a second targeted LLM call is made after the primary
    /// extraction to fix any invalid GHS H/P-codes found by the validator.
    /// CAS check-digit errors are corrected deterministically (no LLM call).
    ///
    /// `None` (the default) leaves the existing behavior completely unchanged.
    pub correction: Option<CorrectionConfig>,
}

impl Default for ConvertConfig {
    fn default() -> Self {
        Self {
            source_language: None,
            source_country: None,
            output_language: Language::default(),
            max_chars: 80_000,
            correction: None,
        }
    }
}

/// Extract text from a PDF or DOCX file and convert it to [`SdsRoot`] via LLM.
///
/// Returns `(SdsRoot, [`ConversionReport`])` with full extraction metadata.
/// Use [`convert_to_json`] for the simpler `(SdsRoot, Vec<String>)` interface.
pub async fn convert_to_json_with_report<B: LlmBackend + Sync>(
    input_path: &Path,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, ConversionReport), SdsError> {
    let text = extractor::extract_text_limited(input_path, config.max_chars).await?;
    if text.trim().is_empty() {
        return Err(SdsError::Extract(
            "No text extracted — document may be image-only or empty".into(),
        ));
    }
    let language_auto_detected = config.source_language.is_none();

    // Resolve the effective source country: explicit override → inferred from language → None.
    let effective_lang_for_country = config.source_language
        .unwrap_or_else(|| crate::language::detect_language(&text));
    let country = config.source_country
        .or_else(|| SourceCountry::infer_from_language(effective_lang_for_country));

    let (mut sds, mut warnings) =
        llm::extract_sds_from_text(backend, &text, config.source_language, country).await?;

    // ── Structural normalization ──────────────────────────────────────────────
    // CAS numbers: split entries that contain embedded newlines (LLM sometimes
    // concatenates multiple CAS numbers into one string with newlines instead
    // of using separate list entries).
    normalize_cas_full_text(&mut sds);

    // HazardIdentification: always present, even for non-hazardous products.
    // The LLM sometimes omits it entirely for HDPE/inert gas/food-grade substances;
    // insert a minimal stub so downstream tools and the QC checker see a valid section.
    ensure_hazard_identification(&mut sds);

    // ── Optional correction pass (opt-in via ConvertConfig::correction) ───────
    let mut corrected_cas_values: Vec<String> = Vec::new();
    if let Some(correction_cfg) = &config.correction {
        let findings = validator::collect_findings(&sds);
        if !findings.is_empty() {
            let result = corrector::apply_correction_pass(
                sds, &text, &findings, backend, correction_cfg,
            )
            .await;
            sds = result.sds;
            warnings.extend(result.notes);
            corrected_cas_values = result.corrected_cas_values;
        }
    }

    let validation_warnings = validator::validate(&sds);
    warnings.extend(validation_warnings);

    // Country-specific validation (always-on when country is known).
    if let Some(c) = country {
        warnings.extend(validator::validate_country(&sds, c));
    }

    // Phase 2: deterministic source-text verification (zero extra API calls).
    let source_warnings = validator::verify_against_source(&sds, &text, &corrected_cas_values);
    warnings.extend(source_warnings);

    // Determine effective source language for the report.
    let effective_lang = config.source_language.unwrap_or_else(|| {
        crate::language::detect_language(&text)
    });
    let mut report = ConversionReport::from_sds(&sds, effective_lang, language_auto_detected, warnings);

    // Attach compliance diff to report when a country is known.
    if let Some(c) = country {
        report.compliance_diff = Some(compliance::generate_compliance_diff(&sds, c));
    }

    Ok((sds, report))
}

/// Extract text from a PDF or DOCX file and convert it to [`SdsRoot`] via LLM.
///
/// Returns `(SdsRoot, Vec<String>)` where the `Vec` contains any warnings:
/// sections skipped due to schema mismatch, and structural validation issues.
///
/// For richer metadata (detected language, empty sections, standardisation notes)
/// use [`convert_to_json_with_report`] instead.
pub async fn convert_to_json<B: LlmBackend + Sync>(
    input_path: &Path,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, Vec<String>), SdsError> {
    let (sds, report) = convert_to_json_with_report(input_path, backend, config).await?;
    Ok((sds, report.warnings))
}

/// Convert raw document bytes to [`SdsRoot`] via LLM, returning a [`ConversionReport`].
///
/// This is the primary entry point for web / API callers that receive file bytes directly.
/// For the simpler `(SdsRoot, Vec<String>)` interface use [`convert_bytes_to_json`].
pub async fn convert_bytes_to_json_with_report<B: LlmBackend + Sync>(
    data: &[u8],
    filename: &str,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, ConversionReport), SdsError> {
    let suffix = Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_ascii_lowercase()))
        .unwrap_or_default();

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
        Ok::<_, SdsError>(f.into_temp_path())
    })
    .await
    .map_err(|e| SdsError::Extract(format!("spawn_blocking panicked: {e}")))??;

    convert_to_json_with_report(tmp.as_ref(), backend, config).await
}

/// Convert raw document bytes to [`SdsRoot`] via LLM.
///
/// Writes the bytes to a temporary file (preserving the original extension for format
/// detection), then delegates to [`convert_bytes_to_json_with_report`].
///
/// For richer metadata use [`convert_bytes_to_json_with_report`] instead.
pub async fn convert_bytes_to_json<B: LlmBackend + Sync>(
    data: &[u8],
    filename: &str,
    backend: &B,
    config: &ConvertConfig,
) -> Result<(SdsRoot, Vec<String>), SdsError> {
    let (sds, report) =
        convert_bytes_to_json_with_report(data, filename, backend, config).await?;
    Ok((sds, report.warnings))
}

/// Split any CAS `full_text` list entries that contain embedded newlines or
/// commas into separate entries.  The LLM occasionally concatenates multiple
/// CAS numbers into one string (e.g. `"590-00-1\n24634-61-5"` or
/// `"64742-54-7, 64742-55-8, 64742-65-0"`) instead of using a list.
fn normalize_cas_full_text(sds: &mut SdsRoot) {
    let Some(comp) = &mut sds.composition else { return };
    let Some(items) = &mut comp.composition_and_concentration else { return };
    for item in items.iter_mut() {
        let Some(ids) = &mut item.substance_identifiers else { continue };
        let Some(identity) = &mut ids.substance_identity else { continue };
        let Some(cas_node) = &mut identity.ca_sno else { continue };
        let Some(texts) = &mut cas_node.full_text else { continue };

        // A single valid CAS number never contains newlines, carriage returns,
        // or commas.  Any such character signals a concatenated multi-CAS string.
        let needs_split = texts.iter().any(|s| {
            s.contains('\n') || s.contains('\r') || s.contains(',') || s.contains(';')
        });
        if !needs_split {
            continue;
        }

        let expanded: Vec<String> = texts
            .drain(..)
            .flat_map(|s| {
                s.split(|c: char| c == '\n' || c == '\r' || c == ',' || c == ';')
                    .map(str::trim)
                    .filter(|t| !t.is_empty())
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .collect();
        *texts = expanded;
    }
}

/// Ensure `HazardIdentification` is always present in the SDS.
///
/// The LLM sometimes omits the section entirely for products with no GHS
/// hazard classification (non-hazardous polymers, inert gases, food-grade
/// substances).  Insert a minimal stub so the section is structurally valid:
/// `HazardLabelling.SignalWord = "N/A"` and an empty `HazardStatement` list.
fn ensure_hazard_identification(sds: &mut SdsRoot) {
    if sds.hazard_identification.is_some() {
        return;
    }
    use crate::schema::{
        HazardIdentification, HazardIdentificationHazardLabelling,
    };
    sds.hazard_identification = Some(HazardIdentification {
        hazard_labelling: Some(HazardIdentificationHazardLabelling {
            signal_word: Some("N/A".to_string()),
            hazard_statement: Some(vec![]),
            ..Default::default()
        }),
        ..Default::default()
    });
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
    let effective_lang_for_country = config.source_language
        .unwrap_or_else(|| crate::language::detect_language(&text));
    let country = config.source_country
        .or_else(|| SourceCountry::infer_from_language(effective_lang_for_country));
    let (sds, mut warnings) =
        llm::extract_sds_from_text(backend, &text, config.source_language, country).await?;
    let validation_warnings = validator::validate(&sds);
    warnings.extend(validation_warnings);
    if let Some(c) = country {
        warnings.extend(validator::validate_country(&sds, c));
    }
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
    let path_owned = input_path.to_path_buf();
    let bytes = tokio::task::spawn_blocking(move || std::fs::read(&path_owned))
        .await
        .map_err(|e| SdsError::Extract(format!("spawn_blocking panicked: {e}")))?
        .map_err(|e| SdsError::Extract(format!("reading PDF: {e}")))?;
    let effective_lang_for_country = config.source_language
        .unwrap_or_else(|| crate::language::detect_language(
            &String::from_utf8_lossy(&bytes)
        ));
    let country = config.source_country
        .or_else(|| SourceCountry::infer_from_language(effective_lang_for_country));
    let (sds, mut warnings) =
        llm::extract_sds_from_pdf_vision(api_key, llm_config, &bytes, config.source_language, country)
            .await?;
    let validation_warnings = validator::validate(&sds);
    warnings.extend(validation_warnings);
    if let Some(c) = country {
        warnings.extend(validator::validate_country(&sds, c));
    }
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
