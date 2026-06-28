# sdsconv

**SDS文書を厚生労働省SDSデータ交換フォーマットJSONへ変換し、スキーマ検証・GHS/CAS検証・コーパス規模品質評価まで行う Python-first / Rust-powered ツールキット。**

[English](README.md) | [中文](README_zh.md)

---

## インストール

```bash
pip install sdsconv                   # Python バインディング
pip install "sdsconv[analysis]"       # + causasv 品質分析
cargo install sdsconv                 # CLI / GUI バイナリ
```

---

## クイックスタート — Python

```python
import sdsconv, os

# SDS文書 → 厚労省標準JSON
data, report = sdsconv.to_json_with_report(
    "sample.pdf",
    lang="ja",
    strict_mhlw=True,
)

# 構造化 findings を取得
findings = sdsconv.validate(data, strict_mhlw=True)

print(f"抽出セクション数: {len(report['populated_sections'])}")
print(f"検知件数: {len(findings)} (HIGH: {sum(1 for f in findings if f['level']=='HIGH')})")

# MHLW JSON を保存
sdsconv.write_json(data, "output.json")
```

コーパス規模評価（人手レビュー不要）:

```python
from sdsconv.eval import eval_corpus

df = eval_corpus(
    input_dir="data/sds_raw",
    output_dir="runs/eval_001",
    jobs=8,
)
print(df[["filename", "overall_score", "grade", "high_count"]].head(20))
```

---

## なぜ sdsconv か

- **MHLW ネイティブ**: 厚生労働省 SDS データ交換フォーマット v1.0（`SDS_Schema_v1.0.json`）への直接変換とスキーマ検証に対応。
- **根拠付き抽出**: LLM が SDS 自由記述文を約200の深いネストフィールドへマッピング。フィールド単位の原文照合によりハルシネーションを検出。
- **コーパス規模品質評価**: `eval_corpus` は数百件の SDS を処理し、ルール別失敗件数・セクションスコア・`causasv_features.csv` を出力。人手レビュー不要。
- **ロックインなし**: Anthropic Claude・OpenAI GPT・Google Gemini・Mistral・Groq・Cohere・任意の OpenAI 互換ローカルエンドポイントに対応。
- **Rust コア**: 抽出・スキーマ検証・GHS/CAS チェック・DOCX/HTML 生成はネイティブコードで実行。Python バインディングは薄いラッパー。

---

## MHLW 準拠

令和7年3月31日公開の厚労省 SDS データ交換フォーマット v1.0 に準拠します。

| ルール | 動作 |
|---|---|
| スキーマ検証 | `SDS_Schema_v1.0.json` に対して検証 |
| 空フィールド除去 | §3.3 準拠で `""`・`null`・`[]`・`{}` を除去 |
| AdditionalInfo | 公式スキーマ外の情報は `AdditionalInfo.FullText` に格納 |
| `--strict-mhlw` | HIGH/CRIT が存在する場合は終了コード1（CLI）または `ValueError`（Python） |
| CRIT/HIGH/MED findings | ルールID・重大度・パス・メッセージを含む構造化レポート |

**検証ルール例:** GHS H/P コード妥当性（GHS Rev.10）、CAS フォーマット＋チェックデジット、Section 2 GHS 整合性（H コード ↔ 絵表示 ↔ 注意喚起語）、Section 3 成分行対応（名前/CAS/濃度）、UN 番号完全性、濃度範囲チェック、コード重複検出など。

品質ベースライン（30件ランダムサンプル、seed=42）:
> CRIT=0 · 平均スコア 89.6 · 主要課題: `S2-HAZARD-NO-PICTOGRAM`・`S15-ZHCN-NO-GB`・`S14-NO-SHIPPING-NAME`

全ルール詳細 → [docs/quality-check_ja.md](docs/quality-check_ja.md)

---

## コーパス評価

人手レビューなしで実行:

```python
from sdsconv.eval import eval_corpus

df = eval_corpus("data/sds_raw", "runs/eval_001", jobs=8)
```

ファイルごとの出力:

| ファイル | 内容 |
|---|---|
| `generated/<stem>.json` | MHLW 標準 JSON |
| `reports/<stem>.json` | ConversionReport（言語・セクション・警告） |
| `findings/<stem>.json` | 構造化 validation findings |
| `summary.csv` | ファイルごとのスコア・グレード |
| `failures_by_rule.csv` | ルール別失敗件数・影響ファイル数 |

