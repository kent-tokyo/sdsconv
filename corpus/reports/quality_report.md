# SDS変換品質レポート

**評価日**: 2026-05-25  
**対象**: `samples/` ディレクトリ内のPDF→JSON変換結果（input01〜input12）  
**変換ツール**: sds-converter (Quality: medium, model: claude-haiku-4-5-20251001, max_chars=30000, max_tokens=16384)

---

## 1. 各ファイルの評価表

| ファイル | スコア | 言語 | ログ状況 | 特記事項 |
|---|---|---|---|---|
| input01 → output01 | ★☆☆☆☆ 失敗 | 日本語（英語と誤検出） | log_01.txt | 暗号化PDF + Vision OCR → JSONパースエラーで変換失敗。output01.json 生成なし |
| input02 → output02 | ★★★☆☆ 普通 | 日本語（英語と誤検出） | log_02.txt | pdf-extractパニック(90ms-RKSJ-H未サポートエンコード) → Vision OCR経由で変換成功。主要フィールド取得済みだがHazardStatement詳細コードなし、ToxicologicalInformationのExposureRouteが空、Compositionの成分濃度情報が簡略 |
| input03 → output03 | ★★★★☆ 良好 | 英語 | なし | PDF直接抽出。英語SDSとして全セクションほぼ抽出。ToxicologicalInformation.ExposureRouteが空オブジェクト、Composition.Concentrationに数値なし（一部成分）、EcologicalInformationの毒性詳細値なし |
| input04 → output04 | ★★★★★ 優秀 | 中国語 | なし | 多言語PDF（中国語）を正確に変換。HazardStatement 7件（H242/H313/H316/H350/H370/H372/H373）+Precautionary 9件をコード付きで抽出。Composition 3成分（CAS番号含む）、物性値（引火点86.7℃、LogPow等）、運輸情報（UN番号3226）まで充実 |
| input05 → output05 | ★★★★☆ 良好 | 日本語 | なし | 日本語SDS。全16セクション中ほぼ抽出済み。HazardStatementにコードなし（テキストのみ）、ToxicologicalInformation.ExposureRouteが空オブジェクト（LD50等の数値なし）、RegulatoryInformationは法令名のみ（詳細なし）。物性値はBoilingPoint/FlashPoint/MeltingPoint/VapourPressure等を取得 |
| input06 → output06 | ★★★☆☆ 普通 | 日本語 | log_06.txt | **FireFightingMeasuresセクションがスキップ**（スキーマ不一致：FullTextが配列で来るが文字列を期待、2回リトライでも解決せず）。Section 5が欠落。HazardIdentificationにHazardLabellingなし（GHS非危険物として単純記述）。物性値は多数取得 |
| input07 → output07 | ★★★☆☆ 普通 | 日本語 | log_07.txt | **StabilityReactivityセクションがスキップ**（スキーマ不一致：HazardousDecompositionProducts.Substanceが配列で来るが文字列を期待、2回リトライでも解決せず）。Section 10欠落。ExposureControlPersonalProtectionは1回目スキップ後リトライで成功。他セクションは概ね抽出済み |
| input08 → output08 | ★★★★☆ 良好 | 日本語 | なし | 大容量PDF（2.4MB）から良好に抽出。物性値項目が最も充実（20項目超）、HazardStatement + Precautionary付き、RegulatoryInformation10法令、Transport国際規制3種。ToxicologicalInformation.ExposureRouteは空、Composition.Concentrationに値なし |
| input09 → output09 | ★★★★☆ 良好 | 日本語（英語と誤検出） | log_09.txt | pdf-extractパニック(FromUtf8Error) → Vision OCR経由で変換。HazardStatement 7件（コード付き）、物性値4種（BoilingPoint/FlashPoint/MeltingPoint/Density）、SupplierInfo完備（Email/Fax含む）。ToxicologicalInformationは1件空 |
| input10 → output10 | ★★★★☆ 良好 | 英語 | なし | 英語SDS（中国企業製）。全セクション抽出。ToxicologicalInformation.ExposureRouteに「Not available」等の付加情報あり（他ファイルでは空オブジェクトのみ）。Colour未抽出（BasePhysicalChemicalPropertiesにColourキーなし）。物性値やや少ない |
| input11 → output11 | ★★★☆☆ 普通 | 日本語 | log_11.txt | **StabilityReactivityセクションがスキップ**（スキーマ不一致：Substanceが配列で来るが文字列を期待、2回リトライでも解決せず）。Section 10欠落。HazardStatement 7件+Precautionary完備、CompanyNameがプレースホルダ「○○○○株式会社」のままで実際の製造元情報が欠損。MolecularFormula/Weightの追加フィールドあり |
| input12 → output12 | ★★★☆☆ 普通 | 日本語（英語と誤検出） | log_12.txt | pdf-extractパニック(FromUtf8Error) → Vision OCR経由で変換。**PhysicalChemicalPropertiesスキップ後リトライ成功**（Vision retry: 1 skipped sections）。ただしPhysicalChemicalPropertiesは物性値が水溶解度のみ（融点・沸点・密度・引火点なし）と情報量が少ない。HazardLabellingにHazardStatement本文なし（SignalWord「なし」のみ） |

