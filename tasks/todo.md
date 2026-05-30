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

## Phase 10: PDF抽出堅牢化 ✅
- [x] extractor.rs: pdftotext（poppler）フォールバック追加 — 日本語CIDフォント（Shift-JIS）PDFでpdf-extractがパニックする問題を修正
  - フォールバック階層: ① pdf-extract → ② pdftotext → ③ tesseract OCR / Claude Vision
  - poppler未インストール環境では既存③OCRへ自然にフォールバック
- [x] extractor.rs: pdftotext の `-utf8` フラグ削除 — poppler v24以降でフラグ廃止（exit 99）→ Shift-JIS PDFが無音でスキップされていたバグ修正

## Phase 11: サーバー修正 ✅
- [x] server: `/api/health` を認証ミドルウェアの外に移動
  - `.layer(require_auth)` はルーター全体に掛かるため health も 401 になっていた
  - `route_layer()` + `merge()` で protected/public を分離し、health は認証不要に
  - AWS LWA（Lambda Web Adapter）/ ロードバランサのヘルスチェックが通るようになった

## Phase 12: セキュリティ監査・バグ修正 ✅
- [x] **SEC-H1** `server/main.rs`: Bearer token比較を `constant_time_eq` によるタイミング攻撃対策比較に変更
- [x] **SEC-H2** `extractor.rs`: `shared_http_client` に `.redirect(Policy::none())` を追加 — リダイレクト経由SSRF防止
- [x] **SEC-H3** `server/main.rs`: `DefaultBodyLimit` を512MB → 50MBに削減（実際のSDS文書に十分な上限）
- [x] **SEC-M1** `extractor.rs`: `is_private_host` のIPv6ブランチを拡張
  - `fc00::/7`（ULAユニークローカル）
  - `fe80::/10`（リンクローカル）
  - `::ffff:` IPv4マップアドレス（プライベート/ループバック）
- [x] **H1** `llm.rs:660` / `llm.rs:926`: リトライ時の `lenient_deserialize` 失敗が `if let Ok` で無音に捨てられていたバグ修正 — `match` + `Err(e) => tracing::warn!` に変更（テキスト抽出・Vision両パス）
- [x] **M1** `llm.rs`: `repair_json` の盲目的 `str::replace` をバイト列ステートマシン `remove_trailing_commas` に置換
  - 文字列内の `,}` パターンを保持（例: `"ends here,}"` が壊れなくなった）
  - 不動点ループでネスト複合トレーリングカンマも解消
- [x] **H2** `enrichment.rs`: `names_similar` を部分文字列チェックからJaccardワード重複（閾値≥0.5）に変更 — 短い汎用語による誤検知を排除
- [x] `llm.rs`: `section!` マクロのスキーマ不一致警告に失敗値の先頭200文字を追加
- [x] `server/Cargo.toml`: `constant_time_eq = "0.3"` 追加
- [x] 新規ユニットテスト8件追加（`repair_json` 3件、`names_similar` 5件）— 全44テストPASS

## Phase 13: CI/CD リリース自動化 ✅
- [x] `.github/workflows/release.yml` 作成 — `v*.*.*` タグ push でトリガー
- [x] **Windows ジョブ** (`windows-latest`): `cargo build --release` → `sds-converter.exe` → `sds-converter-windows-portable.zip` → GitHub Release アップロード
- [x] **macOS ジョブ** (`macos-latest`): arm64 + x86_64 をビルド → `lipo` でユニバーサルバイナリ → `.app` バンドル → `sds-converter-macos.zip` → GitHub Release アップロード
- [x] Homebrew Cask 自動更新ステップ追加（`HOMEBREW_TAP_TOKEN` シークレット設定時のみ動作）
- [x] **バグ修正**: `HOMEBREW_TAP_TOKEN` 未設定時のジョブ失敗を修正
  - `continue-on-error: true` 追加
  - スクリプト内でトークン空チェック → `exit 0`（スキップ）
- [x] v0.2.2 で初回動作確認 — Windows・macOS 両ファイルとも Release に添付済み
- [x] `cargo publish` — sds-converter-core 0.3.2 / sds-converter 0.2.2 公開済み

