use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
use chrono::Local;
use walkdir::WalkDir;

use sdsconv_core::{
    converter::{
        AnthropicBackend, CorrectionConfig, LlmBackend, LlmConfig,
        OpenAiCompatBackend, openai_compat_url,
    },
    convert_from_json, convert_from_template, convert_pdf_to_json_vision,
    convert_to_json_with_report, convert_url_to_json,
    detect_language, detect_language_from_file, detect_language_from_url,
    enrich_composition, prune_empty_fields, validate,
    extract_text, extract_text_from_url,
    ConversionReport, ConvertConfig, Language, SourceCountry, SdsError, SdsRoot,
};

// ---------------------------------------------------------------------------
// Shared enums (Provider, Quality) — used by both CLI and GUI
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum Provider {
    #[default]
    Anthropic,
    Openai,
    Gemini,
    Mistral,
    Groq,
    Cohere,
    Local,
}

impl Provider {
    pub fn all() -> &'static [&'static str] {
        &["anthropic", "openai", "gemini", "mistral", "groq", "cohere", "local"]
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "openai"   => Provider::Openai,
            "gemini"   => Provider::Gemini,
            "mistral"  => Provider::Mistral,
            "groq"     => Provider::Groq,
            "cohere"   => Provider::Cohere,
            "local"    => Provider::Local,
            _          => Provider::Anthropic,
        }
    }

    pub fn api_key_env(self) -> &'static str {
        match self {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::Openai    => "OPENAI_API_KEY",
            Provider::Gemini    => "GEMINI_API_KEY",
            Provider::Mistral   => "MISTRAL_API_KEY",
            Provider::Groq      => "GROQ_API_KEY",
            Provider::Cohere    => "COHERE_API_KEY",
            Provider::Local     => "LOCAL_LLM_API_KEY",
        }
    }

    pub fn default_model(self, quality: Quality) -> &'static str {
        match self {
            Provider::Anthropic => quality.anthropic_model(),
            Provider::Openai    => "gpt-4o-mini",
            Provider::Gemini    => "gemini-2.0-flash",
            Provider::Mistral   => "mistral-small-latest",
            Provider::Groq      => "llama-3.3-70b-versatile",
            Provider::Cohere    => "command-r-plus",
            Provider::Local     => "llama3",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub enum Quality {
    Low,
    #[default]
    Medium,
    High,
    /// Maximum — for very long SDS documents (e.g. multi-section Chinese GB/T 16483).
    /// Uses max_tokens=65_536 and max_chars=120_000.
    Max,
}

impl Quality {
    pub fn all() -> &'static [&'static str] {
        &["low", "medium", "high", "max"]
    }

    pub fn label(self) -> &'static str {
        match self {
            Quality::Low    => "low",
            Quality::Medium => "medium",
            Quality::High   => "high",
            Quality::Max    => "max",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "low"  => Quality::Low,
            "high" => Quality::High,
            "max"  => Quality::Max,
            _      => Quality::Medium,
        }
    }

    pub fn max_chars(self) -> usize {
        match self {
            Quality::Low    =>  15_000,
            Quality::Medium =>  30_000,
            Quality::High   =>  60_000,
            Quality::Max    => 120_000,
        }
    }

    pub fn max_tokens(self) -> u32 {
        match self {
            Quality::Low    =>  8_192,
            Quality::Medium => 32_768, // raised from 16_384: complex zh-cn/zh-tw SDSs exceed 16k
            Quality::High   => 32_768,
            Quality::Max    => 65_536,
        }
    }

    pub fn anthropic_model(self) -> &'static str {
        match self {
            Quality::High | Quality::Max => "claude-sonnet-4-6",
            _                            => "claude-haiku-4-5-20251001",
        }
    }
}

// ---------------------------------------------------------------------------
// Backend (enum-dispatch wrapper — private to this module)
// ---------------------------------------------------------------------------

enum BackendKind {
    Anthropic(AnthropicBackend),
    OpenAiCompat(OpenAiCompatBackend),
}

impl LlmBackend for BackendKind {
    async fn complete(&self, system: &str, user: &str) -> Result<String, SdsError> {
        match self {
            Self::Anthropic(b)    => b.complete(system, user).await,
            Self::OpenAiCompat(b) => b.complete(system, user).await,
        }
    }
}

fn build_backend(
    provider: Provider,
    api_key: String,
    llm_config: LlmConfig,
    base_url: Option<String>,
) -> BackendKind {
    match provider {
        Provider::Anthropic => {
            BackendKind::Anthropic(AnthropicBackend::new(api_key, llm_config))
        }
        Provider::Gemini => {
            BackendKind::OpenAiCompat(OpenAiCompatBackend::gemini(api_key, llm_config))
        }
        other => {
            let key: Option<&'static str> = match other {
                Provider::Openai  => Some("openai"),
                Provider::Mistral => Some("mistral"),
                Provider::Groq    => Some("groq"),
                Provider::Cohere  => Some("cohere"),
                Provider::Local   => Some("local"),
                _                 => None,
            };
            let url = base_url
                .or_else(|| key.and_then(openai_compat_url).map(str::to_string))
                .unwrap_or_default();
            BackendKind::OpenAiCompat(OpenAiCompatBackend::new(api_key, llm_config, url))
        }
    }
}

