# sds-converter

GUI + CLI tool for **bidirectional conversion** between Safety Data Sheet (SDS) documents (Word/PDF) and the Japanese Ministry of Health, Labour and Welfare (MHLW) standard JSON format.

Supports **Japanese**, **English**, **Simplified Chinese**, and **Traditional Chinese**.

[日本語](README_ja.md) | [中文](README_zh.md)

> **Embedding in your Rust project?** Use [`sds-converter-core`](https://crates.io/crates/sds-converter-core) directly.

---

## Download

| Platform | Download |
|---|---|
| **macOS** (Universal — Apple Silicon + Intel) | [sds-converter-macos.zip](https://github.com/kent-tokyo/sds-converter/releases/latest/download/sds-converter-macos.zip) |
| **Windows** (Portable .exe — no install required) | [sds-converter-windows-portable.zip](https://github.com/kent-tokyo/sds-converter/releases/latest/download/sds-converter-windows-portable.zip) |
| **Rust / CLI** | `cargo install sds-converter` |

→ [All releases & changelogs](https://github.com/kent-tokyo/sds-converter/releases)

---

## GUI Mode

Launch the graphical interface by running `sds-converter` **without any arguments**:

```bash
sds-converter
```

The GUI window (820×640) opens with five tabs:

| Tab | Function |
|---|---|
| **Convert** | SDS document (PDF/DOCX/XLSX/HTML/URL) → MHLW standard JSON |
| **Generate** | MHLW JSON → DOCX / HTML / PDF (with optional DOCX template) |
| **Validate** | Structural validation of MHLW JSON with colored ✅⚠❌ results |
| **Extract Text** | Raw text extraction from documents — no LLM API required |
| **Settings** | API key, model name, base URL, quality, language, UI language |

| Convert tab | Generate tab | Extract Text tab |
|---|---|---|
| ![Convert tab](../docs/tab_convert.png) | ![Generate tab](../docs/tab_generate.png) | ![Extract Text tab](../docs/tab_extract.png) |

**Drag & drop** files onto any tab to fill the input field automatically.

Settings are saved to `~/.config/sds-converter/config.toml` and restored on next launch.
The GUI and CLI share the same conversion engine (`tasks.rs`), so results are identical.

---

## Commands

### `to-json` — Convert PDF/Word → MHLW standard JSON

```bash
# Single file (Anthropic Claude, default)
export ANTHROPIC_API_KEY=sk-ant-...
sds-converter to-json --input input.pdf --output output.json

# Specify source language
sds-converter to-json --input sds_en.pdf --output output.json --lang en

# Batch mode — process a whole directory
sds-converter to-json --input-dir ./pdfs/ --output-dir ./json/ --lang ja

# OpenAI GPT (defaults to gpt-4o-mini)
sds-converter to-json --input input.pdf --output output.json \
  --provider openai --api-key $OPENAI_API_KEY

# Google Gemini (defaults to gemini-2.0-flash)
sds-converter to-json --input input.pdf --output output.json \
  --provider gemini --api-key $GEMINI_API_KEY

# Local LLM via Ollama (any OpenAI-compatible endpoint)
sds-converter to-json --input input.pdf --output output.json \
  --provider local --base-url http://localhost:11434/v1 \
  --model llama3.2 --api-key dummy

# From pre-extracted text (skip PDF parsing)
sds-converter to-json --input extracted.txt --output output.json --lang ja
```

| Flag | Default | Description |
|---|---|---|
| `--input` | — | Input PDF, DOCX, XLSX, or TXT file |
| `--input-dir` | — | Input directory (batch — processes all `.pdf`/`.docx`/`.xlsx`/`.xls`) |
| `--output` | — | Output JSON file |
| `--output-dir` | — | Output directory (batch — created if absent) |
| `--provider` | `anthropic` | LLM provider: `anthropic`, `openai`, `gemini`, `mistral`, `groq`, `cohere`, `local` |
| `--api-key` | env var | API key (see provider defaults below) |
| `--model` | per-provider | Model name override |
| `--base-url` | — | Custom OpenAI-compatible endpoint (for `--provider local`) |
| `--lang` | auto-detect | Source document language: `ja`, `en`, `zh-cn`, `zh-tw` |
| `--quality` | `medium` | Preset: `low` (fast/cheap), `medium`, `high` (accurate) |
| `--concurrency` | `4` | Max parallel files in batch mode |

**Provider defaults:**

| `--provider` | Default model | Environment variable |
|---|---|---|
| `anthropic` | `claude-haiku-4-5-20251001` (low/medium) · `claude-sonnet-4-6` (high) | `ANTHROPIC_API_KEY` |
| `openai` | `gpt-4o-mini` | `OPENAI_API_KEY` |
| `gemini` | `gemini-2.0-flash` | `GEMINI_API_KEY` |
| `mistral` | `mistral-small-latest` | `MISTRAL_API_KEY` |
| `groq` | `llama-3.3-70b-versatile` | `GROQ_API_KEY` |
| `cohere` | `command-r-plus` | `COHERE_API_KEY` |
| `local` | `llama3` | `LOCAL_LLM_API_KEY` (optional; defaults to `ollama`) |

### `to-docx` — Convert MHLW standard JSON → Word document

```bash
# Single file (built-in layout)
sds-converter to-docx --input output.json --output result.docx --lang ja

# Batch mode (built-in layout)
sds-converter to-docx --input-dir ./json/ --output-dir ./docx/ --lang en

# Fill a Word template with {{Placeholder}} substitution
sds-converter to-docx --input output.json --output result.docx \
  --template my_template.docx

# Batch mode with template
sds-converter to-docx --input-dir ./json/ --output-dir ./docx/ \
  --template my_template.docx
```

#### Word template format

Prepare a `.docx` file with `{{FieldName}}` placeholders where `FieldName` is
a leaf key from the MHLW JSON schema. The full dot-path is also accepted for
disambiguation.

```
{{TradeNameJP}}          → 製品和名
{{CompanyName}}          → 会社名
{{Phone}}                → 電話番号
{{IssueDate}}            → 発行日
{{Identification.SupplierInformation.CompanyName}}  → フルパス指定
```

Placeholders can appear anywhere in the document — paragraphs, table cells,
headers, and footers. Word sometimes splits typed text across internal runs;
the tool automatically merges such splits before substitution.

| Flag | Default | Description |
|---|---|---|
| `--input` | — | Input JSON file |
| `--input-dir` | — | Input directory (batch — processes all `.json`) |
| `--output` | — | Output DOCX file |
| `--output-dir` | — | Output directory (batch) |
| `--lang` | `ja` | Output language: `ja`, `en`, `zh-cn`, `zh-tw` (without `--template`) |
| `--template` | — | Word template with `{{FieldName}}` placeholders |

### `extract-text` — Extract raw text from PDF/DOCX

Extracts the text that the LLM would receive, without making any API call. Useful for inspecting extraction quality or running the LLM step separately.

```bash
# Save to file
sds-converter extract-text --input input.pdf --output extracted.txt

# Print to stdout
sds-converter extract-text --input input.pdf

# Then feed back into to-json
sds-converter to-json --input extracted.txt --output output.json --lang ja
```

### `validate` — Check a JSON file for structural issues

```bash
# Human-readable output (exits 0 = OK, 1 = warnings found)
sds-converter validate --input output.json

# JSON array output for CI/scripting
sds-converter validate --input output.json --json
```

Checks that key sections (Identification, HazardIdentification, ToxicologicalInformation, etc.) are populated. Exits with code `1` if any issues are found.

---

## Language Support

| Language | `--lang` | Source documents | Output DOCX headings |
|---|---|---|---|
| Japanese | `ja` | JIS Z 7253 compliant SDS | JIS Z 7253 |
| English | `en` | GHS/OSHA HazCom format | GHS Rev.10 / ISO 11014 |
| Simplified Chinese | `zh-cn` | GB/T 16483 format | GB/T 16483-2012 |
| Traditional Chinese | `zh-tw` | CNS 15030 format | CNS 15030 |

---

## Requirements

- Rust 1.75+
- An LLM API key (for `to-json` only) — set the provider's environment variable or pass `--api-key`
  - Anthropic: `ANTHROPIC_API_KEY`
  - OpenAI: `OPENAI_API_KEY`
  - Google Gemini: `GEMINI_API_KEY`
  - Mistral: `MISTRAL_API_KEY`
  - Groq: `GROQ_API_KEY`
  - Cohere: `COHERE_API_KEY`
  - Local LLM (Ollama etc.): use `--provider local --base-url <url>` (no API key required)
- Input files must be **text-based** PDF or DOCX
  - Encrypted PDFs are not supported
  - Scanned/image-only PDFs are not supported (no text to extract)

---

## Rust Library

The conversion engine is available as a standalone library:

| Crate | crates.io | Description |
|---|---|---|
| `sds-converter-core` | [`sds-converter-core`](https://crates.io/crates/sds-converter-core) | LLM-based extraction, DOCX/HTML generation, MHLW schema |

```toml
[dependencies]
sds-converter-core = "0.2"
```

---

## References

- [MHLW — SDS Standard Data Exchange Format (official page)](https://www.mhlw.go.jp/stf/newpage_56484.html) (Japanese)
- [SDS Data Exchange Format Developer Manual (PDF)](https://www.mhlw.go.jp/content/11305000/001467068.pdf) (Japanese)

---

## License

Licensed under either of:
- Apache License, Version 2.0
- MIT License

at your option.