---

## 2. 全体的な傾向・パターン

### 2.1 変換成功率

- 12ファイル中、**11ファイルが変換成功**（output JSON生成）
- **1ファイル失敗**（input01：暗号化PDF + Vision OCRのJSONパースエラー）
- 変換成功率 **91.7%**

### 2.2 言語検出の誤り（memo.txtで報告済み）

input01・input02・input09・input12は日本語文書だが英語と誤検出された。
これらはいずれもpdf-extractが日本語エンコーディングを処理できずパニック（`90ms-RKSJ-H`未サポートや`FromUtf8Error`）が発生しており、フォールバックとして Vision OCR が使用されている。Vision OCR のテキスト認識では言語検出が正しく機能していない可能性がある。

### 2.3 pdf-extractのパニック（処理上の問題）

以下のファイルでpdf-extractライブラリがパニック終了している：
- **input02**: `unsupported encoding 90ms-RKSJ-H`（Shift-JIS系エンコーディング）
- **input09**: `FromUtf8Error`（非UTF-8バイト列）
- **input12**: `FromUtf8Error`（非UTF-8バイト列）

これらはいずれも日本語PDFで発生しており、Vision OCR へのフォールバック機構が正常に働いているが、ターミナル出力にパニックメッセージが残るため、ユーザーにとってエラーと混同されやすい。

### 2.4 スキーマ不一致によるセクションスキップ（構造的な問題）

以下の繰り返しパターンが3件確認された：

| セクション | 影響ファイル | 不一致の原因 |
|---|---|---|
| `FireFightingMeasures` | input06 | `FullText` フィールドが文字列期待なのに配列（`["..."]`）で来る |
| `StabilityReactivity` | input07, input11 | `HazardousDecompositionProducts.Substance` が文字列期待なのに配列で来る |
| `ExposureControlPersonalProtection` | input07（1回目のみ） | `OccupationalExposureLimits` が文字列期待なのにマップで来る（リトライで成功） |
| `PhysicalChemicalProperties` | input12（1回目のみ） | マップ期待なのに文字列型 → Vision retry で成功 |

リトライで成功するケースとしないケースがある。`StabilityReactivity` と `FireFightingMeasures` はリトライ2回でも解決しない（同じ型エラーが再現）。

---

## 3. 問題が多いセクション

### 3.1 StabilityReactivity（最も問題が多い）

- **3ファイル（input07, input11, input12）でスキップ**（ただしinput12はリトライ成功）
- `HazardousDecompositionProducts.Substance` フィールドのスキーマが、LLMが返す配列型（`["CO2", "NH3"]`）とデコーダが期待する文字列型（`"CO2, NH3"`）で一致しない
- このスキーマ定義が実際のSDS記述パターン（複数物質の列挙）と合っていない可能性がある

### 3.2 FireFightingMeasures

- **1ファイル（input06）でスキップ**
- `FullText` が配列で返されるケースでスキーマ不一致

### 3.3 ToxicologicalInformation.ExposureRoute（情報量の問題）

- 全11ファイル中、**ほぼ全件でExposureRouteが空オブジェクト**（`{}`）または数値なし
- LD50・LC50等の急性毒性数値は抽出できていない（フィールドが空）
- output10のみ `"Not available"` の文字列が入っており、他ファイルより情報量が多い

