# SDS JSON 品質チェック（QC）スクリプト 詳細マニュアル

> このページでは、sds-converter が生成した SDS JSON の品質を自動検証する QC スクリプトの仕組みと各チェック項目を詳しく解説します。

[English](quality-check.md) | [中文](quality-check_zh.md)

---

## 目次

1. [概要](#概要)
2. [重要度レベル](#重要度レベル)
3. [終了コードと判定ルール](#終了コードと判定ルール)
4. [セクション別チェック一覧](#セクション別チェック一覧)
   - [Section 1: 化学品の名称等（Identification）](#section-1-化学品の名称等)
   - [Section 2: 危険有害性の要約（HazardIdentification）](#section-2-危険有害性の要約)
   - [Section 3: 組成・成分情報（Composition）](#section-3-組成成分情報)
   - [Section 4: 応急措置（FirstAidMeasures）](#section-4-応急措置)
   - [Section 5: 火災時の措置（FireFightingMeasures）](#section-5-火災時の措置)
   - [Section 6: 漏出時の措置（AccidentalReleaseMeasures）](#section-6-漏出時の措置)
   - [Section 7: 取扱いおよび保管上の注意（HandlingAndStorage）](#section-7-取扱いおよび保管上の注意)
   - [Section 8: ばく露防止・保護措置（ExposureControl）](#section-8-ばく露防止保護措置)
   - [Section 9: 物理的および化学的性質（PhysicalChemicalProperties）](#section-9-物理的および化学的性質)
   - [Section 10: 安定性および反応性（StabilityReactivity）](#section-10-安定性および反応性)
   - [Section 11: 有害性情報（ToxicologicalInformation）](#section-11-有害性情報)
   - [Section 12: 環境影響情報（EcologicalInformation）](#section-12-環境影響情報)
   - [Section 13: 廃棄上の注意（DisposalConsiderations）](#section-13-廃棄上の注意)
   - [Section 14: 輸送上の注意（TransportInformation）](#section-14-輸送上の注意)
   - [Section 15: 適用法令（RegulatoryInformation）](#section-15-適用法令)
   - [Section 16: その他の情報（OtherInformation）](#section-16-その他の情報)
5. [クロスフィールドチェック](#クロスフィールドチェック)
6. [実行方法](#実行方法)
7. [出力例](#出力例)
8. [チェックの改訂履歴](#チェックの改訂履歴)

---

## 概要

QC スクリプトは、LLM が生成した SDS JSON が JIS Z 7253 / GHS に準拠した内容を含んでいるかを自動的に検証します。検証は **ルールベース** で行い、以下を確認します：

- 必須フィールドの存在
- H コード・P コードの書式と整合性
- 物理化学的性質の数値範囲（沸点 > 引火点 など）
- 言語間の整合性（日本語 SDS に中国語の信号語が入っていないか など）
- CAS 番号のチェックデジット検証
- 濃度合計の物理的妥当性（100% を大きく超えないか）

> **注意**: QC はルールベースです。LLM の判断や抽出精度そのものを測定するものではなく、**出力 JSON の整合性と完全性** を検証します。

---

## 重要度レベル

| レベル | 記号 | 意味 | 例 |
|---|---|---|---|
| **CRIT** | 🔴 重大 | JIS Z 7253 違反・ハルシネーション確定・必須セクション欠落 | Section 3（成分情報）が空、日本語以外の SDS に片仮名の物質名 |
| **HIGH** | 🟠 高 | 重大な抽出漏れ・フォーマット違反 | 会社名空、P コードなし（信号語あり）、有害性情報セクション空 |
| **MED** | 🟡 中 | 抽出品質ギャップ・推奨フィールド漏れ | 密度未抽出、引火性あるが引火点なし、P コード数不足 |

---

## 終了コードと判定ルール

```
exit code = 検出した問題の総数（CRIT + HIGH + MED の合計）
```

| exit code | 判定 | 意味 |
|---|---|---|
| `0` | **OK** | 問題なし。全チェック通過 |
| `1` | **WARN** | MED 問題が 1 件のみ（CRIT・HIGH はゼロ） |
| `2+` | **FAIL** | CRIT/HIGH が 1 件以上、または MED が 2 件以上 |

この規約により、`WARN` は「軽微な抽出漏れあり・実用上は問題ない」、`FAIL` は「重大な欠落あり」と区別できます。

---

## セクション別チェック一覧

### Section 1: 化学品の名称等

**JSONフィールド**: `Identification`

| チェック | レベル | 内容 |
|---|---|---|
| 製品名（TradeNameJP / TradeNameEN）の存在 | CRIT | 日本語 SDS で両方空は不正。その他言語でも両方空は失格 |
| 非日本語 SDS に片仮名の製品名 | CRIT | ハルシネーションを検出（例: 英語 SDS に `ベンゼン`） |
| 会社名（CompanyName）の存在 | HIGH | SupplierInformation.CompanyName が空 |
| 非日本語 SDS に会社名が片仮名 | HIGH | 言語の混在を検出 |
| 用途（Use フィールド）の存在 | MED | UseAndUseAdvisedAgainst.Use が空 |
| 緊急連絡先の電話番号桁数 | MED | EmergencyContact エントリに数字が含まれないもの |
| **r23** 供給者電話番号が 7 桁以上 | MED | SupplierInformation.Phone がない、または数字が 7 桁未満 |

**ポイント**: 前 GHS 時代の中国 MSDS は CompanyName が原文に存在しないことが多く、HIGH が出ても「ソース限界」として扱います。

---

### Section 2: 危険有害性の要約

**JSONフィールド**: `HazardIdentification`

#### 信号語（SignalWord）

| チェック | レベル | 内容 |
|---|---|---|
| 信号語の値が有効セットに含まれるか | MED | `危険`/`警告`/`Danger`/`Warning`/`N/A` 等以外の値 |
| 中国語 SDS に片仮名の信号語 | HIGH | 言語混在 |
| 英語 SDS に日本語/中国語の信号語 | MED | ローカライズ漏れ |
| H224（極度引火性）で信号語が `危険` 以外 | HIGH | GHS Cat1 は必ず Danger |
| H226（引火性 Cat3）のみで `危険` | MED | Cat3 は通常 Warning — ただし他の Cat1/2 H コードがなければ |

#### H コード

| チェック | レベル | 内容 |
|---|---|---|
| H コードの書式（`H` + 3 桁 + 任意英字） | HIGH | 書式違反 |
| H コードが存在するのに全て空 | CRIT | HazardStatement リストに項目はあるがコードが空 |
| 信号語ありだが H コードなし | HIGH | 不整合 |
| H コード数 > 12 | MED | 単一物質で 12 超は重複抽出の疑い |
| Danger 信号語だが Cat1/2 H コードがない | MED | 重大度と信号語の不整合 |

**Cat1/2 H コードの判定対象**:
`H200`, `H201`, `H202`, `H220`, `H221`, `H222`, `H224`, `H260`, `H261`, `H300`, `H301`, `H310`, `H311`, `H330`, `H331`, `H314`, `H340`, `H350`, `H360`, `H370`, `H400`, `H410`, `H420` 他

#### P コード

| チェック | レベル | 内容 |
|---|---|---|
| P コード書式（`P` + 3 桁） | MED | 書式違反 |
| 信号語 + H コードありで P コードが 0 | HIGH | ラベリング情報が欠落 |
| Danger 製品で P コード < 4 | MED | GHS Danger には通常 4 つ以上の P コードが必要 |
| Warning 製品で P コード < 3 | MED | Warning でも 3 つ以上推奨 |

#### H コード × P コード 整合性

| H コード | 期待 P コード | チェック内容 |
|---|---|---|
| H224/H225/H226 | P210 | 火気を避ける |
| H300/H301/H302 | P301 or P330 | 飲み込んだ場合の応急処置 |
| H330/H331/H332 | P304 or P261 | 吸入した場合の応急処置 |
| H318/H319 | P305 | 目に入った場合の洗眼処置 |
| H314 | P280, P301, P305 | 皮膚腐食：保護具・応急処置一式 |

#### GHS ピクトグラム・分類

| チェック | レベル | 内容 |
|---|---|---|
| ピクトグラムが有効セット外 | MED | GHS01〜GHS09 または日本語表記以外 |
| H コードありだが Classification セクションが空 | MED | 分類情報の欠落 |
| **r23** H200–H205（爆発物）ありで GHS01 ピクトグラムなし | MED | 爆発物は爆弾ピクトグラム（GHS01）が必須 |
| **r23** H410/H411/H412/H413（環境有害性）ありで GHS09 ピクトグラムなし | MED | 環境有害性は枯れ木・死魚ピクトグラム（GHS09）が必須 |
| **r23** 信号語ありで HazardStatement が完全に空 | HIGH | 信号語だけで危険有害性情報がゼロはラベリング不備 |

---

### Section 3: 組成・成分情報

**JSONフィールド**: `Composition`

| チェック | レベル | 内容 |
|---|---|---|
| CompositionAndConcentration が空 | CRIT | 成分情報がゼロ |
| 混合物指示なのに成分 1 件以下 | MED | CompositionType が mixture 系なのに成分が 1 つだけ |
| CAS 番号の書式（`9999999-99-9` 形式） | HIGH | 書式違反 |
| CAS チェックデジット検証 | HIGH | チェックデジット計算が不一致 |
| 多成分製品で成分の CAS なし | MED | 混合物の各成分に CAS がない |
| 非日本語 SDS に片仮名の物質名 | CRIT | ハルシネーション |
| 分子量が 0 以下または 200,000 超 | HIGH | 物理的に非現実的な値 |
| 濃度フィールドに日付文字列 | HIGH | 抽出エラー（例: `2024-01-01` が濃度として入力） |
| CAS の重複 | MED | 同一 CAS が複数成分に登場 |
| 濃度数値合計 > 102% | MED | 二重カウントまたは抽出エラーの可能性 |
| 単一成分で物質名なし | MED | substance name が全て空 |
| 単一成分で濃度・純度なし | MED | Concentration フィールドが空 |
| **r23** 混合物で成分 > 10 件 | MED | 過剰抽出または CompositionType 不一致の疑い |
| **r23** 濃度フィールドに年号文字列 | HIGH | `"2024"` や `"2024-01-01"` が濃度として格納されている抽出エラー |

**CAS チェックデジット計算例**:
```
CAS: 107-06-2 → "10706" の各桁を右から 1,2,3,4,5 倍して合算 → mod 10 = 2 ✓
```

---

### Section 4: 応急措置

**JSONフィールド**: `FirstAidMeasures`

| チェック | レベル | 内容 |
|---|---|---|
| ExposureRoute に経路情報なし | HIGH | 全経路のテキストが空 |
| 有害製品で応急措置経路 < 2 | MED | 通常は吸入・皮膚・眼・飲込の複数経路が必要 |
| 医師/受診への言及なし | MED | doctor/physician/医師/就医 等のキーワード検索 |
| 眼刺激 H コードで眼への言及なし | MED | H318/H319/H314 → eye/眼/rinse 等 |
| 吸入 H コードで吸入経路テキストなし | MED | H330-H335 → inhal/吸入/fresh air 等 |
| 皮膚 H コードで皮膚接触テキストなし | MED | H314/H315 → skin/皮膚/wash 等 |

---

### Section 5: 火災時の措置

**JSONフィールド**: `FireFightingMeasures`

| チェック | レベル | 内容 |
|---|---|---|
| セクション全体が空 | HIGH | JSON フィールド長 < 30 文字 |
| 消火剤の記載なし | MED | foam/water/CO2/粉末/泡沫/干粉 等のキーワードなし |

**消火剤キーワード（一部）**: foam, water, CO2, carbon dioxide, powder, sand, dry chemical, 泡, 二酸化炭素, 粉末, 砂, 水雾, dry sand, halon, nitrogen, extinguish, surrounding

---

### Section 6: 漏出時の措置

**JSONフィールド**: `AccidentalReleaseMeasures`

| チェック | レベル | 内容 |
|---|---|---|
| セクションが空 | MED | JSON フィールド長 < 30 文字 |
| 具体的な回収・封じ込め記述なし | MED | absorb/collect/sweep/吸収/回収/吸附/収集 等のキーワードなし |

**キーワード（一部）**: absorb, contain, collect, sweep, dike, sand, berm, ventilat, 吸収, 回収, 収集, 砂, 換気, 漏洩, 吸附, 围堤, 通风, scoop, mop, neutraliz

---

### Section 7: 取扱いおよび保管上の注意

**JSONフィールド**: `HandlingAndStorage`

| チェック | レベル | 内容 |
|---|---|---|
| ハンドリング・保管情報が完全に空 | HIGH | |
| 引火性 H コードで熱・点火源への言及なし | MED | H224/H225/H226 → cool/heat/ignition/火気/冷所 等 |
| 水反応性 H コードで乾燥条件への言及なし | MED | H260/H261/H250 → dry/moisture/乾燥 等 |
| 揮発性・有毒 H コードで換気への言及なし | MED | H330-H335, H224-H226 → ventilat/換気/局排/通风 等 |

---

### Section 8: ばく露防止・保護措置

**JSONフィールド**: `ExposureControlPersonalProtection`

| チェック | レベル | 内容 |
|---|---|---|
| セクション全体が空 | HIGH | EngineeringControls/PPE/OEL が全て空 |
| PPE サブフィールドが 2/4 未満 | MED | 有害製品で呼吸・手・目・皮膚のうち 2 つ以上必要 |
| 単一物質の有害製品で OEL なし | MED | 作業環境管理値の抽出漏れ |
| 腐食性（H314）で目・顔面保護が不十分 | MED | face shield/goggles/フェイス/ゴーグル 等のキーワードなし |
| 皮膚・腐食性 H コードで手袋材質の記述なし | MED | nitrile/butyl/neoprene/rubber/ニトリル/ブチル/丁腈 等なし |
| 吸入 H コードで呼吸保護具の種別なし | MED | P2/P3/ABEK/FFP/half mask/full face/防毒/防塵 等なし |
| **r23** OEL フィールドに数値がない | MED | OEL テキストに数字なし（例: ppm/mg/m³ なし）— プレースホルダの疑い |

**手袋材質キーワード**: nitrile, butyl, neoprene, rubber, latex, viton, PVC, polyethylene, ニトリル, ブチル, ネオプレン, ゴム, 丁腈, 丁基, 氯丁, 橡胶

**呼吸保護具種別キーワード**: P1, P2, P3, A1, ABEK, FFP, half mask, full face, SCBA, P100, organic vapor, 防毒, 防じん, 送気, 有机蒸气, 防尘

---

### Section 9: 物理的および化学的性質

**JSONフィールド**: `PhysicalChemicalProperties`

#### 基本物性

| チェック | レベル | 内容 |
|---|---|---|
| 色/外観・物理状態が両方空 | HIGH | |
| 有害製品で臭気（Odour）なし | MED | |
| 密度・比重未抽出 | MED | Densities / Density / RelativeDensity / SpecificGravity いずれもなし |
| 水溶解度未抽出 | MED | SolubilityInWater / Solubility いずれもなし |

#### 引火点（FlashPoint）

| チェック | レベル | 内容 |
|---|---|---|
| 引火点の値が数値でない | HIGH | 文字列が格納されている |
| 引火点が −220〜400°C の範囲外 | MED | 物理的に非現実的な値 |
| 引火性 H コード（H224/225/226）で引火点なし | MED | |
| H224 で引火点 ≥ 23°C | MED | GHS: 極度引火性は FP < 23°C |
| H226 のみで引火点が 23〜60°C 範囲外 | MED | GHS: Cat3 引火性は 23°C ≤ FP < 60°C |

#### 沸点・融点

| チェック | レベル | 内容 |
|---|---|---|
| 引火点 ≥ 沸点 | MED | 物理的に不可能（引火点は沸点より低い） |
| 液体なのに沸点なし（圧縮ガス除く） | MED | |
| 固体・結晶なのに融点なし | MED | |

#### 自然発火温度・蒸気圧・pH

| チェック | レベル | 内容 |
|---|---|---|
| 引火性 H コードで自然発火温度なし | MED | AutoIgnitionTemperature 未抽出 |
| 揮発性・引火性 H コードで蒸気圧なし | MED | H224/225/226/330/331/332 |
| 腐食性・酸性 H コードで pH なし | MED | H314/H290/H318/H319 |
| **r23** 密度が 0.1〜25 g/cm³ 範囲外 | MED | 物理的に非現実的な密度値 |
| **r23** pH が 0〜14 範囲外 | MED | 有効範囲外 — 抽出エラーまたは単位誤り |
| **r23** 自然発火温度が引火点より低い | MED | 熱力学的に不可能（自然発火温度 > 引火点） |
| **r23** 沸点が −200〜3000°C 範囲外 | MED | 物理的に非現実的な値 |

---

### Section 10: 安定性および反応性

**JSONフィールド**: `StabilityReactivity`

| チェック | レベル | 内容 |
|---|---|---|
| セクションが空 | MED | JSON 長 < 30 文字 |
| 避けるべき条件や禁忌物質の記述なし | MED | avoid/heat/incompatible/acid/酸化/禁止 等なし |
| 引火性・爆発性 H コードで分解生成物なし | MED | HazardousDecompositionProducts が空 |

---

### Section 11: 有害性情報

**JSONフィールド**: `ToxicologicalInformation`

| チェック | レベル | 内容 |
|---|---|---|
| セクション全体が空 | HIGH | |
| 急性毒性 H コードで AcuteToxicity なし | MED | H300/H301/H302/H310/H311/H312/H330/H331/H332 |
| 急性毒性 H コードで LD50/LC50 数値なし | MED | 急性毒性値のテキスト記載が必要 |
| H315（皮膚刺激）で SkinCorrosionIrritation なし | MED | |
| H319/H318（眼障害）で EyeDamageOrIrritation なし | MED | |
| H334（呼吸器感作）で Sensitization なし | MED | |
| H350/H351（発がん性）で Carcinogenicity なし | MED | |
| H360/H361（生殖毒性）で ReproductiveToxicity なし | MED | |
| H370-H373（STOT）で SpecificTargetOrgan なし | MED | |
| AcuteToxicity 区分 1/2 だが致死 H コードなし | MED | 有害性分類と H コードの不整合 |
| **r23** H350/H351（発がん性）ありで発がん性評価機関の記述なし | MED | IARC/NTP/ACGIH/WHO 等の分類機関への言及が必要 |

---

### Section 12: 環境影響情報

**JSONフィールド**: `EcologicalInformation`

| チェック | レベル | 内容 |
|---|---|---|
| 環境 H コード（H4xx）ありで EcologicalInformation 空 | HIGH | |
| 環境 H コードありで水生毒性キーワードなし | MED | aquatic/fish/daphnia/algae/LC50/EC50/水生 等 |
| H410/H411 ありで生分解性・生体蓄積キーワードなし | MED | biodeg/bioaccum/BCF/PersistenceDeg 等 |
| 環境 H コードで LogP/Kow/BCF 値なし | MED | partition coefficient / 分配係数 / 辛醇 等 |
| 有害製品で EcologicalInformation が空 | MED | 環境 H コードがなくても有害なら記載推奨 |
| **r23** H420（オゾン層破壊物質）ありでオゾン関連キーワードなし | MED | オゾン破壊係数（ODP）やオゾン層に関する記述が必要 |

---

### Section 13: 廃棄上の注意

**JSONフィールド**: `DisposalConsiderations`

| チェック | レベル | 内容 |
|---|---|---|
| セクションが空 | MED | |
| 廃棄方法・規制の記述なし | MED | inciner/landfill/waste/規制/廃棄/焼却 等のキーワードなし |

---

### Section 14: 輸送上の注意

**JSONフィールド**: `TransportInformation`

| チェック | レベル | 内容 |
|---|---|---|
| セクションが空 | MED | |
| 危険物 H コードありで UN 番号なし | MED | ただし「規制対象外」テキストが明示されている場合は除外 |
| UN 番号ありで容器等級（Packing Group）なし | MED | |
| UN 番号ありで正式品名（Proper Shipping Name）なし | MED | |
| **r23** UN 番号が `UN\d{4}` 形式でない | MED | UN 番号は 4 桁（UN0001〜UN9999）でなければならない |

**危険物判定 H コード**: H224, H225, H226, H300, H301, H302, H310, H311, H314, H330, H331, H332, H270, H271, H272

**「規制対象外」と認識するテキスト**: `not regulated`, `非危険物`, `not dangerous`, `無資料`, `規制されていない`, `規制対象外`, `危険物に該当しない`, `not subject`, `no regulation`, `非危险` 等

---

### Section 15: 適用法令

**JSONフィールド**: `RegulatoryInformation`

| チェック | レベル | 内容 |
|---|---|---|
| セクションが空 | MED | |
| 法令名・規制名のキーワードなし | MED | law/regulation/安全衛生/化審法/GB/REACH/OSHA 等 |
| 日本語 SDS で日本法令なし | MED | 労働安全衛生法/安衛法/化審法/毒劇法/消防法/化管法/PRTR |
| 中国語（zh-cn）SDS で GB 規格なし | MED | GB /GBZ/GB/T/GB13690/GB30000 等 |
| 日本語 SDS で発がん性・環境 H コードがあり化管法/PRTR なし | MED | H350/H351/H340/H341/H400/H410 |

---

### Section 16: その他の情報

**JSONフィールド**: `OtherInformation` / `Datasheet`

| チェック | レベル | 内容 |
|---|---|---|
| SDS 日付（IssueDate/RevisionDate）なし | MED | |
| 日付書式が YYYY-MM-DD でない | MED | |
| 日付年が 2000〜2030 の範囲外 | MED | 1900 年（デフォルト値混入）や未来日付の検出 |
| SDS 日付が 2020 年より古い（5 年超） | MED | 最新化が必要な古い SDS |

---

## クロスフィールドチェック

セクションをまたいだ整合性チェック：

| チェック | レベル | 内容 |
|---|---|---|
| H290（金属腐食性）だが成分名に酸・ハロゲン化物なし | MED | 組成と有害性の不整合 |
| プレースホルダーテキストの検出 | HIGH | `[insert`, `[記入`, `PLACEHOLDER`, `TODO`, `TBD` 等 |
| SDS セクション総数 < 10 | HIGH | 16 セクションのうち 10 未満が populated |
| SDS セクション総数 < 13 | MED | 16 セクションのうち 13 未満が populated |
| **r23** 異なるセクション間で同一テキスト（100 文字超）が存在 | MED | コピー&ペースト的な抽出エラー（同一ブロックが複数箇所に出現） |
| **r23** 成分 3 件以上の混合物で全 H コードが単一ファミリー | MED | H3xx のみ等、部分抽出の疑い（H4xx 環境系が抜けているケース等） |

---

## 実行方法

```bash
# 基本実行
python3 tools/quality_check.py <SDS_JSON_FILE> <LANG>

# LANG: en / ja / zh-cn / zh-tw

# 例
python3 tools/quality_check.py output/sds.json ja

# JSON Lines 出力（機械可読）
python3 tools/quality_check.py output/sds.json ja --jsonl | tail -1 | python3 -m json.tool

# 終了コード確認
echo "Exit: $?"
```

### バッチ実行（ラウンドトリップテスト）

```bash
set -a && source .env && set +a

# 30 件ランダム抽出（言語バランス調整）PDF → JSON → DOCX ラウンドトリップ
bash tools/roundtrip_test.sh 30 2>&1 | tee /tmp/roundtrip.txt

# サマリーのみ表示
grep -E "QC issues|FAIL|to-json" /tmp/roundtrip.txt
```

---

## 出力例

### OK（問題なし）

```
QC-OK: all quality checks passed
```

### WARN（軽微な問題 1 件）

```
QC-MED: Sec9: Density/RelativeDensity not extracted
QC-SUMMARY: 0 CRIT + 0 HIGH + 1 MED = 1 total issues
```

### FAIL（重大な問題あり）

```
QC-HIGH: Sec1: SupplierInformation.CompanyName is empty
QC-HIGH: Sec2: Hazard signal+H-codes present but NO P-codes extracted — labelling incomplete
QC-MED: Sec2: Oral acute-tox H-code but P301 (if swallowed) not found
QC-MED: Sec2: Inhalation H-code but P304 (if inhaled) or P261 (avoid breathing) not found
QC-MED: Sec11: Acute-tox H-code present but no LD50/LC50 value text found
QC-SUMMARY: 0 CRIT + 2 HIGH + 3 MED = 5 total issues
```

---

## チェックの改訂履歴

| バージョン | 主な追加チェック |
|---|---|
| **r21** | 基本セクション構造チェック、H/P コード書式、CAS 書式、FlashPoint 範囲、引火点 × 沸点、GHS ピクトグラム、Danger/Warning P コード最低数（≥3）、言語整合性 |
| **r22** | CAS チェックデジット検証、濃度合計 > 102%、多成分 CAS 必須、Sec6 具体的回収キーワード、Sec7 換気キーワード、Sec8 手袋材質・呼吸保護具種別、Sec9 自然発火温度・pH・蒸気圧、Sec10 分解生成物、Sec12 LogP/BCF、Sec14 正式品名、Sec15 GB/化管法・PRTR、Sec16 SDS 5 年超、Danger P コード ≥4 |
| **r23** | 供給者電話番号桁数、GHS01/GHS09 ピクトグラム整合性、信号語のみで HazardStatement 空（HIGH）、濃度フィールドへの年号混入検出（HIGH）、混合物 > 10 成分、OEL 数値確認、密度・pH・自然発火温度・沸点の範囲検証、H350/351 発がん性機関、H420 オゾン、UN 番号書式、セクション間重複テキスト、混合物 H コード単一ファミリー検出 |

---

## 設計上の判断

### なぜルールベースか

LLM の評価に LLM を使うと、評価の揺らぎが二重になります。QC スクリプトは **決定論的** に実行でき、CI/CD パイプラインに組み込むことができます。

### H コード × P コード クロスチェックの考え方

GHS の「ラベル要素」はハザードクラスごとに P コードが紐付いています。たとえば：

- H330（急性吸入毒性 Cat1）→ P260（蒸気を吸入しないこと）+ P271（屋外/換気の良い場所）+ P304+P340 + P310 などが必要

QC では「P コードが全くない」「最低個数に達しない」を検出します。個々の P コードの適切さまでは検証せず、完全な検証は別途専門家レビューが必要です。

### 「ソース限界」と「ツール起因エラー」の区別

| 種別 | 例 | QC 判定 |
|---|---|---|
| ソース限界 | 前 GHS MSDS（CompanyName・P コードが原文にない） | FAIL — ツール側では対応不可 |
| 抽出ギャップ | 密度が PDF に記載あるが抽出されない | FAIL/WARN — プロンプト改善で対応可能 |
| ツールバグ | serde クラッシュ、CID フォントパニック | ERROR — 変換失敗 |

QC スクリプトは 1 番目と 2 番目を区別しません。2 番目のケースに対してプロンプトや抽出ロジックを改善することがツール品質向上につながります。