// ---------------------------------------------------------------------------
// Log callback type
// ---------------------------------------------------------------------------

pub type LogFn = Arc<dyn Fn(String) + Send + Sync>;

pub fn stdout_log() -> LogFn {
    Arc::new(|msg| eprintln!("{msg}"))
}

// ---------------------------------------------------------------------------
// Param structs
// ---------------------------------------------------------------------------

pub struct ToJsonParams {
    pub input: String,
    pub output: PathBuf,
    pub provider: Provider,
    pub api_key: String,
    pub model: String,
    pub quality: Quality,
    pub lang: Option<Language>,
    /// Country/regulatory-region override. `None` = infer from detected language.
    pub country: Option<SourceCountry>,
    pub base_url: Option<String>,
    pub enrich: bool,
    /// If true, run the validation-driven correction pass after primary LLM extraction.
    ///
    /// Fixes invalid GHS H/P-codes via a targeted second LLM call and corrects
    /// CAS check-digit errors deterministically (no extra API call).
    pub correct: bool,
    /// If true, rename the output file to the MHLW-recommended convention:
    /// `SDS_<IssueDate>_<ProductCode>.json` (with `_NNN` suffix on collision).
    pub use_suggested_filename: bool,
}

pub struct ToDocxParams {
    pub input: PathBuf,
    pub output: PathBuf,
    pub lang: Language,
    pub template: Option<PathBuf>,
}

pub struct ToHtmlParams {
    pub input: PathBuf,
    pub output: PathBuf,
    pub lang: Language,
}

pub struct ToPdfParams {
    pub input: PathBuf,
    pub output: PathBuf,
    pub lang: Language,
}

pub struct ExtractTextParams {
    pub input: String,
    pub output: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Task runners — shared by CLI and GUI
// ---------------------------------------------------------------------------

pub async fn run_to_json(params: ToJsonParams, log: LogFn) -> anyhow::Result<()> {
    let llm_config = LlmConfig {
        model: params.model.clone(),
        max_tokens: params.quality.max_tokens(),
    };
    let is_url = params.input.starts_with("http://") || params.input.starts_with("https://");

    // Auto-detect source language when not specified by the user.
    let source_language = if params.lang.is_some() {
        params.lang
    } else {
        let detected = if is_url {
            detect_language_from_url(&params.input).await.ok()
        } else {
            detect_language_from_file(Path::new(&params.input)).await.ok()
        };
        if let Some(lang) = detected {
            log(format!("Detected language: {} — pass --lang to override", lang.name_en()));
        }
        detected
    };

    let convert_config = ConvertConfig {
        source_language,
        source_country: params.country,
        output_language: Language::default(),
        max_chars: params.quality.max_chars(),
        correction: if params.correct { Some(CorrectionConfig::default()) } else { None },
    };
    let backend = Arc::new(build_backend(
        params.provider,
        params.api_key.clone(),
        llm_config,
        params.base_url.clone(),
    ));

    log(format!("Extracting text from {} ...", params.input));

    // language_auto_detected: track whether the user specified --lang or we inferred it.
    let user_specified_lang = params.lang.is_some();

    let (sds, report) = if is_url {
        // URL: no structured report yet — build one from warnings.
        let (sds, warnings) = convert_url_to_json(&params.input, &*backend, &convert_config)
            .await
            .map_err(anyhow::Error::from)?;
        let eff_lang = source_language.unwrap_or_default();
        let report = ConversionReport::from_sds(&sds, eff_lang, !user_specified_lang, warnings);
        (sds, report)
    } else {
        match convert_to_json_with_report(Path::new(&params.input), &*backend, &convert_config).await {
            Ok(pair) => pair,
            Err(SdsError::ImageOnlyPdf(_)) if params.provider == Provider::Anthropic => {
                log("PDF appears image-only — retrying with Claude vision OCR...".to_string());
                let vision_config = LlmConfig {
                    model: params.model.clone(),
                    max_tokens: params.quality.max_tokens(),
                };
                // For image-only PDFs the text-based language detection may be wrong.
                // Use only the user-specified language, or None for auto-detect from image.
                let vision_convert_config = ConvertConfig {
                    source_language: params.lang,
                    source_country: params.country,
                    output_language: Language::default(),
                    max_chars: params.quality.max_chars(),
                    correction: None,
                };
                let (sds, warnings) = convert_pdf_to_json_vision(
                    Path::new(&params.input),
                    &params.api_key,
                    &vision_config,
                    &vision_convert_config,
                )
                .await
                .context("vision OCR failed")?;
                let eff_lang = params.lang.unwrap_or_default();
                let report = ConversionReport::from_sds(&sds, eff_lang, !user_specified_lang, warnings);
                (sds, report)
            }
            Err(e) => return Err(e.into()),
        }
    };
    for w in &report.warnings {
        log(format!("WARN: {w}"));
    }
    if params.enrich {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        for w in &enrich_composition(&sds, &client).await {
            log(format!("CAS: {w}"));
        }
    }
    // Prune empty/null fields per MHLW §3.3 before writing.
    let json_val = serde_json::to_value(&sds)?;
    let json_val = prune_empty_fields(json_val);
    let json_str = serde_json::to_string_pretty(&json_val)?;
    std::fs::write(&params.output, json_str)
        .with_context(|| format!("writing {}", params.output.display()))?;

    // Optionally rename to the MHLW-recommended filename (§2.1.2).
    let final_output = if params.use_suggested_filename {
        let dir = params.output.parent().unwrap_or(Path::new("."));
        let base = suggested_filename(&sds);
        let candidate = resolve_unique_suggested_path(dir, &base, &params.output);
        if candidate != params.output {
            // On Windows the placeholder must be removed before rename
            #[cfg(windows)]
            let _ = std::fs::remove_file(&candidate);
            std::fs::rename(&params.output, &candidate)
                .with_context(|| format!("rename to {}", candidate.display()))?;
        }
        candidate
    } else {
        params.output.clone()
    };
    log(format!("Saved JSON to {}", final_output.display()));

    // Write <stem>_report.json alongside the output JSON.
    let report_path = report_path_for(&final_output);
    match serde_json::to_string_pretty(&report) {
        Ok(report_json) => {
            if let Err(e) = std::fs::write(&report_path, report_json) {
                log(format!("WARN: could not write report: {e}"));
            } else {
                // Summarise key report fields for the operator.
                log(format!(
                    "Report: {} populated, {} empty section(s) — see {}",
                    report.populated_sections.len(),
                    report.empty_sections.len(),
                    report_path.display()
                ));
                if !report.empty_sections.is_empty() {
                    log(format!("Empty sections: {}", report.empty_sections.join(", ")));
                }
            }
        }
        Err(e) => log(format!("WARN: could not serialise report: {e}")),
    }

    // Write <stem>_compliance_<country>.json only when --country was explicitly set.
    if let (Some(diff), Some(country)) = (&report.compliance_diff, params.country) {
        let slug = country.slug();
        let stem = final_output
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("output");
        let dir = final_output.parent().unwrap_or(Path::new("."));
        let compliance_path = dir.join(format!("{stem}_compliance_{slug}.json"));
        match serde_json::to_string_pretty(diff) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&compliance_path, json) {
                    log(format!("WARN: could not write compliance report: {e}"));
                } else {
                    log(format!(
                        "Compliance diff: {} gap(s) — see {}",
                        diff.gap_count,
                        compliance_path.display()
                    ));
                }
            }
            Err(e) => log(format!("WARN: could not serialise compliance report: {e}")),
        }
    }

    Ok(())
}

