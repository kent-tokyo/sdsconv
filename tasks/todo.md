# SDS Converter Core — TODO

## Phase 0: Scaffolding ✅
- [x] Cargo.toml: package name `sds-converter-core`、edition="2021"修正、lib + bin設定
- [x] src/lib.rs, src/error.rs, src/language.rs の骨格作成
- [x] モジュールディレクトリとプレースホルダmod.rs作成
- [x] cargo check が通ること

## Phase 1: Schema Generation ✅
- [x] tools/generate_schema.py 作成
- [x] コードジェネレータ実行 → src/schema/generated.rs (204 structs)
- [x] SdsRoot round-tripユニットテスト (serialize→deserialize) — 2件PASS

## Phase 2: Core Conversion ✅
- [x] src/error.rs (SdsError)
- [x] converter/extractor.rs (PDF/DOCX/XLSX/TXT抽出 + フォーマット判定)
- [x] converter/llm.rs (LlmBackend trait + AnthropicBackend + OpenAiCompatBackend)
- [x] converter/validator.rs (5種類の構造チェック)
- [x] converter/generator.rs (SdsRoot → .docx、4言語セクション見出し + フィールドレンダリング)
- [x] converter/mod.rs (公開API、ConvertConfig with source/output language)

## Phase 3: CUI ✅
- [x] src/main.rs: clapサブコマンド (to-json, to-docx, validate, extract-text)
- [x] --lang ja/en/zh-cn/zh-tw
- [x] --quality low/medium/high プリセット
- [x] --concurrency バッチ並列数（デフォルト4）
- [x] 7プロバイダ対応: anthropic/openai/gemini/mistral/groq/cohere/local
- [x] validate --json フラグ（CI向けJSON配列出力）

## Phase 4: 品質・信頼性 ✅
- [x] 並列LLM呼び出し（GROUP_A/GROUP_B + tokio::join! で約2倍高速化）
- [x] 429/529 指数バックオフリトライ（最大3回、2^n秒）
- [x] スキップセクション再試行（3回目LLMコールで失敗セクションのみ再抽出）
- [x] PDF抽出を spawn_blocking で非同期化
- [x] Anthropic extended-cache-ttl ベータヘッダ追加
- [x] XLSX/XLS入力サポート（calamine クレート）

## Phase 5: crates.io 公開準備 ✅
- [x] README.md (CLI/APIリファレンス、4言語対応表、全プロバイダ記載)
- [x] README_ja.md / README_zh.md の記述
- [x] Cargo.toml: exclude設定、readme フィールド
- [x] AnthropicBackend/LlmBackend/LlmConfig を root 再エクスポートに追加
- [x] lib.rs クレートレベル doc コメント（docs.rs用）
- [x] cargo publish --dry-run — エラーなし通過

## Phase 6: バグ修正・堅牢化 ✅
- [x] CJK文字化けバグ修正: normalize_split_runs がバイト単位キャスト→文字列スライスに修正
- [x] EcologicalInformation データ欠落修正: LLMスキーマヒントのキー名誤り訂正
- [x] HTTP 400 修正: claude-sonnet-4-x系で assistant prefill 非対応 → prefillを削除
- [x] spawn_blocking 漏れ修正: DOCX/TXT/XLSX抽出を非同期化
- [x] バッチエラーカウント漏れ修正: serialize/writeエラーを正しくカウント
- [x] flatten_sds: 戻り値を Result 化しエラーを伝搬
- [x] セパレータ判定: len()→chars().count() でCJK文字に対応
- [x] ファイルサイズ制限: PDF/DOCX/XLSX=500MB、TXT/JSON=100MB
- [x] ZIPボム対策: テンプレートファイル50MB、エントリ100MB制限
- [x] HTTPタイムアウト: LLMバックエンドに120s総タイム・10s接続タイムアウト追加
- [x] プロンプトインジェクション対策: 文書テキストを <document> タグで囲む
- [x] shared send_with_retry ヘルパー: Anthropic/OpenAICompatで重複ロジックを統合
- [x] collect_files / check_json_file_size ヘルパー: main.rsの重複コード整理
- [x] CHANGELOG.md 作成

## Phase 7: 機能拡張（競合ギャップ解消） ✅
- [x] ghs_codes.rs: GHS Rev.10 H/Pコードデータベース (H200–H420, P101–P503)
  - binary_search による O(log n) バリデーション
  - 複合Pコード (P301+P330+P331) の + 区切り検証
  - h_code_description / p_code_description 説明文マッピング
