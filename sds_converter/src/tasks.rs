use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
use chrono::Local;
use walkdir::WalkDir;

use sds_converter_core::{
    converter::{AnthropicBackend, LlmBackend, LlmConfig, OpenAiCompatBackend, openai_compat_url},
    convert_from_json, convert_from_template, convert_pdf_to_json_vision,
    convert_to_json, convert_url_to_json,
    detect_language_from_file, detect_language_from_url,
    enrich_composition, validate,
    extract_text, extract_text_from_url,
    ConvertConfig, Language, SdsError, SdsRoot,
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
            Quality::Medium => 16_384,
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
    pub base_url: Option<String>,
    pub enrich: bool,
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
        output_language: Language::default(),
        max_chars: params.quality.max_chars(),
    };
    let backend = Arc::new(build_backend(
        params.provider,
        params.api_key.clone(),
        llm_config,
        params.base_url.clone(),
    ));

    log(format!("Extracting text from {} ...", params.input));
    let result = if is_url {
        convert_url_to_json(&params.input, &*backend, &convert_config).await
    } else {
        convert_to_json(Path::new(&params.input), &*backend, &convert_config).await
    };
    let (sds, warnings) = match result {
        Ok(pair) => pair,
        Err(SdsError::ImageOnlyPdf(_)) if !is_url && params.provider == Provider::Anthropic => {
            log("PDF appears image-only — retrying with Claude vision OCR...".to_string());
            let vision_config = LlmConfig {
                model: params.model.clone(),
                max_tokens: params.quality.max_tokens(),
            };
            // For image-only PDFs the text-based language detection ran on sparse/empty
            // text and may be wrong.  Use only the user-specified language (params.lang),
            // falling back to None so the vision model can auto-detect from the image.
            let vision_convert_config = ConvertConfig {
                source_language: params.lang,
                output_language: Language::default(),
                max_chars: params.quality.max_chars(),
            };
            convert_pdf_to_json_vision(
                Path::new(&params.input),
                &params.api_key,
                &vision_config,
                &vision_convert_config,
            )
            .await
            .context("vision OCR failed")?
        }
        Err(e) => return Err(e.into()),
    };
    for w in &warnings {
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
    // Prune empty strings/arrays/objects per MHLW §3.3 before writing.
    let json_val = serde_json::to_value(&sds)?;
    let json_val = prune_empty_strings(json_val);
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
    Ok(())
}

pub async fn run_to_docx(params: ToDocxParams, log: LogFn) -> anyhow::Result<()> {
    let config = ConvertConfig {
        source_language: None,
        output_language: params.lang,
        ..Default::default()
    };
    let input = params.input.clone();
    let output = params.output.clone();
    let template = params.template.clone();

    tokio::task::spawn_blocking(move || -> anyhow::Result<()> {
        check_json_file_size(&input)?;
        let json = std::fs::read_to_string(&input)
            .with_context(|| format!("reading {}", input.display()))?;
        let sds: SdsRoot = serde_json::from_str(&json)?;
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
    use sds_converter_core::converter::html::generate_html;

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
        let bytes = sds_converter_core::converter::generate_pdf(&sds, lang)
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
/// JSON value, per MHLW §3.3: fields with no valid value should be omitted
/// entirely rather than set to `""`.
fn prune_empty_strings(v: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match v {
        Value::Object(map) => {
            let pruned: serde_json::Map<_, _> = map
                .into_iter()
                .filter_map(|(k, v)| {
                    let pv = prune_empty_strings(v);
                    match &pv {
                        Value::String(s) if s.is_empty() => None,
                        Value::Array(a)  if a.is_empty() => None,
                        Value::Object(o) if o.is_empty() => None,
                        _ => Some((k, pv)),
                    }
                })
                .collect();
            Value::Object(pruned)
        }
        Value::Array(arr) => {
            Value::Array(arr.into_iter().map(prune_empty_strings).collect())
        }
        other => other,
    }
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

