use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context as _;
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

    pub fn label(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::Openai    => "openai",
            Provider::Gemini    => "gemini",
            Provider::Mistral   => "mistral",
            Provider::Groq      => "groq",
            Provider::Cohere    => "cohere",
            Provider::Local     => "local",
        }
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
}

impl Quality {
    pub fn all() -> &'static [&'static str] {
        &["low", "medium", "high"]
    }

    pub fn label(self) -> &'static str {
        match self {
            Quality::Low    => "low",
            Quality::Medium => "medium",
            Quality::High   => "high",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "low"  => Quality::Low,
            "high" => Quality::High,
            _      => Quality::Medium,
        }
    }

    pub fn max_chars(self) -> usize {
        match self {
            Quality::Low    => 15_000,
            Quality::Medium => 30_000,
            Quality::High   => 60_000,
        }
    }

    pub fn max_tokens(self) -> u32 {
        16_384
    }

    pub fn anthropic_model(self) -> &'static str {
        match self {
            Quality::High => "claude-sonnet-4-6",
            _             => "claude-haiku-4-5-20251001",
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
            convert_pdf_to_json_vision(
                Path::new(&params.input),
                &params.api_key,
                &vision_config,
                &convert_config,
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
    let json_str = serde_json::to_string_pretty(&sds)?;
    std::fs::write(&params.output, json_str)
        .with_context(|| format!("writing {}", params.output.display()))?;
    log(format!("Saved JSON to {}", params.output.display()));
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

