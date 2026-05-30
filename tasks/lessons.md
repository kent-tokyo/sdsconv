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

### Axum の `.layer()` はルーター全体に掛かる — ヘルスチェックに注意
`Router::new().route(...).route(...).layer(auth_middleware)` と書くと、
auth ミドルウェアが追加されたすべてのルート（ヘルスチェックを含む）に適用される。
AWS LWA / ロードバランサのヘルスチェックは Bearer token を送らないため、
health エンドポイントも 401 になりデプロイが失敗する。

解決策: `route_layer()` + `merge()` で public / protected を分離する。
```rust
// 認証必須ルートにのみ route_layer() を適用
let protected = Router::new()
    .route("/api/to-json", post(to_json))
    .route_layer(middleware::from_fn_with_state(token.clone(), require_auth));

// 認証不要ルートは別の Router に定義
let public = Router::new()
    .route("/api/health", get(health));

// merge 後に共通レイヤー（CORS 等）を適用
let app = public.merge(protected).layer(cors).layer(body_limit);
```
`.nest("/api", ...)` を入れ子にすると URL が `/api/api/...` になるため使わないこと。

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

## PDF抽出

### pdf-extract は CIDフォント（Shift-JIS等）でパニックする
`pdf-extract` クレートは、Shift-JIS エンコーディングを持つ日本語CIDフォントPDFを処理すると
`src/lib.rs` 内の `Result::unwrap()` でパニックする（`FromUtf8Error`）。
`spawn_blocking` がパニックを `JoinError` として捕捉するため実行は継続するが、
戻り値は空文字列になりOCRフォールバックが起動する。PDFにはテキストがあるのでOCRは不要・高コスト。
→ `pdftotext -utf8`（poppler）をpdf-extractとOCRの間に中間フォールバックとして挿入する。
  poppler未インストール環境では `Command::new("pdftotext")` が `Err` になり `None` を返すため、
  既存のOCRフォールバックへ自然に fallthrough する。

### 3段階PDFテキスト抽出フォールバック
1. **pdf-extract**（Rustクレート）— 標準的なテキストPDF（Latin/UTF-8フォント）
2. **pdftotext -utf8**（poppler CLI）— CIDフォント/Shift-JIS日本語PDF
3. **tesseract OCR / Claude Vision** — 画像PDF・スキャンPDF

各段は「200文字未満」を閾値として次段へフォールバック。
`pdftotext` は `poppler-utils` パッケージに含まれ、`pdftoppm`（OCR用ラスタライザ）と同じパッケージ。

## GitHub Actions / CI

### `secrets.XXX` 未設定時のステップ失敗はジョブ全体を落とす
オプションのシークレット（Homebrew tap トークンなど）を参照するステップは、
シークレット未設定時に `Authentication failed` 等でエラー終了し、ジョブ全体が Failure になる。
→ `continue-on-error: true` を追加し、スクリプト先頭でトークン空チェック + `exit 0` を入れること。

```yaml
- name: Update Homebrew Cask
  continue-on-error: true
  env:
    HOMEBREW_TAP_TOKEN: ${{ secrets.HOMEBREW_TAP_TOKEN }}
  run: |
    if [ -z "${HOMEBREW_TAP_TOKEN}" ]; then
      echo "HOMEBREW_TAP_TOKEN not set — skipping"
      exit 0
    fi
    # ... actual work
```

### GitHub Actions の `if:` 条件で secrets を直接参照できない
`if: secrets.FOO != ''` は構文として成立しないことがある（式コンテキスト依存）。
確実に動作するのは上記のようにステップ内スクリプトでチェックする方法。

### eframe (egui) は Windows / macOS でランタイム DLL 不要
`cargo build --release` でビルドした `.exe` は Windows ランナーで単体動作する。
追加の VC++ ランタイムや DLL 配布は不要（Rust の静的リンクにより自己完結）。
macOS も同様 — ユニバーサルバイナリ (lipo) を `.app` バンドルに入れるだけで動作する。

