# sds-converter-core

A Rust library for **bidirectional conversion** between Safety Data Sheet (SDS) documents (Word/PDF) and the Japanese Ministry of Health, Labour and Welfare (MHLW) standard JSON format.

Supports documents in **Japanese**, **English**, **Simplified Chinese**, and **Traditional Chinese**.

[日本語](README_ja.md) | [中文](README_zh.md)

> **Looking for the CLI?** Install [`sds-converter`](https://crates.io/crates/sds-converter) instead.

---

## Features

- **SDS document → JSON**: Extracts text from PDF/DOCX and converts it to the MHLW SDS data exchange format v1.0 via LLM API.
- **JSON → DOCX**: Generates a JIS Z 7253-compliant 16-section Word document from the standard JSON, with localized section headings.
- **Multilingual**: Handles source documents in `ja` / `en` / `zh-CN` / `zh-TW`.
- **Extensible LLM backend**: Ships with Anthropic Claude, OpenAI GPT, and Google Gemini backends. Bring your own by implementing `LlmBackend`.
- **SSRF protection**: URL fetches reject private/loopback/link-local and metadata endpoints; redirect following disabled; full IPv6 coverage (`fc00::/7` ULA, `fe80::/10` link-local, `::ffff:` IPv4-mapped)
- **HTML/URL input**: Accepts `.html`/`.htm` files and `http(s)://` URLs as input
- **GHS/CAS validation**: H-codes (H200–H420) and P-codes (P101–P503) against GHS Rev.10; CAS number format and check-digit validation; optional PubChem enrichment
- **Robust JSON repair**: String-context-aware trailing-comma removal — preserves values like `"ends here,}"` while fixing genuine LLM formatting artefacts

---

## Installation

```toml
[dependencies]
sds-converter-core = "0.3"
```

---

## Library Usage

### Convert SDS document to JSON (Anthropic Claude)

```rust
use sds_converter_core::{
    converter::{AnthropicBackend, LlmConfig},
    convert_to_json, ConvertConfig, Language,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let backend = AnthropicBackend::new(
        std::env::var("ANTHROPIC_API_KEY")?,
        LlmConfig::default(),
    );

    let config = ConvertConfig {
        source_language: Some(Language::Japanese),
        output_language: Language::Japanese,
        ..Default::default()
    };

    let (sds, warnings) = convert_to_json(std::path::Path::new("input.pdf"), &backend, &config).await?;
    for w in &warnings { eprintln!("WARN: {w}"); }
    std::fs::write("output.json", serde_json::to_string_pretty(&sds)?)?;
    Ok(())
}
```

### Convert JSON to Word document

```rust
use sds_converter_core::{convert_from_json, ConvertConfig, Language, SdsRoot};

fn main() -> anyhow::Result<()> {
    let json = std::fs::read_to_string("output.json")?;
    let sds: SdsRoot = serde_json::from_str(&json)?;

    let config = ConvertConfig {
        source_language: None,
        output_language: Language::Japanese,
        ..Default::default()
    };

    convert_from_json(&sds, std::path::Path::new("result.docx"), &config)?;
    Ok(())
}
```

### OpenAI GPT or Google Gemini backend

```rust
use sds_converter_core::{OpenAiCompatBackend, LlmConfig};

// OpenAI GPT
let config = LlmConfig { model: "gpt-4o".into(), max_tokens: 8192 };
let backend = OpenAiCompatBackend::openai(std::env::var("OPENAI_API_KEY")?, config);

// Google Gemini
let config = LlmConfig { model: "gemini-2.0-flash".into(), max_tokens: 8192 };
let backend = OpenAiCompatBackend::gemini(std::env::var("GEMINI_API_KEY")?, config);

// Any OpenAI-compatible endpoint
let backend = OpenAiCompatBackend::new(
    "api-key",
    LlmConfig::default(),
    "https://your-endpoint/v1/chat/completions",
);
```

### Extract raw text from a document

Use `extract_text` to pull the raw text out of a PDF, DOCX, or plain-text file without making an LLM call. Useful for building custom pipelines or inspecting what the LLM receives.

```rust
use sds_converter_core::extract_text;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let text = extract_text(std::path::Path::new("input.pdf")).await?;
    println!("{text}");
    Ok(())
}
```

Supported extensions: `.pdf`, `.docx`, `.xlsx`, `.txt`.

### Validate an extracted SdsRoot

`validate` checks the structural completeness of an `SdsRoot` and returns a list of warning strings. It does not hard-fail — partial results remain usable.

```rust
use sds_converter_core::{validate, SdsRoot};

fn main() -> anyhow::Result<()> {
    let json = std::fs::read_to_string("output.json")?;
    let sds: SdsRoot = serde_json::from_str(&json)?;
    let warnings = validate(&sds);
    if warnings.is_empty() {
        println!("OK");
    } else {
        for w in &warnings { eprintln!("WARN: {w}"); }
    }
    Ok(())
}
```

### Custom LLM backend

Implement the `LlmBackend` trait to use any LLM provider:

```rust
use sds_converter_core::{LlmBackend, SdsError};

struct MyLlmBackend { /* ... */ }

impl LlmBackend for MyLlmBackend {
    async fn complete(&self, system: &str, user: &str) -> Result<String, SdsError> {
        // Call your LLM API and return the raw JSON string response
        todo!()
    }
}
```

---

## JSON Format

The output JSON conforms to the **MHLW SDS Data Exchange Format v1.0** (厚生労働省SDS情報交換のための標準的フォーマット, published 2025-03-31).

The schema covers all 16 sections of JIS Z 7253 with ~200 structured fields.

```json
{
  "Datasheet": {
    "IssueDate": "2024-03-31",
    "SDS-SchemaVersionNo": "1.0"
  },
  "Identification": {
    "TradeProductIdentity": {
      "TradeNameJP": "サンプル製品"
    },
    "SupplierInformation": {
      "CompanyName": "株式会社サンプル",
      "Phone": "03-0000-0000"
    }
  }
}
```

---

## Language Support

| Language | `source_language` / `output_language` | Source document standard | Output DOCX headings |
|---|---|---|---|
| Japanese | `Language::Japanese` | JIS Z 7253 | JIS Z 7253 |
| English | `Language::English` | GHS/OSHA HazCom | GHS Rev.10 / ISO 11014 |
| Simplified Chinese | `Language::ChineseSimplified` | GB/T 16483 | GB/T 16483-2012 |
| Traditional Chinese | `Language::ChineseTraditional` | CNS 15030 | CNS 15030 |

---

## Requirements

- Rust 1.75+
- An LLM API key (for `convert_to_json` only)
  - Anthropic: [Get API key](https://console.anthropic.com/)
  - OpenAI: [Get API key](https://platform.openai.com/)
  - Google Gemini: [Get API key](https://aistudio.google.com/)
- Input files must be **text-based** PDF or DOCX
  - Encrypted PDFs are not supported (text extraction will fail)
  - CID font / Shift-JIS encoded PDFs (common in Japanese documents): handled by `pdftotext` (poppler) fallback
  - Scanned/image-only PDFs: automatically retried via `pdftoppm` + `tesseract` OCR (if installed), or via Claude Vision API (when using Anthropic provider)
  - Full 3-tier PDF fallback: `pdf-extract` -> `pdftotext` -> OCR/Vision

---

## Changelog

### Completed in 0.3.6
- [x] QC r24: 5 new rule-based checks (S1-ZH-NO-EMERGENCY, S7-FLAMMABLE-STORAGE-TEMP, S8-NO-ENG-CONTROLS, S10-NO-INCOMPATIBLE, CROSS-STALE-DATE)
- [x] QC r24: S8-OEL-NO-NUMERIC false-positive fixes — Chinese unit-before-value format, additional "no OEL" exemption phrases
- [x] QC r24: S5-EMPTY threshold 30→15 chars (reduces false positives for brief Chinese firefighting sections)
- [x] Round-trip test: JSONL parsing fix, validator string-array handling; r24 baseline 30/30 success, CRIT=0, HIGH=9, MED=176
- [x] QC r25: fix S2-EXPLOSIVE-NO-GHS01 / S2-ENV-NO-GHS09 false-negatives (substring "01"/"09" in dates/H-codes); new S3-NAME-IS-CAS (HIGH) and S16-REVISION-BEFORE-ISSUE (HIGH)
- [x] Round-trip test r25 baseline: 30/30 success, CRIT=0, HIGH=13, MED=175
- [x] QC r26: S2-FLAMMABLE-NO-GHS02, S2-CORROSIVE-NO-GHS05, S2-ACUTETOX-NO-GHS06 (all MED) — pictogram/H-code consistency for flammable, corrosive, and acute-tox Cat 1–3; S4-H314-NO-REMOVE-CLOTHING (MED) — P361 compliance
- [x] Round-trip test r26 baseline: 30/30 success, CRIT=0, HIGH=14, MED=181
- [x] LLM prompt: Section 1 Use fallback — source phrase captured when Section 1.2 exists but no specific use is listed (e.g. `'无相关详细资料'`)
- [x] LLM prompt: Section 8 OEL "not required" detection — `不要求` / `无需监控` / `不适用` and similar phrases now stored in `AdditionalInfo.FullText` instead of being silently omitted
- [x] LLM prompt: Section 9 Densities always extracted; VapourPressure added for flammable/volatile products (H224/H225/H226/H330–H332)
- [x] LLM prompt: Section 12 `PersistenceDegradability.BiologicalDegradability` always populated when the source subsection exists

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
