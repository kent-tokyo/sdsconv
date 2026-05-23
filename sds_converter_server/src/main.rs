//! REST API server for sds-converter.
//!
//! # Environment variables
//! - `PORT`                — listen port (default 3000)
//! - `ANTHROPIC_API_KEY`   — Anthropic API key
//! - `OPENAI_API_KEY`      — OpenAI API key
//! - `GEMINI_API_KEY`      — Google Gemini API key
//! - `MISTRAL_API_KEY`, `GROQ_API_KEY`, `COHERE_API_KEY`, `LOCAL_LLM_API_KEY`
//!
//! # Endpoints
//! - `GET  /api/health`    — liveness probe
//! - `POST /api/to-json`   — multipart SDS document → MHLW JSON
//! - `POST /api/to-docx`   — JSON body (SdsRoot) → DOCX download
//! - `POST /api/to-html`   — JSON body (SdsRoot) → HTML
//! - `POST /api/validate`  — JSON body (SdsRoot) → warning list

use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Query},
    http::{header, Response, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use sds_converter_core::{
    convert_bytes_to_json, convert_from_json, converter::html::generate_html,
    enrich_composition, openai_compat_url, validate, AnthropicBackend, ConvertConfig,
    Language, LlmBackend, LlmConfig, OpenAiCompatBackend, SdsError, SdsRoot,
};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

struct AppError(anyhow::Error);

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let body = json!({"error": self.0.to_string()});
        (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self { AppError(e) }
}
impl From<SdsError> for AppError {
    fn from(e: SdsError) -> Self { AppError(anyhow::anyhow!("{e}")) }
}
impl From<axum::extract::multipart::MultipartError> for AppError {
    fn from(e: axum::extract::multipart::MultipartError) -> Self {
        AppError(anyhow::anyhow!("{e}"))
    }
}
impl From<reqwest::Error> for AppError {
    fn from(e: reqwest::Error) -> Self { AppError(anyhow::anyhow!("{e}")) }
}
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self { AppError(anyhow::anyhow!("{e}")) }
}

type ApiResult<T> = Result<T, AppError>;

// ---------------------------------------------------------------------------
// LLM backend enum-dispatch (mirrors tasks.rs in the CLI crate)
// ---------------------------------------------------------------------------

enum Backend {
    Anthropic(AnthropicBackend),
    OpenAiCompat(OpenAiCompatBackend),
}

impl LlmBackend for Backend {
    async fn complete(&self, system: &str, user: &str) -> Result<String, SdsError> {
        match self {
            Self::Anthropic(b)    => b.complete(system, user).await,
            Self::OpenAiCompat(b) => b.complete(system, user).await,
        }
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn api_key_env(provider: &str) -> &'static str {
    match provider {
        "openai"  => "OPENAI_API_KEY",
        "gemini"  => "GEMINI_API_KEY",
        "mistral" => "MISTRAL_API_KEY",
        "groq"    => "GROQ_API_KEY",
        "cohere"  => "COHERE_API_KEY",
        "local"   => "LOCAL_LLM_API_KEY",
        _         => "ANTHROPIC_API_KEY",
    }
}

fn resolve_api_key(provider: &str) -> anyhow::Result<String> {
    let env = api_key_env(provider);
    std::env::var(env)
        .ok()
        .filter(|k| !k.is_empty())
        .ok_or_else(|| anyhow::anyhow!("Server env var {env} is not set for provider '{provider}'"))
}

fn quality_max_chars(quality: &str) -> usize {
    match quality {
        "low"  => 15_000,
        "high" => 60_000,
        _      => 30_000,
    }
}

fn default_model(provider: &str, quality: &str) -> &'static str {
    match provider {
        "openai"  => "gpt-4o-mini",
        "gemini"  => "gemini-2.0-flash",
        "mistral" => "mistral-small-latest",
        "groq"    => "llama-3.3-70b-versatile",
        "cohere"  => "command-r-plus",
        "local"   => "llama3",
        _ => match quality {
            "high" => "claude-sonnet-4-6",
            _      => "claude-haiku-4-5-20251001",
        },
    }
}

fn build_backend(provider: &str, api_key: String, llm_config: LlmConfig) -> Backend {
    match provider {
        "gemini" => Backend::OpenAiCompat(OpenAiCompatBackend::gemini(api_key, llm_config)),
        p => match openai_compat_url(p) {
            Some(url) => Backend::OpenAiCompat(
                OpenAiCompatBackend::new(api_key, llm_config, url.to_string()),
            ),
            None => Backend::Anthropic(AnthropicBackend::new(api_key, llm_config)),
        },
    }
}

fn parse_lang(s: Option<&str>) -> Option<Language> {
    match s? {
        "ja"    => Some(Language::Japanese),
        "en"    => Some(Language::English),
        "zh-cn" => Some(Language::ChineseSimplified),
        "zh-tw" => Some(Language::ChineseTraditional),
        _       => None,
    }
}

// ---------------------------------------------------------------------------
// Route: GET /api/health
// ---------------------------------------------------------------------------

