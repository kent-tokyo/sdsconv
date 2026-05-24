# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- **GUI minimum window size** (`app.rs`): Added `with_min_inner_size([640.0, 480.0])` to `ViewportBuilder` so the window cannot be resized below a usable minimum (H1).
- **Busy-poll repaint interval reduced** (`app.rs`): Background task polling interval reduced from 100 ms to 250 ms, cutting CPU usage by ~60% during long conversions while keeping the UI responsive (M8).
- **Log panel height uncapped** (`app.rs`): Removed `.max_height(160.0)` from the log `ScrollArea`; the panel now fills the available vertical space freely (M2).
- **TextEdit fields are now responsive** (`app.rs`): All nine input fields replaced `add_sized([W, 20.0], ...)` with `desired_width(avail_width - offset).max(150.0)`, making them resize with the window (M10).
- **Frame::NONE** (`app.rs`): Replaced deprecated `egui::Frame::none()` with `egui::Frame::NONE` (M1).

### Added

- **Keyboard shortcuts** (`app.rs`): Ctrl+Q quits the application, F1 opens the manual window, Ctrl+O opens the file picker for the current tab (H3).
- **API key show/hide toggle** (`app.rs`): A small button next to the API key field toggles password masking; icon label switches between the locale's "Show" and "Hide" strings (L1).
- **Error modal and About dialog use `egui::Modal`** (`app.rs`): Both dialogs are now true backdrop modals handled by egui — Escape and backdrop click close them automatically via `ModalResponse::should_close()` (H4, M7).
- **Settings saved message auto-clears** (`app.rs`): After clicking Save, `settings_saved_at` records the instant and `ctx.request_repaint_after(3 s + 50 ms)` is called to guarantee a repaint fires even when the user is idle. The `logic()` method clears the message once 3 seconds have elapsed (M9).
- **Drag-and-drop file type validation** (`app.rs`): Dropped files are partitioned into accepted/rejected by extension using `Vec::partition()`. Accepted extensions are tab-aware (e.g. JSON-only on Generate/Validate tabs). Rejected files trigger a localised log warning (L2, L3).
- **Hint text on all input fields** (`app.rs`): Every `TextEdit::singleline` now carries a `.hint_text(...)` placeholder describing the expected input (M4).
- **i18n for file-dialog filter labels** (`app.rs`, `Strings`): All `add_filter("SDS", ...)` / `add_filter("JSON", ...)` etc. now use locale-specific `lbl_filter_*` strings from the `Strings` struct (M5).
- **i18n for OK/Skip/Show/Hide buttons** (`app.rs`, `Strings`): Welcome screen Skip button and modal OK buttons use `s.btn_skip` / `s.btn_ok`; API key toggle uses `s.btn_show_key` / `s.btn_hide_key` (M6, L4).
- **Strings struct extended** (`app.rs`): Added 10 new fields — `btn_ok`, `btn_skip`, `btn_show_key`, `btn_hide_key`, `msg_drop_rejected`, `lbl_filter_sds`, `lbl_filter_json`, `lbl_filter_doc`, `lbl_filter_word`, `lbl_filter_txt`; dead `btn_clear_files` field removed. All three locales (en/ja/zh-cn) updated.

### Fixed

- **Empty-input/output validation now uses error modal** (`app.rs`): `start_convert`, `start_generate`, and `start_validate` previously wrote "no input/output path" as a plain log message. They now set `self.error_modal` so the error surfaces as a blocking dialog (H2).
- **Log clone eliminated** (`app.rs`): Log rendering previously cloned the entire `Vec<String>` before releasing the mutex. The `ScrollArea` now renders directly while holding the `MutexGuard`, saving an allocation on every frame (M3).
- **Redundant "clear files" buttons removed** (`app.rs`): Small "clear files" buttons in the Convert and Validate batch-mode panels were redundant — switching to single mode already clears the list. Both buttons and the dead `btn_clear_files` Strings field have been removed (L7).

### Added (continued from prior unreleased)

- **Automated Windows & macOS release builds** (`.github/workflows/release.yml`): pushing a `v*.*.*` tag now automatically builds `sds-converter-windows-portable.zip` (Windows x86_64) and `sds-converter-macos.zip` (macOS Universal — Apple Silicon + Intel) and attaches them to the GitHub Release. Homebrew Cask auto-update is skipped gracefully when `HOMEBREW_TAP_TOKEN` is not configured.

### Security