[causasv](https://github.com/kent-tokyo/causasv) で失敗要因を因果分析:

```python
from sdsconv.causasv_bridge import print_ranking
print_ranking("runs/eval_001/causasv_features.csv")
```

---

## CLI

```bash
# PDF/DOCX/XLSX/HTML/URL → MHLW JSON
sdsconv to-json --input input.pdf --output output.json --lang ja

# 補正パス＋PubChem照合付き
sdsconv to-json --input input.pdf --output output.json --correct --enrich

# JSON → Word文書（JIS Z 7253 16項目）
sdsconv to-docx --input output.json --output result.docx --lang ja

# JSON → HTML（印刷対応・A4・inline CSS）
sdsconv to-html --input output.json --output result.html --lang ja

# strict MHLW モードで検証
sdsconv validate --input output.json --strict-mhlw

# バッチ処理（ディレクトリ単位）
sdsconv to-json --input-dir data/ --output-dir out/ --jobs 8

# コーパス評価
sdsconv eval-corpus --input-dir data/sds_raw --output-dir runs/eval_001 --jobs 8
```

CLI 詳細リファレンス → [sdsconv/README_ja.md](./sdsconv/README_ja.md)

---

## REST API

```bash
# サーバー起動（デフォルト: 127.0.0.1:3000）
SDS_SERVER_TOKEN=secret sdsconv-server

# PDF を変換
curl -X POST http://localhost:3000/api/to-json \
  -H "Authorization: Bearer secret" \
  -F "file=@input.pdf"
```

エンドポイント: `POST /api/to-json` · `POST /api/to-docx` · `POST /api/to-html` · `POST /api/validate` · `GET /api/health`

---

## GUI

引数なしで起動するとグラフィカルインターフェースが開きます:

```bash
sdsconv
```

5タブ構成: **変換** · **文書生成** · **検証** · **テキスト抽出** · **設定**

デスクトップアプリ: [macOS](https://github.com/kent-tokyo/sdsconv/releases/latest/download/sdsconv-macos.zip) · [Windows](https://github.com/kent-tokyo/sdsconv/releases/latest/download/sdsconv-windows-portable.zip) · `brew install --cask sdsconv`

---

## 対応入力・言語・バックエンド

**入力形式:** PDF（テキスト・CID/Shift-JIS フォント・スキャン）· DOCX · XLSX · TXT · HTML · URL

**ソース言語:** `ja`（JIS Z 7253）· `en`（GHS/OSHA HazCom）· `zh-cn`（GB/T 16483）· `zh-tw`（CNS 15030）

**LLM バックエンド:** Anthropic Claude · OpenAI GPT · Google Gemini · Mistral · Groq · Cohere · ローカル（OpenAI 互換エンドポイント）

---

## 開発者向け

**Rust ライブラリ:**

```toml
[dependencies]
sdsconv-core = "0.3"
```

Rust API 詳細 → [sdsconv_core/README_ja.md](./sdsconv_core/README_ja.md)

**クレート:** [`sdsconv`](https://crates.io/crates/sdsconv) · [`sdsconv-core`](https://crates.io/crates/sdsconv-core)

**Python パッケージ:** [`sdsconv`](https://pypi.org/project/sdsconv/) — `pip install sdsconv`

---

## セキュリティ・プライバシー

- **クラウド LLM の注意事項**: クラウド LLM バックエンドを使用する場合、SDS 文書のテキストが API プロバイダーに送信されます。機密情報・営業秘密を含む SDS をクラウド API に送信しないでください。
- **ローカル運用**: `--backend local` と任意の OpenAI 互換エンドポイント（Ollama・LM Studio 等）を使用することで完全オフライン運用が可能です。データは機器外に出ません。
- **raw SDS コーパス**: `corpus/raw/` · `data/sds_raw/` を `.gitignore` に追加してください。`corpus/manifest.jsonl`（URL と sha256 ハッシュのみ）のみコミットしてください。
- **REST サーバー**: タイミング攻撃対策済み Bearer token 認証、IPv6 フルカバレッジの SSRF 対策、リダイレクト無効化 HTTP クライアント、50 MB アップロード上限。

---

## 競合製品との比較

→ [docs/comparison.md](docs/comparison.md)

---

## 参考リンク

- [厚生労働省 — SDS情報交換のための標準的フォーマット等の公開について](https://www.mhlw.go.jp/stf/newpage_56484.html)
- [SDSデータ交換フォーマット データ利用マニュアル（PDF）](https://www.mhlw.go.jp/content/11305000/001467068.pdf)
- [JSON 品質チェック詳細マニュアル — 53 チェック項目をセクション別に解説](docs/quality-check_ja.md) ([English](docs/quality-check.md) / [中文](docs/quality-check_zh.md))
- [CHANGELOG](CHANGELOG.md)

---

## ライセンス

MIT または Apache-2.0 — どちらかを選択。
