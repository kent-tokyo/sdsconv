# sds-converter-core

安全データシート（SDS）文書（Word/PDF）と厚生労働省が定める標準フォーマット（JSON）を**双方向に変換**するRustライブラリです。

**日本語**・英語・簡体字中国語・繁体字中国語のSDS文書に対応。

> **CLIツールをお探しですか？** [`sds-converter`](https://crates.io/crates/sds-converter) をインストールしてください。

---

## 特徴

- **SDS文書 → JSON**: PDF/DOCX/XLSX/TXTからテキストを抽出し、厚生労働省のSDS情報交換標準フォーマット v1.0（JSON）に変換します。並列抽出・自動リトライ対応。
- **JSON → DOCX**: 標準JSONからJIS Z 7253準拠の16項目Word文書を生成します。言語別の項目見出しに対応。
- **多言語対応**: `ja` / `en` / `zh-CN` / `zh-TW` の入出力に対応。
- **LLMバックエンドを拡張可能**: Anthropic Claude、OpenAI GPT、Google Gemini、Mistral、Groq、Cohere の実装を同梱。`LlmBackend`トレイトを実装すれば任意のLLMを使用可能。
- **SSRF対策**: URLフェッチはプライベート/ループバック/リンクローカル/メタデータIPを自動拒否。リダイレクト無効化、IPv6フルカバレッジ（`fc00::/7` ULA、`fe80::/10` リンクローカル、`::ffff:` IPv4マップアドレス）
- **HTML/URL入力対応**: `.html`/`.htm` ファイルおよび `http(s)://` URLを入力として受け付け
- **GHS/CASバリデーション**: GHS Rev.10準拠のH-code (H200–H420)・P-code (P101–P503) 検証、CAS番号フォーマット＋チェックデジット検証、PubChem照合（`enrich_composition`）
- **堅牢なJSONリペア**: 文字列内コンテキストを保持するトレーリングカンマ除去（例: `"ends here,}"` を破壊しない）

---

## インストール

```toml
[dependencies]
sds-converter-core = "0.3"
```

---

## ライブラリ使用方法

### SDS文書をJSONに変換する（Anthropic Claude）

```rust
use sds_converter_core::{
    AnthropicBackend, LlmConfig,
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

### JSONをWord文書に変換する

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

### OpenAI GPT / Google Gemini バックエンド

```rust
use sds_converter_core::{OpenAiCompatBackend, LlmConfig};

// OpenAI GPT
let config = LlmConfig { model: "gpt-4o-mini".into(), max_tokens: 8192 };
let backend = OpenAiCompatBackend::openai(std::env::var("OPENAI_API_KEY")?, config);

// Google Gemini
let config = LlmConfig { model: "gemini-2.0-flash".into(), max_tokens: 8192 };
let backend = OpenAiCompatBackend::gemini(std::env::var("GEMINI_API_KEY")?, config);

// 任意のOpenAI互換エンドポイント（Ollama等）
let backend = OpenAiCompatBackend::new(
    "api-key",
    LlmConfig::default(),
    "https://your-endpoint/v1/chat/completions",
);
```

### 文書からテキストを抽出する

LLM呼び出しなしでPDF/DOCX/XLSXのテキストを抽出します。カスタムパイプラインの構築やLLMへの入力内容の確認に使用できます。

```rust
use sds_converter_core::extract_text;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let text = extract_text(std::path::Path::new("input.pdf")).await?;
    println!("{text}");
    Ok(())
}
```

対応拡張子: `.pdf`、`.docx`、`.xlsx`、`.txt`

### SdsRoot の構造を検証する

`validate` は `SdsRoot` の構造的な完全性をチェックし、警告メッセージのリストを返します。エラーで中断はしません — 部分的な結果もそのまま使用できます。

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

### カスタムLLMバックエンド

`LlmBackend`トレイトを実装することで任意のLLMプロバイダーを使用できます：

```rust
use sds_converter_core::{LlmBackend, SdsError};