pub async fn run_to_docx(params: ToDocxParams, log: LogFn) -> anyhow::Result<()> {
    let input = params.input.clone();
    let output = params.output.clone();
    let template = params.template.clone();
    let explicit_lang = params.lang;

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        check_json_file_size(&input)?;
        let json = std::fs::read_to_string(&input)
            .with_context(|| format!("reading {}", input.display()))?;
        let sds: SdsRoot = serde_json::from_str(&json)?;

        // If the caller left the language at the default (Japanese), auto-detect
        // from the JSON content so that Chinese/English SDS get correct headings.
        let lang = if explicit_lang == Language::default() {
            let detected = detect_language(&json);
            if detected != Language::default() {
                tracing::info!("to-docx: auto-detected language {:?} from JSON content", detected);
            }
            detected
        } else {
            explicit_lang
        };

        let config = ConvertConfig {
            source_language: None,
            output_language: lang,
            ..Default::default()
        };
        if let Some(tmpl) = template {
            convert_from_template(&sds, &tmpl, &output)?;
        } else {
            convert_from_json(&sds, &output, &config)?;
        }
        Ok(())
    })
    .await
    .unwrap_or_else(|e| Err(anyhow::anyhow!("task panicked: {e}")))?;

    log(format!("Saved DOCX to {}", params.output.display()));
    Ok(())
}

pub async fn run_to_html(params: ToHtmlParams, log: LogFn) -> anyhow::Result<()> {
    use sdsconv_core::converter::html::generate_html;

    let input = params.input.clone();
    let output = params.output.clone();
    let lang = params.lang;

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        check_json_file_size(&input)?;
        let json = std::fs::read_to_string(&input)
            .with_context(|| format!("reading {}", input.display()))?;
        let sds: SdsRoot = serde_json::from_str(&json)?;
        let html = generate_html(&sds, lang)?;
        std::fs::write(&output, html)
            .with_context(|| format!("writing {}", output.display()))?;
        Ok(())
    })
    .await
    .unwrap_or_else(|e| Err(anyhow::anyhow!("task panicked: {e}")))?;

    log(format!("Saved HTML to {}", params.output.display()));
    Ok(())
}

pub async fn run_validate(input: PathBuf, log: LogFn) -> anyhow::Result<Vec<String>> {
    let warnings = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<String>> {
        check_json_file_size(&input)?;
        let raw = std::fs::read_to_string(&input)
            .with_context(|| format!("reading {}", input.display()))?;
        let sds: SdsRoot = serde_json::from_str(&raw)?;
        Ok(validate(&sds))
    })
    .await
    .unwrap_or_else(|e| Err(anyhow::anyhow!("task panicked: {e}")))?;

    if warnings.is_empty() {
        log("OK: no issues found".to_string());
    } else {
        for w in &warnings {
            log(format!("WARN: {w}"));
        }
    }
    Ok(warnings)
}

