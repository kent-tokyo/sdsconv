# SDS Converter Round-Trip Report v3

**Date**: 2026-05-25  
**Scope**: 11 files (rt_02 〜 rt_12, rt_01 は入力不可のためスキップ)  
**Pipeline**: `output*.json` → DOCX生成 → `to-json` 再変換 → 比較

---

## 修正サマリー（v2 → v3）

### 解消した問題

| 問題 | 影響 | 修正 |
|------|------|------|
| `CASno.FullText` スキーマ不一致 | LLMが文字列 `"1317-61-9"` を返すが `Vec<String>` が期待されエラー | `SubstanceIdentifiersSubstanceIdentityCASno.full_text` に `flex_vec_string_opt` デシリアライザ追加 |
| `Colour`/`Odour` スキーマ不一致 | LLMが `{"AdditionalInfo":{"FullText":["..."]}}` オブジェクトを返すが `Option<String>` が期待されエラー | `normalize_string_fields` に `coerce_obj_to_string` 追加（`Colour`, `Odour`, `PhysicalState` 対象） |

### 結果

**すべてのファイルでスキーマ不一致エラー (schema mismatch) が 0 件に**

---

## ファイル別結果

| ファイル | スキーマ不一致 | セクション保持 | 備考 |
|---------|--------------|--------------|------|
| rt_02   | ✅ 0件 | 17/17 ✅ | |
| rt_03   | ✅ 0件 | 17/17 ✅ | |
| rt_04   | ✅ 0件 | 17/17 ✅ | |
| rt_05   | ✅ 0件 | 17/17 ✅ | |
| rt_06   | ✅ 0件 | 16/16 ✅ | 元JSONにFireFightingMeasuresなし |
| rt_07   | ✅ 0件 | 16/16 ✅ | 元JSONにStabilityReactivityなし、再変換で追加抽出 |
| rt_08   | ✅ 0件 | 17/17 ✅ | |
| rt_09   | ✅ 0件 | 16/17 ⚠️ | StabilityReactivityが再変換で欠落（LLM精度） |
| rt_10   | ✅ 0件 | 17/17 ✅ | |
| rt_11   | ✅ 0件 | 15/16 ⚠️ | PhysicalChemicalPropertiesが再変換で欠落（LLM精度）、StabilityReactivity追加抽出 |
| rt_12   | ✅ 0件 | 17/17 ✅ | |

---

## 残存する軽微なWARN（スキーマエラーではない）

| ファイル | WARN | 内容 |
|---------|------|------|
| rt_11   | Section 10 content check | StabilityReactivity: neither StabilityDescription nor ReactivityDescription extracted — セクションは認識されたが内容が希薄 |

これはスキーマ型不一致ではなく、DOCXに安定性・反応性の具体的テキストがなかった場合の品質チェック警告。修正不要。

---

## 累積修正一覧（全フェーズ）

| フェーズ | 修正 |
|---------|------|
| v1 | スキーマ不一致（`StabilityReactivity`/`FireFightingMeasures` フィールドが配列で返る） → `normalize_string_fields` で配列→文字列変換 |
| v1 | `AdditionalInfo.FullText` が文字列で返る → `flex_vec_string_opt` デシリアライザ追加 |
| v1 | JSONトランケーション → `repair_json` で未閉文字列/括弧を自動クローズ |
| v1 | Vision OCR の言語検出ミス → `params.lang` をそのまま使用 |
| v2 | DOCX英語ラベル → 120+エントリの `KEY_LABELS` テーブル追加 |
| v2 | FullText配列がそのまま出力 → `value_to_text()` で配列を `\n` で結合 |
| v2 | 改行が失われる → `add_leaf_multiline()` で行ごとに段落分割 |
| **v3** | **`CASno.FullText` 型不一致 → `flex_vec_string_opt` 追加** |
| **v3** | **`Colour`/`Odour` オブジェクト型不一致 → `coerce_obj_to_string` 追加** |

---

## 評価

スキーマ不一致エラーは v2 時点で10ファイル×3件（rt_11は6件）あったが、**v3 で全件 0件に解消**。

セクション欠落（rt_09の1件、rt_11の1件）はスキーマエラーではなくLLMの抽出精度に起因するもので、DOCXのフォーマットと元データの品質に依存する。