- **Timing-safe Bearer token comparison** (`sds_converter_server`): replaced `t == token.as_str()` with `constant_time_eq(t.as_bytes(), token.as_bytes())` to eliminate timing side-channel attacks on the authentication check. New dependency: `constant_time_eq = "0.3"`.
- **HTTP redirect disabled on URL fetch client** (`extractor.rs`): `shared_http_client` now sets `.redirect(Policy::none())`. Previously, an attacker could bypass the SSRF `is_private_host` guard by redirecting through a public URL to a private address.
- **IPv6 SSRF guard extended** (`extractor.rs`): `is_private_host` now blocks `fc00::/7` (ULA unique-local), `fe80::/10` (link-local), and `::ffff:` IPv4-mapped addresses whose embedded IPv4 is loopback/private/link-local. Previously only `::1` and `::` were blocked.
- **Upload size limit reduced to 50 MB** (`sds_converter_server`): `DefaultBodyLimit` lowered from 512 MB to 50 MB — sufficient for any real SDS document. The previous limit allowed DoS via memory exhaustion on large uploads.
- **REST API server now requires authentication** (`sds_converter_server`): Bearer token auth via `SDS_SERVER_TOKEN` env var (auto-generates and prints a random token if not set). Default bind address changed from `0.0.0.0` to `127.0.0.1` (`SDS_SERVER_BIND` override)
- **CORS restricted to localhost** (`sds_converter_server`): replaced `CorsLayer::permissive()` with an allowlist of `http://localhost` and `http://127.0.0.1` only
- **Concurrency cap on REST server** (`sds_converter_server`): `ConcurrencyLimitLayer(10)` prevents resource exhaustion from concurrent requests
- **SSRF protection** (`extractor.rs`): URL fetches now reject private/loopback/link-local/metadata IP addresses and hostnames (`localhost`, `169.254.x.x`, RFC-1918 ranges, `::1`) before issuing the HTTP request
- **Prompt injection mitigation strengthened** (`llm.rs`): `</document>` occurrences in document text are escaped to `</_document>` before insertion into LLM user messages
- **LLM error body no longer forwarded to API clients** (`sds_converter_server`): full provider error responses are logged server-side only; clients receive a sanitized `{"error": "LLM API request failed", "status": N}` response

### Added

- **MHLW §3.3 compliance — empty field pruning** (`tasks.rs`): `prune_empty_strings` post-processes LLM output to remove `""`, `[]`, and `{}` values before writing the JSON file
- **Recommended filename output** (`tasks.rs`, `config.rs`, `app.rs`, `main.rs`): `--suggested-name` CLI flag and GUI Settings checkbox output the file as `SDS_<IssueDate>_<ProductCode>.json` per the MHLW §2.1.2 naming convention. Filename conflicts are resolved atomically
- **`SdsError::display_safe()`** (`error.rs`): new method that returns a sanitized error message safe for external/client display (strips LLM provider error bodies)
- **`ProductNoUser` extraction** (`llm.rs`): added `ProductNoUser` array to the MHLW schema example hint so LLMs extract the field
- **Scanned PDF OCR fallback** (`extractor.rs`): When `pdf_extract` yields fewer than 200 characters
  (image-only / scanned PDF), the extractor automatically shells out to `pdftoppm` (poppler) +
  `tesseract` CLI. Pages are rasterised at 300 dpi, OCR'd with `jpn+eng` (falls back to `eng` if the
  Japanese pack is absent), and the combined text is fed into the normal LLM pipeline. If neither
  tool is installed, a clear install hint is returned as the error message. No new crate dependencies.

- **GUI application (eframe/egui)**: Hybrid launcher — runs as a GUI window when invoked with no
  arguments; falls back to CLI mode when arguments are present. Cross-platform (macOS, Windows,
  Linux). Five tabs: SDS→JSON Convert, Document Generate, Validate, Extract Text, Settings.

- **Extract Text tab** (`app.rs`, `tasks.rs`): New fifth tab for raw text extraction without
  LLM. Accepts local files (PDF, DOCX, XLSX, TXT, HTML) and URLs (`http://`/`https://`). Result
  is shown inline (capped at 50,000 characters) with an option to save to a file.
  `run_extract_text` function added to `tasks.rs`.

- **Model name and base URL fields** (`app.rs`, `config.rs`): Settings tab now exposes model name
  and base URL text fields. `config.base_url: String` added to `AppConfig`; both values are
  forwarded to `ToJsonParams` at conversion time.

- **DOCX template picker** (`app.rs`): Generate tab shows a template file picker when the DOCX
  output format is selected. The chosen path maps to the existing `ToDocxParams.template` field.