### 3.4 EcologicalInformation（情報量の問題）

- 全件で`AquaticAcuteToxicity.Result`・`AquaticChronicToxicity.Result`が空オブジェクト（`{}`）
- EC50・LC50等の生態毒性数値が抽出されていない

### 3.5 RegulatoryInformation（情報量の問題）

- 全件で`Regulations`が空オブジェクトの配列（`[{}]`）
- 各法規への該当・非該当の詳細情報が抽出されていない
- 法令名（LegislationName）のみで内容なし

### 3.6 Composition.Concentration

- 複数ファイルで濃度のNumericRangeWithUnitAndQualifierにExactValueがない（Unitのみ）
- input03のCASno `1317-61-9`の成分など、濃度値が元のSDSにない場合は記入できないが、一部は元文書に記載があっても抽出できていない可能性がある

---

## 4. 改善提案

### 4.1 スキーマの柔軟化（最優先）

**`StabilityReactivity.HazardousDecompositionProducts.Substance`** および **`FireFightingMeasures.FullText`** の型定義を、文字列のみでなく配列も受け付けるよう拡張すること（`String | Vec<String>` のようなUnion型対応）。これにより3件のセクション欠落が解消される。

具体的には、sds-converter-coreのデシリアライゼーション処理で `serde(untagged)` またはカスタムデシリアライザを用いて、配列が来た場合は改行・読点で結合した文字列として扱う方式が有効。

### 4.2 言語検出の改善

pdf-extractがパニックして Vision OCR フォールバックになる場合、テキスト抽出の結果から言語を推定せずに `--lang` 引数のデフォルトを日本語にするか、Vision OCR経由の場合はClaudeのレスポンスから言語を判定する仕組みを追加すること。

### 4.3 pdf-extractのパニックをcatchして静かに処理

`unwrap()` によるパニックがターミナルに出力されユーザーを混乱させる。`catch_unwind` によるパニックキャッチ、またはより適切なエラーハンドリング（`?`伝播）に置き換えること。

### 4.4 ToxicologicalInformationおよびEcologicalInformationの抽出精度向上

現状、毒性値・生態毒性値の数値データ（LD50, EC50等）がほぼ全件で空になっている。プロンプト設計の見直し（数値フィールドの抽出を明示的に指示する）が必要。これらのフィールドが空でも「データなし」を明示的にセットするか、元文書に記載がある場合には抽出できるよう改善する。

### 4.5 RegulatoryInformationの詳細抽出

法令名のみで内容が空のケースが多い。各法令の「該当/非該当」「指定品目名」等の詳細をLLMに抽出させるプロンプトの強化が必要。

### 4.6 暗号化PDFへの対応（input01）

暗号化PDFは現在Vision OCRにもフォールバックするが、output01はJSONパースエラーで失敗した。Vision OCRのレスポンスが不完全なJSONを返した場合のリカバリー処理（部分的なJSONの補完や、トークン上限による切断への対策）が必要。

---

## 5. まとめ

| 評価 | 件数 | ファイル |
|---|---|---|
| ★★★★★ 優秀 | 1件 | output04（中国語SDS） |
| ★★★★☆ 良好 | 4件 | output03, output05, output08, output09, output10 |
| ★★★☆☆ 普通 | 4件 | output02, output06, output07, output11, output12 |
| ★★☆☆☆ 不良 | 0件 | — |
| ★☆☆☆☆ 失敗 | 1件 | output01（生成なし） |

> ※良好が5件あるため合計が12件でない（再集計: ★★★★★ 1件、★★★★☆ 5件、★★★☆☆ 5件、★☆☆☆☆ 1件）

全体的に、主要フィールド（ProductName, IssueDate, HazardStatement, Composition, PhysicalChemicalProperties, FirstAidMeasures等）の抽出は良好に機能している。最大の課題はスキーマ不一致によるセクション欠落（FireFightingMeasures, StabilityReactivity）と、毒性・生態毒性・法規詳細の数値情報が抽出されていない点である。日本語エンコーディング問題は既知バグとして認識されており（memo.txt参照）、優先的な対処が必要。