async fn health() -> Json<serde_json::Value> {
    Json(json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ---------------------------------------------------------------------------
// Route: POST /api/to-json
// Accepts: multipart/form-data  { file: <bytes>, filename: <str> }
// Query:   provider, quality, lang, enrich
// Returns: application/json (SdsRoot) + X-Warnings header
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ToJsonQuery {
    provider: Option<String>,
    quality:  Option<String>,
    lang:     Option<String>,
    enrich:   Option<bool>,
}

async fn to_json(
    Query(q): Query<ToJsonQuery>,
    mut multipart: Multipart,
) -> ApiResult<impl IntoResponse> {
    let provider = q.provider.as_deref().unwrap_or("anthropic");
    let quality  = q.quality.as_deref().unwrap_or("medium");

    let api_key = resolve_api_key(provider).map_err(anyhow::Error::from)?;

    // -- Read multipart fields --
    let mut file_bytes: Option<Vec<u8>> = None;
    let mut filename = String::from("upload.pdf");

    while let Some(field) = multipart.next_field().await? {
        match field.name().unwrap_or("") {
            "file" => {
                if let Some(fname) = field.file_name() {
                    filename = fname.to_string();
                }
                file_bytes = Some(field.bytes().await?.to_vec());
            }
            "filename" => {
                filename = field.text().await.unwrap_or(filename);
            }
            _ => {}
        }
    }

    let data = file_bytes
        .ok_or_else(|| anyhow::anyhow!("Missing 'file' field in multipart body"))?;

    let llm_config = LlmConfig {
        model:      default_model(provider, quality).to_string(),
        max_tokens: 16_384,
    };
    let config = ConvertConfig {
        source_language: parse_lang(q.lang.as_deref()),
        output_language: Language::default(),
        max_chars:       quality_max_chars(quality),
    };
    let backend = build_backend(provider, api_key, llm_config);

    let (sds, warnings) = convert_bytes_to_json(&data, &filename, &backend, &config).await?;

    if q.enrich.unwrap_or(false) {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()?;
        enrich_composition(&sds, &client).await;
    }

    let warnings_str = warnings.join("; ");
    let mut response = Json(sds).into_response();
    if !warnings_str.is_empty() {
        if let Ok(hval) = warnings_str.parse() {
            response.headers_mut().insert("X-Warnings", hval);
        }
    }
    Ok(response)
}

// ---------------------------------------------------------------------------
// Route: POST /api/to-docx
// Accepts: application/json (SdsRoot)
// Query:   lang
// Returns: application/vnd.openxmlformats… binary
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LangQuery {
    lang: Option<String>,
}

async fn to_docx(
    Query(q): Query<LangQuery>,
    Json(sds): Json<SdsRoot>,
) -> ApiResult<impl IntoResponse> {
    let lang   = parse_lang(q.lang.as_deref()).unwrap_or(Language::Japanese);
    let config = ConvertConfig {
        source_language: None,
        output_language: lang,
        ..Default::default()
    };

    let bytes = tokio::task::spawn_blocking(move || -> anyhow::Result<Vec<u8>> {
        let tmp = tempfile::Builder::new().suffix(".docx").tempfile()?;
        convert_from_json(&sds, tmp.path(), &config)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(std::fs::read(tmp.path())?)
    })
    .await
    .map_err(|e| anyhow::anyhow!("task panicked: {e}"))??;

    let response = Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        )
        .header(
            header::CONTENT_DISPOSITION,
            r#"attachment; filename="sds.docx""#,
        )
        .body(Body::from(bytes))
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(response)
}

// ---------------------------------------------------------------------------
// Route: POST /api/to-html
// Accepts: application/json (SdsRoot)
// Query:   lang
// Returns: text/html
// ---------------------------------------------------------------------------

async fn to_html(
    Query(q): Query<LangQuery>,
    Json(sds): Json<SdsRoot>,
) -> ApiResult<impl IntoResponse> {
    let lang = parse_lang(q.lang.as_deref()).unwrap_or(Language::Japanese);

    let html = tokio::task::spawn_blocking(move || generate_html(&sds, lang))
        .await
        .map_err(|e| anyhow::anyhow!("task panicked: {e}"))??;

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html; charset=utf-8")
        .body(Body::from(html))
        .map_err(|e| anyhow::anyhow!("{e}"))?)
}

// ---------------------------------------------------------------------------
// Route: POST /api/validate
// Accepts: application/json (SdsRoot)
// Returns: application/json (array of warning strings)
// ---------------------------------------------------------------------------

async fn validate_handler(Json(sds): Json<SdsRoot>) -> Json<Vec<String>> {
    Json(validate(&sds))
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);

    // 512 MB upload limit
    let body_limit = DefaultBodyLimit::max(512 * 1024 * 1024);

    let app = Router::new()
        .route("/api/health",   get(health))
        .route("/api/to-json",  post(to_json))
        .route("/api/to-docx",  post(to_docx))
        .route("/api/to-html",  post(to_html))
        .route("/api/validate", post(validate_handler))
        .layer(body_limit)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http());

    let addr = format!("0.0.0.0:{port}");
    tracing::info!("sds-converter API server listening on http://{addr}");

    let listener = tokio::net::TcpListener::bind(&addr).await
        .unwrap_or_else(|e| panic!("cannot bind to {addr}: {e}"));
    axum::serve(listener, app).await
        .unwrap_or_else(|e| panic!("server error: {e}"));
}
