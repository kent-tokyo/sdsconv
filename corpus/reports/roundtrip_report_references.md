# SDS Converter — references ラウンドトリップ評価レポート

**日時**: 2026-05-25  
**対象**: references/sds/ja/ からランダム選出 10件  
**パイプライン**: PDF→JSON（Step1）→ DOCX（Step2）→ JSON（Step3）  
**品質設定**: `--quality medium`（max_tokens=16384）  

---

## 全体結果

| ファイル | サプライヤ | Step1 | Step2 | Step3 | セクション保持 | Step3 mismatch |
|---------|-----------|-------|-------|-------|--------------|----------------|
| ref_01 W01W0101-0041 | fujifilm_wako | ✅ | ✅ | ✅ | 16/16 + Composition追加 | 0件 |
| ref_02 W01W0115-0182 | fujifilm_wako | ❌ | — | — | — | — |
| ref_03 46001_r | eneos | ✅ | ✅ | ✅ | 17/17 | 0件 |
| ref_04 J_20326 | kanto_chemical | ✅ | ✅ | ✅ | 17/17 | 0件 |
| ref_05 W01W0115-0108 | fujifilm_wako | ✅ | ✅ | ✅ | 17/17 | 0件 |
| ref_06 W01W0112-0153 | fujifilm_wako | ✅ | ✅ | ✅ | 16/16 | 0件 |
| ref_07 W01W0108-0140 | fujifilm_wako | ✅ | ✅ | ✅ | 17/17 | 0件 |
| ref_08 W01W0103-0223 | fujifilm_wako | ✅ | ✅ | ✅ | 17/17 | 0件 |
| ref_09 J_23003 | kanto_chemical | ✅ | ✅ | ✅ | 16/16 | 0件 |
| ref_10 W01W0113-0099 | fujifilm_wako | ✅ | ✅ | ✅ | 17/17 | 0件 |

**結果: ✅9件 ❌1件（ref_02のみ）**

---

## 発見した問題と対処

### 問題1: pdftotext `-utf8` フラグ廃止（修正済み）

**症状**: fujifilm_wako / eneos など Shift-JIS CID フォント使用 PDFs が全件 Step1 で失敗。エラー: `Extraction failed: No text extracted`

**根本原因**:
- `pdf-extract` は Shift-JIS CID フォントでパニック → `spawn_blocking` がキャッチ → 空文字列
- `pdftotext` フォールバックが `-utf8` フラグ付きで起動
- poppler v24 以降 `-utf8` は廃止オプション（exit code 99）→ フォールバック失敗
- tesseract 未インストール → `SdsError::ImageOnlyPdf` へ
- Vision OCR が呼ばれず → 最終エラー

**修正**: `extractor.rs` の `pdftotext_fallback()` から `-utf8` フラグを削除。現代の pdftotext はデフォルトで UTF-8 出力。

**効果**: Shift-JIS フォント PDF が pdftotext 経由で正常にテキスト抽出可能になった。

---

### 問題2: ref_02 medium quality でJSONパースエラー（既知の制限）

**症状**: `Error: LLM response parse error: Invalid JSON: expected `,` or `}` at line 169 column 97`

**根本原因**:  
PDF テキストが長大なため、LLM レスポンスが max_tokens=16384（medium）で途中カット。`repair_json` はトレーリングカンマ・未閉括弧を修復できるが、文字列途中の切断は修復不可。

**回避策**: `--quality high`（max_tokens=32768）で再実行 → 17/17 セクション正常抽出。

> **推奨**: 長尺 SDS（15〜20ページ超）は `--quality high` を使用。

---

## Step1 schema mismatch について

| ファイル | Step1 mismatch | Step3 mismatch | 評価 |
|---------|---------------|---------------|------|
| ref_01  | 3件 | 0件 | PDF→JSON変換時のLLM出力揺れ。DOCX→JSON再変換では解消 |
| ref_05  | 1件 | 0件 | 同上 |
| ref_06  | 3件 | 0件 | 同上 |

Step1 の mismatch はすべて DOCX→JSON（Step3）では 0件に解消。  
DOCX は sds-converter が生成するためスキーマ準拠のテキストが出力され、LLMが正確に構造化できるため。

---

## 今回の修正コミット

| コミット | 内容 |
|---------|------|
| `829fce5` | `CASno.FullText` flex deserialization + `Colour`/`Odour` オブジェクト→文字列正規化 |
| `f60561b` | `pdftotext -utf8` フラグ削除（poppler v24+ 対応） |

---

## 総評

- **ラウンドトリップ成功率**: 9/10 件（90%）
- **Step3 スキーマ不一致**: 全成功ファイルで **0件**
- **セクション保持率**: 16〜17/17（100% または StabilityReactivity などが PDF に記載なし）
- **残存課題**: 長尺 PDF の medium quality トークン不足 → quality=high で解決可能