- [x] validator.rs: HazardStatementCode H-code 検証、PrecautionaryStatementCode P-code 検証
- [x] validator.rs: CAS番号フォーマット + チェックデジット検証 (validate_cas_format)
- [x] enrichment.rs: PubChem REST API による CAS照合
  - lookup_cas / enrich_composition 公開API
  - CasWarning::NotFound / NameMismatch
- [x] extractor.rs: HTML/URL入力対応
  - InputFormat::Html (.html/.htm) / Url (http/https)
  - detect_format_str(input: &str) 追加
  - extract_text_from_html_str: scraper による可視テキスト抽出（表セルはタブ区切り）
  - extract_text_from_url / extract_text_from_url_limited
- [x] converter/html.rs: generate_html() — UTF-8 HTML5 + inline CSS (to-html 用)
  - @media print 対応
  - 表・リスト・見出しレイアウト
  - 4言語対応 (ja/en/zh-Hans/zh-Hant)
- [x] converter/mod.rs: convert_url_to_json() 追加
- [x] main.rs: to-html サブコマンド (single + batch)
- [x] main.rs: to-pdf サブコマンド (soffice --headless、single + batch)
- [x] main.rs: to-json --input がURL文字列を受け付けるように変更
- [x] main.rs: to-json --enrich フラグ (PubChem CAS照合)
- [x] Cargo.toml (core): scraper = "0.21" 追加
- [x] Cargo.toml (cli): reqwest 追加

## Phase 8: GUIアプリケーション ✅
- [x] eframe/egui GUI（引数なし起動でウィンドウ表示、引数あり→CUIモード）
- [x] 5タブ: SDS→JSON変換 / 文書生成 / 検証 / テキスト抽出 / 設定
- [x] バッチモード: 変換・検証タブで複数ファイル一括処理
- [x] ドラッグ&ドロップ入力（全タブ、ホバーオーバーレイ付き）
- [x] 設定ファイル永続化（config.toml、Unix 0o600権限）
- [x] モデル名 / base URL フィールド（設定タブ、AppConfigに追加）
- [x] DOCXテンプレート選択（生成タブ、DOCX形式選択時のみ表示）
- [x] テキスト抽出タブ（ExtractText、API不要、URL対応、インライン表示）
- [x] BusyGuard RAII / エラーモーダル / ログパネル（max 500行）
- [x] 日本語 / English / 简体中文 UI対応
- [x] プロバイダ別APIキー取得リンク・オンボーディングバナー

## Phase 9: セキュリティ強化・バグ修正・品質向上 ✅
- [x] extractor.rs: URLレスポンス50MB上限 + Content-Lengthプリチェック
- [x] extractor.rs: bytes-vs-chars バグ修正（日本語テキストが1/3に切り詰められるバグ）
- [x] extractor.rs: SSRF対策 — プライベート/ループバック/メタデータIPを拒否
- [x] mod.rs: convert_pdf_to_json_vision内のブロッキングstd::fs::readをspawn_blockingに変更
- [x] enrichment.rs: PubChem呼び出しに250msレート制限 + HTTP 429リトライ
- [x] tasks.rs: resolve_unique_suggested_pathのTOCTOU競合をcreate_new(true)で解消
- [x] tasks.rs: prune_empty_strings — §3.3準拠の空フィールド除去
- [x] tasks.rs + config.rs + app.rs + main.rs: 推奨ファイル名出力（--suggested-nameフラグ + GUIチェックボックス）
- [x] app.rs: ログVec500行上限の実装（UIラベルと実装が不一致だったバグ）
- [x] app.rs: start_generateに出力パス空チェックを追加
- [x] app.rs: async closureでの日本語ハードコードを除去（i18n対応）
- [x] error.rs: SdsError::display_safe() — クライアント安全なエラーメッセージ
- [x] llm.rs: </document>エスケープでプロンプトインジェクション対策強化
- [x] llm.rs: ProductNoUserをMHLW_SCHEMA_HINTに追加
- [x] server: Bearer tokenによる認証（SDS_SERVER_TOKEN）+ デフォルトbindを127.0.0.1に変更
- [x] server: CORS制限（permissive → localhostのみ）
- [x] server: ConcurrencyLimitLayer(10)で同時接続制限
- [x] server: LLMエラーボディをクライアントに返さないよう修正
- [x] generator.rs + html.rs: lang_index/section_nameをpub(crate)化し重複削除

## 残タスク
- [ ] `cargo publish` で実際に公開（core → CLI の順）— crates.io 登録済み後に実施
- [ ] generator.rs: 表レイアウトDOCX（Section 3 Composition 4列表、Section 2 H/P 2列表、Section 9 物性 2列表）
- [ ] harumi 対応: HTML→PDF 純Rust生成（harumi の render_html_to_pdf API 追加待ち）
