# Lessons Learned

## スキーマ設計

### struct命名戦略
フルパス連結でPascalCase化（衝突回避）。
14のリーフ名が衝突するため(Result×13, Condition×8等)、パスプレフィックスは必須。

### 全フィールドがOption<T>
仕様に必須フィールドなし。Vec<T>フィールドも Option<Vec<T>>で表現
（「キー不在」と「空配列」の区別がスペック適合に必要）。

### serdeのrename_all = "PascalCase"は使用不可
以下4フィールドはフィールドレベルの#[serde(rename)]が必須:
- "SDS-SchemaVersionNo" (ハイフン含む)
- "Dose/Concentration" (スラッシュ含む)
- "gazetteNo" (camelCase)
- "substance" (小文字, row 331)

### docx-rs vs docx-rust
- docx-rs 0.4: .docx生成専用 (write-only)
- docx-rust 0.1: 読み書き両用、入力Wordの解析に使用

## アーキテクチャ

### ライブラリ設計方針 (sds-converter-core)
main.rsは薄いCLIラッパーのみ。コア機能はlib.rsから公開し、
ユーザーが自分のコードからimportして使えるようにする。
crates.ioではsds-converter-coreとして公開。

### PDF抽出品質
pdf-extractはテキストPDFで有効。スキャン画像PDFは空or文字化けの可能性。
→ 検出してユーザーに分かりやすいエラーを表示する。

### PDF生成 (JSON→PDF) の複雑さ
日本語フォントのサブセット埋め込みが必要で実装コスト高。
v1はdocxのみ。ユーザーはWord/LibreOfficeのprint-to-PDFで対応可能。

## LLM統合設計

### LlmBackend trait (rig-inspired)
Rustではasync fn in trait (Rust 1.75+) が使えるため async_trait クレート不要。
静的ディスパッチ (impl LlmBackend) として実装し、
`dyn LlmBackend`が必要なユーザーはenum_dispatch等を使う。
traitは`complete(system, user) -> Result<String>` の1メソッドのみで十分。

### assistant prefill でJSON強制
Anthropic APIのassistant roleに`{`をprefillとして送ることで
JSON出力を強制できる。レスポンスに先頭の`{`を付け直してから
serde_jsonでデシリアライズする。

### 並列LLM呼び出しには B: Sync が必要
`tokio::join!` で2つのfutureを並行実行するとき、両futureがSendである必要がある。
`&B: Send` を満たすには `B: Sync` バウンドが必要。
`extract_sds_from_text<B: LlmBackend + Sync>` のように明示的に追加すること。

### LLM APIの非決定性（temperature=0でも残る）
temperature=0を設定しても、Anthropicの分散推論によりレスポンスに約9%の変動がある。
これはAPIレベルの非決定性であり、コードバグではない。
テストで確認した場合、再試行ロジック（スキップセクション再抽出）でカバーできる範囲が現実的な上限。

### スキップセクション再試行設計
lenient_deserialize が失敗セクションの「キー名のみ」（例: "HandlingAndStorage"）を返すことで
3回目のLLMコールでそのセクションだけを再抽出するロジックが綺麗に実装できる。
エラーメッセージ全体を返すと再試行ロジックが複雑になる。

## 多言語対応

### Language enum の設計
`source_language: Option<Language>` (Noneで自動判別) と
`output_language: Language` (DOCX見出し言語) を分離。
CLIはValueEnum: ja / en / zh-cn / zh-tw で受け取り変換。

### 多言語セクション名は各国の実際のSDS規格を参照すること
単なる日本語からの機械翻訳は誤り。各国規格を根拠にする:
- EN: GHS Rev.10 (UN) / ISO 11014:2020 / OSHA HazCom 2012
- 简体中文: GB/T 16483-2012 (中国国家标准)
- 繁體中文: CNS 15030 (台灣 GHS 標準)
DocタイトルもGB/T 16483では「安全技术说明书」(安全データ表ではない)。

### Pythonコードジェネレータのバグ対策
definitions内のネスト構造体名の衝突: path[1]だけでなく
path[1:]をすべて連結する必要あり (path_to_rust_type の修正)。
Rustキーワード (use, type等) はr#プレフィックスが必要。

## 開発環境

