use std::path::Path;

use docx_rs::*;
use serde_json::Value;

use crate::error::SdsError;
use crate::language::Language;
use crate::schema::SdsRoot;

// JIS Z 7253 section names in 4 languages.
// Order: (Japanese, English, ChineseSimplified, ChineseTraditional)
//
// Sources:
//   EN   — GHS Rev.10 (UN) / ISO 11014:2020 / OSHA HazCom 2012
//   zhCN — GB/T 16483-2012 (中国国家标准)
//   zhTW — CNS 15030 (台灣 GHS 標準)
pub(crate) const SECTION_NAMES: &[(&str, &str, &str, &str)] = &[
    ("化学品及び会社情報",         "Identification",                            "化学品及其企业标识",   "化學品與廠商資料"),
    ("危険有害性の要約",           "Hazard(s) Identification",                  "危险性概述",           "危害辨識資料"),
    ("組成及び成分情報",           "Composition / Information on Ingredients",  "成分/组成信息",        "成分辨識資料"),
    ("応急措置",                   "First-Aid Measures",                        "急救措施",             "急救措置"),
    ("火災時の措置",               "Fire-Fighting Measures",                    "消防措施",             "滅火措施"),
    ("漏出時の措置",               "Accidental Release Measures",               "泄漏应急处理",         "洩漏處理方法"),
    ("取扱い及び保管上の注意",     "Handling and Storage",                      "操作处置与储存",       "安全處置與儲存方法"),
    ("ばく露防止及び保護措置",     "Exposure Controls / Personal Protection",   "接触控制/个体防护",    "暴露預防措施"),
    ("物理的及び化学的性質",       "Physical and Chemical Properties",          "理化特性",             "物理及化學性質"),
    ("安定性及び反応性",           "Stability and Reactivity",                  "稳定性和反应性",       "安定性及反應性"),
    ("有害性情報",                 "Toxicological Information",                 "毒理学信息",           "毒性資料"),
    ("環境影響情報",               "Ecological Information",                    "生态学信息",           "生態資料"),
    ("廃棄上の注意",               "Disposal Considerations",                   "废弃处置",             "廢棄處置方法"),
    ("輸送上の注意",               "Transport Information",                     "运输信息",             "運送資料"),
    ("適用法令",                   "Regulatory Information",                    "法规信息",             "法規資料"),
    ("その他の情報",               "Other Information",                         "其他信息",             "其他資料"),
];

// Mapping: SdsRoot JSON key → section index (0-based matching SECTION_NAMES)
pub(crate) const SECTION_KEYS: &[&str] = &[
    "Identification",
    "HazardIdentification",
    "Composition",
    "FirstAidMeasures",
    "FireFightingMeasures",
    "AccidentalReleaseMeasures",
    "HandlingAndStorage",
    "ExposureControlPersonalProtection",
    "PhysicalChemicalProperties",
    "StabilityReactivity",
    "ToxicologicalInformation",
    "EcologicalInformation",
    "DisposalConsiderations",
    "TransportInformation",
    "RegulatoryInformation",
    "OtherInformation",
];

pub(crate) const DOCUMENT_TITLE: &[&str] = &[
    "安全データシート",         // Japanese (JIS Z 7253)
    "Safety Data Sheet",        // English (GHS/ISO 11014)
    "安全技术说明书",           // ChineseSimplified (GB/T 16483)
    "安全資料表",               // ChineseTraditional (CNS 15030)
];

pub(crate) fn lang_index(lang: Language) -> usize {
    match lang {
        Language::Japanese => 0,
        Language::English => 1,
        Language::ChineseSimplified => 2,
        Language::ChineseTraditional => 3,
    }
}

pub(crate) fn section_name(section_idx: usize, lang: Language) -> &'static str {
    let row = &SECTION_NAMES[section_idx];
    match lang {
        Language::Japanese => row.0,
        Language::English => row.1,
        Language::ChineseSimplified => row.2,
        Language::ChineseTraditional => row.3,
    }
}

// ---------------------------------------------------------------------------
// Key label translation
// ---------------------------------------------------------------------------

/// Structural wrapper keys whose children are rendered directly without a label.
const TRANSPARENT_KEYS: &[&str] = &[
    "BasePhysicalChemicalProperties",
    "NumericRangeWithUnitAndQualifier",
    "SingleValueWithUnit",
    "SingleValueWithUnitAndQualifier",
    "RangeValue",
    "OtherPhysicalChemicalProperty",
];

