# sds-converter-core

用于**双向转换**安全数据表（SDS）文档（Word/PDF）与日本厚生劳动省（MHLW）标准JSON格式的Rust库。

支持**日语、英语、简体中文、繁体中文**的SDS文档处理。

> **需要命令行工具？** 请安装 [`sds-converter`](https://crates.io/crates/sds-converter)。

---

## 功能特点

- **SDS文档 → JSON**: 从PDF/DOCX/XLSX/TXT中提取文本，并转换为符合MHLW SDS数据交换标准格式v1.0的JSON。支持并行提取与自动重试。
- **JSON → DOCX**: 从标准JSON生成符合JIS Z 7253规范的16节Word文档，支持多语言节标题。
- **多语言支持**: 支持 `ja` / `en` / `zh-CN` / `zh-TW` 的输入和输出。
- **可扩展LLM后端**: 内置Anthropic Claude、OpenAI GPT、Google Gemini、Mistral、Groq、Cohere实现。通过实现 `LlmBackend` trait可接入任意LLM。
- **SSRF防护**: URL抓取自动拒绝私有/回环/链路本地/元数据IP地址；禁用重定向；完整IPv6覆盖（`fc00::/7` ULA、`fe80::/10` 链路本地、`::ffff:` IPv4映射地址）
- **HTML/URL输入支持**: 支持 `.html`/`.htm` 文件和 `http(s)://` URL作为输入
- **GHS/CAS验证**: 依据GHS Rev.10验证H码（H200–H420）和P码（P101–P503），CAS编号格式及校验位验证，支持PubChem富集（`enrich_composition`）
- **健壮的JSON修复**: 具有字符串上下文感知的尾随逗号删除——保留 `"ends here,}"` 等字符串值不受损坏

---

## 安装

```toml
[dependencies]
sds-converter-core = "0.3"
```

---

## 库使用方法

### 将SDS文档转换为JSON（Anthropic Claude）

```rust
use sds_converter_core::{
    AnthropicBackend, LlmConfig,
    convert_to_json, ConvertConfig, Language,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let backend = AnthropicBackend::new(
        std::env::var("ANTHROPIC_API_KEY")?,
        LlmConfig::default(),
    );

    let config = ConvertConfig {
        source_language: Some(Language::ChineseSimplified),
        output_language: Language::ChineseSimplified,
        ..Default::default()
    };

    let (sds, warnings) = convert_to_json(std::path::Path::new("input.pdf"), &backend, &config).await?;
    for w in &warnings { eprintln!("WARN: {w}"); }
    std::fs::write("output.json", serde_json::to_string_pretty(&sds)?)?;
    Ok(())
}
```

### 将JSON转换为Word文档

```rust
use sds_converter_core::{convert_from_json, ConvertConfig, Language, SdsRoot};

fn main() -> anyhow::Result<()> {
    let json = std::fs::read_to_string("output.json")?;
    let sds: SdsRoot = serde_json::from_str(&json)?;

    let config = ConvertConfig {
        source_language: None,
        output_language: Language::ChineseSimplified,
        ..Default::default()
    };

    convert_from_json(&sds, std::path::Path::new("result.docx"), &config)?;
    Ok(())
}
```

### OpenAI GPT / Google Gemini 后端

```rust
use sds_converter_core::{OpenAiCompatBackend, LlmConfig};

// OpenAI GPT
let config = LlmConfig { model: "gpt-4o-mini".into(), max_tokens: 8192 };
let backend = OpenAiCompatBackend::openai(std::env::var("OPENAI_API_KEY")?, config);

// Google Gemini
let config = LlmConfig { model: "gemini-2.0-flash".into(), max_tokens: 8192 };
let backend = OpenAiCompatBackend::gemini(std::env::var("GEMINI_API_KEY")?, config);

// 任意OpenAI兼容端点（Ollama等本地LLM）
let backend = OpenAiCompatBackend::new(
    "api-key",
    LlmConfig::default(),
    "https://your-endpoint/v1/chat/completions",
);
```

### 从文档中提取原始文本

无需调用LLM即可从PDF/DOCX/XLSX中提取文本。可用于构建自定义处理流程或检查LLM接收的输入内容。

```rust
use sds_converter_core::extract_text;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let text = extract_text(std::path::Path::new("input.pdf")).await?;
    println!("{text}");
    Ok(())
}
```

支持的扩展名：`.pdf`、`.docx`、`.xlsx`、`.txt`

### 验证SdsRoot的结构完整性

`validate` 检查 `SdsRoot` 的结构完整性并返回警告消息列表。不会中断执行——部分提取结果仍可使用。

```rust
use sds_converter_core::{validate, SdsRoot};

fn main() -> anyhow::Result<()> {
    let json = std::fs::read_to_string("output.json")?;
    let sds: SdsRoot = serde_json::from_str(&json)?;
    let warnings = validate(&sds);
    if warnings.is_empty() {
        println!("OK");
    } else {
        for w in &warnings { eprintln!("WARN: {w}"); }
    }
    Ok(())
}
```

### 自定义LLM后端

实现 `LlmBackend` trait即可接入任意LLM提供商：

```rust
use sds_converter_core::{LlmBackend, SdsError};

struct MyLlmBackend { /* ... */ }

impl LlmBackend for MyLlmBackend {
    async fn complete(&self, system: &str, user: &str) -> Result<String, SdsError> {
        // 调用LLM API并返回原始JSON字符串
        todo!()
    }
}
```

---

## JSON格式

输出JSON符合**厚生劳动省SDS数据交换标准格式v1.0**（2025年3月31日发布）。

涵盖JIS Z 7253全16节，约200个结构化字段。

```json
{
  "Datasheet": {
    "IssueDate": "2024-03-31",
    "SDS-SchemaVersionNo": "1.0"
  },
  "Identification": {
    "TradeProductIdentity": {
      "TradeNameJP": "示例产品"
    },
    "SupplierInformation": {
      "CompanyName": "示例化学株式会社",
      "Phone": "03-0000-0000"
    }
  }
}
```

---

## 语言支持

| 语言 | `source_language` / `output_language` | 源文档标准 | 输出DOCX标题 |
|---|---|---|---|
| 日语 | `Language::Japanese` | JIS Z 7253 | JIS Z 7253 |
| 英语 | `Language::English` | GHS/OSHA HazCom | GHS Rev.10 / ISO 11014 |
| 简体中文 | `Language::ChineseSimplified` | GB/T 16483 | GB/T 16483-2012 |
| 繁体中文 | `Language::ChineseTraditional` | CNS 15030 | CNS 15030 |

---

## 运行要求

- Rust 1.75及以上
- LLM API密钥（仅 `convert_to_json` 时需要）
  - Anthropic: [获取API密钥](https://console.anthropic.com/)
  - OpenAI: [获取API密钥](https://platform.openai.com/)
  - Google Gemini: [获取API密钥](https://aistudio.google.com/)
- 输入文件须为基于文本的PDF/DOCX/XLSX/TXT
  - 不支持加密PDF（文本提取将失败）
  - CID字体/Shift-JIS编码PDF（日语文档常见）：通过 `pdftotext`（poppler）回退处理
  - 扫描图像PDF：若已安装 `pdftoppm` + `tesseract` 则自动OCR重试，或使用Claude Vision API（使用Anthropic提供商时）
  - PDF三级回退：`pdf-extract` -> `pdftotext` -> OCR/Vision

---

## 更新日志

### 0.3.6 已完成
- [x] QC r24：新增5条规则（S1-ZH-NO-EMERGENCY、S7-FLAMMABLE-STORAGE-TEMP、S8-NO-ENG-CONTROLS、S10-NO-INCOMPATIBLE、CROSS-STALE-DATE）
- [x] QC r24：S8-OEL-NO-NUMERIC 误报修复 — 中文"单位→数值"格式识别、新增"无需OEL"豁免短语
- [x] QC r24：S5-EMPTY 阈值 30→15 字符（减少中文简短灭火信息的误报）
- [x] 循环测试：修复 JSONL 解析及验证器字符串数组处理；r24 基线 30/30 成功，CRIT=0、HIGH=9、MED=176
- [x] QC r25：修复 S2-EXPLOSIVE-NO-GHS01 / S2-ENV-NO-GHS09 漏报（日期/H码中"01"/"09"子串误跳过）；新增 S3-NAME-IS-CAS（HIGH）、S16-REVISION-BEFORE-ISSUE（HIGH）
- [x] 循环测试 r25 基线：30/30 成功，CRIT=0、HIGH=13、MED=175
- [x] QC r26：S2-FLAMMABLE-NO-GHS02、S2-CORROSIVE-NO-GHS05、S2-ACUTETOX-NO-GHS06（均为 MED）— 易燃、腐蚀性、急性毒性 Cat 1–3 象形图一致性检查；S4-H314-NO-REMOVE-CLOTHING（MED）— P361 脱除污染衣物合规
- [x] 循环测试 r26 基线：30/30 成功，CRIT=0、HIGH=14、MED=181
- [x] LLM提示词：第1节Use回退 — 第1.2节存在但无具体用途时，将原文（如`'无相关详细资料'`）存入Use数组
- [x] LLM提示词：第8节OEL"不要求"检测 — `不要求`/`无需监控`/`不适用`等表述存入`AdditionalInfo.FullText`（不再省略）
- [x] LLM提示词：第9节Densities必须提取；易燃/挥发性产品（H224/H225/H226/H330–H332）提取VapourPressure
- [x] LLM提示词：第12节存在持续性/降解性子节时，必须填充`PersistenceDegradability.BiologicalDegradability`

---

## 参考链接

- [厚生劳动省 — SDS信息交换标准格式发布页面](https://www.mhlw.go.jp/stf/newpage_56484.html)（日语）
- [SDS数据交换格式开发者手册（PDF）](https://www.mhlw.go.jp/content/11305000/001467068.pdf)（日语）

---

## 许可证

以下两种许可证任选其一：
- Apache License, Version 2.0
- MIT License