### macOS Universal Binary の作り方（GitHub Actions）
```yaml
- run: cargo build --release --target aarch64-apple-darwin -p <crate>
- run: cargo build --release --target x86_64-apple-darwin -p <crate>
- run: |
    lipo -create \
      target/aarch64-apple-darwin/release/<bin> \
      target/x86_64-apple-darwin/release/<bin> \
      -output target/<bin>-universal
```
`dtolnay/rust-toolchain@stable` に `targets: aarch64-apple-darwin,x86_64-apple-darwin` を指定する。

### リリース資産のファイル名は README のリンクと完全一致させること
`/releases/latest/download/<filename>` のURLは資産名に完全一致が必要。
ワークフローのzipファイル名と README のダウンロードリンク名が1文字でも違うと 404 になる。

## セキュリティ設計（追加）

### タイミング攻撃対策: Bearer token比較は `constant_time_eq` を使うこと
`t == token.as_str()` のような通常の文字列比較は、最初の不一致バイトで早期終了するため
タイミング攻撃（timing side-channel）の脆弱性がある。短いトークンほど高速に拒否され、
攻撃者が差分を計測できる。
→ `constant_time_eq::constant_time_eq(t.as_bytes(), token.as_bytes())` を使用すること。
`constant_time_eq = "0.3"` を `Cargo.toml` に追加する。

### HTTPリダイレクトはSSRFの迂回路になる
`reqwest` のデフォルト設定はリダイレクトを自動追跡する（最大10回）。
SSRF対策のホストチェックはリクエスト前に行うが、最初のURLが公開ホストであっても
レスポンスが `Location: http://169.254.169.254/...` にリダイレクトすれば対策を回避できる。
→ URL取得用クライアントには `.redirect(reqwest::redirect::Policy::none())` を設定すること。

### IPv6プライベートアドレスはループバックだけではない
`v6.is_loopback() || v6.is_unspecified()` だけでは不十分。以下も拒否すること:
- `fc00::/7` — ULAユニークローカル（RFC 4193）
- `fe80::/10` — リンクローカル（RFC 4291）
- `::ffff:x.x.x.x` — IPv4マップアドレス（RFC 4291 §2.5.5.2）: 埋め込みIPv4を抽出して `is_private()` / `is_loopback()` / `is_link_local()` を検査

### アップロードサイズ上限は実際のユースケースに合わせて設定すること
Axum の `DefaultBodyLimit::max(512 * 1024 * 1024)` (512MB) は現実的なSDS文書には過大。
実際のSDS文書は数MB以下なので50MBが適切。過大な上限はメモリ枯渇DoSのリスクになる。
→ `DefaultBodyLimit::max(50 * 1024 * 1024)` を使用すること。

### `repair_json` の盲目的 `str::replace` は文字列内容を破壊する
`s.replace(",}", "}")` のような全域置換は、JSON文字列値内に `,}` というパターンが
含まれる場合（例: `"note": "ends here,}"`）にその値を破壊してしまう。
→ バイト列ステートマシンで `in_string` 状態を追跡し、文字列外のみ置換すること。
エスケープシーケンス（`\"`）の処理も忘れずに。不動点ループで多重トレーリングカンマも解消。

### 化学物質名の類似判定に部分文字列包含を使わない
`a.contains(&b) || b.contains(&a)` は「acid」が「acetic acid」に含まれるような
短い汎用語で誤検知を起こす。また非常に長い名前に対してO(n²)の計算量になる。
→ Jaccardワード重複係数（交叉数/和集合数 ≥ 0.5）を使用すること。
O(n)で動作し、単語レベルの類似度をより正確に捉える。

## テキスト処理

### Rustの `String::len()` はバイト数を返す（文字数ではない）
CJK文字（日中韓）は1文字=3バイトのUTF-8。
`text.len() > max_chars` という比較はmax_charsを「文字数」として使っているつもりでも
実際にはバイト数制限になり、日本語テキストは意図の1/3しかLLMに渡らない。
→ 文字数制限には `text.chars().count()` を使い、
切り詰めのバイトオフセットは `text.char_indices().nth(n)` で求めること。