/// Translation table: JSON key → (Japanese, English, ChineseSimplified, ChineseTraditional)
const KEY_LABELS: &[(&str, &str, &str, &str, &str)] = &[
    // --- Datasheet ---
    ("IssueDate",                      "発行日",                      "Issue Date",                      "发布日期",           "發行日期"),
    ("RevisionDate",                   "改訂日",                      "Revision Date",                   "修订日期",           "修訂日期"),
    ("RevisionNo",                     "改訂番号",                    "Revision No.",                    "修订号",             "修訂號"),
    ("SDS-SchemaVersionNo",            "SDSスキーマバージョン",        "SDS Schema Version",              "SDS格式版本",        "SDS格式版本"),
    // --- Section 1: Identification ---
    ("ProductName",                    "製品名",                      "Product Name",                    "产品名称",           "產品名稱"),
    ("ProductCode",                    "製品コード",                  "Product Code",                    "产品代码",           "產品代碼"),
    ("CompanyName",                    "会社名",                      "Company Name",                    "公司名称",           "公司名稱"),
    ("AddressLine",                    "住所",                        "Address",                         "地址",               "地址"),
    ("Telephone",                      "電話番号",                    "Telephone",                       "电话",               "電話"),
    ("EmergencyTelephone",             "緊急連絡先",                  "Emergency Telephone",             "应急电话",           "緊急電話"),
    ("UseDescription",                 "用途",                        "Intended Use",                    "用途",               "用途"),
    ("SDSCode",                        "SDS番号",                     "SDS Code",                        "SDS编号",            "SDS編號"),
    // --- Section 2: HazardIdentification ---
    ("GHSClassification",              "GHS分類",                     "GHS Classification",              "GHS分类",            "GHS分類"),
    ("HazardStatement",                "危険有害性情報",               "Hazard Statement",                "危险性说明",         "危害警告訊息"),
    ("PrecautionaryStatement",         "注意書き",                    "Precautionary Statement",         "防范说明",           "防範說明"),
    ("GHSPictogram",                   "絵表示",                      "GHS Pictogram",                   "象形图",             "象形圖"),
    ("SignalWord",                     "注意喚起語",                  "Signal Word",                     "警示词",             "警示語"),
    ("HazardClass",                    "危険有害性クラス",             "Hazard Class",                    "危险类别",           "危害類別"),
    ("HazardCategory",                 "危険有害性区分",               "Hazard Category",                 "危险种类",           "危害種類"),
    ("OtherHazards",                   "その他の危険有害性",           "Other Hazards",                   "其他危害",           "其他危害"),
    // --- Section 3: Composition ---
    ("Substance",                      "物質",                        "Substance",                       "物质",               "物質"),
    ("Mixture",                        "混合物",                      "Mixture",                         "混合物",             "混合物"),
    ("ChemicalName",                   "化学物質名",                  "Chemical Name",                   "化学品名称",         "化學品名稱"),
    ("CASno",                          "CAS番号",                     "CAS No.",                         "CAS编号",            "CAS編號"),
    ("Concentration",                  "濃度",                        "Concentration",                   "浓度",               "濃度"),
    ("MolecularFormula",               "分子式",                      "Molecular Formula",               "分子式",             "分子式"),
    // --- Section 4: FirstAidMeasures ---
    ("InhalationFirstAid",             "吸入した場合",                "Inhalation",                      "吸入",               "吸入"),
    ("SkinContactFirstAid",            "皮膚に付着した場合",          "Skin Contact",                    "皮肤接触",           "皮膚接觸"),
    ("EyeContactFirstAid",             "眼に入った場合",              "Eye Contact",                     "眼睛接触",           "眼睛接觸"),
    ("IngestionFirstAid",              "飲み込んだ場合",              "Ingestion",                       "食入",               "食入"),
    ("MedicalTreatment",               "医師への注意",                "Notes to Physician",              "对医生的特别提示",   "醫師注意事項"),
    // --- Section 5: FireFightingMeasures ---
    ("SuitableExtinguishingMedia",     "適切な消火剤",                "Suitable Extinguishing Media",    "适用灭火剂",         "適用滅火劑"),
    ("UnsuitableExtinguishingMedia",   "使ってはならない消火剤",      "Unsuitable Extinguishing Media",  "不适用灭火剂",       "不適用滅火劑"),
    ("FireAndExplosionHazards",        "火災及び爆発の危険性",        "Fire and Explosion Hazards",      "火灾爆炸危险性",     "火災及爆炸危害"),
    ("FireFightingProcedures",         "消火活動上の注意",            "Fire-fighting Procedures",        "灭火注意事项",       "滅火注意事項"),
    // --- Section 6: AccidentalReleaseMeasures ---
    ("PersonalPrecautions",            "人体に対する注意事項",        "Personal Precautions",            "个体防护措施",       "個人防護措施"),
    ("ContainmentAndCleanUp",          "漏出物の回収方法",            "Containment and Clean-up",        "泄漏物控制和清除",   "洩漏物控制與清除"),
    // --- Section 7: HandlingAndStorage ---
    ("HandlingPrecautions",            "取扱い上の注意",              "Handling Precautions",            "操作注意事项",       "操作注意事項"),
    ("StorageConditions",              "保管条件",                    "Storage Conditions",              "储存条件",           "儲存條件"),
    // --- Section 8: ExposureControl ---
    ("OccupationalExposureLimit",      "管理濃度・許容濃度",          "Occupational Exposure Limit",     "职业接触限值",       "職業暴露限值"),
    ("EngControlMeasures",             "設備対策",                    "Engineering Controls",            "工程控制措施",       "工程控制措施"),
    ("RespiratoryProtection",          "呼吸用保護具",                "Respiratory Protection",          "呼吸防护",           "呼吸防護"),
    ("HandProtection",                 "手の保護具",                  "Hand Protection",                 "手部防护",           "手部防護"),
    ("EyeProtection",                  "眼の保護具",                  "Eye Protection",                  "眼睛防护",           "眼睛防護"),
    ("SkinBodyProtection",             "皮膚及び身体の保護具",        "Skin/Body Protection",            "皮肤和身体防护",     "皮膚及身體防護"),
    ("HygieneMeasures",                "衛生上の注意",                "Hygiene Measures",                "卫生措施",           "衛生措施"),
    // --- Section 9: PhysicalChemicalProperties ---
    ("PhysicalState",                  "物理的状態",                  "Physical State",                  "物理状态",           "物理狀態"),
    ("Colour",                         "色",                          "Colour",                          "颜色",               "顏色"),
    ("Odour",                          "臭い",                        "Odour",                           "气味",               "氣味"),
    ("OdourThreshold",                 "臭気閾値",                    "Odour Threshold",                 "气味阈值",           "氣味閾值"),
    ("pH",                             "pH",                          "pH",                              "pH",                 "pH"),
    ("MeltingFreezingPoint",           "融点・凝固点",                "Melting/Freezing Point",          "熔点/凝固点",        "熔點/凝固點"),
    ("InitialBoilingPoint",            "初留点・沸点範囲",            "Initial Boiling Point",           "初沸点和沸程",       "初沸點"),
    ("FlashPoint",                     "引火点",                      "Flash Point",                     "闪点",               "閃火點"),
    ("EvaporationRate",                "蒸発速度",                    "Evaporation Rate",                "蒸发速率",           "蒸發速率"),
    ("Flammability",                   "引火性",                      "Flammability",                    "易燃性",             "易燃性"),
    ("UpperFlammabilityLimit",         "爆発上限界",                  "Upper Flammability Limit",        "爆炸上限",           "爆炸上限"),
    ("LowerFlammabilityLimit",         "爆発下限界",                  "Lower Flammability Limit",        "爆炸下限",           "爆炸下限"),
    ("VapourPressure",                 "蒸気圧",                      "Vapour Pressure",                 "蒸气压",             "蒸氣壓"),
    ("VapourDensity",                  "蒸気密度",                    "Vapour Density",                  "蒸气密度",           "蒸氣密度"),
    ("RelativeDensity",                "密度",                        "Relative Density",                "相对密度",           "相對密度"),
    ("Solubility",                     "溶解度",                      "Solubility",                      "溶解性",             "溶解度"),
    ("PartitionCoefficient",           "オクタノール/水分配係数",     "Partition Coefficient",           "辛醇/水分配系数",    "辛醇/水分配係數"),
    ("AutoIgnitionTemperature",        "自然発火温度",                "Auto-ignition Temperature",       "自燃温度",           "自燃溫度"),
    ("DecompositionTemperature",       "分解温度",                    "Decomposition Temperature",       "分解温度",           "分解溫度"),
    ("KinematicViscosity",             "動粘性率",                    "Kinematic Viscosity",             "运动粘度",           "動粘度"),
    ("ExplosiveProperties",            "爆発性",                      "Explosive Properties",            "爆炸性",             "爆炸性"),
    ("OxidisingProperties",            "酸化性",                      "Oxidising Properties",            "氧化性",             "氧化性"),
    // --- Section 10: StabilityReactivity ---
    ("Reactivity",                     "反応性",                      "Reactivity",                      "反应性",             "反應性"),
    ("ChemicalStability",              "化学的安定性",                "Chemical Stability",              "化学稳定性",         "化學穩定性"),
    ("HazardousReactions",             "危険有害反応可能性",          "Hazardous Reactions",             "危险反应",           "危險反應"),
    ("ConditionsToAvoid",              "避けるべき条件",              "Conditions to Avoid",             "应避免的条件",       "應避免的條件"),
    ("IncompatibleMaterials",          "混触危険物質",                "Incompatible Materials",          "禁配物",             "禁配物"),
    ("HazardousDecompositionProducts", "危険有害な分解生成物",        "Hazardous Decomposition Products","危险的分解产物",     "危險分解產物"),
    // --- Section 11: Toxicological ---
    ("AcuteToxicity",                  "急性毒性",                    "Acute Toxicity",                  "急性毒性",           "急性毒性"),
    ("SkinCorrosionIrritation",        "皮膚腐食性・刺激性",          "Skin Corrosion/Irritation",       "皮肤腐蚀/刺激",      "皮膚腐蝕性/刺激性"),
    ("EyeDamageIrritation",            "眼への損傷・刺激性",          "Eye Damage/Irritation",           "严重眼损伤/刺激",    "眼睛損傷/刺激性"),
    ("RespiratoryOrSkinSensitization", "呼吸器・皮膚感作性",          "Respiratory/Skin Sensitization",  "呼吸/皮肤过敏",      "呼吸/皮膚過敏"),
    ("GermCellMutagenicity",           "生殖細胞変異原性",            "Germ Cell Mutagenicity",          "生殖细胞致突变性",   "生殖細胞致突變性"),
    ("Carcinogenicity",                "発がん性",                    "Carcinogenicity",                 "致癌性",             "致癌性"),
    ("ReproductiveToxicity",           "生殖毒性",                    "Reproductive Toxicity",           "生殖毒性",           "生殖毒性"),
    ("STOTSingleExposure",             "特定標的臓器毒性（単回）",    "STOT Single Exposure",            "一次性接触毒性",     "單次暴露毒性"),
    ("STOTRepeatedExposure",           "特定標的臓器毒性（反復）",    "STOT Repeated Exposure",          "反复接触毒性",       "重複暴露毒性"),
    ("AspirationHazard",               "誤えん有害性",                "Aspiration Hazard",               "吸入危害",           "吸入危害"),
    ("ExposureRoute",                  "ばく露経路",                  "Exposure Route",                  "接触途径",           "暴露途徑"),
    ("ToxicologicalEffect",            "毒性の影響",                  "Toxicological Effects",           "毒理学效应",         "毒理學效應"),
    // --- Section 12: Ecological ---
    ("AquaticAcuteToxicity",           "水生環境急性有害性",          "Aquatic Acute Toxicity",          "水生生物急性毒性",   "水生生物急毒性"),
    ("AquaticChronicToxicity",         "水生環境慢性有害性",          "Aquatic Chronic Toxicity",        "水生生物慢性毒性",   "水生生物慢毒性"),
    ("Persistence",                    "残留性・分解性",              "Persistence/Degradability",       "持久性/降解性",      "持久性/降解性"),
    ("Bioaccumulation",                "生体蓄積性",                  "Bioaccumulation",                 "生物富集性",         "生物蓄積性"),
    ("MobilityInSoil",                 "土壌中の移動性",              "Mobility in Soil",                "土壤中的流动性",     "土壤中移動性"),
    // --- Section 13: Disposal ---
    ("DisposalMethod",                 "廃棄方法",                    "Disposal Method",                 "废弃处置方法",       "廢棄處置方法"),
    ("WasteContainerMethod",           "廃棄物容器の取扱い",          "Waste Container",                 "废弃物容器",         "廢棄物容器"),
    // --- Section 14: Transport ---
    ("UNNo",                           "国連番号",                    "UN No.",                          "联合国编号",         "聯合國編號"),
    ("ShippingName",                   "品名",                        "Proper Shipping Name",            "运输专用名称",       "正式運送名稱"),
    ("PackingGroup",                   "容器等級",                    "Packing Group",                   "包装类别",           "包裝等級"),
    ("MarinePollutant",                "海洋汚染物質",                "Marine Pollutant",                "海洋污染物",         "海洋污染物"),
    // --- Section 15: Regulatory ---
    ("Regulations",                    "適用法令",                    "Regulations",                     "适用法规",           "適用法規"),
    ("ChemicalSafetyReport",           "化学品安全評価",              "Chemical Safety Report",          "化学品安全评估",     "化學品安全評估"),
    // --- Shared numeric / value fields ---
    ("NumericValue",                   "値",                          "Value",                           "数值",               "數值"),
    ("Unit",                           "単位",                        "Unit",                            "单位",               "單位"),
    ("Qualifier",                      "比較記号",                    "Qualifier",                       "限定词",             "限定詞"),
    ("MinValue",                       "最小値",                      "Min Value",                       "最小值",             "最小值"),
    ("MaxValue",                       "最大値",                      "Max Value",                       "最大值",             "最大值"),
    ("TestMethod",                     "試験方法",                    "Test Method",                     "试验方法",           "試驗方法"),
    ("Species",                        "生物種",                      "Species",                         "物种",               "物種"),
    ("Route",                          "ばく露経路",                  "Exposure Route",                  "接触途径",           "暴露途徑"),
    ("Result",                         "結果",                        "Result",                          "结果",               "結果"),
    ("EC50",                           "EC50",                        "EC50",                            "EC50",               "EC50"),
    ("LC50",                           "LC50",                        "LC50",                            "LC50",               "LC50"),
    ("LD50",                           "LD50",                        "LD50",                            "LD50",               "LD50"),
    ("ReferencesAndDataSources",       "参考文献",                    "References",                      "参考文献",           "參考文獻"),
    ("OtherInfo",                      "その他",                      "Other Information",               "其他信息",           "其他資料"),
    // --- Additional keys seen in LLM-generated output ---
    ("SupplierInformation",            "供給者情報",                  "Supplier Information",            "供应商信息",         "供應商資訊"),
    ("EmergencyContact",               "緊急連絡先",                  "Emergency Contact",               "紧急联系方式",       "緊急聯絡方式"),
    ("UseAndUseAdvisedAgainst",        "推奨用途及び使用上の制限",    "Recommended Use & Restrictions",  "推荐用途及限制",     "推薦用途及限制"),
    ("Use",                            "推奨用途",                    "Recommended Use",                 "推荐用途",           "推薦用途"),
    ("UseAdvisedAgainst",              "使用上の制限",                "Restrictions on Use",             "使用限制",           "使用限制"),
    ("Address",                        "住所",                        "Address",                         "地址",               "地址"),
    ("Phone",                          "電話番号",                    "Telephone",                       "电话",               "電話"),
    ("Fax",                            "FAX番号",                     "Fax",                             "传真",               "傳真"),
    ("Email",                          "メールアドレス",              "Email",                           "电子邮件",           "電子郵件"),
    ("TradeProductIdentity",           "製品識別",                    "Product Identity",                "产品标识",           "產品識別"),
    ("TradeNameJP",                    "商品名（日本語）",             "Trade Name (JP)",                 "商品名(日语)",        "商品名(日文)"),
    ("TradeNameEN",                    "商品名（英語）",               "Trade Name (EN)",                 "商品名(英语)",        "商品名(英文)"),
    ("ItemName",                       "製品名",                      "Item Name",                       "产品名称",           "品項名稱"),
    ("GenericName",                    "一般名",                      "Generic Name",                    "通用名称",           "通用名稱"),
    ("ProductNoUser",                  "製品番号（ユーザー）",        "Product No. (User)",              "用户产品编号",       "使用者產品編號"),
    ("IupacName",                      "IUPAC名",                     "IUPAC Name",                      "IUPAC名称",          "IUPAC名稱"),
    ("MolecularWeight",                "分子量",                      "Molecular Weight",                "分子量",             "分子量"),
    ("Classification",                 "GHS分類",                     "GHS Classification",              "GHS分类",            "GHS分類"),
    ("Category",                       "区分",                         "Category",                        "类别",               "類別"),
    // Classification sub-groups
    ("PhysicochemicalEffect",          "物理化学的危険性",             "Physical/Chemical Hazards",       "物理化学危险",       "物理化學危險性"),
    ("HealthEffect",                   "健康有害性",                   "Health Hazards",                  "健康危害",           "健康有害性"),
    ("EnvironmentalEffect",            "環境有害性",                   "Environmental Hazards",           "环境危害",           "環境有害性"),
    // PhysicochemicalEffect fields
    ("Explosives",                     "爆発物",                       "Explosives",                      "爆炸物",             "爆炸物"),
    ("FlammableGases",                 "引火性ガス",                   "Flammable Gases",                 "易燃气体",           "易燃氣體"),
    ("FlammableAerosols",              "引火性エアゾール",              "Flammable Aerosols",              "易燃气雾剂",         "易燃氣霧劑"),
    ("OxidisingGases",                 "支燃性・酸化性ガス",           "Oxidising Gases",                 "氧化性气体",         "氧化性氣體"),
    ("GasesUnderPressure",             "高圧ガス",                     "Gases Under Pressure",            "加压气体",           "加壓氣體"),
    ("FlammableLiquids",               "引火性液体",                   "Flammable Liquids",               "易燃液体",           "易燃液體"),
    ("FlammableSolids",                "引火性固体",                   "Flammable Solids",                "易燃固体",           "易燃固體"),
    ("SelfreactiveSubstancesAndMixtures","自己反応性物質等",           "Self-reactive Substances",        "自反应物质和混合物", "自反應物質及混合物"),
    ("PyrophoricLiquids",              "自然発火性液体",               "Pyrophoric Liquids",              "自燃液体",           "自燃液體"),
    ("PyrophoricSolids",               "自然発火性固体",               "Pyrophoric Solids",               "自燃固体",           "自燃固體"),
    ("SelfheatingSubstancesAndMixtures","自己発熱性物質等",            "Self-heating Substances",         "自热物质和混合物",   "自熱物質及混合物"),
    ("SubstancesWhichInContactWithWaterEmitFlammableGases","水反応可燃性物質等","Water-reactive Substances","遇水放出易燃气体","遇水放出易燃氣體"),
    ("OxidisingLiquids",               "酸化性液体",                   "Oxidising Liquids",               "氧化性液体",         "氧化性液體"),
    ("OxidisingSolids",                "酸化性固体",                   "Oxidising Solids",                "氧化性固体",         "氧化性固體"),
    ("OrganicPeroxides",               "有機過酸化物",                 "Organic Peroxides",               "有机过氧化物",       "有機過氧化物"),
    ("CorrosiveToMetals",              "金属腐食性物質",               "Corrosive to Metals",             "金属腐蚀物",         "金屬腐蝕物"),
    ("DesensitizedExplosives",         "鈍感化爆発物",                 "Desensitized Explosives",         "脱敏爆炸物",         "鈍感爆炸物"),
    // HealthEffect fields (in Classification)
    ("AcuteToxicityOral",              "急性毒性（経口）",             "Acute Toxicity (Oral)",           "急性毒性（经口）",   "急性毒性（口服）"),
    ("AcuteToxicityDermal",            "急性毒性（皮膚）",             "Acute Toxicity (Dermal)",         "急性毒性（皮肤）",   "急性毒性（皮膚）"),
    ("AcuteToxicityInhalationGas",     "急性毒性（吸入：ガス）",       "Acute Toxicity (Inhal. Gas)",     "急性毒性（吸入：气体）","急性毒性（吸入：氣體）"),
    ("AcuteToxicityInhalationVapour",  "急性毒性（吸入：蒸気）",       "Acute Toxicity (Inhal. Vapour)",  "急性毒性（吸入：蒸气）","急性毒性（吸入：蒸氣）"),
    ("AcuteToxicityInhalationDustOrMist","急性毒性（吸入：粉じん・ミスト）","Acute Toxicity (Inhal. Dust/Mist)","急性毒性（吸入：粉尘/雾）","急性毒性（吸入：粉塵/霧）"),
    ("EyeDamageOrIrritation",          "眼に対する重篤な損傷性・眼刺激性","Eye Damage/Irritation",       "严重眼损伤/刺激",    "眼睛損傷/刺激性"),
    ("RespiratorySensitisation",       "呼吸器感作性",                 "Respiratory Sensitisation",       "呼吸道致敏",         "呼吸道致敏"),
    ("SkinSensitisation",              "皮膚感作性",                   "Skin Sensitisation",              "皮肤致敏",           "皮膚致敏"),
    ("ReproductiveToxicity",           "生殖毒性",                     "Reproductive Toxicity",           "生殖毒性",           "生殖毒性"),
    ("SpecificTargetOrganSE",          "特定標的臓器毒性（単回ばく露）","STOT Single Exposure",           "特异性靶器官毒性（一次接触）","特定標的器官毒性（單次暴露）"),
    ("SpecificTargetOrganRE",          "特定標的臓器毒性（反復ばく露）","STOT Repeated Exposure",         "特异性靶器官毒性（反复接触）","特定標的器官毒性（反覆暴露）"),
    ("AspirationHazard",               "誤えん有害性",                 "Aspiration Hazard",               "吸入危害",           "吸入危害"),
    ("TargetOrgan",                    "標的臓器",                     "Target Organ",                    "靶器官",             "標的器官"),
    ("Lactation",                      "授乳に対する影響",             "Effects on Lactation",            "哺乳影响",           "哺乳影響"),
    // EnvironmentalEffect fields
    ("AquaticToxicityAcute",           "水生環境有害性（急性）",       "Aquatic Toxicity (Acute)",        "水生毒性（急性）",   "水生毒性（急性）"),
    ("AquaticToxicityChronic",         "水生環境有害性（慢性）",       "Aquatic Toxicity (Chronic)",      "水生毒性（慢性）",   "水生毒性（慢性）"),
    ("HazardousOzoneLayer",            "オゾン層への有害性",           "Hazardous to Ozone Layer",        "危害臭氧层",         "危害臭氧層"),
    ("HazardLabelling",                "危険有害性表示",               "Hazard Labelling",                "危险标签",           "危害標示"),
    ("HazardStatementCode",            "危険有害性情報コード",         "Hazard Statement Code",           "危险性说明代码",     "危害警告碼"),
    ("PrecautionaryStatements",        "注意書き",                    "Precautionary Statements",        "防范说明",           "防範說明"),
    ("PrecautionaryStatementCode",     "注意書きコード",              "Precautionary Statement Code",    "防范说明代码",       "防範說明碼"),
    ("Prevention",                     "予防",                        "Prevention",                      "预防",               "預防"),
    ("Response",                       "応急措置",                    "Response",                        "应急响应",           "緊急應變"),
    ("CompositionAndConcentration",    "組成・濃度",                  "Composition and Concentration",   "组成和浓度",         "組成與濃度"),
    ("CompositionType",                "組成の種類",                  "Composition Type",                "组成类型",           "組成類型"),
    ("SubstanceIdentifiers",           "物質識別情報",                "Substance Identifiers",           "物质标识信息",       "物質識別資訊"),
    ("SubstanceIdentity",              "物質識別",                    "Substance Identity",              "物质标识",           "物質識別"),
    ("SubstanceNames",                 "物質名",                      "Substance Names",                 "物质名称",           "物質名稱"),
    ("FirstAidEye",                    "眼への接触",                  "Eye Contact",                     "眼睛接触",           "眼睛接觸"),
    ("FirstAidSkin",                   "皮膚への接触",                "Skin Contact",                    "皮肤接触",           "皮膚接觸"),
    ("FirstAidInhalation",             "吸入した場合",                "Inhalation",                      "吸入",               "吸入"),
    ("FirstAidIngestion",              "飲み込んだ場合",              "Ingestion",                       "食入",               "食入"),
    ("MediaToBeUsed",                  "使用する消火剤",              "Extinguishing Media to Use",      "使用灭火剂",         "使用滅火劑"),
    ("SpecialProtectiveEquipmentForFirefighters", "消火活動者用保護具", "Special Protective Equipment for Firefighters", "消防员特殊防护装备", "消防員特殊防護裝備"),
    ("HumanExposureAndEmergencyMeasuress", "人体への注意事項",       "Human Exposure Precautions",      "人员防护措施",       "人員防護措施"),
    ("EnvironmentalPrecautions",       "環境への注意事項",            "Environmental Precautions",       "环境注意事项",       "環境注意事項"),
    ("ContainmentAndCleaningUp",       "回収・清掃方法",              "Containment and Clean-up",        "泄漏物控制和清除",   "洩漏物控制與清除"),
    ("SafeHandling",                   "安全な取扱い",                "Safe Handling",                   "安全操作",           "安全操作"),
    ("HandlingPrecautions",            "取扱い上の注意",              "Handling Precautions",            "操作注意事项",       "操作注意事項"),
    ("TechnicalMeasuresAndStorageConditions", "技術的措置・保管条件", "Technical Measures and Storage",  "技术措施和储存条件", "技術措施及儲存條件"),
    ("ProtectiveMeasures",             "保護措置",                    "Protective Measures",             "保护措施",           "保護措施"),
    ("VentilationCondition",           "換気条件",                    "Ventilation Conditions",          "通风条件",           "通風條件"),
    ("Storage",                        "保管",                        "Storage",                         "储存",               "儲存"),
    ("ConditionsForSafeStorage",       "安全な保管条件",              "Conditions for Safe Storage",     "安全储存条件",       "安全儲存條件"),
    ("AppropriateEngineeringControls", "設備対策",                    "Appropriate Engineering Controls","工程控制措施",       "工程控制措施"),
    ("PersonalProtectionEquipment",    "個人保護具",                  "Personal Protective Equipment",   "个人防护装备",       "個人防護裝備"),
    ("SkinProtection",                 "皮膚の保護具",                "Skin Protection",                 "皮肤防护",           "皮膚防護"),
    ("BoilingPointRelated",            "沸点関連",                    "Boiling Point",                   "沸点",               "沸點"),
    ("MeltingPointRelated",            "融点関連",                    "Melting Point",                   "熔点",               "熔點"),
    ("Densities",                      "密度",                        "Density",                         "密度",               "密度"),
    ("Solubilities",                   "溶解度",                      "Solubility",                      "溶解性",             "溶解度"),
    ("WaterSolubility",                "水溶解度",                    "Water Solubility",                "水溶性",             "水溶解度"),
    ("OtherSolubility",                "その他の溶解度",              "Other Solubility",                "其他溶解性",         "其他溶解度"),
    ("ReactivityDescription",          "反応性",                      "Reactivity Description",          "反应性描述",         "反應性描述"),
    ("StabilityDescription",           "安定性",                      "Stability Description",           "稳定性描述",         "穩定性描述"),
    ("MaterialsToAvoid",               "避けるべき物質",              "Materials to Avoid",              "禁配物",             "避免接觸物質"),
    ("AdditionalToxicologicalInformation", "追加毒性情報",            "Additional Toxicological Info",   "附加毒理学信息",     "附加毒理資訊"),
    ("EcotoxicologicalInformation",    "生態毒性情報",                "Ecotoxicological Information",    "生态毒理学信息",     "生態毒理資訊"),
    ("AdditionalEcotoxInformation",    "追加生態毒性情報",            "Additional Ecotox Information",   "附加生态毒理学信息", "附加生態毒理資訊"),
    ("BiologicalDegradability",        "生物分解性",                  "Biological Degradability",        "生物降解性",         "生物降解性"),
    ("AbioticDegradation",             "非生物的分解性",              "Abiotic Degradation",             "非生物降解",         "非生物降解"),
    ("PersistenceDegradability",       "残留性・分解性",              "Persistence/Degradability",       "持久性/降解性",      "持久性/降解性"),
    ("Disposal",                       "廃棄",                        "Disposal",                        "废弃处置",           "廢棄處置"),
    ("ProductWaste",                   "製品廃棄物",                  "Product Waste",                   "产品废弃物",         "產品廢棄物"),
    ("PackagingWaste",                 "包装廃棄物",                  "Packaging Waste",                 "包装废弃物",         "包裝廢棄物"),
    ("TransportationType",             "輸送手段",                    "Transportation Type",             "运输方式",           "運輸方式"),
    ("DomesticRegulations",            "国内法規",                    "Domestic Regulations",            "国内法规",           "國內法規"),
    ("InternationalRegulations",       "国際法規",                    "International Regulations",       "国际法规",           "國際法規"),
    ("OtherLegislation",               "その他の法令",                "Other Legislation",               "其他法规",           "其他法規"),
    ("Legislation",                    "法令",                        "Legislation",                     "法规",               "法規"),
    ("LegislationName",                "法令名",                      "Legislation Name",                "法规名称",           "法規名稱"),
    ("RegulationName",                 "規制名",                      "Regulation Name",                 "法规名称",           "規制名稱"),
    ("RelatedDocuments",               "関連文書",                    "Related Documents",               "相关文件",           "相關文件"),
    ("RevisionInformation",            "改訂情報",                    "Revision Information",            "修订信息",           "修訂資訊"),
    ("LastUpdateDate",                 "最終更新日",                  "Last Update Date",                "最后更新日期",       "最後更新日期"),
    ("Version",                        "バージョン",                  "Version",                         "版本",               "版本"),
    ("Desclaimer",                     "免責事項",                    "Disclaimer",                      "免责声明",           "免責聲明"),
    ("ExactValue",                     "値",                          "Value",                           "数值",               "數值"),
    ("Value",                          "値",                          "Value",                           "数值",               "數值"),
];

