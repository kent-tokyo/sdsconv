mod app;
mod config;
mod evidence;
mod tasks;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use clap::{Parser, Subcommand, ValueEnum};
use futures::stream::{self, StreamExt};
use indicatif::{ProgressBar, ProgressStyle};
use sdsconv_core::{Language, SourceCountry, SdsRoot};

use tasks::{
    EvalCorpusParams, LogFn, Provider, Quality,
    ToDocxParams, ToHtmlParams, ToJsonParams, ToPdfParams,
    check_json_file_size, collect_files,
};

// ---------------------------------------------------------------------------
// CLI-specific enums (thin wrappers around tasks::{Provider, Quality})
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, ValueEnum, Default)]
enum CliQuality {
    Low,
    #[default]
    Medium,
    High,
    /// Maximum output tokens (65 536) for very long SDS documents.
    Max,
}

impl From<CliQuality> for Quality {
    fn from(q: CliQuality) -> Self {
        match q {
            CliQuality::Low    => Quality::Low,
            CliQuality::Medium => Quality::Medium,
            CliQuality::High   => Quality::High,
            CliQuality::Max    => Quality::Max,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum CliLanguage {
    Ja,
    En,
    ZhCn,
    ZhTw,
}

impl From<CliLanguage> for Language {
    fn from(l: CliLanguage) -> Self {
        match l {
            CliLanguage::Ja   => Language::Japanese,
            CliLanguage::En   => Language::English,
            CliLanguage::ZhCn => Language::ChineseSimplified,
            CliLanguage::ZhTw => Language::ChineseTraditional,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum CliCountry {
    /// Japan (JIS Z 7253 / MHLW)
    Jp,
    /// China (GB/T 16483)
    Cn,
    /// Taiwan (CNS 15030)
    Tw,
    /// Korea (K-GHS Rev.6)
    Kr,
}

impl From<CliCountry> for SourceCountry {
    fn from(c: CliCountry) -> Self {
        match c {
            CliCountry::Jp => SourceCountry::Japan,
            CliCountry::Cn => SourceCountry::China,
            CliCountry::Tw => SourceCountry::Taiwan,
            CliCountry::Kr => SourceCountry::Korea,
        }
    }
}

#[derive(Clone, Copy, ValueEnum, Default)]
enum CliProvider {
    #[default]
    Anthropic,
    Openai,
    Gemini,
    Mistral,
    Groq,
    Cohere,
    Local,
}

impl From<CliProvider> for Provider {
    fn from(p: CliProvider) -> Self {
        match p {
            CliProvider::Anthropic => Provider::Anthropic,
            CliProvider::Openai    => Provider::Openai,
            CliProvider::Gemini    => Provider::Gemini,
            CliProvider::Mistral   => Provider::Mistral,
            CliProvider::Groq      => Provider::Groq,
            CliProvider::Cohere    => Provider::Cohere,
            CliProvider::Local     => Provider::Local,
        }
    }
}

// ---------------------------------------------------------------------------
// Clap CLI definition
// ---------------------------------------------------------------------------

#[derive(Parser)]
#[command(name = "sdsconv", about = "Convert between SDS documents and MHLW standard JSON")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Convert a PDF, Word document, HTML file, or URL to MHLW standard JSON
    ToJson {
        #[arg(short, long, conflicts_with = "input_dir")]
        input: Option<String>,
        #[arg(long, conflicts_with = "input")]
        input_dir: Option<PathBuf>,
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,
        #[arg(long, conflicts_with = "output")]
        output_dir: Option<PathBuf>,
        /// API key (or set ANTHROPIC_API_KEY / OPENAI_API_KEY env var)
        #[arg(long, hide_env_values = true)]
        api_key: Option<String>,
        #[arg(long, value_enum)]
        lang: Option<CliLanguage>,
        /// Target country for country-specific extraction rules, validation, and compliance report.
        /// Inferred from --lang when omitted (zh-cn → cn, zh-tw → tw, ja → jp).
        #[arg(long, value_enum)]
        country: Option<CliCountry>,
        #[arg(long)]
        model: Option<String>,
        #[arg(long, value_enum, default_value = "anthropic")]
        provider: CliProvider,
        #[arg(long)]
        base_url: Option<String>,
        #[arg(long, value_enum, default_value = "medium")]
        quality: CliQuality,
        #[arg(long, default_value = "4")]
        concurrency: usize,
        #[arg(long)]
        enrich: bool,
        /// Run the validation-driven correction pass after primary extraction.
        ///
        /// Fixes invalid GHS H/P-codes via a targeted second LLM call and
        /// corrects CAS check-digit errors deterministically.
        #[arg(long)]
        correct: bool,
        /// Use the MHLW-recommended filename: SDS_<date>_<product_code>.json
        #[arg(long)]
        suggested_name: bool,
    },

    /// Convert MHLW standard JSON to a Word document
    ToDocx {
        #[arg(short, long, conflicts_with = "input_dir")]
        input: Option<PathBuf>,
        #[arg(long, conflicts_with = "input")]
        input_dir: Option<PathBuf>,
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,
        #[arg(long, conflicts_with = "output")]
        output_dir: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "ja")]
        lang: CliLanguage,
        #[arg(long)]
        template: Option<PathBuf>,
    },

    /// Validate a MHLW standard JSON file and report structural issues
    Validate {
        #[arg(short, long)]
        input: PathBuf,
        /// Output warnings as a JSON array (useful for CI)
        #[arg(long)]
        json: bool,
        /// Strict MHLW mode: exit 1 if any HIGH or CRIT finding is present
        #[arg(long)]
        strict_mhlw: bool,
    },

    /// Convert MHLW standard JSON to an HTML document
    ToHtml {
        #[arg(short, long, conflicts_with = "input_dir")]
        input: Option<PathBuf>,
        #[arg(long, conflicts_with = "input")]
        input_dir: Option<PathBuf>,
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,
        #[arg(long, conflicts_with = "output")]
        output_dir: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "ja")]
        lang: CliLanguage,
    },

    /// Convert MHLW standard JSON to a PDF via LibreOffice (requires `soffice` in PATH)
    ToPdf {
        #[arg(short, long, conflicts_with = "input_dir")]
        input: Option<PathBuf>,
        #[arg(long, conflicts_with = "input")]
        input_dir: Option<PathBuf>,
        #[arg(short, long, conflicts_with = "output_dir")]
        output: Option<PathBuf>,
        #[arg(long, conflicts_with = "output")]
        output_dir: Option<PathBuf>,
        #[arg(long, value_enum, default_value = "ja")]
        lang: CliLanguage,
    },

    /// Extract raw text from a document or URL (no LLM)
    ExtractText {
        #[arg(short, long)]
        input: String,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Detect the language of an SDS document or URL
    DetectLang {
        #[arg(short, long)]
        input: String,
    },

    /// Evaluate a corpus of SDS files and produce quality scores + causasv features
    EvalCorpus {
        /// Directory containing SDS files (PDF/DOCX/XLSX/HTML/TXT)
        #[arg(long)]
        input_dir: PathBuf,
        /// Output directory for all reports and generated JSON
        #[arg(long)]
        output_dir: PathBuf,
        /// Number of parallel workers
        #[arg(long, default_value = "8")]
        jobs: usize,
        /// Exit 1 if any HIGH or CRIT finding exists across the corpus
        #[arg(long)]
        strict_mhlw: bool,
        /// Run validation-driven correction pass (extra LLM call)
        #[arg(long)]
        correct: bool,
        /// Enrich CAS numbers via PubChem
        #[arg(long)]
        enrich: bool,
        /// Source language (omit for auto-detect)
        #[arg(long, value_enum)]
        lang: Option<CliLanguage>,
        #[arg(long, value_enum)]
        country: Option<CliCountry>,
        #[arg(long, value_enum, default_value = "anthropic")]
        provider: CliProvider,
        #[arg(long, env = "ANTHROPIC_API_KEY")]
        api_key: Option<String>,
        #[arg(long, default_value = "claude-sonnet-4-5")]
        model: String,
        #[arg(long, value_enum, default_value = "medium")]
        quality: CliQuality,
        #[arg(long)]
        base_url: Option<String>,
        /// Limit number of files processed (useful for smoke tests)
        #[arg(long)]
        max_files: Option<usize>,
        /// Path to quality_check.py (default: tools/quality_check.py)
        #[arg(long)]
        qc_script: Option<PathBuf>,
    },
}

// ---------------------------------------------------------------------------
// Entry point — hybrid launcher
// ---------------------------------------------------------------------------

fn main() -> anyhow::Result<()> {
    // Suppress the noisy panic backtrace that Rust's default panic hook emits for
    // pdf-extract crate panics.  Those panics are always caught by std::panic::catch_unwind
    // in sdsconv_core::converter::extractor and do not represent real failures —
    // they happen when pdf-extract encounters Shift-JIS / CID-font encoded PDFs, after
    // which the code falls back to pdftotext / OCR automatically.
    //
    // We install a custom hook that forwards everything to the original hook *except*
    // panics originating from files inside the pdf-extract crate.
    {
        let default_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let file = info.location().map(|l| l.file()).unwrap_or("");
            if file.contains("pdf-extract") || file.contains("pdf_extract") {
                // Panic is caught upstream by catch_unwind — silently discard.
                return;
            }
            default_hook(info);
        }));
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    if std::env::args().len() > 1 {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?
            .block_on(run_cli())
    } else {
        app::run_gui()
    }
}

