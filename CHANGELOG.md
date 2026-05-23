# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