## 多国SDS対応

### CAS連結文字列
一部のSDS（特に中国製）では、複数のCAS番号を `\n`・カンマ・セミコロンで連結した1つの文字列として格納している。常に `\n`/`\r`/`,`/`;` で分割してトリムすること。`normalize_cas_full_text()` 関数がこの処理を担当する。

### 非GHS形式のMSDS（前GHS時代の中国語SDS）
化工仪器网・ichemistry等の中国語SDSは、標準的なP-codeが存在しない前GHS時代のMSDS形式を使用していることがある。LLMは記述からH-codeを正しく推論できるが、P-codeは原文に存在しない。これは抽出バグではなく、ソースSDS自体の品質上の問題として扱うこと。

### 国別バリデーション vs 抽出品質
国別バリデーション警告（例: GB/T 16483では24時間緊急連絡先が必要）は、コア抽出品質チェックとは別に管理すること。これらはソースSDS内の規制遵守上のギャップを示すものであり、抽出器のバグではない。

### 品質チェッカーのフィールドパス精度
実際のスキーマ（generated.rs）に照らしてJSONフィールドパスを必ず検証すること。例: 物質名は `SubstanceIdentity.SubstanceNames` ではなく `SubstanceIdentifiers.SubstanceNames`、SDS日付は `Datasheet.SDSDate` ではなく `Datasheet.IssueDate` に格納されている。

### 空のJSONセクション vs 抽出失敗
`ToxicologicalInformation`/`EcologicalInformation` が `[{}]` や `{}` となっている場合、ソースSDS が「データなし」と記載していることの正しい反映であり、バグではない。内容が意味あるものかチェックする前に、JSONの構造文字（`{`・`}`・`[`・`]`）を除去すること。

## LLM抽出品質チェック（QCフォールスポジティブ）

### H226 + Dangerシグナルの誤検知
H226（第3類引火性液体）単独でDangerシグナルを持つ場合は通常の警告対象だが、同一製品にH314/H370/H350等の「Danger相当Hコード」が他に存在する場合は、そのDangerシグナルは他コードによるものであり誤検知となる。
QCチェック実装では `other_danger = any(h for h in hcodes if h != "H226" and h starts with danger_prefix)` で他のDangerレベルコードがあれば警告をスキップすること。

### FireFighting「適切な消火方法を使用」の誤検知
"Use appropriate extinguishing measures" / "適切な消火措置" のような汎用的な記述は、特定の消火剤名（foam/water/CO2等）を列挙しない形式で規格準拠のSDS（特に英語・中国語）に多く見られる。QCでは "appropriate"/"extinguish"/"灭火"/"消火" といったキーワードも許容すること。

### Transport「規制されていない」の日本語表現
"非危険物"だけでなく "規制されていない"/"規制対象外"/"危険物に該当しない"/"not subject"/"no regulation" 等の表現もチェックに含めること。

### StabilityReactivity「該当データなし」の誤検知
安定性・反応性セクションで "無相関詳細資料"/"无相关详细资料"/"N/A"/"ConditionsToAvoid"（JSONキー名）等の「データなし」表現が含まれていれば、有意な内容があるとみなしてよい。QCの文字列比較は case-sensitive で行い、JSONキー名自体も検索対象に含めること（`sr_str` ではなく `sr_lower` を使うと JSONキー名が消えて検出できない）。

### 吸入FirstAidのキーワード vs キー存在確認
FirstAid テキストに "inhalation" 等のキーワードがなくても、`FirstAidInhalation` というキーが JSON に存在すれば吸入ルート対応とみなせる。テキスト内容のキーワード検索より先に、JSONキー名の存在を確認すること（`route_keys_lower` の辞書で `"firstaidinhalation" in route_keys_lower` を優先）。