struct MyLlmBackend { /* ... */ }

impl LlmBackend for MyLlmBackend {
    async fn complete(&self, system: &str, user: &str) -> Result<String, SdsError> {
        // LLM APIを呼び出し、生のJSON文字列を返す
        todo!()
    }
}
```

---

## JSONフォーマット

出力JSONは**厚生労働省SDS情報交換標準フォーマット v1.0**（2025年3月31日公開）に準拠しています。

JIS Z 7253の全16項目を約200の構造化フィールドでカバーしています。

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

## 言語対応

| 言語 | `source_language` / `output_language` | ソース文書規格 | 出力DOCX見出し |
|---|---|---|---|
| 日本語 | `Language::Japanese` | JIS Z 7253 | JIS Z 7253 |
| 英語 | `Language::English` | GHS/OSHA HazCom | GHS Rev.10 / ISO 11014 |
| 簡体字中国語 | `Language::ChineseSimplified` | GB/T 16483 | GB/T 16483-2012 |
| 繁体字中国語 | `Language::ChineseTraditional` | CNS 15030 | CNS 15030 |

---

## 動作環境

- Rust 1.75以上
- LLM APIキー（`convert_to_json` 使用時のみ必要）
  - Anthropic: [APIキー取得](https://console.anthropic.com/)
  - OpenAI: [APIキー取得](https://platform.openai.com/)
  - Google Gemini: [APIキー取得](https://aistudio.google.com/)
- 入力ファイルはテキストベースのPDF/DOCX/XLSX/TXT
  - 暗号化PDFは変換不可（テキスト抽出に失敗します）
  - CIDフォント/Shift-JISエンコードPDF（日本語文書に多い）：`pdftotext`（poppler）フォールバックで処理
  - スキャン画像PDF：`pdftoppm` + `tesseract`（インストール済みの場合）またはClaude Vision API（Anthropicプロバイダ使用時）で自動リトライ
  - PDF抽出3段階フォールバック：`pdf-extract` -> `pdftotext` -> OCR/Vision

---

## 更新履歴

### 0.3.6 で完了
- [x] QC r24: 新規5ルール（S1-ZH-NO-EMERGENCY・S7-FLAMMABLE-STORAGE-TEMP・S8-NO-ENG-CONTROLS・S10-NO-INCOMPATIBLE・CROSS-STALE-DATE）
- [x] QC r24: S8-OEL-NO-NUMERIC 誤検知修正 — 中国語「単位→数値」形式・「OEL不要」表現の除外パターン追加
- [x] QC r24: S5-EMPTY 閾値 30→15 文字（中国語の簡潔な消火情報の誤検知削減）
- [x] ラウンドトリップテスト: JSONLパース修正・バリデータ文字列配列対応; r24ベースライン 30/30 成功、CRIT=0・HIGH=9・MED=176
- [x] LLMプロンプト: Section 1 Use フォールバック — セクション1.2が存在するが使用目的不明の場合、ソーステキストを Use 配列に格納（`'无相关详细资料'` 等）
- [x] LLMプロンプト: Section 8 OEL「不要求」検出 — `不要求`/`无需监控`/`不适用` 等を `AdditionalInfo.FullText` に格納（省略しない）
- [x] LLMプロンプト: Section 9 Densities 必須抽出、引火性製品（H224/H225/H226/H330–H332）の VapourPressure 抽出
- [x] LLMプロンプト: Section 12 残留性/分解性サブセクション存在時は `PersistenceDegradability.BiologicalDegradability` を常に格納

---

## 参考リンク

- [厚生労働省 — SDS情報交換のための標準的フォーマット等の公開について](https://www.mhlw.go.jp/stf/newpage_56484.html)
- [SDSデータ交換フォーマット データ利用マニュアル（PDF）](https://www.mhlw.go.jp/content/11305000/001467068.pdf)

---

## ライセンス

以下のいずれかを選択：
- Apache License, Version 2.0
- MIT License