- **Drag & drop input** (`app.rs`): Files dropped anywhere on the window are routed to the
  appropriate tab's input field. A semi-transparent overlay reading "Drop files here" appears on
  hover.

- **Settings persistence** (`config.rs`): App configuration is written to and read from
  `~/.config/sds-converter/config.toml` (created with Unix 0o600 permissions). Includes API keys,
  provider, model name, base URL, output directory, language, and quality preset.

- **BusyGuard RAII, error modals, and log panel** (`app.rs`): `BusyGuard` ensures the busy flag
  is always cleared on drop. Recoverable errors surface as modal dialogs. A collapsible log panel
  retains the last 500 lines.

- **Batch mode** (`app.rs`): Convert and Validate tabs support multi-file batch processing.

- **Multi-language UI** (`app.rs`): Interface strings are available in Japanese, English, and
  Simplified Chinese; selected via the Settings tab.

- **Provider API key links and onboarding banner** (`app.rs`): Settings tab displays clickable
  links to the API key page for each provider. First-run onboarding banner guides new users.

### Fixed

- **`repair_json` corrupted string values containing `,}` patterns** (`llm.rs`): the blind `str::replace(",}", "}")` loop rewrote JSON string values such as `"note": "ends here,}"`, producing invalid JSON. Replaced with a byte-level state machine (`remove_trailing_commas`) that tracks `in_string` state (including `\"` escape handling), wrapped in a fixpoint loop. Existing tests still pass; 3 new regression tests added.
- **Silent discard of retry-parse errors** (`llm.rs`): both the text-extraction retry (`llm.rs:660`) and the vision-path retry (`llm.rs:926`) used `if let Ok(...)` on the result of `lenient_deserialize`, silently swallowing parse errors. Replaced with `match` + `Err(e) => tracing::warn!(...)` so failures are always visible in logs.
- **False-positive chemical name matching** (`enrichment.rs`): `names_similar` used substring containment (`a.contains(&b) || b.contains(&a)`), causing short generic words (e.g. `"acid"`) to match unrelated names (e.g. `"hydrochloric acid"`). Replaced with Jaccard word-overlap (intersection/union ≥ 0.5). 5 new unit tests added.
- **`section!` macro schema-mismatch warning lacks context** (`llm.rs`): the `WARN` log only reported the serde error message. Now also logs the first 200 characters of the failing JSON value, making it much easier to diagnose LLM output schema drift.
- **`/api/health` blocked by auth middleware** (`sds_converter_server`): The `require_auth` middleware was applied via `.layer()` to the entire router, causing `GET /api/health` to return 401 for unauthenticated callers (e.g. AWS LWA / load-balancer health checks). Fixed by splitting into a protected router (`.route_layer(require_auth)`) merged with a public router containing only the health route.
- **Japanese CID font PDF panic** (`extractor.rs`): `pdf-extract` panics with `FromUtf8Error` when processing PDFs that use CID fonts (e.g. Shift-JIS encoded Japanese text). The panic was caught by `spawn_blocking` and silently converted to an empty string, causing unnecessary OCR fallback. Added `pdftotext -utf8` (poppler) as a middle tier between `pdf-extract` and OCR: full 3-tier fallback chain is now `pdf-extract` -> `pdftotext` -> tesseract/Vision. `pdftotext` is silently skipped if poppler is not installed.
- URL response body now capped at 50 MB (Content-Length pre-check + streaming byte cap) to prevent OOM on large responses
- CJK text truncation: `out.len()` (byte count) was compared against `max_chars` (character count), causing Japanese text to be cut at 1/3 the intended length. Fixed to use `chars().count()`
- Blocking `std::fs::read` replaced with `tokio::task::spawn_blocking` inside `convert_pdf_to_json_vision` to avoid stalling the Tokio executor during image-only PDF processing
- Log panel now enforces the documented 500-line maximum (was unbounded despite the "max 500" label)
- `start_generate` now validates the output path before spawning the async task, matching the guard in `start_convert`
- Validation result "No issues" message now uses the i18n `Strings` struct instead of a hardcoded Japanese literal
- TOCTOU race in `resolve_unique_suggested_path`: replaced `exists()` check with atomic `OpenOptions::create_new(true)` to prevent concurrent batch runs from overwriting each other's output
- PubChem enrichment no longer silently drops results on HTTP 429: adds 250 ms inter-request delay and retries once with 1 s backoff

## [0.2.0] - 2026-05-23

### Fixed

- **CJK character corruption in template filling** (`template.rs`): `normalize_split_runs` was
  casting individual UTF-8 bytes to `char`, corrupting multi-byte characters such as Japanese,
  Chinese, and Korean text. Rewrote the function to use string-slice–based output, preserving
  full Unicode text outside placeholders.