### PersistenceDegradability の文字列チェック
`PersistenceDegradability` というキー名を含む文字列を small-case 変換（`eco_lower`）で検索すると `"persistencedegradability"` になり、`"PersistenceDeg"` 等のサブストリングでマッチしない。JSON文字列は case-sensitive なので `eco_str`（変換前）で検索するか、`"degradab"` 等の小文字でも一致するキーワードを使うこと。

## LLM抽出 — 「なし」表現の検出

### OEL「不要求」フレーズ群
中国語・日本語SDSでは職業ばく露限値が設定されていない場合に "不要求"/"无需监控"/"不适用"/"无职業接触限值" 等と記載する。LLMがこれらを無視して ExposureControlPersonalProtection.OccupationalExposureLimits を省略しないよう、プロンプトにこれらのフレーズを明示的に列挙し "include one entry with AdditionalInfo.FullText quoting that source phrase" と指示すること。

### Use フィールドの省略防止
Section 1.2 が存在するが具体的な使用目的が記載されていない場合（'無相関詳細情報'/'无相关详细资料'/'no specific use listed'等）、LLMは Use 配列を省略しがち。"never omit the Use key when Section 1.2 is present" と明示し、データなしのソーステキストを1エントリとして格納するよう指示すること。

## 中国旧規格（GB 13690-92）の引火点分類

ichemistry等の古い中国語SDSは GB 13690-92（前GHS時代）の引火点分類（甲A/甲B/乙/丙等）を使用している。この分類はGHS Cat.2/3と1対1対応しないため、LLMが推論で H225 vs H226 を誤ることがある。例：FP=16°C の液体が "高闪点" 分類（丙類）で記載されている場合、GHS的には H225 だが旧分類の "丙" は H226 に誤対応する。これはソース品質の問題であり、抽出バグではない。

---

## Round 22 分析（quality_check_r22.py / 30件）

### 最頻出 QC 問題トップ 10（件数）

| 件数 | 問題 | 種別 |
|---|---|---|
| 10 | CompanyName empty | ソース限界（前 GHS ichemistry） |
| 10 | 手袋材質未指定（nitrile/butyl 等） | 抽出ギャップ → Phase 18 対応可 |
| 8 | pH 未抽出（腐食性 H コード） | 抽出ギャップ → Phase 18 対応可 |
| 7 | NO P-codes | ソース限界（前 GHS MSDS） |
| 7 | GB 標準規格参照なし（zh-cn） | ソース限界（旧 MSDS） |
| 6 | Proper Shipping Name 未抽出 | 抽出ギャップ → Phase 18 対応可 |
| 6 | AutoIgnitionTemperature 未抽出 | 抽出ギャップ → Phase 18 対応可 |
| 5 | Density/RelativeDensity 未抽出 | 一部継続課題 |
| 3 | LogP/Kow/BCF 未抽出 | 抽出ギャップ → Phase 18 対応可 |
| 3 | 廃棄方法の記述なし（zh-tw） | ソース品質（大学系 MSDS） |

### Phase 18 プロンプト改善候補

1. **Sec8 手袋材質**: HandProtection 抽出時に「具体的な材質名（nitrile/butyl rubber/neoprene/天然ゴム等）を必ず記載」と指示
2. **Sec9 pH**: 腐食性・酸性製品（H314/H290/H318/H319）では pH 値を必ず抽出
3. **Sec9 AutoIgnitionTemperature**: 引火性液体（H224/H225/H226）では自然発火温度を抽出
4. **Sec12 LogP/BCF**: 環境 H コード（H4xx）では分配係数・生体濃縮係数を抽出
5. **Sec14 ProperShippingName**: UN 番号がある場合は正式品名を必ず抽出

### zh-cn ichemistry の構造的限界

前 GHS 時代（〜2003 年）の中国 MSDS は GHS 形式に対応していない：
- CompanyName が原文に存在しない（会社リスト形式で管理）
- P コードが存在しない（GHS ラベルがない）
- GB 30000 系参照がない（代わりに GB 13690-92 等の旧規格）
これらは抽出ギャップではなくソース品質限界。r22 QC で HIGH が出るのは仕様通り。
