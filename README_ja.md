# sds-converter

安全データシート（SDS）文書（Word/PDF）と厚生労働省が定める標準フォーマット（JSON）を**双方向に変換**するRustワークスペースです。

**日本語**・英語・簡体字中国語・繁体字中国語のSDS文書に対応。

---

## クレート構成

| クレート | 説明 |
|---|---|
| [`sds-converter-core`](./sds_converter_core/) | Rustライブラリ — LLMによる抽出・DOCX生成・MHLWスキーマ |
| [`sds-converter`](./sds_converter/) | CLIバイナリ — `to-json`・`to-docx`・`validate`・`extract-text` サブコマンド |

---

## 特徴

- **SDS文書 → JSON**: PDF/DOCX/XLSX/TXTからテキストを抽出し、厚生労働省のSDS情報交換標準フォーマット v1.0（JSON）に変換します。並列抽出・自動リトライ対応。
- **JSON → DOCX**: 標準JSONからJIS Z 7253準拠の16項目Word文書を生成します。言語別の項目見出しに対応。
- **多言語対応**: `ja` / `en` / `zh-CN` / `zh-TW` の入出力に対応。
- **LLMバックエンドを拡張可能**: Anthropic Claude、OpenAI GPT、Google Gemini、Mistral、Groq、Cohere の実装を同梱。`LlmBackend`トレイトを実装すれば任意のLLMを使用可能。
- **ライブラリ + CLI**: Rustライブラリとして組み込み利用、またはCLIとして単独利用できます。

---

## なぜLLMを使うのか

SDS文書は**非構造化の文章**であり、スプレッドシートのような定形データではありません。同じ規格に準拠していても、文書ごとに以下のような差異があります：

- **項目順序の違い** — メーカーによって16項目の記載順が異なる
- **表現・表記の多様性** — 同じデータが「≥99.5%」「99.5%以上」「約100%含有」など様々な表現で書かれる
- **見出し名の差異** — JIS Z 7253、GHS/OSHA HazCom、GB/T 16483、CNS 15030で同じ概念に異なるラベルが使われる
- **多言語の混在** — 日本語SDS内に英語の化学物質名・CAS番号が混在することが多い

厚生労働省の標準フォーマットには**約200の深くネストされたフィールド**があります。文書のバリエーションごとにルールベースのパーサを書くことは非現実的です。LLMは人間と同様に文書を読み、書式に依存せず自由形式のテキストを正しいスキーマフィールドにマッピングし、多言語文書もネイティブに処理できます。

`LlmBackend`トレイトにより抽出エンジンを差し替え可能で、Claude・GPT-4o・Geminiや将来の新モデルにも対応できます。

---

## クイックスタート

```bash
# CLIをインストール
cargo install sds-converter

# PDF → MHLW標準JSON
export ANTHROPIC_API_KEY=sk-ant-...
sds-converter to-json --input input.pdf --output output.json

# JSON → Word文書
sds-converter to-docx --input output.json --output result.docx --lang ja
```

CLIの詳細は [`sds-converter` README](./sds_converter/README.md)、ライブラリAPIは [`sds-converter-core` README](./sds_converter_core/README.md) を参照してください。

---

## 言語対応

| 言語 | `--lang` | ソース文書形式 | 出力DOCX見出し |
|---|---|---|---|
| 日本語 | `ja` | JIS Z 7253準拠SDS | JIS Z 7253 |
| 英語 | `en` | GHS/OSHA HazCom形式 | GHS Rev.10 / ISO 11014 |
| 簡体字中国語 | `zh-cn` | GB/T 16483形式 | GB/T 16483-2012 |
| 繁体字中国語 | `zh-tw` | CNS 15030形式 | CNS 15030 |

---

## 競合製品との比較

### オープンソースツール

| | **sds-converter**（本ツール） | [sds_parser](https://github.com/astepe/sds_parser) | [tungsten](https://github.com/CrucibleSDS/tungsten) |
|---|---|---|---|
| 言語 | Rust | Python | Python |
| AI/LLM | あり（差し替え可能） | なし（正規表現） | なし（ルールベース） |
| 厚労省JSON | あり | なし | なし |
| 双方向変換 | あり（↔ DOCX） | なし | なし |
| 多言語対応 | ja / en / zh-CN / zh-TW | 限定的 | 英語のみ |

### 商用製品（日本）

| | **sds-converter**（本ツール） | [SDS Meister](https://www.kcs.co.jp/ja/service/ind/general/chemical/sds.html) | [SmartSDS](https://smartsds.jp/) | [Dr.EHS Chemical](https://www.iad.co.jp/drehs/chemical2/) |
|---|---|---|---|---|
| 提供元 | — | さくらケーシーエス | テクノヒル | アイアンドディー |
| AI | あり（自前APIキー） | なし | あり（翻訳） | AI-OCR |
| 厚労省JSON | あり | あり | あり | あり |
| PDF→JSON変換 | あり | なし（作成専用） | 一部（日本語のみ） | あり |
| オープンソース | あり（MIT/Apache-2.0） | なし | なし | なし |

### 商用製品（海外）

| | **sds-converter**（本ツール） | [Affinda](https://www.affinda.com/documents/material-safety-data-sheet) | [SDS Manager API](https://sdsmanager.com/) | [safetydatasheetapi.com](https://safetydatasheetapi.com/) | [EcoOnline](https://www.ecoonline.com/) |
|---|---|---|---|---|---|
| AI/LLM | 差し替え可能なLLM | LLM（学習型） | NLP/ML | ML + OCR | AI/NLP |
| 入力 | PDF / DOCX | PDF / Word | PDF | PDF（スキャン含む） | PDF |
| 出力 | 厚労省JSON + DOCX | カスタムJSON | JSON / XML | JSON / XML / CSV | 内部データのみ |
| オープンソース | あり | なし | なし | なし | なし |

**本ツールの強み**: 厚生労働省標準JSON・双方向変換（JSON→DOCX）・クラウド不要のローカル実行・差し替え可能なLLMバックエンドに対応する、唯一のオープンソースソリューションです。

---

## ライセンス

以下のいずれかを選択：
- Apache License, Version 2.0
- MIT License