/// Translate a JSON field key to the display label for the given language.
/// Falls back to the raw key name if no translation is registered.
fn translate_key(key: &str, lang: Language) -> String {
    let li = lang_index(lang);
    for &(k, ja, en, zh_cn, zh_tw) in KEY_LABELS {
        if k == key {
            return match li {
                0 => ja,
                1 => en,
                2 => zh_cn,
                _ => zh_tw,
            }
            .to_string();
        }
    }
    key.to_string()
}

// ---------------------------------------------------------------------------
// DOCX generation
// ---------------------------------------------------------------------------

// Verify the parallel arrays are in sync at compile time.
const _: () = assert!(
    SECTION_NAMES.len() == SECTION_KEYS.len(),
    "SECTION_NAMES and SECTION_KEYS must have the same length"
);

/// Generates a JIS Z 7253-compliant 16-section .docx file from SDS data.
pub fn generate_docx(sds: &SdsRoot, output_path: &Path, lang: Language) -> Result<(), SdsError> {
    let title = DOCUMENT_TITLE[lang_index(lang)];
    let root_val = serde_json::to_value(sds)
        .map_err(|e| SdsError::Docx(format!("serialize error: {e}")))?;

    let mut doc = Docx::new();
    doc = doc.add_paragraph(
        Paragraph::new().add_run(Run::new().add_text(title).bold().size(32)),
    );

    // Datasheet metadata block (date, version — not a numbered section)
    if let Some(ds) = root_val.get("Datasheet") {
        doc = render_object_fields(doc, ds, 0, lang);
    }

    // 16 numbered sections
    for (i, key) in SECTION_KEYS.iter().enumerate() {
        let heading = format!("{}. {}", i + 1, section_name(i, lang));
        doc = doc.add_paragraph(
            Paragraph::new()
                .add_run(Run::new().add_text(heading).bold().size(24)),
        );

        if let Some(val) = root_val.get(*key) {
            match i {
                1 => doc = render_section2_hp_table(doc, val, lang),   // Section 2 H/P codes
                2 => doc = render_section3_composition_table(doc, val, lang), // Section 3 Composition
                8 => doc = render_section9_properties_table(doc, val, lang),  // Section 9 Physical props
                _ => doc = render_value(doc, val, 0, lang),
            }
        }
    }

    let file = std::fs::File::create(output_path)?;
    doc.build()
        .pack(file)
        .map_err(|e| SdsError::Docx(format!("pack failed: {e:?}")))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Recursive JSON-value renderer
// ---------------------------------------------------------------------------

fn render_value(doc: Docx, val: &Value, indent: usize, lang: Language) -> Docx {
    match val {
        Value::Object(_) => render_object_fields(doc, val, indent, lang),
        Value::Array(items) => {
            let mut d = doc;
            // Pure scalar arrays → join as one text block (e.g. FullText: ["line1","line2"])
            let all_scalars = items
                .iter()
                .all(|v| !matches!(v, Value::Object(_) | Value::Array(_)));
            if all_scalars {
                let text = items
                    .iter()
                    .map(value_to_text)
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
                    .join("\n");
                d = add_leaf_multiline(d, &text, indent);
            } else {
                for (i, item) in items.iter().enumerate() {
                    if matches!(item, Value::Object(_)) {
                        if items.len() > 1 {
                            d = d.add_paragraph(
                                Paragraph::new()
                                    .indent(Some(indent_twips(indent)), None, None, None)
                                    .add_run(Run::new().add_text(format!("[{}]", i + 1)).bold()),
                            );
                        }
                        d = render_value(d, item, indent + 1, lang);
                    } else {
                        d = add_leaf_multiline(d, &value_to_text(item), indent);
                    }
                }
            }
            d
        }
        _ => add_leaf_multiline(doc, &value_to_text(val), indent),
    }
}

fn render_object_fields(doc: Docx, val: &Value, indent: usize, lang: Language) -> Docx {
    let Value::Object(map) = val else { return doc };
    let mut d = doc;
    for (key, child) in map {
        // FullText: render content as plain paragraphs, no label
        if key == "FullText" {
            let text = value_to_text(child);
            if !text.is_empty() {
                d = add_leaf_multiline(d, &text, indent);
            }
            continue;
        }

        // AdditionalInfo: render FullText content as plain paragraphs, no label
        if key == "AdditionalInfo" {
            if let Some(full_text) = child.get("FullText") {
                let text = value_to_text(full_text);
                if !text.is_empty() {
                    d = add_leaf_multiline(d, &text, indent);
                }
            }
            continue;
        }

        // Transparent wrapper keys: render children directly without a label
        if TRANSPARENT_KEYS.iter().any(|&k| k == key.as_str()) {
            d = render_value(d, child, indent, lang);
            continue;
        }

        match child {
            Value::Object(_) => {
                let label = translate_key(key, lang);
                d = add_label(d, &label, indent);
                d = render_value(d, child, indent + 1, lang);
            }
            Value::Array(items) if items.is_empty() => {}
            Value::Array(_) => {
                let label = translate_key(key, lang);
                d = add_label(d, &label, indent);
                d = render_value(d, child, indent + 1, lang);
            }
            Value::Null => {}
            leaf => {
                let label = translate_key(key, lang);
                let text = value_to_text(leaf);
                d = add_label_value_multiline(d, &label, &text, indent);
            }
        }
    }
    d
}

// ---------------------------------------------------------------------------
// Text conversion
// ---------------------------------------------------------------------------

fn value_to_text(val: &Value) -> String {
    match val {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        // Arrays are joined with newlines so FullText arrays render as readable text
        Value::Array(items) => items
            .iter()
            .map(value_to_text)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("\n"),
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

// ---------------------------------------------------------------------------
// Paragraph builders
// ---------------------------------------------------------------------------

fn indent_twips(indent: usize) -> i32 {
    (indent as i32) * 360 // 360 twips ≈ 0.25 inch per level
}

fn add_label(doc: Docx, label: &str, indent: usize) -> Docx {
    doc.add_paragraph(
        Paragraph::new()
            .indent(Some(indent_twips(indent)), None, None, None)
            .add_run(Run::new().add_text(label).bold()),
    )
}

/// Render multi-line text as separate paragraphs (split on `\n`).
fn add_leaf_multiline(doc: Docx, text: &str, indent: usize) -> Docx {
    let mut d = doc;
    for line in text.split('\n') {
        let line = line.trim_end_matches('\r').trim();
        if !line.is_empty() {
            d = d.add_paragraph(
                Paragraph::new()
                    .indent(Some(indent_twips(indent)), None, None, None)
                    .add_run(Run::new().add_text(line)),
            );
        }
    }
    d
}

/// Render `label: value` on one paragraph; continuation lines are indented one extra level.
fn add_label_value_multiline(doc: Docx, label: &str, value: &str, indent: usize) -> Docx {
    if value.is_empty() {
        return doc;
    }
    let lines: Vec<&str> = value
        .split('\n')
        .map(|l| l.trim_end_matches('\r').trim())
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return doc;
    }
    // First line: bold label + value on the same paragraph
    let mut d = doc.add_paragraph(
        Paragraph::new()
            .indent(Some(indent_twips(indent)), None, None, None)
            .add_run(Run::new().add_text(format!("{label}: ")).bold())
            .add_run(Run::new().add_text(lines[0])),
    );
    // Remaining lines: continuation paragraphs with one extra indent level
    for line in &lines[1..] {
        d = d.add_paragraph(
            Paragraph::new()
                .indent(Some(indent_twips(indent + 1)), None, None, None)
                .add_run(Run::new().add_text(*line)),
        );
    }
    d
}

// ---------------------------------------------------------------------------
// Table helpers
// ---------------------------------------------------------------------------

/// Page usable width in twips (A4 with 2.5 cm margins ≈ 8505 twips).
const PAGE_WIDTH: usize = 8505;

fn cell_text(text: &str, width: usize) -> TableCell {
    TableCell::new()
        .width(width, WidthType::Dxa)
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text(text)))
}