// ---------------------------------------------------------------------------
// CLI handler
// ---------------------------------------------------------------------------

async fn run_cli() -> anyhow::Result<()> {
    let cfg = config::AppConfig::load();
    let cli = Cli::parse();
    let log: LogFn = tasks::stdout_log();

    match cli.command {
        Commands::ToJson {
            input, input_dir, output, output_dir,
            api_key, lang, country, model, provider, base_url, quality, concurrency,
            enrich, correct, suggested_name,
        } => {
            let provider = Provider::from(provider);
            let quality  = Quality::from(quality);
            let api_key  = resolve_api_key(api_key, provider, &cfg)?;
            let model    = model.unwrap_or_else(|| provider.default_model(quality).to_string());
            let country  = country.map(SourceCountry::from);

            eprintln!(
                "Quality: {} (max_chars={}, max_tokens={}, model={})",
                quality.label(), quality.max_chars(), quality.max_tokens(), model
            );

            match (input, input_dir) {
                (Some(input), None) => {
                    let output = output.ok_or_else(|| anyhow::anyhow!("--output required"))?;
                    tasks::run_to_json(ToJsonParams {
                        input, output, provider, api_key, model, quality,
                        lang: lang.map(Language::from), country, base_url, enrich, correct,
                        use_suggested_filename: suggested_name,
                    }, Arc::clone(&log)).await?;
                }
                (None, Some(dir)) => {
                    let out_dir = output_dir.ok_or_else(|| anyhow::anyhow!("--output-dir required"))?;
                    std::fs::create_dir_all(&out_dir)?;
                    batch_to_json(
                        &dir, &out_dir, provider, api_key, model, quality,
                        lang.map(Language::from), country, base_url, concurrency, enrich, correct,
                        suggested_name,
                    ).await;
                }
                _ => anyhow::bail!("Specify either --input or --input-dir"),
            }
        }

        Commands::ToDocx { input, input_dir, output, output_dir, lang, template } => {
            match (input, input_dir) {
                (Some(input), None) => {
                    let output = output.ok_or_else(|| anyhow::anyhow!("--output required"))?;
                    tasks::run_to_docx(
                        ToDocxParams { input, output, lang: Language::from(lang), template },
                        Arc::clone(&log),
                    ).await?;
                }
                (None, Some(dir)) => {
                    let out_dir = output_dir.ok_or_else(|| anyhow::anyhow!("--output-dir required"))?;
                    std::fs::create_dir_all(&out_dir)?;
                    batch_to_docx(&dir, &out_dir, Language::from(lang), template.as_deref())?;
                }
                _ => anyhow::bail!("Specify either --input or --input-dir"),
            }
        }

        Commands::Validate { input, json, strict_mhlw } => {
            let findings = tasks::run_validate(input, Arc::clone(&log)).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&findings)?);
            }
            if strict_mhlw {
                let has_high_or_crit = findings
                    .iter()
                    .any(|f| f.level == "HIGH" || f.level == "CRIT");
                if has_high_or_crit {
                    eprintln!("strict-mhlw: HIGH/CRIT findings present — see warnings above.");
                    std::process::exit(1);
                }
            } else if !findings.is_empty() && !json {
                std::process::exit(1);
            }
        }

        Commands::ToHtml { input, input_dir, output, output_dir, lang } => {
            match (input, input_dir) {
                (Some(input), None) => {
                    let output = output.ok_or_else(|| anyhow::anyhow!("--output required"))?;
                    tasks::run_to_html(
                        ToHtmlParams { input, output, lang: Language::from(lang) },
                        Arc::clone(&log),
                    ).await?;
                }
                (None, Some(dir)) => {
                    let out_dir = output_dir.ok_or_else(|| anyhow::anyhow!("--output-dir required"))?;
                    std::fs::create_dir_all(&out_dir)?;
                    batch_to_html(&dir, &out_dir, Language::from(lang))?;
                }
                _ => anyhow::bail!("Specify either --input or --input-dir"),
            }
        }

        Commands::ToPdf { input, input_dir, output, output_dir, lang } => {
            match (input, input_dir) {
                (Some(input), None) => {
                    let output = output.ok_or_else(|| anyhow::anyhow!("--output required"))?;
                    tasks::run_to_pdf(
                        ToPdfParams { input, output, lang: Language::from(lang) },
                        Arc::clone(&log),
                    ).await?;
                }
                (None, Some(dir)) => {
                    let out_dir = output_dir.ok_or_else(|| anyhow::anyhow!("--output-dir required"))?;
                    std::fs::create_dir_all(&out_dir)?;
                    batch_to_pdf(&dir, &out_dir, Language::from(lang))?;
                }
                _ => anyhow::bail!("Specify either --input or --input-dir"),
            }
        }

        Commands::ExtractText { input, output } => {
            use anyhow::Context as _;
            use sdsconv_core::{extract_text, extract_text_from_url};
            let is_url = input.starts_with("http://") || input.starts_with("https://");
            let text = if is_url {
                extract_text_from_url(&input).await?
            } else {
                extract_text(Path::new(&input)).await?
            };
            match output {
                Some(path) => {
                    std::fs::write(&path, &text)
                        .with_context(|| format!("writing {}", path.display()))?;
                    eprintln!("Saved text to {}", path.display());
                }
                None => print!("{text}"),
            }
        }

        Commands::DetectLang { input } => {
            use sdsconv_core::{detect_language_from_file, detect_language_from_url};
            let is_url = input.starts_with("http://") || input.starts_with("https://");
            let lang = if is_url {
                detect_language_from_url(&input).await?
            } else {
                detect_language_from_file(Path::new(&input)).await?
            };
            println!("{} ({})", lang.name_en(), lang.bcp47());
        }

        Commands::EvalCorpus {
            input_dir, output_dir, jobs, strict_mhlw, correct, enrich,
            lang, country, provider, api_key, model, quality, base_url,
            max_files, qc_script,
        } => {
            let provider = Provider::from(provider);
            let api_key  = resolve_api_key(api_key, provider, &cfg)?;
            let lang     = lang.map(Language::from);
            let country  = country.map(SourceCountry::from);
            // Default qc_script: tools/quality_check.py relative to current dir
            let qc_script = qc_script.unwrap_or_else(|| PathBuf::from("tools/quality_check.py"));
            std::fs::create_dir_all(&output_dir)?;
            tasks::run_eval_corpus(EvalCorpusParams {
                input_dir, output_dir, provider, api_key,
                model, quality: Quality::from(quality),
                lang, country, base_url, jobs, correct, enrich, strict_mhlw,
                max_files, qc_script,
            }, log).await?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// API key resolution
// ---------------------------------------------------------------------------

fn resolve_api_key(
    cli_key: Option<String>,
    provider: Provider,
    cfg: &config::AppConfig,
) -> anyhow::Result<String> {
    if let Some(k) = cli_key {
        eprintln!(
            "Warning: passing --api-key on the command line may expose it to other local \
             processes. Prefer setting the environment variable instead."
        );
        return Ok(k);
    }
    let env_var = provider.api_key_env();
    if let Ok(k) = std::env::var(env_var) {
        return Ok(k);
    }
    if provider == Provider::Local {
        return Ok("ollama".to_string());
    }
    // Fall back to config file if provider matches
    if !cfg.api_key.is_empty() && Provider::from_str(&cfg.provider) == provider {
        return Ok(cfg.api_key.clone());
    }
    anyhow::bail!("API key not provided. Set --api-key or {env_var}")
}

// ---------------------------------------------------------------------------
// Batch helpers — CLI only (with indicatif progress bars)
// The per-file work delegates to the shared tasks:: runners.
// ---------------------------------------------------------------------------

fn make_pb(total: u64) -> Arc<ProgressBar> {
    let pb = ProgressBar::new(total);
    pb.set_style(
        ProgressStyle::with_template("[{elapsed_precise}] {bar:40.cyan/blue} {pos}/{len} {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );
    Arc::new(pb)
}

#[allow(clippy::too_many_arguments)]
async fn batch_to_json(
    input_dir: &Path,
    output_dir: &Path,
    provider: Provider,
    api_key: String,
    model: String,
    quality: Quality,
    lang: Option<Language>,
    country: Option<SourceCountry>,
    base_url: Option<String>,
    concurrency: usize,
    enrich: bool,
    correct: bool,
    use_suggested_filename: bool,
) {
    let files = collect_files(input_dir, &["pdf", "docx", "xlsx", "xls"]);
    let total = files.len();
    if total == 0 {
        eprintln!("No files found in {}", input_dir.display());
        return;
    }
    let pb = make_pb(total as u64);
    let output_dir = Arc::new(output_dir.to_path_buf());
    let ok     = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let failed = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    stream::iter(files)
        .map(|path| {
            let pb         = Arc::clone(&pb);
            let output_dir = Arc::clone(&output_dir);
            let ok         = Arc::clone(&ok);
            let failed     = Arc::clone(&failed);
            let api_key    = api_key.clone();
            let model      = model.clone();
            let base_url   = base_url.clone();
            async move {
                let stem = match path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()) {
                    Some(s) => s.to_string(),
                    None => {
                        pb.println(format!("[SKIP] {}: no file stem", path.display()));
                        failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        pb.inc(1);
                        return;
                    }
                };
                let out_path = output_dir.join(format!("{stem}.json"));
                pb.set_message(stem.clone());
                let pb2 = Arc::clone(&pb);
                let log: LogFn = Arc::new(move |msg| { pb2.println(msg); });
                let result = tasks::run_to_json(ToJsonParams {
                    input: path.to_string_lossy().into_owned(),
                    output: out_path,
                    provider, api_key, model, quality, lang, country, base_url, enrich, correct,
                    use_suggested_filename,
                }, log).await;
                match result {
                    Ok(_)  => { ok.fetch_add(1, std::sync::atomic::Ordering::Relaxed); }
                    Err(e) => {
                        pb.println(format!("[ERROR] {}: {e}", path.display()));
                        failed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                pb.inc(1);
            }
        })
        .buffer_unordered(concurrency.max(1))
        .collect::<Vec<_>>()
        .await;

    let ok     = ok.load(std::sync::atomic::Ordering::Relaxed);
    let failed = failed.load(std::sync::atomic::Ordering::Relaxed);
    pb.finish_and_clear();
    eprintln!("Done: {ok} ok, {failed} failed");
}

fn batch_to_docx(
    input_dir: &Path,
    output_dir: &Path,
    lang: Language,
    template: Option<&Path>,
) -> anyhow::Result<()> {
    use sdsconv_core::{convert_from_json, convert_from_template, ConvertConfig};
    let files = collect_files(input_dir, &["json"]);
    let total = files.len();
    if total == 0 { eprintln!("No .json files found"); return Ok(()); }
    let pb = make_pb(total as u64);
    let (mut ok, mut failed) = (0usize, 0usize);
    for path in &files {
        let stem = match path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => { failed += 1; pb.inc(1); continue; }
        };
        pb.set_message(stem.clone());
        let out_path = output_dir.join(format!("{stem}.docx"));
        let result = check_json_file_size(path)
            .and_then(|_| std::fs::read_to_string(path).map_err(anyhow::Error::from))
            .and_then(|raw| {
                // Auto-detect lang per file when the default (Japanese) was not explicitly set.
                let effective_lang = if lang == Language::default() {
                    sdsconv_core::detect_language(&raw)
                } else {
                    lang
                };
                let file_config = ConvertConfig {
                    source_language: None,
                    output_language: effective_lang,
                    ..Default::default()
                };
                serde_json::from_str::<SdsRoot>(&raw)
                    .map_err(anyhow::Error::from)
                    .and_then(|sds| {
                        if let Some(tmpl) = template {
                            convert_from_template(&sds, tmpl, &out_path).map_err(anyhow::Error::from)
                        } else {
                            convert_from_json(&sds, &out_path, &file_config).map_err(anyhow::Error::from)
                        }
                    })
            });
        match result {
            Ok(_)  => ok += 1,
            Err(e) => { pb.println(format!("[ERROR] {}: {e}", path.display())); failed += 1; }
        }
        pb.inc(1);
    }
    pb.finish_and_clear();
    eprintln!("Done: {ok} ok, {failed} failed");
    Ok(())
}

fn batch_to_html(input_dir: &Path, output_dir: &Path, lang: Language) -> anyhow::Result<()> {
    use sdsconv_core::{converter::html::generate_html, ConvertConfig};
    let files = collect_files(input_dir, &["json"]);
    let total = files.len();
    if total == 0 { eprintln!("No .json files found"); return Ok(()); }
    let pb = make_pb(total as u64);
    let config = ConvertConfig { source_language: None, output_language: lang, ..Default::default() };
    let (mut ok, mut failed) = (0usize, 0usize);
    for path in &files {
        let stem = match path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => { failed += 1; pb.inc(1); continue; }
        };
        pb.set_message(stem.clone());
        let out_path = output_dir.join(format!("{stem}.html"));
        let result = check_json_file_size(path)
            .and_then(|_| std::fs::read_to_string(path).map_err(anyhow::Error::from))
            .and_then(|raw| serde_json::from_str::<SdsRoot>(&raw).map_err(anyhow::Error::from))
            .and_then(|sds| generate_html(&sds, config.output_language).map_err(anyhow::Error::from))
            .and_then(|html| std::fs::write(&out_path, html).map_err(anyhow::Error::from));
        match result {
            Ok(_)  => ok += 1,
            Err(e) => { pb.println(format!("[ERROR] {}: {e}", path.display())); failed += 1; }
        }
        pb.inc(1);
    }
    pb.finish_and_clear();
    eprintln!("Done: {ok} ok, {failed} failed");
    Ok(())
}

fn batch_to_pdf(input_dir: &Path, output_dir: &Path, lang: Language) -> anyhow::Result<()> {
    let files = collect_files(input_dir, &["json"]);
    let total = files.len();
    if total == 0 { eprintln!("No .json files found"); return Ok(()); }
    let pb = make_pb(total as u64);
    let (mut ok, mut failed) = (0usize, 0usize);
    for path in &files {
        let stem = match path.file_stem().and_then(|s| s.to_str()).filter(|s| !s.is_empty()) {
            Some(s) => s.to_string(),
            None => { failed += 1; pb.inc(1); continue; }
        };
        pb.set_message(stem.clone());
        let out_path = output_dir.join(format!("{stem}.pdf"));
        let result = check_json_file_size(path)
            .and_then(|_| std::fs::read_to_string(path).map_err(anyhow::Error::from))
            .and_then(|raw| serde_json::from_str::<SdsRoot>(&raw).map_err(anyhow::Error::from))
            .and_then(|sds| {
                sdsconv_core::converter::generate_pdf(&sds, lang)
                    .map_err(anyhow::Error::from)
                    .and_then(|bytes| std::fs::write(&out_path, bytes).map_err(anyhow::Error::from))
            });
        match result {
            Ok(_)  => ok += 1,
            Err(e) => { pb.println(format!("[ERROR] {}: {e}", path.display())); failed += 1; }
        }
        pb.inc(1);
    }
    pb.finish_and_clear();
    eprintln!("Done: {ok} ok, {failed} failed");
    Ok(())
}
