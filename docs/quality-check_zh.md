# SDS JSON 质量检查（QC）脚本 — 详细手册

> 本页面详细说明 sds-converter 生成的 SDS JSON 文件的自动质量验证机制及各项检查规则。

[English](quality-check.md) | [日本語](quality-check_ja.md)

---

## 目录

1. [概述](#概述)
2. [严重程度级别](#严重程度级别)
3. [退出代码与判定规则](#退出代码与判定规则)
4. [按 SDS 章节的检查列表](#按-sds-章节的检查列表)
   - [第1节：化学品标识（Identification）](#第1节化学品标识)
   - [第2节：危险性概述（HazardIdentification）](#第2节危险性概述)
   - [第3节：成分/组成信息（Composition）](#第3节成分组成信息)
   - [第4节：急救措施（FirstAidMeasures）](#第4节急救措施)
   - [第5节：消防措施（FireFightingMeasures）](#第5节消防措施)
   - [第6节：泄漏应急处理（AccidentalReleaseMeasures）](#第6节泄漏应急处理)
   - [第7节：操作处置与储存（HandlingAndStorage）](#第7节操作处置与储存)
   - [第8节：接触控制/个体防护（ExposureControl）](#第8节接触控制个体防护)
   - [第9节：理化特性（PhysicalChemicalProperties）](#第9节理化特性)
   - [第10节：稳定性和反应性（StabilityReactivity）](#第10节稳定性和反应性)
   - [第11节：毒理学信息（ToxicologicalInformation）](#第11节毒理学信息)
   - [第12节：生态学信息（EcologicalInformation）](#第12节生态学信息)
   - [第13节：废弃处置（DisposalConsiderations）](#第13节废弃处置)
   - [第14节：运输信息（TransportInformation）](#第14节运输信息)
   - [第15节：法规信息（RegulatoryInformation）](#第15节法规信息)
   - [第16节：其他信息（OtherInformation）](#第16节其他信息)
5. [跨字段检查](#跨字段检查)
6. [使用方法](#使用方法)
7. [输出示例](#输出示例)
8. [版本修订历史](#版本修订历史)

---

## 概述

QC 脚本自动验证 LLM 生成的 SDS JSON 是否符合 JIS Z 7253 / GHS 要求。验证采用**基于规则**的方式，检查以下内容：

- 必填字段是否存在
- H 码和 P 码的格式与相互一致性
- 理化特性数值范围（例如沸点 > 闪点）
- 跨语言一致性（例如中文 SDS 中不应出现片假名信号词）
- CAS 号校验位验证
- 浓度总和的合理性（不应大幅超过 100%）

> **注意**：QC 脚本是基于规则的系统，检测的是**输出 JSON 的一致性和完整性**，而非 LLM 的判断力或提取精度本身。

---

## 严重程度级别

| 级别 | 符号 | 含义 | 示例 |
|---|---|---|---|
| **CRIT** | 🔴 严重 | 违反 JIS Z 7253 · 确认为幻觉 · 必填节缺失 | 第3节（成分信息）为空 · 非日文 SDS 中出现片假名物质名称 |
| **HIGH** | 🟠 高 | 重大提取遗漏 · 格式违规 | 公司名称为空 · 有信号词但无 P 码 · 毒理学信息节为空 |
| **MED** | 🟡 中 | 提取质量差距 · 推荐字段缺失 | 密度未提取 · 易燃产品无闪点 · P 码数量不足 |

---

## 退出代码与判定规则

```
退出代码 = 检测到的问题总数（CRIT + HIGH + MED 合计）
```

| 退出代码 | 判定 | 含义 |
|---|---|---|
| `0` | **OK** | 无问题，全部检查通过 |
| `1` | **WARN** | 仅 1 个 MED 问题（无 CRIT 或 HIGH） |
| `2+` | **FAIL** | 有 1 个以上 CRIT/HIGH，或有 2 个以上 MED |

此规范中，`WARN` 表示"轻微提取遗漏，实际使用基本无影响"，`FAIL` 表示"存在重大缺失"。

---

## 按 SDS 章节的检查列表

### 第1节：化学品标识

**JSON 字段**：`Identification`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 产品名称（TradeNameJP / TradeNameEN）存在 | CRIT | 任何语言的 SDS 两者均为空均为无效 |
| 非日文 SDS 中产品名含片假名 | CRIT | 检测幻觉（例如英文 SDS 中出现 `ベンゼン`） |
| 公司名称（CompanyName）存在 | HIGH | `SupplierInformation.CompanyName` 为空 |
| 非日文 SDS 中公司名含片假名 | HIGH | 跨语言污染 |
| 用途（Use 字段）存在 | MED | `UseAndUseAdvisedAgainst.Use` 为空 |
| 紧急联系含电话号码 | MED | EmergencyContact 条目中无数字 |

**说明**：前 GHS 时代的中国 MSDS（如 ichemistry）原文中通常没有公司名称。此情况下出现 HIGH 属于数据源质量限制，而非提取错误。

---

### 第2节：危险性概述

**JSON 字段**：`HazardIdentification`

#### 信号词（SignalWord）

| 检查项 | 级别 | 说明 |
|---|---|---|
| 信号词在有效集合内 | MED | 必须为：`危险`/`警告`/`Danger`/`Warning`/`N/A`/`不适用`等 |
| 中文 SDS 中信号词含片假名 | HIGH | 跨语言污染 |
| 英文 SDS 中信号词为非英文 | MED | 本地化错误 |
| H224（极度易燃）但信号词非 `Danger` | HIGH | GHS Cat1 必须使用 Danger |
| 仅有 H226 但信号词为 `Danger`（无其他 Cat1/2 码） | MED | Cat3 易燃液体通常为 Warning |

#### H 码

| 检查项 | 级别 | 说明 |
|---|---|---|
| H 码格式（`H` + 3 位数字 + 可选字母） | HIGH | 格式违规 |
| HazardStatement 有条目但代码全为空 | CRIT | 结构性不一致 |
| 有信号词但无 H 码 | HIGH | 不一致 |
| H 码数量 > 12 | MED | 单一物质超过 12 个疑似重复提取 |
| Danger 信号词但无 Cat1/2 H 码 | MED | 严重程度与信号词不匹配 |

**Cat1/2 H 码判定范围**：
`H200–H205`, `H220–H225`, `H260`, `H261`, `H270`, `H271`, `H300`, `H301`, `H310`, `H311`,
`H330`, `H331`, `H314`, `H340`, `H350`, `H360`, `H370`, `H400`, `H410`, `H420` 等

#### P 码

| 检查项 | 级别 | 说明 |
|---|---|---|
| P 码格式（`P` + 3 位数字） | MED | 格式违规 |
| 有信号词 + H 码但 P 码为零 | HIGH | 标签信息不完整 |
| Danger 产品 P 码少于 4 个 | MED | GHS Danger 级通常需要 ≥4 个防范说明 |
| Warning 产品 P 码少于 3 个 | MED | Warning 级通常需要 ≥3 个 |

#### H 码 × P 码一致性

| H 码 | 期望 P 码 | 检查说明 |
|---|---|---|
| H224/H225/H226 | P210 | 远离热源/火焰 |
| H300/H301/H302 | P301 或 P330 | 误食后急救措施 |
| H330/H331/H332 | P304 或 P261 | 吸入后急救措施 |
| H318/H319 | P305 | 眼部接触后冲洗 |
| H314 | P280, P301, P305 | 腐蚀性：防护装备 + 急救套组 |

#### GHS 象形图与分类

| 检查项 | 级别 | 说明 |
|---|---|---|
| 象形图不在有效集合内 | MED | 必须为 GHS01–GHS09 或对应的日文表述 |
| 有 H 码但 Classification 节缺失 | MED | 分类信息缺失 |

---

### 第3节：成分/组成信息

**JSON 字段**：`Composition`

| 检查项 | 级别 | 说明 |
|---|---|---|
| CompositionAndConcentration 为空 | CRIT | 未提取到任何成分信息 |
| CompositionType 为混合物但仅 1 种物质 | MED | 混合物标识与成分数量矛盾 |
| CAS 号格式（`9999999-99-9`） | HIGH | 格式违规 |
| CAS 校验位验证 | HIGH | 计算校验位不匹配 |
| 多组分产品某成分缺少 CAS | MED | 混合物每种成分均应有 CAS 号 |
| 非日文 SDS 中物质名含片假名 | CRIT | 检测到幻觉 |
| 分子量 ≤ 0 或 > 200,000 | HIGH | 物理上不合理的数值 |
| 浓度字段中含日期字符串 | HIGH | 提取错误（如 `2024-01-01` 被存为浓度） |
| CAS 号重复 | MED | 同一 CAS 出现在多个组分中 |
| 数值浓度之和 > 102% | MED | 可能存在重复计算或提取错误 |
| 单组分产品无物质名称 | MED | 所有名称字段均为空 |
| 单组分产品无浓度/纯度 | MED | Concentration 字段为空 |

**CAS 校验位计算示例**：
```
CAS: 107-06-2 → 数字 "10706" 从右到左乘以 1,2,3,4,5 → 求和取模 10 = 2 ✓
```

---

### 第4节：急救措施

**JSON 字段**：`FirstAidMeasures`

| 检查项 | 级别 | 说明 |
|---|---|---|
| ExposureRoute 无非空路径 | HIGH | 所有暴露途径文本均为空 |
| 危险品产品急救路径少于 2 条 | MED | 通常需要吸入、皮肤、眼睛、误食等多条路径 |
| 危险产品无医生/就医提及 | MED | 关键词：doctor/physician/medical/医师/就医 等 |
| 眼部危害 H 码但无眼部急救文本 | MED | H318/H319/H314 → eye/眼/rinse/洗眼 等 |
| 吸入 H 码但无吸入急救文本 | MED | H330–H335 → inhal/吸入/fresh air 等 |
| 皮肤 H 码但无皮肤接触急救文本 | MED | H314/H315 → skin/皮肤/wash 等 |

---

### 第5节：消防措施

**JSON 字段**：`FireFightingMeasures`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 章节完全为空 | HIGH | JSON 字段长度 < 30 字符 |
| 未提及灭火剂 | MED | 无关键词：foam/water/CO2/powder/干粉/泡沫/水雾 等 |

**灭火剂关键词（部分）**：foam, water, CO2, carbon dioxide, powder, sand, dry chemical, halon, nitrogen, extinguish, 泡, 二氧化碳, 粉末, 砂, 灭火, 水雾, 干砂, 干粉

---

### 第6节：泄漏应急处理

**JSON 字段**：`AccidentalReleaseMeasures`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 章节为空 | MED | JSON 字段长度 < 30 字符 |
| 无具体回收/围控方法描述 | MED | 无关键词：absorb/collect/sweep/吸附/回收/收集/围堤/通风 等 |

---

### 第7节：操作处置与储存

**JSON 字段**：`HandlingAndStorage`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 操作处置与储存信息完全缺失 | HIGH | |
| 易燃 H 码但未提及热源/点火源 | MED | H224/H225/H226 → cool/heat/ignition/火源/远离 等 |
| 遇水反应 H 码但未提及干燥条件 | MED | H260/H261/H250 → dry/moisture/水分 等 |
| 挥发性/有毒 H 码但未提及通风 | MED | H330–H335, H224–H226 → ventilat/通风/排气/局排 等 |

---

### 第8节：接触控制/个体防护

**JSON 字段**：`ExposureControlPersonalProtection`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 章节完全为空 | HIGH | 工程控制、PPE 和 OEL 均缺失 |
| 危险产品 PPE 子字段少于 2/4 | MED | 呼吸/手/眼/皮肤防护，至少需要 2 项 |
| 危险单一物质产品无 OEL | MED | 职业接触限值提取缺失 |
| H314（腐蚀性）但无护目/面罩提及 | MED | 眼部防护须提及：face shield/goggles/护目镜/面罩 等 |
| 皮肤/腐蚀性 H 码但手套材质未说明 | MED | HandProtection 须注明材质：nitrile/butyl/neoprene/丁腈/丁基 等 |
| 吸入 H 码但呼吸器型号未说明 | MED | RespiratoryProtection 须注明类型：P2/ABEK/FFP/防毒/防尘 等 |

**手套材质关键词**：nitrile, butyl, neoprene, rubber, latex, viton, PVC, polyethylene, 丁腈, 丁基, 氯丁, 橡胶, 乳胶

**呼吸器类型关键词**：P1, P2, P3, A1, ABEK, FFP, half mask, full face, SCBA, P100, organic vapor, 防毒, 防尘, 有机蒸气, 空气呼吸器

---

### 第9节：理化特性

**JSON 字段**：`PhysicalChemicalProperties`

#### 基本物性

| 检查项 | 级别 | 说明 |
|---|---|---|
| 颜色/外观和物理状态均缺失 | HIGH | |
| 危险产品气味（Odour）未提取 | MED | |
| 密度/相对密度未提取 | MED | Densities / Density / RelativeDensity / SpecificGravity 均缺失 |
| 水溶解度未提取 | MED | SolubilityInWater / Solubility 均缺失 |

#### 闪点（FlashPoint）

| 检查项 | 级别 | 说明 |
|---|---|---|
| 闪点值非数值 | HIGH | 存储了字符串而非数字 |
| 闪点超出 −220 至 400°C 范围 | MED | 物理上不合理的数值 |
| 易燃 H 码（H224/225/226）但无闪点 | MED | |
| H224（极度易燃）但闪点 ≥ 23°C | MED | GHS：极度易燃要求 FP < 23°C |
| 仅 H226 但闪点不在 23–60°C 范围内 | MED | GHS：Cat3 易燃液体要求 23°C ≤ FP < 60°C |

#### 沸点与熔点

| 检查项 | 级别 | 说明 |
|---|---|---|
| 闪点 ≥ 沸点 | MED | 物理上不可能 |
| 液态产品但无沸点（非压缩气体） | MED | |
| 固态/结晶态产品但无熔点 | MED | |

#### 自燃温度、蒸气压、pH

| 检查项 | 级别 | 说明 |
|---|---|---|
| 易燃 H 码但无自燃温度 | MED | AutoIgnitionTemperature 未提取 |
| 挥发性/易燃 H 码但无蒸气压 | MED | H224/225/226/330/331/332 |
| 腐蚀性/酸性 H 码但无 pH | MED | H314/H290/H318/H319 |

---

### 第10节：稳定性和反应性

**JSON 字段**：`StabilityReactivity`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 章节为空 | MED | JSON 长度 < 30 字符 |
| 未提及应避免的条件或不相容物质 | MED | 无关键词：avoid/heat/incompatible/acid/氧化/禁止/分解 等 |
| 易燃/爆炸性 H 码但分解产物缺失 | MED | HazardousDecompositionProducts 为空 |

---

### 第11节：毒理学信息

**JSON 字段**：`ToxicologicalInformation`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 章节完全为空 | HIGH | |
| 急性毒性 H 码但 AcuteToxicity 未提取 | MED | H300/H301/H302/H310/H311/H312/H330/H331/H332 |
| 急性毒性 H 码但无 LD50/LC50 数值文本 | MED | 需要数值毒性数据 |
| H315（皮肤刺激）但 SkinCorrosionIrritation 缺失 | MED | |
| H319/H318（眼损伤）但 EyeDamageOrIrritation 缺失 | MED | |
| H334（呼吸道致敏）但 Sensitization 缺失 | MED | |
| H350/H351（致癌性）但 Carcinogenicity 缺失 | MED | |
| H360/H361（生殖毒性）但 ReproductiveToxicity 缺失 | MED | |
| H370–H373（STOT）但 SpecificTargetOrgan 缺失 | MED | |
| AcuteToxicity Cat1/2 但第2节无致死 H 码 | MED | 危害分类与 H 码不一致 |

---

### 第12节：生态学信息

**JSON 字段**：`EcologicalInformation`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 环境 H 码（H4xx）存在但章节为空 | HIGH | |
| 环境 H 码但无水生毒性关键词 | MED | aquatic/fish/daphnia/algae/LC50/EC50/水生 等 |
| H410/H411 但无生物降解/生物蓄积关键词 | MED | biodeg/bioaccum/BCF/PersistenceDeg 等 |
| 环境 H 码但无 LogP/Kow/BCF 数值 | MED | partition coefficient / 分配系数 / 辛醇 等 |
| 危险产品但 EcologicalInformation 为空 | MED | 即使没有 H4xx 码，也建议填写基本生态数据 |

---

### 第13节：废弃处置

**JSON 字段**：`DisposalConsiderations`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 章节为空 | MED | |
| 无废弃方法或法规参考 | MED | 无关键词：inciner/landfill/waste/regulation/废物/焚烧/废弃 等 |

---

### 第14节：运输信息

**JSON 字段**：`TransportInformation`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 章节缺失 | MED | |
| 危险品 H 码存在但无 UN 编号 | MED | 除非原文明确说明"不受管制" |
| 有 UN 编号但无包装类别（Packing Group） | MED | |
| 有 UN 编号但无正式运输品名（Proper Shipping Name） | MED | |

**触发 UN 检查的危险品 H 码**：H224, H225, H226, H300, H301, H302, H310, H311, H314, H330, H331, H332, H270, H271, H272

**"不受管制"识别文本**：`not regulated`, `非危険物`, `not dangerous`, `無資料`, `非危险`, `规制されていない`, `not subject`, `no regulation` 等

---

### 第15节：法规信息

**JSON 字段**：`RegulatoryInformation`

| 检查项 | 级别 | 说明 |
|---|---|---|
| 章节为空 | MED | |
| 无可识别的法律/法规名称 | MED | law/regulation/安全衛生/化審法/GB/REACH/OSHA 等 |
| 日文 SDS 但无日本法规参考 | MED | 労働安全衛生法/化審法/毒劇法/消防法/化管法/PRTR |
| zh-cn SDS 但无 GB 标准参考 | MED | GB /GBZ/GB/T/GB13690/GB30000 等 |
| 日文 SDS 含致癌/环境 H 码但无化管法/PRTR | MED | H350/H351/H340/H341/H400/H410 |

---

### 第16节：其他信息

**JSON 字段**：`OtherInformation` / `Datasheet`

| 检查项 | 级别 | 说明 |
|---|---|---|
| SDS 日期（IssueDate/RevisionDate）未提取 | MED | |
| 日期格式非 YYYY-MM-DD | MED | |
| 日期年份超出 2000–2030 范围 | MED | 检测默认值 1900 或不合理的未来日期 |
| SDS 日期早于 2020 年（超过 5 年） | MED | 可能需要更新 |

---

## 跨字段检查

跨节一致性检查：

| 检查项 | 级别 | 说明 |
|---|---|---|
| H290（对金属腐蚀性）但成分名中无酸/卤化物 | MED | 危害性与成分不一致 |
| 检测到占位符文本 | HIGH | `[insert`, `[記入`, `PLACEHOLDER`, `TODO`, `TBD` 等 |
| 16 节中少于 10 节有内容 | HIGH | |
| 16 节中少于 13 节有内容 | MED | |

---

## 使用方法

```bash
# 基本用法
python3 quality_check_r22.py <SDS_JSON_FILE> <LANG>

# LANG: en / ja / zh-cn / zh-tw

# 示例
python3 quality_check_r22.py output/sds.json zh-cn

# 查看退出代码
echo "Exit: $?"
```

### 批量执行（结合测试脚本）

```bash
set -a && source .env && set +a

bash test_round22.sh \
  target/release/sds-converter \
  /tmp/sds_out \
  "Phase17" \
  2>&1 | tee /tmp/result.txt

# 仅显示摘要
grep "SUMMARY" /tmp/result.txt
```

---

## 输出示例

### OK — 无问题

```
QC-OK: all quality checks passed
```

### WARN — 1 个轻微问题

```
QC-MED: Sec9: Density/RelativeDensity not extracted
QC-SUMMARY: 0 CRIT + 0 HIGH + 1 MED = 1 total issues
```

### FAIL — 存在重大问题

```
QC-HIGH: Sec1: SupplierInformation.CompanyName is empty
QC-HIGH: Sec2: Hazard signal+H-codes present but NO P-codes extracted — labelling incomplete
QC-MED: Sec2: Oral acute-tox H-code but P301 (if swallowed) not found
QC-MED: Sec2: Inhalation H-code but P304 (if inhaled) or P261 (avoid breathing) not found
QC-MED: Sec11: Acute-tox H-code present but no LD50/LC50 value text found
QC-SUMMARY: 0 CRIT + 2 HIGH + 3 MED = 5 total issues
```

---

## 版本修订历史

| 版本 | 主要新增检查 |
|---|---|
| **r21** | 基本节结构、H/P 码格式、CAS 格式、闪点范围、闪点 vs 沸点、GHS 象形图、Danger/Warning P 码最低数量（≥3）、跨语言一致性 |
| **r22** | CAS 校验位验证、浓度总和 > 102%、混合物各组分 CAS、第6节回收关键词、第7节通风要求、第8节手套材质和呼吸器类型、第9节自燃温度/pH/蒸气压、第10节分解产物、第12节 LogP/BCF、第14节正式运输品名、第15节 GB 标准/化管法 PRTR、第16节 SDS 超过5年、Danger P 码提升至 ≥4 |

---

## 设计说明

### 为何采用基于规则的方式

用 LLM 评估 LLM 输出会使不确定性翻倍。QC 脚本**确定性**运行，可集成到 CI/CD 流水线中。

### H 码 × P 码交叉检查的原理

GHS 为每种危险类别规定了对应的防范说明。例如：

- H330（急性吸入毒性 Cat1）→ 预期包含 P260、P271、P304+P340、P310 等

QC 脚本检测"完全没有 P 码"和"低于最低数量"两种情况，但不验证单个 P 码是否合适。完整验证仍需专家审核。

### 区分"数据源限制"与"工具错误"

| 类型 | 示例 | QC 判定 |
|---|---|---|
| 数据源限制 | 前 GHS MSDS（原文中无公司名或 P 码） | FAIL — 工具无法解决 |
| 提取差距 | PDF 中有密度但未被提取 | FAIL/WARN — 可通过改进提示词解决 |
| 工具错误 | serde 崩溃、CID 字体崩溃 | ERROR — 转换失败 |

QC 脚本不区分前两种类型。针对第二种情况改进提示词和提取逻辑是提升工具质量分数的主要途径。