fn cell_bold(text: &str, width: usize) -> TableCell {
    TableCell::new()
        .width(width, WidthType::Dxa)
        .shading(Shading::new().fill("D9D9D9"))
        .add_paragraph(Paragraph::new().add_run(Run::new().add_text(text).bold()))
}

// ---------------------------------------------------------------------------
// Section 2: H/P code 2-column tables
// ---------------------------------------------------------------------------

fn render_section2_hp_table(doc: Docx, val: &Value, lang: Language) -> Docx {
    let mut d = doc;

    // Render Classification and other top-level fields as normal
    if let Value::Object(map) = val {
        for (key, child) in map {
            if key == "HazardLabelling" {
                d = render_hazard_labelling_tables(d, child, lang);
            } else {
                {
                    let mut m = serde_json::Map::new();
                    m.insert(key.clone(), child.clone());
                    d = render_value(d, &Value::Object(m), 0, lang);
                }
            }
        }
    }
    d
}

fn render_hazard_labelling_tables(doc: Docx, val: &Value, lang: Language) -> Docx {
    let mut d = doc;

    // Signal word and pictograms as plain text
    for key in &["SignalWord", "HazardPictogram"] {
        if let Some(v) = val.get(key) {
            let label = translate_key(key, lang);
            d = add_label_value_multiline(d, &label, &value_to_text(v), 0);
        }
    }

    let (col_code, col_desc) = match lang {
        Language::Japanese          => ("コード", "説明"),
        Language::English           => ("Code", "Description"),
        Language::ChineseSimplified => ("代码", "说明"),
        Language::ChineseTraditional=> ("代碼", "說明"),
    };

    // H-code table
    if let Some(stmts) = val.get("HazardStatement").and_then(|v| v.as_array()) {
        let h_label = translate_key("HazardStatement", lang);
        d = add_label(d, &h_label, 0);
        let w_code = 1200usize;
        let w_desc = PAGE_WIDTH - w_code;
        let header = TableRow::new(vec![cell_bold(col_code, w_code), cell_bold(col_desc, w_desc)]);
        let rows: Vec<TableRow> = std::iter::once(header).chain(stmts.iter().map(|s| {
            let code = s.get("HazardStatementCode").map(value_to_text).unwrap_or_default();
            let desc = s.get("FullText").map(value_to_text).unwrap_or_default();
            TableRow::new(vec![cell_text(&code, w_code), cell_text(&desc, w_desc)])
        })).collect();
        d = d.add_table(Table::new(rows));
    }

    // P-code tables by category
    if let Some(ps) = val.get("PrecautionaryStatements") {
        let p_label = translate_key("PrecautionaryStatements", lang);
        d = add_label(d, &p_label, 0);
        let w_code = 1200usize;
        let w_desc = PAGE_WIDTH - w_code;
        for cat in &["Prevention", "Response", "Storage", "Disposal"] {
            if let Some(stmts) = ps.get(cat).and_then(|v| v.as_array()) {
                if stmts.is_empty() { continue; }
                let cat_label = translate_key(cat, lang);
                d = add_label(d, &cat_label, 1);
                let header = TableRow::new(vec![cell_bold(col_code, w_code), cell_bold(col_desc, w_desc)]);
                let rows: Vec<TableRow> = std::iter::once(header).chain(stmts.iter().map(|s| {
                    let code = s.get("PrecautionaryStatementCode").map(value_to_text).unwrap_or_default();
                    let desc = s.get("FullText").map(value_to_text).unwrap_or_default();
                    TableRow::new(vec![cell_text(&code, w_code), cell_text(&desc, w_desc)])
                })).collect();
                d = d.add_table(Table::new(rows));
            }
        }
    }

    // AdditionalInfo
    if let Some(ai) = val.get("AdditionalInfo").and_then(|v| v.get("FullText")) {
        let text = value_to_text(ai);
        if !text.is_empty() { d = add_leaf_multiline(d, &text, 0); }
    }
    d
}