- **EcologicalInformation data silently dropped** (`llm.rs`): The LLM schema hint used
  `"AquaticToxicity"` (non-existent key) instead of `"AquaticAcuteToxicity"` /
  `"AquaticChronicToxicity"`. serde silently discarded the data on deserialization. Fixed hint to
  match the actual `generated.rs` struct. Similarly corrected `PersistenceDegradability` field
  names (`BiologicalDegradability`, `AbioticDegradation`, `RapidDegradability`).

- **`--quality high` fails with HTTP 400** (`llm.rs`): Anthropic claude-sonnet-4-x and newer
  models reject requests that end with an assistant turn (prefill). Removed the assistant prefill
  message from `AnthropicBackend::complete`; `strip_json_fences` already handles any extra
  wrapping the model may add.

- **Sync I/O blocking the tokio thread pool** (`extractor.rs`): DOCX, TXT, and XLSX extraction
  were calling blocking `std::fs` APIs directly inside `async` functions. Wrapped each format in
  `tokio::task::spawn_blocking`, matching the existing PDF path.

- **Batch error counter not incremented on JSON serialize / write failure** (`main.rs`): Errors
  during `serde_json::to_string_pretty` and `std::fs::write` inside `batch_to_json` were silently
  ignored. Both paths now increment `failed` and emit an `[ERROR]` message.

- **`flatten_sds` silently returns empty map on serialization failure** (`template.rs`): Changed
  return type from `HashMap<String, String>` to `Result<HashMap<String, String>, SdsError>` and
  propagated the error with `?`.

- **Separator filter counts bytes instead of characters** (`extractor.rs`): `trimmed.len() >= 3`
  was a byte-length check; replaced with `trimmed.chars().count() >= 3` for correctness with
  multi-byte CJK separator characters.

### Added

- **GHS Rev.10 H/P code validation** (`ghs_codes.rs`, `validator.rs`): New `ghs_codes` module
  with static lookup tables for all valid H-codes (H200–H420) and P-codes (P101–P503).
  `validate` now checks every `HazardStatementCode` and `PrecautionaryStatementCode` against
  the database and emits a warning for any unrecognised code. Compound P-codes like
  `P301+P330+P331` are split on `+` before validation.

- **CAS number format + check-digit validation** (`validator.rs`): `validate` now checks every
  CAS number in `CompositionAndConcentration` for correct `\d{2-7}-\d{2}-\d` format and a
  valid check digit (weighted sum mod 10), emitting a warning for each malformed entry.

- **PubChem CAS enrichment** (`enrichment.rs`): New `enrichment` module with `lookup_cas` /
  `enrich_composition` API. The CLI `to-json` command now accepts an `--enrich` flag that
  looks up each CAS number in PubChem and reports mismatches between the PubChem IUPAC name
  and the SDS substance name.

- **HTML/URL input support** (`extractor.rs`): `to-json` and `extract-text` now accept a URL
  (`http://` / `https://`) as `--input` in addition to local files. HTML files (`.html`/`.htm`)
  are also supported. Text is extracted with `scraper` (table cells tab-separated, noise elements
  skipped). New dependency: `scraper = "0.21"`.

- **`to-html` subcommand** (`html.rs`, `main.rs`): Generates a self-contained UTF-8 HTML5
  document from MHLW standard JSON with inline CSS (including `@media print`). Object fields
  are rendered as key-value tables; `CompositionAndConcentration` and other object arrays are
  rendered as column tables. Batch mode (`--input-dir`) supported.

- **`to-pdf` subcommand** (`main.rs`): Converts MHLW standard JSON to PDF via
  `soffice --headless --convert-to pdf` (LibreOffice). Requires `soffice` in PATH; fails with
  a clear error message if not found. Batch mode (`--input-dir`) supported.

- **Input file size limits**: Input files are now rejected before reading if they exceed limits —
  500 MB for PDF / DOCX / XLSX, 100 MB for TXT / JSON — preventing OOM on oversized uploads.

- **ZIP bomb protection in template filling** (`template.rs`): Template files are checked against
  a 50 MB limit before opening; individual ZIP entries are read through `Read::take(100 MB)`.

- **HTTP timeouts on LLM backends** (`llm.rs`): Both `AnthropicBackend` and
  `OpenAiCompatBackend` now set a 120 s total timeout and a 10 s connect timeout via
  `reqwest::Client::builder`.