pub async fn run_to_pdf(params: ToPdfParams, log: LogFn) -> anyhow::Result<()> {
    let input = params.input.clone();
    let output = params.output.clone();
    let lang = params.lang;

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        check_json_file_size(&input)?;
        let json = std::fs::read_to_string(&input)
            .with_context(|| format!("reading {}", input.display()))?;
        let sds: SdsRoot = serde_json::from_str(&json)?;
        let bytes = sdsconv_core::converter::generate_pdf(&sds, lang)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        std::fs::write(&output, bytes)
            .with_context(|| format!("writing {}", output.display()))
    })
    .await
    .unwrap_or_else(|e| Err(anyhow::anyhow!("task panicked: {e}")))?;

    log(format!("Saved PDF to {}", params.output.display()));
    Ok(())
}

pub async fn run_extract_text(params: ExtractTextParams, log: LogFn) -> anyhow::Result<String> {
    log(format!("Extracting text from {} ...", params.input));
    let is_url = params.input.starts_with("http://") || params.input.starts_with("https://");
    let text = if is_url {
        extract_text_from_url(&params.input).await
            .map_err(|e| anyhow::anyhow!("{e}"))?
    } else {
        extract_text(Path::new(&params.input)).await
            .map_err(|e| anyhow::anyhow!("{e}"))?
    };
    if let Some(out) = &params.output {
        std::fs::write(out, &text)
            .with_context(|| format!("writing {}", out.display()))?;
        log(format!("Saved text to {}", out.display()));
    } else {
        log(format!("[OK] Extracted {} chars", text.len()));
    }
    Ok(text)
}

// ---------------------------------------------------------------------------
// eval-corpus
// ---------------------------------------------------------------------------

pub struct EvalCorpusParams {
    pub input_dir:  PathBuf,
    pub output_dir: PathBuf,
    pub provider:   Provider,
    pub api_key:    String,
    pub model:      String,
    pub quality:    Quality,
    pub lang:       Option<Language>,
    pub country:    Option<SourceCountry>,
    pub base_url:   Option<String>,
    pub jobs:       usize,
    pub correct:    bool,
    pub enrich:     bool,
    pub strict_mhlw: bool,
    pub max_files:  Option<usize>,
    /// Path to quality_check.py. Defaults to `tools/quality_check.py` next to the binary.
    pub qc_script:  PathBuf,
}

/// One row written to manifest.jsonl and causasv_features.csv.
#[derive(serde::Serialize, Clone)]
pub struct EvalRecord {
    pub filename:              String,
    pub file_type:             String,
    pub file_size_kb:          f64,
    pub text_length_chars:     usize,
    pub extraction_time_ms:    u64,
    pub source_language:       String,
    pub detected_country:      String,
    pub populated_section_count: usize,
    pub empty_section_count:   usize,
    pub cas_count_in_source:   usize,
    pub h_code_count_in_source: usize,
    pub p_code_count_in_source: usize,
    pub un_count_in_source:    usize,
    pub cas_coverage:          f32,
    pub h_code_coverage:       f32,
    pub p_code_coverage:       f32,
    pub un_coverage:           f32,
    pub critical_count:        usize,
    pub high_count:            usize,
    pub medium_count:          usize,
    pub overall_score:         f32,
    pub grade:                 String,
    pub json_ok:               bool,
    pub error:                 String,
}