// ---------------------------------------------------------------------------
// Section 3: Composition 4-column table
// ---------------------------------------------------------------------------

fn render_section3_composition_table(doc: Docx, val: &Value, lang: Language) -> Docx {
    let mut d = doc;

    // CompositionType and other metadata as plain text
    if let Value::Object(map) = val {
        for (key, child) in map {
            if key == "CompositionAndConcentration" {
                d = render_composition_table(d, child, lang);
            } else {
                let label = translate_key(key, lang);
                d = add_label_value_multiline(d, &label, &value_to_text(child), 0);
            }
        }
    }
    d
}

fn render_composition_table(doc: Docx, val: &Value, lang: Language) -> Docx {
    let items = match val.as_array() {
        Some(a) if !a.is_empty() => a,
        _ => return render_value(doc, val, 0, lang),
    };

    let (h_name, h_cas, h_conc, h_note) = match lang {
        Language::Japanese           => ("成分名", "CAS番号", "濃度", "備考"),
        Language::English            => ("Substance Name", "CAS No.", "Concentration", "Note"),
        Language::ChineseSimplified  => ("成分名", "CAS号", "浓度", "备注"),
        Language::ChineseTraditional => ("成分名", "CAS號", "濃度", "備註"),
    };

    let w = [3000usize, 1800, 1200, PAGE_WIDTH - 3000 - 1800 - 1200];
    let header = TableRow::new(vec![
        cell_bold(h_name, w[0]), cell_bold(h_cas, w[1]),
        cell_bold(h_conc, w[2]), cell_bold(h_note, w[3]),
    ]);

    let mut rows = vec![header];
    for item in items {
        let ids = item.get("SubstanceIdentifiers");

        // Substance name: IupacName or GenericName or CommonName
        let name = ids.and_then(|i| i.get("SubstanceNames")).and_then(|n| {
            n.get("IupacName").or_else(|| n.get("GenericName"))
        }).map(value_to_text)
        .or_else(|| ids.and_then(|i| i.get("CommonName")).and_then(|c| c.get("OtherName")).and_then(|a| a.as_array()).and_then(|a| a.first()).map(value_to_text))
        .unwrap_or_default();

        // CAS number
        let cas = ids.and_then(|i| i.get("SubstanceIdentity")).and_then(|si| si.get("CASno")).and_then(|c| c.get("FullText")).map(value_to_text).unwrap_or_default();

        // Concentration
        let conc = item.get("Concentration").map(|c| {
            if let Some(nr) = c.get("NumericRangeWithUnitAndQualifier") {
                let unit = nr.get("Unit").map(value_to_text).unwrap_or_default();
                let val = nr.get("ExactValue").and_then(|v| v.get("Value")).map(value_to_text)
                    .or_else(|| {
                        let lo = nr.get("LowerValue").and_then(|v| v.get("Value")).map(value_to_text).unwrap_or_default();
                        let hi = nr.get("UpperValue").and_then(|v| v.get("Value")).map(value_to_text).unwrap_or_default();
                        if lo.is_empty() && hi.is_empty() { None }
                        else { Some(format!("{lo}–{hi}")) }
                    }).unwrap_or_default();
                format!("{val} {unit}").trim().to_string()
            } else {
                value_to_text(c)
            }
        }).unwrap_or_default();

        // Note / AdditionalInfo
        let note = item.get("AdditionalInfo").and_then(|ai| ai.get("FullText")).map(value_to_text).unwrap_or_default();

        rows.push(TableRow::new(vec![
            cell_text(&name, w[0]), cell_text(&cas, w[1]),
            cell_text(&conc, w[2]), cell_text(&note, w[3]),
        ]));
    }

    doc.add_table(Table::new(rows))
}

