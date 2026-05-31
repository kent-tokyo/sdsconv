# SDS JSON Quality Check (QC) Script — Detailed Manual

> This page describes the automated quality verification system for SDS JSON files produced by sds-converter, covering every check rule and its rationale.

[日本語](quality-check_ja.md) | [中文](quality-check_zh.md)

---

## Table of Contents

1. [Overview](#overview)
2. [Severity Levels](#severity-levels)
3. [Exit Codes and Result Classification](#exit-codes-and-result-classification)
4. [Checks by SDS Section](#checks-by-sds-section)
   - [Section 1: Chemical Product Identification](#section-1-chemical-product-identification)
   - [Section 2: Hazard Identification](#section-2-hazard-identification)
   - [Section 3: Composition / Information on Ingredients](#section-3-composition--information-on-ingredients)
   - [Section 4: First Aid Measures](#section-4-first-aid-measures)
   - [Section 5: Firefighting Measures](#section-5-firefighting-measures)
   - [Section 6: Accidental Release Measures](#section-6-accidental-release-measures)
   - [Section 7: Handling and Storage](#section-7-handling-and-storage)
   - [Section 8: Exposure Controls / Personal Protection](#section-8-exposure-controls--personal-protection)
   - [Section 9: Physical and Chemical Properties](#section-9-physical-and-chemical-properties)
   - [Section 10: Stability and Reactivity](#section-10-stability-and-reactivity)
   - [Section 11: Toxicological Information](#section-11-toxicological-information)
   - [Section 12: Ecological Information](#section-12-ecological-information)
   - [Section 13: Disposal Considerations](#section-13-disposal-considerations)
   - [Section 14: Transport Information](#section-14-transport-information)
   - [Section 15: Regulatory Information](#section-15-regulatory-information)
   - [Section 16: Other Information](#section-16-other-information)
5. [Cross-Field Checks](#cross-field-checks)
6. [Usage](#usage)
7. [Sample Output](#sample-output)
8. [Revision History](#revision-history)

---

## Overview

The QC script automatically verifies whether an LLM-generated SDS JSON conforms to JIS Z 7253 / GHS requirements. Verification is **rule-based** and checks:

- Presence of mandatory fields
- H-code and P-code format and mutual consistency
- Physical/chemical property numeric ranges (e.g. boiling point > flash point)
- Cross-language consistency (e.g. no katakana signal word in a Chinese SDS)
- CAS number check-digit validation
- Concentration sum plausibility (must not greatly exceed 100%)

> **Note**: The QC script is rule-based. It measures the **consistency and completeness of the output JSON**, not the LLM's judgement or extraction accuracy per se.

---

## Severity Levels

| Level | Symbol | Meaning | Example |
|---|---|---|---|
| **CRIT** | 🔴 Critical | JIS Z 7253 violation · confirmed hallucination · mandatory section missing | Section 3 (Composition) empty · katakana substance name in a non-Japanese SDS |
| **HIGH** | 🟠 High | Significant extraction omission · format violation | Company name empty · no P-codes despite signal word · Toxicological Information section empty |
| **MED** | 🟡 Medium | Extraction quality gap · recommended field missing | Density not extracted · flash point absent for flammable product · P-code count below threshold |

---

## Exit Codes and Result Classification

```
exit code = total number of issues detected (CRIT + HIGH + MED combined)
```

| Exit code | Result | Meaning |
|---|---|---|
| `0` | **OK** | No issues. All checks passed |
| `1` | **WARN** | Exactly 1 MED issue (no CRIT or HIGH) |
| `2+` | **FAIL** | 1 or more CRIT/HIGH, or 2 or more MED issues |

This convention means `WARN` = "minor extraction gap, acceptable for most use cases" and `FAIL` = "significant omission present".

---

## Checks by SDS Section

### Section 1: Chemical Product Identification

**JSON field**: `Identification`

| Check | Level | Description |
|---|---|---|
| Product name (TradeNameJP / TradeNameEN) present | CRIT | Both empty is invalid for any SDS |
| Katakana product name in non-Japanese SDS | CRIT | Detects hallucination (e.g. `ベンゼン` in an English SDS) |
| Company name (CompanyName) present | HIGH | `SupplierInformation.CompanyName` is empty |
| Katakana company name in non-Japanese SDS | HIGH | Cross-language contamination |
| Use field present | MED | `UseAndUseAdvisedAgainst.Use` is empty |
| Emergency contact contains phone digits | MED | EmergencyContact entry has no numeric digits |
| **r23** Supplier phone has ≥ 7 digits | MED | `SupplierInformation.Phone` is absent or contains fewer than 7 digits |
| **r24** zh-cn/zh-tw SDS with no emergency contact | HIGH | Chinese regulations (GB/T 16483) require a 24h emergency contact; missing entirely |

**Note**: Pre-GHS Chinese MSDS files (e.g. from ichemistry) often lack a CompanyName in the source document. A HIGH flag in such cases reflects a source quality limitation, not an extraction bug.

---

### Section 2: Hazard Identification

**JSON field**: `HazardIdentification`

#### Signal Word

| Check | Level | Description |
|---|---|---|
| Signal word is from the valid set | MED | Must be one of: `危険`/`警告`/`Danger`/`Warning`/`N/A`/`不適用`/etc. |
| Katakana signal word in Chinese SDS | HIGH | Cross-language contamination |
| Non-English signal word in English SDS | MED | Localization error |
| H224 (extremely flammable) without `Danger` signal | HIGH | GHS Cat 1 always requires Danger |
| H226 alone with `Danger` signal (no other Cat1/2 codes) | MED | Cat 3 flammable liquid is normally Warning |

#### H-codes

| Check | Level | Description |
|---|---|---|
| H-code format (`H` + 3 digits + optional letter) | HIGH | Format violation |
| HazardStatement entries present but all codes empty | CRIT | Structural inconsistency |
| Signal word present but no H-codes | HIGH | Inconsistency |
| More than 12 H-codes | MED | Unusually high count for a single substance — likely duplication |
| Danger signal but no Cat 1/2 H-code found | MED | Severity mismatch |

**Cat 1/2 H-codes checked**:
`H200–H205`, `H220–H225`, `H260`, `H261`, `H270`, `H271`, `H280`, `H281`, `H290`,
`H300`, `H301`, `H310`, `H311`, `H330`, `H331`, `H314`, `H317`, `H318`, `H334`,
`H340`, `H341`, `H350`, `H351`, `H360`, `H361`, `H370`, `H371`, `H400`, `H410`, `H420`

#### P-codes

| Check | Level | Description |
|---|---|---|
| P-code format (`P` + 3 digits) | MED | Format violation |
| Signal word + H-codes present but zero P-codes | HIGH | Labelling information incomplete |
| Danger product with fewer than 4 P-codes | MED | GHS Danger level typically requires ≥4 precautionary statements |
| Warning product with fewer than 3 P-codes | MED | Warning level typically requires ≥3 |

#### H-code × P-code Consistency

| H-code | Expected P-code(s) | Check description |
|---|---|---|
| H224/H225/H226 | P210 | Keep away from heat/flames |
| H300/H301/H302 | P301 or P330 | If swallowed: first aid |
| H330/H331/H332 | P304 or P261 | If inhaled: first aid |
| H318/H319 | P305 | If in eyes: rinse |
| H314 | P280, P301, P305 | Corrosion: PPE + first aid set |

#### GHS Pictograms and Classification

| Check | Level | Description |
|---|---|---|
| Pictogram outside valid set | MED | Must be GHS01–GHS09 or Japanese equivalents |
| H-codes present but Classification section missing | MED | Classification information absent |
| **r23** H200–H205 (explosive) present but GHS01 pictogram absent | MED | GHS explosive hazard always requires the exploding bomb pictogram |
| **r23** H410/H411/H412/H413 (environmental hazard) present but GHS09 absent | MED | Environmental hazard requires the dead tree & fish pictogram |
| **r23** SignalWord present but HazardStatement completely absent | HIGH | Signal word without any hazard statements indicates incomplete labelling |

---

### Section 3: Composition / Information on Ingredients

**JSON field**: `Composition`

| Check | Level | Description |
|---|---|---|
| CompositionAndConcentration is empty | CRIT | No ingredients extracted |
| CompositionType is mixture but only 1 substance | MED | Mixture indication contradicts substance count |
| CAS number format (`9999999-99-9`) | HIGH | Format violation |
| CAS check-digit validation | HIGH | Computed check digit does not match |
| Multi-component product: component missing CAS | MED | Each ingredient in a mixture should have a CAS number |
| Katakana substance name in non-Japanese SDS | CRIT | Hallucination detected |
| Molecular weight ≤ 0 or > 200,000 | HIGH | Physically implausible value |
| Date string in Concentration field | HIGH | Extraction error (e.g. `2024-01-01` stored as concentration) |
| Duplicate CAS numbers | MED | Same CAS appears in multiple components |
| Sum of numeric concentrations > 102% | MED | Possible double-counting or extraction error |
| Single-component product with no substance name | MED | All name fields are empty |
| Single-component product with no concentration/purity | MED | Concentration field is empty |
| **r23** Mixture with > 10 components | MED | Unusually high count — likely over-extraction or CompositionType mismatch |
| **r23** Concentration field contains a year-like string | HIGH | e.g., `"2024"` or `"2024-01-01"` stored as concentration — extraction error |

**CAS check-digit example**:
```
CAS: 107-06-2 → digits "10706" multiplied right-to-left by 1,2,3,4,5 → sum mod 10 = 2 ✓
```

---

### Section 4: First Aid Measures

**JSON field**: `FirstAidMeasures`

| Check | Level | Description |
|---|---|---|
| ExposureRoute has no non-empty routes | HIGH | All route texts are empty |
| Hazardous product with fewer than 2 first-aid routes | MED | Typically inhalation, skin, eye, and ingestion routes are all needed |
| No physician/doctor mention for hazardous product | MED | Keywords: doctor/physician/medical/医師/就医/seek medical |
| Eye hazard H-code but no eye first-aid text | MED | H318/H319/H314 → eye/眼/rinse/洗眼 etc. |
| Inhalation H-code but no inhalation first-aid text | MED | H330–H335 → inhal/吸入/fresh air etc. |
| Skin hazard H-code but no skin contact first-aid text | MED | H314/H315 → skin/皮膚/wash etc. |

---

### Section 5: Firefighting Measures

**JSON field**: `FireFightingMeasures`

| Check | Level | Description |
|---|---|---|
| Section is completely empty | HIGH | JSON field length < 15 characters |
| No extinguishing agent mentioned | MED | No keywords such as foam/water/CO2/powder/dry chemical/泡/粉末/干粉 |

**Extinguishing agent keywords (partial)**: foam, water, CO2, carbon dioxide, powder, sand, dry chemical, halon, nitrogen, inert gas, extinguish, 泡, 二酸化炭素, 粉末, 砂, 消火, 灭火, 水雾, dry sand, surrounding, appropriate

---

### Section 6: Accidental Release Measures

**JSON field**: `AccidentalReleaseMeasures`

| Check | Level | Description |
|---|---|---|
| Section is empty | MED | JSON field length < 30 characters |
| No specific cleanup/containment method described | MED | No keywords: absorb/collect/sweep/dike/sand/berm/ventilat/吸収/回収/吸附/収集/围堤/通风 |

---

### Section 7: Handling and Storage

**JSON field**: `HandlingAndStorage`

| Check | Level | Description |
|---|---|---|
| Handling and storage information completely absent | HIGH | |
| Flammable H-code but no heat/ignition source mention | MED | H224/H225/H226 → cool/heat/ignition/flame/spark/火気/冷所/远离 |
| Water-reactive H-code but no dry/moisture mention | MED | H260/H261/H250 → dry/moisture/water/乾燥/防湿 |
| Volatile/toxic H-code but no ventilation mention | MED | H330–H335, H224–H226 → ventilat/exhaust/fume hood/換気/局排/通风/排気 |
| **r24** Flammable product but no storage temperature/cool mention | MED | H224/H225/H226 — storage section should mention cool conditions or specific temperature limits |

---

### Section 8: Exposure Controls / Personal Protection

**JSON field**: `ExposureControlPersonalProtection`

| Check | Level | Description |
|---|---|---|
| Section completely empty | HIGH | EngineeringControls, PPE, and OEL are all absent |
| Hazardous product with fewer than 2/4 PPE sub-fields | MED | Respiratory/hand/eye/skin protection — at least 2 required |
| Hazardous single-substance product with no OEL | MED | Occupational exposure limit extraction missing |
| H314 (corrosive) but no face shield/goggles mention | MED | eye protection must mention face shield/goggles/フェイス/ゴーグル/面罩 |
| Skin/corrosive H-code but glove material not specified | MED | HandProtection must name material: nitrile/butyl/neoprene/rubber/ニトリル/丁腈 etc. |
| Inhalation H-code but respirator type not specified | MED | RespiratoryProtection must name type: P2/ABEK/FFP/half mask/full face/SCBA/防毒/防塵 |
| **r23** OEL present but contains no numeric value | MED | OEL field text has no number (e.g., `ppm`, `mg/m³`) — likely a text placeholder |
| **r24** Hazardous product with no engineering controls specified | MED | EngineeringControls field empty for products with H-codes — ventilation/fume hood/local exhaust should be described |

**Glove material keywords**: nitrile, butyl, neoprene, rubber, latex, viton, PVC, polyethylene, ニトリル, ブチル, ネオプレン, ゴム, 丁腈, 丁基, 氯丁, 橡胶

**Respirator type keywords**: P1, P2, P3, A1, ABEK, FFP, half mask, full face, SCBA, P100, organic vapor, 防毒, 防じん, 送気, 有机蒸气, 防尘

---

### Section 9: Physical and Chemical Properties

**JSON field**: `PhysicalChemicalProperties`

#### Basic Properties

| Check | Level | Description |
|---|---|---|
| Both colour/appearance and physical state absent | HIGH | |
| Odour not extracted for hazardous product | MED | |
| Density/relative density not extracted | MED | Densities / Density / RelativeDensity / SpecificGravity — all absent |
| Water solubility not extracted | MED | SolubilityInWater / Solubility — both absent |

#### Flash Point

| Check | Level | Description |
|---|---|---|
| Flash point value is not numeric | HIGH | A string was stored instead |
| Flash point outside −220 to 400°C range | MED | Physically implausible |
| Flammable H-code (H224/225/226) but no flash point | MED | |
| H224 (extremely flammable) but flash point ≥ 23°C | MED | GHS: extremely flammable requires FP < 23°C |
| H226 alone but flash point outside 23–60°C | MED | GHS: Cat 3 flammable requires 23°C ≤ FP < 60°C |

#### Boiling Point and Melting Point

| Check | Level | Description |
|---|---|---|
| Flash point ≥ boiling point | MED | Physically impossible |
| Physical state is liquid but no boiling point (non-gas) | MED | |
| Physical state is solid/crystalline but no melting point | MED | |

#### Auto-ignition Temperature, Vapour Pressure, pH

| Check | Level | Description |
|---|---|---|
| Flammable H-code but no auto-ignition temperature | MED | AutoIgnitionTemperature not extracted |
| Volatile/flammable H-code but no vapour pressure | MED | H224/225/226/330/331/332 |
| Corrosive/acidic H-code but no pH | MED | H314/H290/H318/H319 |
| **r23** Density value outside 0.1–25 g/cm³ | MED | Physically implausible density (any common substance fits within this range) |
| **r23** pH value outside 0–14 | MED | Impossible pH — likely extraction error or unit mismatch |
| **r23** Auto-ignition temperature below flash point | MED | Auto-ignition temperature must be higher than flash point (thermodynamic constraint) |
| **r23** Boiling point outside −200 to 3000 °C | MED | Physically implausible value |

---

### Section 10: Stability and Reactivity

**JSON field**: `StabilityReactivity`

| Check | Level | Description |
|---|---|---|
| Section is empty | MED | JSON length < 30 characters |
| No conditions to avoid or incompatible materials mentioned | MED | No keywords: avoid/heat/incompatible/acid/酸化/禁止/avoid/分解/stable |
| Flammable/explosive H-code but decomposition products absent | MED | HazardousDecompositionProducts is empty |
| **r24** Reactive/oxidizer H-code but no incompatible materials listed | MED | H272/H290/H314 — incompatible materials (acids, bases, oxidizers etc.) should be listed |

---

### Section 11: Toxicological Information

**JSON field**: `ToxicologicalInformation`

| Check | Level | Description |
|---|---|---|
| Section completely empty | HIGH | |
| Acute-tox H-code but AcuteToxicity not extracted | MED | H300/H301/H302/H310/H311/H312/H330/H331/H332 |
| Acute-tox H-code but no LD50/LC50 value text | MED | Numeric toxicity value required |
| H315 (skin irritation) but SkinCorrosionIrritation absent | MED | |
| H319/H318 (eye damage) but EyeDamageOrIrritation absent | MED | |
| H334 (respiratory sensitizer) but Sensitization absent | MED | |
| H350/H351 (carcinogen) but Carcinogenicity absent | MED | |
| H360/H361 (reproductive tox) but ReproductiveToxicity absent | MED | |
| H370–H373 (STOT) but SpecificTargetOrgan absent | MED | |
| AcuteToxicity Cat 1/2 but no lethal H-code in Section 2 | MED | Hazard classification vs. H-code inconsistency |
| **r23** H350/H351 present but no carcinogenicity agency reference | MED | Carcinogenic classification should cite IARC/NTP/ACGIH/WHO or equivalent |

---

### Section 12: Ecological Information

**JSON field**: `EcologicalInformation`

| Check | Level | Description |
|---|---|---|
| Environmental H-code (H4xx) present but section empty | HIGH | |
| Environmental H-code but no aquatic toxicity keywords | MED | aquatic/fish/daphnia/algae/LC50/EC50/水生 etc. |
| H410/H411 but no biodegradability/bioaccumulation keywords | MED | biodeg/bioaccum/BCF/PersistenceDeg etc. |
| Environmental H-code but no LogP/Kow/BCF value | MED | partition coefficient / 分配係数 / 辛醇 etc. |
| Hazardous product with empty EcologicalInformation | MED | Even without H4xx codes, basic eco data recommended |
| **r23** H420 (ozone-depleting substance) present but no ODP/ozone keywords | MED | Ozone depletion potential or ozone layer reference expected in section 12 |

---

### Section 13: Disposal Considerations

**JSON field**: `DisposalConsiderations`

| Check | Level | Description |
|---|---|---|
| Section is empty | MED | |
| No disposal method or regulation reference | MED | No keywords: inciner/landfill/waste/regulation/廃棄/焼却/废物/焚烧 |

---

### Section 14: Transport Information

**JSON field**: `TransportInformation`

| Check | Level | Description |
|---|---|---|
| Section is missing | MED | |
| Dangerous goods H-code present but no UN number | MED | Unless "not regulated" is explicitly stated in the source |
| UN number found but Packing Group not extracted | MED | |
| UN number found but Proper Shipping Name not extracted | MED | |
| **r23** UN number does not match `UN\d{4}` format | MED | UN numbers must be 4-digit (UN0001–UN9999) |

**Dangerous goods H-codes triggering the UN check**:
H224, H225, H226, H300, H301, H302, H310, H311, H314, H330, H331, H332, H270, H271, H272

**"Not regulated" recognition patterns**:
`not regulated`, `非危険物`, `not dangerous`, `無資料`, `規制されていない`, `規制対象外`,
`危険物に該当しない`, `not subject`, `no regulation`, `非危险` etc.

---

### Section 15: Regulatory Information

**JSON field**: `RegulatoryInformation`

| Check | Level | Description |
|---|---|---|
| Section is empty | MED | |
| No recognizable law or regulation name | MED | law/regulation/安全衛生/化審法/GB/REACH/OSHA etc. |
| Japanese SDS but no Japanese law reference | MED | 労働安全衛生法/安衛法/化審法/毒劇法/消防法/化管法/PRTR |
| zh-cn SDS but no GB standard reference | MED | GB /GBZ/GB/T/GB13690/GB30000 etc. |
| ja SDS with carcinogenic/environmental H-code but no PRTR/化管法 | MED | H350/H351/H340/H341/H400/H410 |

---

### Section 16: Other Information

**JSON field**: `OtherInformation` / `Datasheet`

| Check | Level | Description |
|---|---|---|
| SDS date (IssueDate/RevisionDate) not extracted | MED | |
| Date format is not YYYY-MM-DD | MED | |
| Date year outside 2000–2030 range | MED | Detects default value 1900 or implausible future dates |
| SDS date is before 2020 (older than 5 years) | MED | May require update |

---

## Cross-Field Checks

Consistency checks spanning multiple sections:

| Check | Level | Description |
|---|---|---|
| H290 (corrosive to metals) but no acid/halide in composition | MED | Hazard vs. composition inconsistency |
| Placeholder text detected | HIGH | `[insert`, `[記入`, `PLACEHOLDER`, `TODO`, `TBD` etc. |
| Fewer than 10 of 16 SDS sections populated | HIGH | |
| Fewer than 13 of 16 SDS sections populated | MED | |
| **r23** Identical text (> 100 chars) in two different sections | MED | Copy-paste artefact — same block repeated verbatim across sections |
| **r23** All H-codes from a single H-code family for mixture with ≥ 3 components | MED | Suggests partial extraction (e.g., only H3xx hazards extracted, H4xx ecological ignored) |
| **r24** SDS date older than 5 years from current date | MED | IssueDate/RevisionDate more than 5 years old — may need regulatory re-review |

---

## Usage

```bash
# Basic usage
python3 tools/quality_check.py <SDS_JSON_FILE> <LANG>

# LANG: en / ja / zh-cn / zh-tw

# Example
python3 tools/quality_check.py output/sds.json ja

# Machine-readable JSON Lines output
python3 tools/quality_check.py output/sds.json ja --jsonl | tail -1 | python3 -m json.tool

# Check exit code
echo "Exit: $?"
```

### Batch execution (round-trip test)

```bash
set -a && source .env && set +a

# 30 random PDFs (balanced across languages), full round-trip PDF → JSON → DOCX
bash tools/roundtrip_test.sh 30 2>&1 | tee /tmp/roundtrip.txt

# Show summary only
grep -E "QC issues|FAIL|to-json" /tmp/roundtrip.txt
```

---

## Sample Output

### OK — no issues

```
QC-OK: all quality checks passed
```

### WARN — 1 minor issue

```
QC-MED: Sec9: Density/RelativeDensity not extracted
QC-SUMMARY: 0 CRIT + 0 HIGH + 1 MED = 1 total issues
```

### FAIL — significant issues

```
QC-HIGH: Sec1: SupplierInformation.CompanyName is empty
QC-HIGH: Sec2: Hazard signal+H-codes present but NO P-codes extracted — labelling incomplete
QC-MED: Sec2: Oral acute-tox H-code but P301 (if swallowed) not found
QC-MED: Sec2: Inhalation H-code but P304 (if inhaled) or P261 (avoid breathing) not found
QC-MED: Sec11: Acute-tox H-code present but no LD50/LC50 value text found
QC-SUMMARY: 0 CRIT + 2 HIGH + 3 MED = 5 total issues
```

---

## Revision History

| Version | Key additions |
|---|---|
| **r21** | Basic section structure, H/P-code format, CAS format, FlashPoint range, flash point vs. boiling point, GHS pictogram validation, Danger/Warning P-code minimum counts (≥3), cross-language consistency |
| **r22** | CAS check-digit validation, concentration sum > 102%, per-substance CAS in mixtures, Sec6 cleanup keywords, Sec7 ventilation for volatile/toxic, Sec8 glove material and respirator type, Sec9 auto-ignition temperature / pH / vapour pressure, Sec10 decomposition products, Sec12 LogP/BCF, Sec14 Proper Shipping Name, Sec15 GB standards / 化管法 PRTR, Sec16 SDS older than 5 years, Danger P-code count raised to ≥4 |
| **r23** | Supplier phone digit count, GHS01/GHS09 pictogram–H-code consistency, SignalWord without HazardStatement (HIGH), concentration year-string detection (HIGH), mixture > 10 components, OEL numeric value check, density/pH/auto-ignition/boiling point range validation, H350/351 carcinogenicity agency, H420 ozone keywords, UN number format, cross-section duplicate text, single-family H-code detection for complex mixtures |
| **r24** | S1-ZH-NO-EMERGENCY for zh-cn/zh-tw, S7-FLAMMABLE-STORAGE-TEMP, S8-NO-ENG-CONTROLS, S10-NO-INCOMPATIBLE, CROSS-STALE-DATE; S5-EMPTY threshold 30→15; S8-OEL-NO-NUMERIC Chinese unit-before-value exemption and additional "no OEL" phrase patterns |

---

## Design Rationale

### Why rule-based?

Evaluating LLM output with another LLM doubles the non-determinism. The QC script runs **deterministically** and can be integrated into CI/CD pipelines.

### H-code × P-code cross-check philosophy

GHS assigns specific precautionary statements to each hazard class. For example:

- H330 (acute inhalation toxicity Cat 1) → P260, P271, P304+P340, P310 etc. are expected

The QC script detects "no P-codes at all" and "below minimum count" but does not verify the appropriateness of individual P-codes. Complete verification still requires expert review.

### Distinguishing "source limitation" from "tool-caused error"

| Category | Example | QC result |
|---|---|---|
| Source limitation | Pre-GHS MSDS (no CompanyName or P-codes in the original) | FAIL — not fixable by the tool |
| Extraction gap | Density present in PDF but not extracted | FAIL/WARN — addressable by prompt improvement |
| Tool bug | serde crash, CID font panic | ERROR — conversion failed |

The QC script does not distinguish between the first and second categories. Improving prompts and extraction logic for the second category is the primary path to higher quality scores.