pub async fn run_eval_corpus(params: EvalCorpusParams, log: LogFn) -> anyhow::Result<()> {
    use crate::evidence::{extract_evidence, match_evidence};
    use sdsconv_core::extract_text_limited;
    use std::sync::atomic::Ordering::Relaxed;

    // Create output subdirectories.
    let out = &params.output_dir;
    for sub in &["generated_json", "extracted_text", "validation_reports", "evidence_reports"] {
        std::fs::create_dir_all(out.join(sub))?;
    }

    let extensions = &["pdf", "docx", "xlsx", "xls", "txt", "html", "htm"];
    let mut files = collect_files(&params.input_dir, extensions);
    if let Some(max) = params.max_files {
        files.truncate(max);
    }
    let total = files.len();
    if total == 0 {
        log(format!("No SDS files found in {}", params.input_dir.display()));
        return Ok(());
    }
    log(format!("eval-corpus: {} files, {} workers", total, params.jobs));

    let manifest_path = out.join("manifest.jsonl");
    let manifest_file = Arc::new(std::sync::Mutex::new(
        std::fs::OpenOptions::new()
            .create(true).write(true).truncate(true)
            .open(&manifest_path)?
    ));

    let records: Arc<std::sync::Mutex<Vec<EvalRecord>>> = Arc::new(std::sync::Mutex::new(Vec::new()));

    let ok_count     = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let failed_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let pb = {
        use indicatif::{ProgressBar, ProgressStyle};
        let pb = ProgressBar::new(total as u64);
        pb.set_style(
            ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
                .unwrap_or_else(|_| ProgressStyle::default_bar()),
        );
        Arc::new(pb)
    };

    let qc_script   = Arc::new(params.qc_script.clone());
    let output_dir  = Arc::new(params.output_dir.clone());
    let provider    = params.provider;
    let api_key     = params.api_key.clone();
    let model       = params.model.clone();
    let quality     = params.quality;
    let lang        = params.lang;
    let country     = params.country;
    let base_url    = params.base_url.clone();
    let correct     = params.correct;
    let enrich      = params.enrich;

    use futures::stream::{self, StreamExt};
    stream::iter(files)
        .map(|path| {
            let pb           = Arc::clone(&pb);
            let output_dir   = Arc::clone(&output_dir);
            let qc_script    = Arc::clone(&qc_script);
            let records      = Arc::clone(&records);
            let manifest_file = Arc::clone(&manifest_file);
            let ok_count     = Arc::clone(&ok_count);
            let failed_count = Arc::clone(&failed_count);
            let api_key      = api_key.clone();
            let model        = model.clone();
            let base_url     = base_url.clone();
            let log2         = Arc::clone(&log);

            async move {
                let stem = match path.file_stem().and_then(|s| s.to_str()) {
                    Some(s) => s.to_string(),
                    None => { pb.inc(1); return; }
                };
                pb.set_message(stem.clone());

                let file_type = path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
                    .to_lowercase();
                let file_type2 = file_type.clone();
                let file_size_kb = std::fs::metadata(&path)
                    .map(|m| m.len() as f64 / 1024.0)
                    .unwrap_or(0.0);

                let pb3 = Arc::clone(&pb);
                let inner_log: LogFn = Arc::new(move |msg| { pb3.println(msg); });

                let result: anyhow::Result<EvalRecord> = async {
                    // 1. Extract text
                    let t0 = std::time::Instant::now();
                    let max_chars = quality.max_chars();
                    let text = extract_text_limited(&path, max_chars).await
                        .map_err(|e| anyhow::anyhow!("extract: {e}"))?;
                    // Save extracted text
                    let txt_path = output_dir.join("extracted_text").join(format!("{stem}.txt"));
                    let _ = std::fs::write(&txt_path, &text);

                    // 2. Convert to JSON
                    let json_path = output_dir.join("generated_json").join(format!("{stem}.json"));
                    let to_json_result = run_to_json(ToJsonParams {
                        input: path.to_string_lossy().into_owned(),
                        output: json_path.clone(),
                        provider, api_key: api_key.clone(), model: model.clone(),
                        quality, lang, country, base_url: base_url.clone(),
                        enrich, correct, use_suggested_filename: false,
                    }, Arc::clone(&inner_log)).await;

                    let extraction_time_ms = t0.elapsed().as_millis() as u64;
                    let json_ok = to_json_result.is_ok();
                    let error_msg = to_json_result.as_ref().err().map(|e| e.to_string()).unwrap_or_default();

                    // Read report for language/section info
                    let report_path = output_dir.join("generated_json").join(format!("{stem}_report.json"));
                    let report: Option<ConversionReport> = std::fs::read_to_string(&report_path)
                        .ok()
                        .and_then(|s| serde_json::from_str(&s).ok());
                    let source_language = report.as_ref()
                        .map(|r| r.source_language.clone())
                        .unwrap_or_default();
                    let detected_country = report.as_ref()
                        .and_then(|r| r.compliance_diff.as_ref())
                        .map(|d| d.target_country.clone())
                        .unwrap_or_default();
                    let populated_section_count = report.as_ref()
                        .map(|r| r.populated_sections.len())
                        .unwrap_or(0);
                    let empty_section_count = report.as_ref()
                        .map(|r| r.empty_sections.len())
                        .unwrap_or(0);

                    // 3. Evidence matching
                    let ev = extract_evidence(&text);
                    let cas_count_in_source  = ev.cas.len();
                    let h_code_count_in_source = ev.h_codes.len();
                    let p_code_count_in_source = ev.p_codes.len();
                    let un_count_in_source   = ev.un_numbers.len();

                    let json_val: serde_json::Value = if json_path.exists() {
                        std::fs::read_to_string(&json_path)
                            .ok()
                            .and_then(|s| serde_json::from_str(&s).ok())
                            .unwrap_or(serde_json::Value::Null)
                    } else {
                        serde_json::Value::Null
                    };
                    let cov = match_evidence(&ev, &json_val);
                    // Save evidence report
                    let ev_path = output_dir.join("evidence_reports").join(format!("{stem}_ev.json"));
                    let ev_report = serde_json::json!({
                        "file": stem,
                        "evidence": {
                            "cas": ev.cas, "h_codes": ev.h_codes,
                            "p_codes": ev.p_codes, "un_numbers": ev.un_numbers,
                            "signal_words": ev.signal_words,
                        },
                        "coverage": {
                            "cas": cov.cas, "h_codes": cov.h_codes,
                            "p_codes": cov.p_codes, "un_numbers": cov.un_numbers,
                        }
                    });
                    let _ = std::fs::write(&ev_path, serde_json::to_string_pretty(&ev_report).unwrap_or_default());

                    // 4. QC via quality_check.py
                    // quality_check.py requires a lang arg: ja / en / zh-cn / zh-tw
                    let qc_lang = match source_language.as_str() {
                        "zh-CN" => "zh-cn",
                        "zh-TW" => "zh-tw",
                        "en"    => "en",
                        _       => "ja",
                    };
                    let (critical_count, high_count, medium_count, qc_lines) =
                        run_qc_script(&qc_script, &json_path, qc_lang, &log2).await;
                    // Save QC findings
                    let qc_path = output_dir.join("validation_reports").join(format!("{stem}_qc.jsonl"));
                    let _ = std::fs::write(&qc_path, qc_lines.join("\n"));

                    // 5. Score
                    // Flat penalty: CRIT=-40, HIGH=-5, MED=-1 (capped at 0)
                    let penalty = critical_count * 40 + high_count * 5 + medium_count;
                    let overall_score = (100.0f32 - penalty as f32).max(0.0);
                    let grade = compute_grade(overall_score, critical_count, high_count);

                    Ok(EvalRecord {
                        filename: path.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string(),
                        file_type, file_size_kb,
                        text_length_chars: text.chars().count(),
                        extraction_time_ms, source_language, detected_country,
                        populated_section_count, empty_section_count,
                        cas_count_in_source, h_code_count_in_source,
                        p_code_count_in_source, un_count_in_source,
                        cas_coverage: cov.cas, h_code_coverage: cov.h_codes,
                        p_code_coverage: cov.p_codes, un_coverage: cov.un_numbers,
                        critical_count, high_count, medium_count,
                        overall_score, grade, json_ok,
                        error: error_msg,
                    })
                }.await;

                match result {
                    Ok(rec) => {
                        // Append to manifest.jsonl
                        if let Ok(line) = serde_json::to_string(&rec) {
                            use std::io::Write;
                            let _ = manifest_file.lock().map(|mut f| writeln!(f, "{line}"));
                        }
                        records.lock().unwrap().push(rec);
                        ok_count.fetch_add(1, Relaxed);
                    }
                    Err(e) => {
                        pb.println(format!("[ERROR] {stem}: {e}"));
                        failed_count.fetch_add(1, Relaxed);
                        // Write a failed record
                        let rec = EvalRecord {
                            filename: stem.clone(), file_type: file_type2,
                            file_size_kb, text_length_chars: 0, extraction_time_ms: 0,
                            source_language: String::new(), detected_country: String::new(),
                            populated_section_count: 0, empty_section_count: 0,
                            cas_count_in_source: 0, h_code_count_in_source: 0,
                            p_code_count_in_source: 0, un_count_in_source: 0,
                            cas_coverage: 0.0, h_code_coverage: 0.0,
                            p_code_coverage: 0.0, un_coverage: 0.0,
                            critical_count: 0, high_count: 0, medium_count: 0,
                            overall_score: 0.0, grade: "D".into(),
                            json_ok: false, error: e.to_string(),
                        };
                        if let Ok(line) = serde_json::to_string(&rec) {
                            use std::io::Write;
                            let _ = manifest_file.lock().map(|mut f| writeln!(f, "{line}"));
                        }
                    }
                }
                pb.inc(1);
            }
        })
        .buffer_unordered(params.jobs.max(1))
        .collect::<Vec<_>>()
        .await;

    let ok     = ok_count.load(Relaxed);
    let failed = failed_count.load(Relaxed);
    pb.finish_and_clear();
    log(format!("eval-corpus done: {ok} ok, {failed} failed"));

    // Aggregate reports
    let recs = records.lock().unwrap().clone();
    write_summary(&params.output_dir, &recs)?;
    write_failures_by_rule(&params.output_dir, &recs)?;
    write_causasv_features(&params.output_dir, &recs)?;

    if params.strict_mhlw {
        let has_crit_or_high = recs.iter().any(|r| r.critical_count > 0 || r.high_count > 0);
        if has_crit_or_high {
            anyhow::bail!("strict-mhlw: HIGH/CRIT findings present in corpus");
        }
    }

    Ok(())
}