// ---------------------------------------------------------------------------
// Section 9: Physical/Chemical properties 2-column table
// ---------------------------------------------------------------------------

fn render_section9_properties_table(doc: Docx, val: &Value, lang: Language) -> Docx {
    let mut d = doc;
    if let Value::Object(map) = val {
        for (key, child) in map {
            if key == "BasePhysicalChemicalProperties" {
                d = render_properties_table(d, child, lang);
            } else {
                {
                    let mut m = serde_json::Map::new();
                    m.insert(key.clone(), child.clone());
                    d = render_value(d, &Value::Object(m), 0, lang);
                }
            }
        }
    }
    d
}

fn render_properties_table(doc: Docx, val: &Value, lang: Language) -> Docx {
    let Value::Object(map) = val else { return render_value(doc, val, 0, lang) };
    if map.is_empty() { return doc; }

    let (col_prop, col_val) = match lang {
        Language::Japanese           => ("項目", "値"),
        Language::English            => ("Property", "Value"),
        Language::ChineseSimplified  => ("项目", "值"),
        Language::ChineseTraditional => ("項目", "值"),
    };

    let w_prop = 3000usize;
    let w_val  = PAGE_WIDTH - w_prop;

    let header = TableRow::new(vec![cell_bold(col_prop, w_prop), cell_bold(col_val, w_val)]);
    let mut rows = vec![header];

    for (key, child) in map {
        let label = translate_key(key, lang);
        let text = extract_property_text(child);
        if text.is_empty() { continue; }
        rows.push(TableRow::new(vec![cell_text(&label, w_prop), cell_text(&text, w_val)]));
    }

    if rows.len() == 1 { return doc; } // only header, no data
    doc.add_table(Table::new(rows))
}