### Rustのデバッグビルドはディスク容量を大量消費する
`target/debug/` が数GB になりやすく、`/private/tmp` パーティションが枯渇することがある。
ビルドエラーで "No space left on device" が出たら `rm -rf target/debug/` で解消できる。
リリースビルド (`cargo build --release`) はデバッグビルドより小さいが、依然として注意が必要。

## 外部クレートのAPI調査

### scraper 0.21 に ElementRef::matches() は存在しない
当初、HTMLノードのスキップ判定に `el.matches(selector)` を使う計画だったが、
scraper 0.21 の `ElementRef` にはこのメソッドがない。
→ タグ名を `el.value().name()` で文字列として取得し、`matches!(tag, "script" | "style" | ...)` で比較する。
ドキュメントを信頼するより先に `cargo doc --open` でAPIを確認すること。

### URL を PathBuf 型で保持しようとしない
`input: PathBuf` フィールドを持つ clap 引数に URL 文字列を渡すと、
ファイルシステムのパスとして解釈されて存在チェックで失敗する。
URL も受け付けたい引数は `input: String` にし、
`starts_with("http://") || starts_with("https://")` で分岐する。

## LLM API制約

### Anthropic claude-sonnet-4-x 以降は assistant prefill を受け付けない
`messages` 配列の末尾に `{"role": "assistant", "content": "{"}` を追加して
JSON出力を強制する "prefill" テクニックは、claude-sonnet-4-x 系で HTTP 400 になる。
→ prefill を削除し、JSON出力はシステムプロンプトで指示する。
`strip_json_fences` でモデルが追加する ``` 等を後処理すれば十分。

## harumi (PDF生成ライブラリ)

### harumi 0.3.0 は既存PDFへのオーバーレイ専用
harumi は「既存PDFにCJKテキストを検索可能レイヤーとして追加する」ライブラリであり、
新規PDFをゼロから生成するAPIは持っていない（0.3.0時点）。
sds-converter の `to-pdf` に組み込むには `render_html_to_pdf(html, options)` のような
新APIが harumi 側に追加される必要がある（作者への要望として記録済み）。
→ 現状の `to-pdf` は soffice --headless に依存するCLI実装で維持する。

## セキュリティ設計

### URLフェッチのSSRF対策は必須
`reqwest` はデフォルトでリダイレクトを追跡し、任意のURLに接続する。
ユーザー指定URLを受け付ける場合は、リクエスト前にホストを解決し
プライベート/ループバック/リンクローカル/メタデータIPを拒否すること。
→ `is_private_host(host: &str) -> bool` ヘルパーをextractor.rsに実装済み。

### LLMユーザーメッセージ内のプロンプトインジェクション対策
`<document>…</document>` タグでドキュメントテキストを囲んでも、
テキスト内に `</document>` が含まれていればタグを閉じることができる。
→ 挿入前に `text.replace("</document>", "</_document>")` でエスケープすること。

### サーバーのデフォルトバインドアドレスはloopbackにすべき
開発用サーバーは `0.0.0.0` にバインドするとLAN全体に公開される。
デフォルトを `127.0.0.1` にし、`SDS_SERVER_BIND` 環境変数で上書きできるようにする。

## 非同期設計

### async fn内でのブロッキングI/Oは禁止
`convert_pdf_to_json_vision` のように `async fn` 内で `std::fs::read` を呼ぶと
Tokioのエグゼキュータスレッドが丸ごとブロックされ、GUIが凍結する。
→ `tokio::task::spawn_blocking(|| std::fs::read(...))` を使うこと。
codebaseでは他のブロッキングI/O（DOCX/XLSX抽出等）も既にspawn_blockingを使用している。

## ファイル操作

### ファイル名衝突解決はTOCTOU競合に注意
`exists()` でチェックして別の処理でファイルを作成する実装は競合状態（TOCTOU）になる。
並行バッチ変換で2つのタスクが同じパスを「空き」と判断し、後者が前者を上書きする。
→ `OpenOptions::new().write(true).create_new(true).open(path)` を使って
ファイル名を原子的に確保すること。

## テキスト処理

### Rustの `String::len()` はバイト数を返す（文字数ではない）
CJK文字（日中韓）は1文字=3バイトのUTF-8。
`text.len() > max_chars` という比較はmax_charsを「文字数」として使っているつもりでも
実際にはバイト数制限になり、日本語テキストは意図の1/3しかLLMに渡らない。
→ 文字数制限には `text.chars().count()` を使い、
切り詰めのバイトオフセットは `text.char_indices().nth(n)` で求めること。