fn compute_grade(score: f32, crit: usize, high: usize) -> String {
    if crit == 0 && high == 0 && score >= 90.0 { "A".into() }
    else if crit == 0 && high <= 3 && score >= 80.0 { "B".into() }
    else if crit == 0 && high <= 10 && score >= 65.0 { "C".into() }
    else { "D".into() }
}

/// Invoke quality_check.py for a JSON file via subprocess.
/// Returns (crit_count, high_count, med_count, raw_jsonl_lines).
async fn run_qc_script(
    qc_script: &Path,
    json_path: &Path,
    lang: &str,
    log: &LogFn,
) -> (usize, usize, usize, Vec<String>) {
    if !json_path.exists() {
        return (0, 0, 0, Vec::new());
    }
    let result = tokio::task::spawn_blocking({
        let qc   = qc_script.to_path_buf();
        let json = json_path.to_path_buf();
        let lang = lang.to_string();
        move || {
            std::process::Command::new("python3")
                .arg(&qc)
                .arg(&json)
                .arg(&lang)
                .arg("--jsonl")
                .output()
        }
    }).await;

    let output = match result {
        Ok(Ok(o))  => o,
        Ok(Err(e)) => { log(format!("WARN: quality_check.py spawn failed: {e}")); return (0, 0, 0, Vec::new()); }
        Err(e)     => { log(format!("WARN: quality_check.py task panicked: {e}")); return (0, 0, 0, Vec::new()); }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let lines: Vec<String> = stdout.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    // quality_check.py --jsonl appends ONE summary JSON at the end:
    // {"crit": N, "high": N, "med": N, "issues": [{...}, ...]}
    let (crit, high, med) = lines.iter().rev()
        .find_map(|line| {
            let v = serde_json::from_str::<serde_json::Value>(line).ok()?;
            let c = v.get("crit").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
            let h = v.get("high").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
            let m = v.get("med").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
            Some((c, h, m))
        })
        .unwrap_or((0, 0, 0));

    (crit, high, med, lines)
}

fn write_summary(out_dir: &Path, recs: &[EvalRecord]) -> anyhow::Result<()> {
    if recs.is_empty() { return Ok(()); }
    let n = recs.len() as f32;
    let avg_score = recs.iter().map(|r| r.overall_score).sum::<f32>() / n;
    let total_crit = recs.iter().map(|r| r.critical_count).sum::<usize>();
    let total_high = recs.iter().map(|r| r.high_count).sum::<usize>();
    let total_med  = recs.iter().map(|r| r.medium_count).sum::<usize>();
    let grade_counts = {
        let mut m = std::collections::HashMap::new();
        for r in recs { *m.entry(r.grade.as_str()).or_insert(0usize) += 1; }
        m
    };

    let summary = serde_json::json!({
        "total_files": recs.len(),
        "json_ok": recs.iter().filter(|r| r.json_ok).count(),
        "avg_score": avg_score,
        "grade_distribution": grade_counts,
        "total_critical": total_crit,
        "total_high": total_high,
        "total_medium": total_med,
        "avg_cas_coverage": recs.iter().map(|r| r.cas_coverage).sum::<f32>() / n,
        "avg_h_code_coverage": recs.iter().map(|r| r.h_code_coverage).sum::<f32>() / n,
    });
    std::fs::write(
        out_dir.join("summary.json"),
        serde_json::to_string_pretty(&summary)?,
    )?;

    // Markdown summary
    let grades_md = ["A", "B", "C", "D"].iter()
        .map(|g| format!("{g}: {}", grade_counts.get(g).unwrap_or(&0)))
        .collect::<Vec<_>>()
        .join(" / ");
    let md = format!(
        "# eval-corpus summary\n\n\
         | Metric | Value |\n\
         |--------|-------|\n\
         | Files | {} |\n\
         | JSON ok | {} |\n\
         | Avg score | {:.1} |\n\
         | Grades | {grades_md} |\n\
         | CRIT | {total_crit} |\n\
         | HIGH | {total_high} |\n\
         | MED  | {total_med} |\n\
         | Avg CAS coverage | {:.0}% |\n\
         | Avg H-code coverage | {:.0}% |\n",
        recs.len(),
        recs.iter().filter(|r| r.json_ok).count(),
        avg_score,
        recs.iter().map(|r| r.cas_coverage).sum::<f32>() / n * 100.0,
        recs.iter().map(|r| r.h_code_coverage).sum::<f32>() / n * 100.0,
    );
    std::fs::write(out_dir.join("summary.md"), md)?;
    Ok(())
}

fn write_failures_by_rule(out_dir: &Path, recs: &[EvalRecord]) -> anyhow::Result<()> {
    // Read all QC JSONL files and aggregate by rule
    use std::io::Write;
    let qc_dir = out_dir.join("validation_reports");
    let mut rule_map: std::collections::HashMap<String, (String, usize, std::collections::HashSet<String>)> = Default::default();

    for entry in std::fs::read_dir(&qc_dir).into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") { continue; }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let file_stem = stem.trim_end_matches("_qc").to_string();
        if let Ok(content) = std::fs::read_to_string(&path) {
            // quality_check.py --jsonl: the summary JSON is the last JSON-parseable line
            // It contains an "issues" array: [{"level": "HIGH", "rule": "...", "message": "..."}]
            let summary: Option<serde_json::Value> = content.lines()
                .filter_map(|l| serde_json::from_str(l).ok())
                .last();
            if let Some(v) = summary {
                if let Some(issues) = v.get("issues").and_then(|i| i.as_array()) {
                    for issue in issues {
                        let rule  = issue["rule"].as_str().unwrap_or("UNKNOWN").to_string();
                        let level = issue["level"].as_str().unwrap_or("?").to_string();
                        let e = rule_map.entry(rule).or_insert_with(|| (level.clone(), 0, Default::default()));
                        e.1 += 1;
                        e.2.insert(file_stem.clone());
                    }
                }
            }
        }
    }

    let mut rows: Vec<(String, String, usize, usize)> = rule_map.into_iter()
        .map(|(rule, (level, count, files))| (rule, level, count, files.len()))
        .collect();
    rows.sort_by(|a, b| b.2.cmp(&a.2));

    let mut f = std::fs::File::create(out_dir.join("failures_by_rule.csv"))?;
    writeln!(f, "rule_id,level,count,affected_files")?;
    for (rule, level, count, files) in &rows {
        writeln!(f, "{rule},{level},{count},{files}")?;
    }

    // failures_by_section.csv
    let section_prefixes = ["S1","S2","S3","S4","S5","S6","S7","S8","S9","S10","S11","S12","S13","S14","S15","S16"];
    let mut f2 = std::fs::File::create(out_dir.join("failures_by_section.csv"))?;
    writeln!(f2, "section,total_failures,high_crit_failures")?;
    for sec in &section_prefixes {
        let total: usize = rows.iter().filter(|(r,_,_,_)| r.starts_with(sec)).map(|(_,_,c,_)| c).sum();
        let hc: usize    = rows.iter()
            .filter(|(r,l,_,_)| r.starts_with(sec) && (l == "HIGH" || l == "CRIT"))
            .map(|(_,_,c,_)| c).sum();
        writeln!(f2, "{sec},{total},{hc}")?;
    }
    Ok(())
}