- **Prompt injection mitigation** (`llm.rs`): Document text is wrapped in
  `<document>…</document>` tags and the system prompt instructs the model to treat that content
  as data only.

- **Explicit schema mismatch warnings** (`llm.rs`): When a section's JSON value cannot be
  deserialized into its target struct, a `WARN` log is emitted rather than silently substituting
  an empty default.

- **Retry failure logging** (`llm.rs`): If the retry LLM call also fails, the error is now
  logged at `WARN` level instead of being silently ignored.

- **API key exposure warning** (`main.rs`): Passing `--api-key` on the command line now prints a
  warning recommending the use of environment variables instead.

- **`bounds check` in DOCX generator** (`generator.rs`): An `assert_eq!` at the top of
  `generate_docx` verifies that `SECTION_NAMES` and `SECTION_KEYS` have equal length, catching
  accidental mismatches at startup.

- **`check_json_file_size` helper** (`main.rs`): Validates JSON input size before reading in
  `validate`, `to-docx` single-file, and `batch_to_docx` modes.

### Changed

- **O(N²) → O(N) placeholder substitution** (`template.rs`): `apply_substitutions` previously
  called `String::replace` once per key (hundreds of iterations × document size). Replaced with a
  single forward scan using `str::find("{{")`.

- **`repair_json` skipped for already-valid JSON** (`llm.rs`): `lenient_deserialize` now tries
  `serde_json::from_str` first and only falls back to `repair_json` when the initial parse fails,
  avoiding an unnecessary allocation on every successful response.

- **Eliminated `clone()` in section deserialization** (`llm.rs`): `lenient_deserialize` now uses
  `map.remove(key)` to take ownership of each section value, removing the `v.clone()` call before
  `serde_json::from_value`.

- **`tracing::debug!` → `tracing::trace!` for LLM output** (`llm.rs`): The full JSON response
  body is now logged only at `TRACE` level, keeping document content out of normal `DEBUG` logs.

- **Shared `send_with_retry` helper** (`llm.rs`): Extracted duplicate retry-and-backoff logic
  from `AnthropicBackend` and `OpenAiCompatBackend` into a single `send_with_retry` function.

- **`collect_files` helper** (`main.rs`): Extracted duplicate directory-traversal logic from
  `batch_to_json` and `batch_to_docx` into a shared helper.

- **I/O errors annotated with file paths** (`main.rs`): `std::fs::read_to_string` and
  `std::fs::write` calls in single-file modes now attach the file path via `anyhow::Context`.

- **`escape_xml` handles quotes** (`template.rs`): Added `"` → `&quot;` and `'` → `&apos;`
  escaping, which was previously missing.

- **`is_content_xml` simplified** (`template.rs`): Removed redundant explicit matches for
  `header1/2/3.xml` and `footer1/2/3.xml`; the `starts_with` predicates already cover them.

- **`unwrap()` on URL construction removed** (`llm.rs`): `OpenAiCompatBackend::openai()` and
  `::gemini()` now embed URL literals directly instead of calling `.unwrap()` on
  `openai_compat_url()`.

- **Empty file-stem guard in batch modes** (`main.rs`): Files with an empty or missing stem are
  now skipped with a `[SKIP]` message and counted as failures instead of silently producing
  dot-files.

## [0.1.2] - 2025-05-21

### Added

- Multi-language README files (ja, en, zh-CN, zh-TW) with MHLW reference links
- Cargo.toml SEO metadata improvements (keywords, categories, documentation links)

## [0.1.1] - 2025-05-15

### Added

- Word template filling (`to-docx --template`): replaces `{{FieldName}}` placeholders in `.docx`
  templates with values extracted from MHLW standard JSON
- Full-path placeholder support (`{{Section.SubSection.Field}}`)
- Run-split normalization: handles Word's tendency to split typed words across `<w:r>` runs
- XML-safe value escaping (`&`, `<`, `>`)

## [0.1.0] - 2025-05-01

### Added

- Initial release: bidirectional conversion between SDS documents and MHLW standard JSON
- LLM-based extraction from PDF, DOCX, TXT, and XLSX inputs
- DOCX generation from JSON (JIS Z 7253 16-section format, 4 languages)
- Parallel batch conversion with configurable concurrency
- Multi-provider LLM support: Anthropic, OpenAI, Gemini, Mistral, Groq, Cohere, Ollama
- Quality presets (low / medium / high) with per-preset model and token-limit defaults
- `validate` subcommand for structural JSON checking
- `extract-text` subcommand for raw text extraction without LLM
- Exponential backoff retry on HTTP 429 / 529 responses
