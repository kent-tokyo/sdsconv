# sds-converter

安全データシート（SDS）文書（Word/PDF）と厚生労働省が定める標準フォーマット（JSON）を**双方向に変換**するGUI + CLIツールです。

**日本語**・英語・簡体字中国語・繁体字中国語のSDS文書に対応。

[English](README.md) | [中文](README_zh.md)

> **Rustプロジェクトへの組み込みには？** [`sds-converter-core`](https://crates.io/crates/sds-converter-core) を直接使用してください。

---

## ダウンロード

| プラットフォーム | ダウンロード |
|---|---|
| **macOS**（ユニバーサル — Apple Silicon + Intel） | [sds-converter-macos.zip](https://github.com/kent-tokyo/sds-converter/releases/latest/download/sds-converter-macos.zip) |
| **Windows**（ポータブル .exe — インストール不要） | [sds-converter-windows-portable.zip](https://github.com/kent-tokyo/sds-converter/releases/latest/download/sds-converter-windows-portable.zip) |
| **Rust / CLI** | `cargo install sds-converter` |

→ [全リリース・更新履歴](https://github.com/kent-tokyo/sds-converter/releases)

---

## GUIモード

`sds-converter` を**引数なし**で起動するとGUIウィンドウが開きます:

```bash
sds-converter
```

5つのタブを持つウィンドウ（820×640）が起動します:

| タブ | 機能 |
|---|---|
| **変換** | SDS文書（PDF/DOCX/XLSX/HTML/URL）→ MHLW標準JSON |
| **文書生成** | MHLW JSON → DOCX / HTML / PDF（DOCXテンプレート対応） |
| **検証** | MHLW JSONの構造検証（OK/警告/エラーをカラー表示） |
| **テキスト抽出** | 文書からテキスト抽出（LLM API不要） |
| **設定** | APIキー・モデル名・base URL・品質・言語・UI言語 |

| 変換タブ | 文書生成タブ | テキスト抽出タブ |
|---|---|---|
| ![変換タブ](../docs/tab_convert.png) | ![文書生成タブ](../docs/tab_generate.png) | ![テキスト抽出タブ](../docs/tab_extract.png) |

ファイルを任意のタブにドラッグ&ドロップすると入力フィールドに自動入力されます。

設定は `~/.config/sds-converter/config.toml` に保存され、次回起動時に復元されます。
GUIとCLIは同じ変換エンジン（`tasks.rs`）を使用するため、変換結果は同一です。

---

## コマンド

### `to-json` — PDF/Word → 厚生労働省標準JSON

```bash
# 単体ファイル（Anthropic Claude、デフォルト）
export ANTHROPIC_API_KEY=sk-ant-...
sds-converter to-json --input input.pdf --output output.json

# ソース言語を指定
sds-converter to-json --input sds_en.pdf --output output.json --lang en

# バッチモード — ディレクトリ全体を処理
sds-converter to-json --input-dir ./pdfs/ --output-dir ./json/ --lang ja

# OpenAI GPT（デフォルト: gpt-4o-mini）
sds-converter to-json --input input.pdf --output output.json \
  --provider openai --api-key $OPENAI_API_KEY

# Google Gemini（デフォルト: gemini-2.0-flash）
sds-converter to-json --input input.pdf --output output.json \
  --provider gemini --api-key $GEMINI_API_KEY

# ローカルLLM（Ollama等、OpenAI互換エンドポイント）
sds-converter to-json --input input.pdf --output output.json \
  --provider local --base-url http://localhost:11434/v1 \
  --model llama3.2 --api-key dummy

# 抽出済みテキストから変換（PDF解析をスキップ）
sds-converter to-json --input extracted.txt --output output.json --lang ja
```

| フラグ | デフォルト | 説明 |
|---|---|---|
| `--input` | — | 入力ファイル（PDF / DOCX / XLSX / TXT） |
| `--input-dir` | — | 入力ディレクトリ（バッチ：`.pdf`/`.docx`/`.xlsx`/`.xls` を処理） |
| `--output` | — | 出力JSONファイル |
| `--output-dir` | — | 出力ディレクトリ（バッチ：存在しない場合は作成） |
| `--provider` | `anthropic` | LLMプロバイダ：`anthropic` / `openai` / `gemini` / `mistral` / `groq` / `cohere` / `local` |
| `--api-key` | 環境変数 | APIキー（後述のプロバイダ別デフォルトを参照） |
| `--model` | プロバイダ別 | モデル名の上書き |
| `--base-url` | — | OpenAI互換エンドポイント（`--provider local` 用） |
| `--lang` | 自動検出 | ソース文書の言語：`ja` / `en` / `zh-cn` / `zh-tw` |
| `--quality` | `medium` | プリセット：`low`（高速・低コスト）/ `medium` / `high`（高精度） |
| `--concurrency` | `4` | バッチモードの最大並列数 |
| `--suggested-name` | — | 出力ファイルを `SDS_<発行日>_<品番>.json` にリネーム（厚労省§2.1.2推奨命名規則） |

**プロバイダ別デフォルト:**

| `--provider` | デフォルトモデル | 環境変数 |
|---|---|---|
| `anthropic` | `claude-haiku-4-5-20251001`（low/medium）・`claude-sonnet-4-6`（high） | `ANTHROPIC_API_KEY` |
| `openai` | `gpt-4o-mini` | `OPENAI_API_KEY` |
| `gemini` | `gemini-2.0-flash` | `GEMINI_API_KEY` |
| `mistral` | `mistral-small-latest` | `MISTRAL_API_KEY` |
| `groq` | `llama-3.3-70b-versatile` | `GROQ_API_KEY` |
| `cohere` | `command-r-plus` | `COHERE_API_KEY` |
| `local` | `llama3` | `LOCAL_LLM_API_KEY`（省略可、デフォルト `ollama`） |

### `to-docx` — 厚生労働省標準JSON → Wordドキュメント

```bash
# 単体ファイル（組み込みレイアウト）
sds-converter to-docx --input output.json --output result.docx --lang ja

# バッチモード（組み込みレイアウト）
sds-converter to-docx --input-dir ./json/ --output-dir ./docx/ --lang en

# Wordテンプレートへの {{プレースホルダー}} 置換
sds-converter to-docx --input output.json --output result.docx \
  --template my_template.docx

# バッチモード + テンプレート
sds-converter to-docx --input-dir ./json/ --output-dir ./docx/ \
  --template my_template.docx
```

#### Wordテンプレートの書式

`.docx` ファイルに `{{フィールド名}}` プレースホルダーを配置します。`フィールド名` はMHLW JSONスキーマのリーフキーです。完全なドットパスも使用できます。

```
{{TradeNameJP}}          → 製品和名
{{CompanyName}}          → 会社名
{{Phone}}                → 電話番号
{{IssueDate}}            → 発行日
{{Identification.SupplierInformation.CompanyName}}  → フルパス指定
```

プレースホルダーは段落・表セル・ヘッダー・フッターのどこにでも配置できます。Wordが内部runを分割して記録している場合でも、自動的にマージしてから置換します。

| フラグ | デフォルト | 説明 |
|---|---|---|
| `--input` | — | 入力JSONファイル |
| `--input-dir` | — | 入力ディレクトリ（バッチ：`.json` を処理） |
| `--output` | — | 出力DOCXファイル |
| `--output-dir` | — | 出力ディレクトリ（バッチ） |
| `--lang` | `ja` | 出力言語：`ja` / `en` / `zh-cn` / `zh-tw`（`--template` なしの場合） |
| `--template` | — | `{{フィールド名}}` プレースホルダー付きWordテンプレート |

### `extract-text` — PDF/DOCXからテキスト抽出

LLMに渡されるテキストをAPIコールなしで確認できます。抽出品質の検査や、LLMステップを別途実行する場合に便利です。

```bash
# ファイルに保存
sds-converter extract-text --input input.pdf --output extracted.txt

# 標準出力に表示
sds-converter extract-text --input input.pdf

# 抽出テキストをto-jsonに渡す
sds-converter to-json --input extracted.txt --output output.json --lang ja
```

### `validate` — JSONファイルの構造検証

```bash
# 人が読める形式（終了コード 0=OK、1=警告あり）
sds-converter validate --input output.json

# JSON配列出力（CI/スクリプト用）
sds-converter validate --input output.json --json
```

主要セクション（Identification・HazardIdentification・ToxicologicalInformation 等）の充足度を確認します。問題があれば終了コード `1` で終了します。

---

## 言語サポート

| 言語 | `--lang` | ソース文書 | 出力DOCX見出し |
|---|---|---|---|
| 日本語 | `ja` | JIS Z 7253準拠SDS | JIS Z 7253 |
| 英語 | `en` | GHS/OSHA HazCom形式 | GHS Rev.10 / ISO 11014 |
| 簡体字中国語 | `zh-cn` | GB/T 16483形式 | GB/T 16483-2012 |
| 繁体字中国語 | `zh-tw` | CNS 15030形式 | CNS 15030 |

---

## 必要環境

- Rust 1.75以上
- LLM APIキー（`to-json` のみ必要）— プロバイダの環境変数を設定するか `--api-key` で渡す
  - Anthropic: `ANTHROPIC_API_KEY`
  - OpenAI: `OPENAI_API_KEY`
  - Google Gemini: `GEMINI_API_KEY`
  - Mistral: `MISTRAL_API_KEY`
  - Groq: `GROQ_API_KEY`
  - Cohere: `COHERE_API_KEY`
  - ローカルLLM（Ollama等）: `--provider local --base-url <url>`（APIキー不要）
- 入力ファイルは**テキストベース**のPDFまたはDOCXであること
  - 暗号化PDFは非対応
  - CIDフォント/Shift-JISエンコードPDF（日本語文書に多い）：`pdftotext`（poppler）フォールバックで処理
  - スキャン画像PDF：`pdftoppm` + `tesseract`（インストール済みの場合）またはClaude Vision API（`--provider anthropic` 使用時）で自動リトライ
  - PDF抽出3段階フォールバック：`pdf-extract` -> `pdftotext` -> OCR/Vision

---

## 更新履歴

### 0.3.6 / 0.2.6 で完了
- [x] QC r24: 新規5ルール（S1-ZH-NO-EMERGENCY・S7-FLAMMABLE-STORAGE-TEMP・S8-NO-ENG-CONTROLS・S10-NO-INCOMPATIBLE・CROSS-STALE-DATE）
- [x] QC r24: S8-OEL-NO-NUMERIC 誤検知修正 — 中国語「単位→数値」形式・「OEL不要」表現の除外パターン追加
- [x] QC r24: S5-EMPTY 閾値 30→15 文字（中国語の簡潔な消火情報の誤検知削減）
- [x] ラウンドトリップテスト: JSONLパース修正・バリデータ文字列配列対応; r24ベースライン 30/30 成功、CRIT=0・HIGH=9・MED=176
- [x] QC r25: S2-EXPLOSIVE-NO-GHS01 / S2-ENV-NO-GHS09 偽陰性バグ修正（日付・Hコード内 "01"/"09" による誤スキップ）；新ルール S3-NAME-IS-CAS（HIGH）・S16-REVISION-BEFORE-ISSUE（HIGH）
- [x] ラウンドトリップテスト r25 ベースライン: 30/30 成功、CRIT=0・HIGH=13・MED=175
- [x] QC r26: S2-FLAMMABLE-NO-GHS02・S2-CORROSIVE-NO-GHS05・S2-ACUTETOX-NO-GHS06（全て MED）— 引火性・腐食性・急性毒性 Cat 1–3 のピクトグラム整合性チェック；S4-H314-NO-REMOVE-CLOTHING（MED）— P361 汚染衣類脱去指示
- [x] ラウンドトリップテスト r26 ベースライン: 30/30 成功、CRIT=0・HIGH=14・MED=181
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