fn write_causasv_features(out_dir: &Path, recs: &[EvalRecord]) -> anyhow::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(out_dir.join("causasv_features.csv"))?;
    writeln!(f, "filename,file_type,file_size_kb,text_length_chars,extraction_time_ms,\
                 source_language,detected_country,populated_section_count,empty_section_count,\
                 cas_count_in_source,h_code_count_in_source,p_code_count_in_source,un_count_in_source,\
                 cas_coverage,h_code_coverage,p_code_coverage,un_coverage,\
                 critical_count,high_count,medium_count,overall_score,grade")?;
    for r in recs {
        writeln!(f,
            "{},{},{:.1},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.3},{:.3},{},{},{},{:.1},{}",
            r.filename, r.file_type, r.file_size_kb,
            r.text_length_chars, r.extraction_time_ms,
            r.source_language, r.detected_country,
            r.populated_section_count, r.empty_section_count,
            r.cas_count_in_source, r.h_code_count_in_source,
            r.p_code_count_in_source, r.un_count_in_source,
            r.cas_coverage, r.h_code_coverage, r.p_code_coverage, r.un_coverage,
            r.critical_count, r.high_count, r.medium_count,
            r.overall_score, r.grade,
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

pub const MAX_JSON_INPUT_BYTES: u64 = 100 * 1024 * 1024;

pub fn check_json_file_size(path: &Path) -> anyhow::Result<()> {
    let size = std::fs::metadata(path)
        .map_err(|e| anyhow::anyhow!("file stat failed for {}: {e}", path.display()))?
        .len();
    if size > MAX_JSON_INPUT_BYTES {
        anyhow::bail!(
            "input file too large ({} bytes, limit {} MB): {}",
            size,
            MAX_JSON_INPUT_BYTES / 1024 / 1024,
            path.display()
        );
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// JSON post-processing helpers (MHLW §3.3)
// ---------------------------------------------------------------------------

/// Recursively remove empty strings, empty arrays, and empty objects from a
/// Returns the path for the conversion report file: `<stem>_report.json`.
///
/// Examples:
///   `output.json`          → `output_report.json`
///   `SDS_2024-01-01_X.json`→ `SDS_2024-01-01_X_report.json`
fn report_path_for(json_output: &Path) -> PathBuf {
    let stem = json_output
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let dir = json_output.parent().unwrap_or(Path::new("."));
    dir.join(format!("{stem}_report.json"))
}

// ---------------------------------------------------------------------------
// Recommended filename helpers (MHLW §2.1.2)
// ---------------------------------------------------------------------------

/// Sanitize a string for use as a filename component.
/// Keeps alphanumerics and hyphens; replaces everything else with `_`.
/// Trims leading/trailing underscores.
fn sanitize_for_filename(s: &str) -> String {
    let raw: String = s
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    raw.trim_matches('_').to_string()
}

/// Generate the MHLW-recommended filename base (without extension or serial suffix).
///
/// Format: `SDS_<date>_<product_code>`
/// - `<date>`: `Datasheet.IssueDate` with `-` removed ("2024-03-31" → "20240331").
///   Falls back to today's date if absent.
/// - `<product_code>`: first element of `Identification.TradeProductIdentity.ProductNoUser`,
///   then `TradeNameEN`, then "NoCode".
pub fn suggested_filename(sds: &SdsRoot) -> String {
    let date = sds
        .datasheet
        .as_ref()
        .and_then(|d| d.issue_date.as_ref())
        .map(|d| d.replace('-', ""))
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| Local::now().format("%Y%m%d").to_string());

    let code = sds
        .identification
        .as_ref()
        .and_then(|id| id.trade_product_identity.as_ref())
        .and_then(|t| {
            t.product_no_user
                .as_ref()
                .and_then(|v| v.first())
                .cloned()
                .or_else(|| t.trade_name_en.clone())
        })
        .map(|s| sanitize_for_filename(&s))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "NoCode".to_string());

    format!("SDS_{date}_{code}")
}

/// Resolve a unique output path under `dir` using the suggested base name.
/// Appends `_001`, `_002`, … when the base name is already taken.
/// Uses atomic create-new to claim the path, avoiding TOCTOU races.
fn resolve_unique_suggested_path(dir: &Path, base: &str, avoid: &Path) -> PathBuf {
    // Try base.json first (atomically)
    let candidate = dir.join(format!("{base}.json"));
    if candidate == avoid || try_claim_path(&candidate) {
        return candidate;
    }
    for n in 1u32..=9999 {
        let c = dir.join(format!("{base}_{n:03}.json"));
        if try_claim_path(&c) {
            return c;
        }
    }
    // Fallback: use timestamp to avoid collision
    dir.join(format!("{base}_{}.json", std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)))
}

/// Atomically create a placeholder file to claim the path.
/// Returns true if the path was successfully claimed (or already owned by this process
/// as a zero-byte file). Caller should overwrite with actual content.
fn try_claim_path(path: &Path) -> bool {
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .is_ok()
}

pub fn collect_files(dir: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|e| extensions.iter().any(|x| x.eq_ignore_ascii_case(e)))
                .unwrap_or(false)
        })
        .collect();
    files.sort();
    files
}