## Phase 14: GUI UI改善 ✅
- [x] **H1** 最小ウィンドウサイズ設定 — `with_min_inner_size([640.0, 480.0])` によりウィンドウが小さくなりすぎない
- [x] **H2** バリデーションエラーの一貫性 — 入力/出力パス空の場合は `error_modal` を使用（ログに流すだけだった問題修正）（変換・生成・検証タブ）
- [x] **H3** キーボードショートカット — Ctrl+Q でアプリ終了、F1 でマニュアル表示、Ctrl+O でファイルピッカー起動
- [x] **H4** エラーモーダルを `egui::Modal` に変換 — バックドロップ付き真モーダル、Escape・バックドロップクリックで閉じる（エラー・Aboutダイアログ）
- [x] **M1** `Frame::NONE` — 非推奨 `Frame::none()` を修正
- [x] **M2** ログパネル高さ制限を撤廃 — `.max_height(160.0)` 削除でパネルが自由に拡張できるように
- [x] **M3** ログレンダリングのクローン排除 — ロックを保持したまま `ScrollArea` をレンダリング
- [x] **M4** 全 `TextEdit` フィールドに `hint_text` を追加
- [x] **M5** ファイルダイアログフィルターラベルをi18n化 — `lbl_filter_*` 文字列を使用
- [x] **M6** モーダル・ウェルカム画面のOK/Skipボタンをi18n化 — `btn_ok`、`btn_skip` 使用
- [x] **M7** Aboutダイアログを `egui::Modal` に変換（キーボード・バックドロップ対応）
- [x] **M8** busy-pollリペイントを100ms→250msに削減
- [x] **M9** 設定保存の「Saved」メッセージ自動クリア — `Instant` + `request_repaint_after(3s+50ms)` でアイドル中も確実に発火
- [x] **M10** `TextEdit` フィールドを `desired_width(avail - offset)` でレスポンシブ化（固定 `add_sized` を廃止）
- [x] **L1** APIキー表示/非表示トグルボタン追加（`show_api_key` フィールド）
- [x] **L2** D&Dでドロップされたファイルを拡張子でバリデーション — 拒否時にログ警告
- [x] **L3** D&D受け入れ拡張子をタブごとに設定
- [x] **L4** ウェルカム画面のskipボタンをi18n化 — `btn_skip` 使用
- [x] **L7** 変換・検証タブのバッチモードから冗長な「ファイルクリア」ボタンを削除
- [x] `Strings` 構造体に10フィールド追加（`btn_ok`、`btn_skip`、`btn_show_key`、`btn_hide_key`、`msg_drop_rejected`、`lbl_filter_{sds,json,doc,word,txt}`）、死んだ `btn_clear_files` フィールドを削除。3言語すべて（en/ja/zh-cn）更新
- [x] `SdsApp` 構造体に `settings_saved_at`、`show_api_key`、`open_file_dialog_requested` フィールド追加

## Phase 15: スキーマ互換性強化 ✅
- [x] schema/generated.rs: `SubstanceIdentifiersSubstanceIdentityCASno.full_text` に `flex_vec_string_opt` デシリアライザ追加 — LLMが裸の文字列でCAS番号を返す場合の `invalid type: string, expected a sequence` エラーを修正
- [x] llm.rs: `coerce_obj_to_string` ヘルパー追加 — `Colour`/`Odour`/`PhysicalState` が `{"AdditionalInfo":{"FullText":[...]}}` オブジェクトで返された場合に文字列へ正規化
- [x] llm.rs: `extract_text_from_value` ヘルパー追加 — String・Array・AdditionalInfoオブジェクトから統一的にテキスト抽出
- [x] llm.rs ユニットテスト2件追加: `normalize_colour_odour_from_additional_info_object`、`casno_full_text_flex_deserialization`

## Phase 16: 多国SDS対応・補正パス・バリデーター強化 ✅
- [x] SourceCountry enum（Japan/China/Taiwan/Korea）+ `--lang` からの自動推論（country.rs）
- [x] `--country cn|tw|kr|jp` CLI明示的上書きフラグ
- [x] 国別LLM抽出ルールのシステムプロンプト注入（CN: GB/T 16483、TW: CNS 15030、KR: K-GHS）
- [x] validate_country()による各規格準拠チェック（GB/T 16483 / CNS 15030 / K-GHS Rev.6）
- [x] ComplianceDiffReport + generate_compliance_diff()（compliance.rs、ConversionReport.compliance_diffに含める）
- [x] normalize_cas_full_text(): `\n`/`\r`/`,`/`;` 区切りのCAS連結文字列を個別エントリに分割
- [x] ensure_hazard_identification(): 非危険物でLLMがHazardIdentificationを省略した際の最小スタブ挿入
- [x] 補正パス: CorrectionConfig + CorrectionResult（corrector.rs、`--correct` フラグで有効化）
- [x] H-codeマッピングテーブル拡張（zh-cn/zh-tw表現追加）+ 複合ハザード分割指示
- [x] P-codeアノテーション除去（Pコードフィールドから `[H315]` 形式の括弧内H-codeを除去）
- [x] Visionパスへのリテキストパスと同等のCRITICAL指示適用
- [x] バリデーター強化: 濃度フィールド内の日付検出・製品名プレースホルダー検出・分類網羅性チェック・中国語キーワード（氯/氟/酐/酰等）によるH290クロスチェック・混合物対応AcuteToxicityカテゴリ vs H-codeクロスチェック

## Phase 17: LLM抽出品質向上（Round 21テスト） ✅
- [x] Section 1 Use フォールバック — セクション1.2が存在するがUseが不明の場合、ソーステキスト（'無相関詳細情報'/'无相关详细资料'/'no specific use listed'等）を1エントリとして Use 配列に格納、キー自体を省略しない
- [x] Section 8 OEL「不要求」フレーズ検出 — '不要求'/'无需监控'/'不适用'/'无职業接触限值'/'no limits established'/'not required'/'no monitoring required'等を「制限値なし」として認識し AdditionalInfo.FullText に格納
- [x] Section 9 Densities 必須抽出 — 密度・相対密度・比重を Densities 配列へ（数値→NumericRangeWithUnitAndQualifier、テキスト→AdditionalInfo.FullText）
- [x] Section 9 VapourPressure — H224/H225/H226/H330/H331/H332 を持つ引火性・揮発性製品で明示的に抽出指示を追加
- [x] Section 12 PersistenceDegradability — 残留性/分解性サブセクションが存在する場合は BiologicalDegradability を常に格納（データなしの場合は'該当データなし'/'无相关数据'）

## 残タスク
- [ ] generator.rs: 表レイアウトDOCX（Section 3 Composition 4列表、Section 2 H/P 2列表、Section 9 物性 2列表）
- [x] harumi 対応: HTML→PDF 純Rust生成 — harumi v0.4.0 の `html` feature で `render_html_to_pdf` を使用（`converter/pdf.rs` 実装済み）
