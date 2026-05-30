# sds-converter

安全データシート（SDS）文書（Word/PDF）と厚生労働省が定める標準フォーマット（JSON）を**双方向に変換**するGUI + CLIツールです。

**日本語**・英語・簡体字中国語・繁体字中国語のSDS文書に対応。

[English](README.md) | [中文](README_zh.md)

---

## ダウンロード

| プラットフォーム | ダウンロード |
|---|---|
| **macOS**（Homebrew） | `brew tap kent-tokyo/sds-converter && brew install --cask sds-converter` |
| **macOS**（直接ダウンロード — ユニバーサル、Apple Silicon + Intel） | [sds-converter-macos.zip](https://github.com/kent-tokyo/sds-converter/releases/latest/download/sds-converter-macos.zip) |
| **Windows**（ポータブル .exe — インストール不要） | [sds-converter-windows-portable.zip](https://github.com/kent-tokyo/sds-converter/releases/latest/download/sds-converter-windows-portable.zip) |
| **Rust / CLI** | `cargo install sds-converter` |

→ [全リリース・更新履歴](https://github.com/kent-tokyo/sds-converter/releases)

> **Windows の注意：** SmartScreen に「Windows によって PC が保護されました」と表示された場合は、**「詳細情報」→「実行」**をクリックしてください。

---

## GUI

`sds-converter` を**引数なし**で起動（またはダウンロードしたアプリをダブルクリック）するとGUIが開きます:

```bash
sds-converter
```

5つのタブを持つウィンドウが起動します:

| タブ | 機能 |
|---|---|
| **変換** | SDS文書（PDF/DOCX/XLSX/HTML/URL）→ MHLW標準JSON |
| **文書生成** | MHLW JSON → DOCX / HTML / PDF（DOCXテンプレート対応） |
| **検証** | MHLW JSONの構造検証（OK/警告/エラーをカラー表示） |
| **テキスト抽出** | 文書からテキスト抽出（LLM API不要） |
| **設定** | APIキー・モデル名・base URL・品質・言語・UI言語 |

| 変換タブ | 文書生成タブ | テキスト抽出タブ |
|---|---|---|
| ![変換タブ](docs/tab_convert.png) | ![文書生成タブ](docs/tab_generate.png) | ![テキスト抽出タブ](docs/tab_extract.png) |

ファイルを任意のタブに**ドラッグ&ドロップ**すると入力フィールドに自動入力されます。
設定は `~/.config/sds-converter/config.toml` に保存され、次回起動時に復元されます。

---

## 特徴

- **SDS文書 → JSON**: PDF/DOCX/XLSX/TXT/**HTML・URL** からテキストを抽出し、厚生労働省のSDS情報交換標準フォーマット v1.0（JSON）に変換します。並列抽出・自動リトライ対応。PDFは3段階フォールバックで処理: `pdf-extract` → `pdftotext`（CIDフォント/Shift-JIS日本語PDF）→ `pdftoppm`+`tesseract` OCRまたはClaude Vision API（スキャンPDF）。
- **JSON → DOCX**: 標準JSONからJIS Z 7253準拠の16項目Word文書を生成します。言語別の項目見出しに対応。
- **JSON → HTML**: inline CSS と `@media print` 対応の自己完結型HTML5文書を生成します（`to-html`）。
- **JSON → PDF**: LibreOffice CLI経由でPDFに変換します（`to-pdf`、要 `soffice`）。
- **GHS/CASバリデーション**: GHS Rev.10準拠のH-code（H200–H420）・P-code（P101–P503）検証、CAS番号フォーマット＋チェックデジット検証。`--enrich` フラグでPubChem照合も可能。
- **多国SDS対応**: `--lang` からソース国を自動推論（zh-cn→中国、zh-tw→台湾、ja→日本）。`--country cn|tw|kr|jp` で明示的に上書き可能。国別LLM抽出ルールをシステムプロンプトに注入 — 中国（GB/T 16483）: 24時間緊急連絡先・GBZ 2 OEL・GB 13690規制参照；台湾（CNS 15030）: CNS見出し・NERC緊急連絡先；韓国（K-GHS Rev.6）: KEC番号・KOSHA参照・K-REACH状況。国別バリデーション（`validate_country()`）とコンプライアンスギャップレポート（`ComplianceDiffReport`）を `ConversionReport` に含めます。
- **バリデーション駆動補正パス**: `--correct` フラグで第2のLLMコールが有効になり、バリデーターが検出した無効なGHS H/P-codeを修正します。CASチェックデジット補正はLLMコールなしで確定的に実行されます。
- **多言語対応**: `ja` / `en` / `zh-CN` / `zh-TW` の入出力に対応。
- **LLMバックエンドを拡張可能**: Anthropic Claude、OpenAI GPT、Google Gemini、Mistral、Groq、Cohere の実装を同梱。`LlmBackend`トレイトを実装すれば任意のLLMを使用可能。
- **ライブラリ + CLI**: Rustライブラリとして組み込み利用、またはCLIとして単独利用できます。
- **セキュリティ強化済みRESTサーバー**: タイミング攻撃対策済みBearer token認証（`constant_time_eq`）、IPv6フルカバレッジのSSRF対策（`fc00::/7`・`fe80::/10`・IPv4マップアドレス）、リダイレクト無効化HTTPクライアント、50MBアップロード上限。

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

# URLから直接変換
sds-converter to-json --input https://example.com/sds.html --output output.json

# JSON → Word文書
sds-converter to-docx --input output.json --output result.docx --lang ja

# JSON → HTML（印刷対応・A4）
sds-converter to-html --input output.json --output result.html --lang ja

# JSON → PDF（LibreOffice が必要）
sds-converter to-pdf --input output.json --output result.pdf --lang ja

# JSONをバリデーション（GHSコード・CAS番号検証含む）
sds-converter validate --input output.json

# 変換後にPubChem照合（--enrich）
sds-converter to-json --input input.pdf --output output.json --enrich

# 中国語SDS（GB/T 16483）を国指定＋補正パスで変換
sds-converter to-json --input input.pdf --output output.json --lang zh-cn --country cn --correct
```

CLIの詳細は [`sds-converter` README](./sds_converter/README.md)、ライブラリAPIは [`sds-converter-core` README](./sds_converter_core/README.md) を参照してください。

---

## 開発者向け

| クレート | 説明 |
|---|---|
| [`sds-converter`](https://crates.io/crates/sds-converter) | CLI + GUIバイナリ |
| [`sds-converter-core`](https://crates.io/crates/sds-converter-core) | Rustライブラリ — LLM抽出・DOCX/HTML生成・MHLWスキーマ |

```toml
[dependencies]
sds-converter-core = "0.3"
```

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
| 双方向変換 | あり（DOCX + HTML + PDF） | なし | なし |
| HTML/URL入力 | あり | なし | なし |
| GHS/CAS検証 | あり | なし | なし |
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

**本ツールの強み**: 厚生労働省標準JSON・双方向変換（JSON→DOCX/HTML/PDF）・クラウド不要のローカル実行・GHS Rev.10バリデーション・PubChem照合・差し替え可能なLLMバックエンドに対応する、唯一のオープンソースソリューションです。

---

## ロードマップ

### 次期リリース（0.3.x）
- [ ] DOCXの表レイアウト — 第3項（成分情報・4列表）、第2項（H/Pコード・2列表）、第9項（物性・2列表）

### 0.3.5 / 0.2.5 で完了
- [x] 多国SDS対応（`--country cn|tw|kr|jp`）— 国別LLM抽出ルール注入・コンプライアンスギャップレポート生成
- [x] バリデーション駆動補正パス（`--correct`）— 無効H/P-codeを第2LLMコールで修正、CASチェックデジット確定的補正
- [x] CAS連結文字列の正規化 — `\n`・カンマ・セミコロン区切りの複数CASを個別エントリに分割
- [x] 非危険物スタブ挿入 — 非危険物でLLMがHazardIdentificationを省略した際の最小スタブ挿入
- [x] zh-cn/zh-tw表現を追加したH-codeマッピングテーブル拡張・複合ハザード分割指示
- [x] P-codeアノテーション除去 — Pコードフィールドから `[H315]` 形式の括弧内H-codeを除去
- [x] VisionパスへのテキストパスのCRITICAL指示適用
- [x] バリデーター強化: 濃度フィールド内の日付検出・製品名プレースホルダー検出・分類網羅性チェック・中国語キーワードによるH290クロスチェック・混合物対応AcuteToxicityクロスチェック

### 計画中
- [x] GUIアプリケーション（eframe/egui）— 変換・文書生成・検証・テキスト抽出・設定タブ、ドラッグ&ドロップ対応、設定永続化、3言語UI
- [x] crates.io公開（`sds-converter-core` + `sds-converter`）
- [ ] HTML/DOCXへのGHS絵表示（ピクトグラム）埋め込み

### 外部依存待ち
- [x] 純Rust PDF生成 — [`harumi`](https://crates.io/crates/harumi) v0.4.0 の `html` feature で `render_html_to_pdf` が利用可能になりました
- [x] スキャンPDFのOCR対応 — `pdftoppm` + `tesseract` CLI でフォールバック（テキスト抽出が200文字未満のとき自動起動）
- [x] 日本語CIDフォントPDFの `pdftotext` フォールバック — Shift-JISエンコードPDFで `pdf-extract` がパニックする問題を修正
- [x] スキーマ互換性強化（v0.3.3）— `CASno.FullText` への `flex_vec_string_opt` 追加、`Colour`/`Odour`/`PhysicalState` の JSON オブジェクト→文字列変換、`pdftotext` フォールバックの `-utf8` 廃止オプション削除

---

## 参考リンク

- [厚生労働省 — SDS情報交換のための標準的フォーマット等の公開について](https://www.mhlw.go.jp/stf/newpage_56484.html)
- [SDSデータ交換フォーマット データ利用マニュアル（PDF）](https://www.mhlw.go.jp/content/11305000/001467068.pdf)

---

## ライセンス

以下のいずれかを選択：
- Apache License, Version 2.0
- MIT License