/// Extract a readable text value from a physical property node.
/// Handles NumericRangeWithUnitAndQualifier, AdditionalInfo, and plain strings.
fn extract_property_text(val: &Value) -> String {
    match val {
        Value::Object(map) => {
            // NumericRangeWithUnitAndQualifier
            if let Some(nr) = map.get("NumericRangeWithUnitAndQualifier") {
                let unit = nr.get("Unit").map(value_to_text).unwrap_or_default();
                let num = nr.get("ExactValue").and_then(|v| v.get("Value")).map(value_to_text)
                    .or_else(|| {
                        let lo = nr.get("LowerValue").and_then(|v| v.get("Value")).map(value_to_text).unwrap_or_default();
                        let hi = nr.get("UpperValue").and_then(|v| v.get("Value")).map(value_to_text).unwrap_or_default();
                        if lo.is_empty() && hi.is_empty() { None }
                        else { Some(format!("{lo}–{hi}")) }
                    }).unwrap_or_default();
                let qual = nr.get("Qualifier").map(value_to_text).unwrap_or_default();
                let text = format!("{qual} {num} {unit}").trim().to_string();
                if !text.is_empty() { return text; }
            }
            // AdditionalInfo.FullText
            if let Some(ai) = map.get("AdditionalInfo").and_then(|a| a.get("FullText")) {
                return value_to_text(ai);
            }
            // FullText directly
            if let Some(ft) = map.get("FullText") {
                return value_to_text(ft);
            }
            // Fallback: collect all scalar values
            map.values().map(value_to_text).filter(|s| !s.is_empty()).collect::<Vec<_>>().join("; ")
        }
        _ => value_to_text(val),
    }
}
