# sds-converter

用于**双向转换**安全数据表（SDS）文档（Word/PDF）与日本厚生劳动省（MHLW）标准JSON格式的GUI + CLI工具。

支持**日语、英语、简体中文、繁体中文**的SDS文档处理。

[English](README.md) | [日本語](README_ja.md)

> **嵌入Rust项目？** 请直接使用 [`sds-converter-core`](https://crates.io/crates/sds-converter-core)。

---

## 下载

| 平台 | 下载 |
|---|---|
| **macOS**（通用版 — Apple Silicon + Intel） | [sds-converter-macos.zip](https://github.com/kent-tokyo/sds-converter/releases/latest/download/sds-converter-macos.zip) |
| **Windows**（便携版 .exe — 无需安装） | [sds-converter-windows-portable.zip](https://github.com/kent-tokyo/sds-converter/releases/latest/download/sds-converter-windows-portable.zip) |
| **Rust / CLI** | `cargo install sds-converter` |

→ [全部版本与更新日志](https://github.com/kent-tokyo/sds-converter/releases)

---

## GUI模式

无参数运行 `sds-converter` 即可启动图形界面：

```bash
sds-converter
```

将打开一个包含五个标签页的窗口（820×640）：

| 标签页 | 功能 |
|---|---|
| **转换** | SDS文档（PDF/DOCX/XLSX/HTML/URL）→ MHLW标准JSON |
| **文档生成** | MHLW JSON → DOCX / HTML / PDF（支持DOCX模板） |
| **验证** | MHLW JSON结构验证（OK/警告/错误彩色显示） |
| **文本提取** | 从文档提取原始文本（无需LLM API） |
| **设置** | API密钥、模型名称、Base URL、质量、语言、界面语言 |

| 转换标签页 | 文档生成标签页 | 文本提取标签页 |
|---|---|---|
| ![转换标签页](../docs/tab_convert.png) | ![文档生成标签页](../docs/tab_generate.png) | ![文本提取标签页](../docs/tab_extract.png) |

将文件拖放至任意标签页可自动填充输入字段。

设置保存至 `~/.config/sds-converter/config.toml`，下次启动时自动恢复。
GUI与CLI共用相同的转换引擎（`tasks.rs`），转换结果完全一致。

---

## 命令

### `to-json` — PDF/Word → MHLW标准JSON

```bash
# 单文件（默认使用Anthropic Claude）
export ANTHROPIC_API_KEY=sk-ant-...
sds-converter to-json --input input.pdf --output output.json

# 指定源文档语言
sds-converter to-json --input sds_en.pdf --output output.json --lang en

# 批量模式 — 处理整个目录
sds-converter to-json --input-dir ./pdfs/ --output-dir ./json/ --lang ja

# OpenAI GPT（默认: gpt-4o-mini）
sds-converter to-json --input input.pdf --output output.json \
  --provider openai --api-key $OPENAI_API_KEY

# Google Gemini（默认: gemini-2.0-flash）
sds-converter to-json --input input.pdf --output output.json \
  --provider gemini --api-key $GEMINI_API_KEY

# 本地LLM（Ollama等，OpenAI兼容端点）
sds-converter to-json --input input.pdf --output output.json \
  --provider local --base-url http://localhost:11434/v1 \
  --model llama3.2 --api-key dummy

# 从已提取的文本转换（跳过PDF解析）
sds-converter to-json --input extracted.txt --output output.json --lang ja
```

| 参数 | 默认值 | 说明 |
|---|---|---|
| `--input` | — | 输入文件（PDF / DOCX / XLSX / TXT） |
| `--input-dir` | — | 输入目录（批量：处理 `.pdf`/`.docx`/`.xlsx`/`.xls`） |
| `--output` | — | 输出JSON文件 |
| `--output-dir` | — | 输出目录（批量：不存在时自动创建） |
| `--provider` | `anthropic` | LLM提供商：`anthropic` / `openai` / `gemini` / `mistral` / `groq` / `cohere` / `local` |
| `--api-key` | 环境变量 | API密钥（参见下方各提供商默认值） |
| `--model` | 各提供商默认 | 覆盖模型名称 |
| `--base-url` | — | OpenAI兼容端点（用于 `--provider local`） |
| `--lang` | 自动检测 | 源文档语言：`ja` / `en` / `zh-cn` / `zh-tw` |
| `--quality` | `medium` | 预设：`low`（快速/低成本）/ `medium` / `high`（高精度） |
| `--concurrency` | `4` | 批量模式的最大并发数 |
| `--suggested-name` | — | 将输出文件重命名为 `SDS_<发行日>_<品号>.json`（符合MHLW §2.1.2推荐命名规范） |

**各提供商默认值：**

| `--provider` | 默认模型 | 环境变量 |
|---|---|---|
| `anthropic` | `claude-haiku-4-5-20251001`（low/medium）· `claude-sonnet-4-6`（high） | `ANTHROPIC_API_KEY` |
| `openai` | `gpt-4o-mini` | `OPENAI_API_KEY` |
| `gemini` | `gemini-2.0-flash` | `GEMINI_API_KEY` |
| `mistral` | `mistral-small-latest` | `MISTRAL_API_KEY` |
| `groq` | `llama-3.3-70b-versatile` | `GROQ_API_KEY` |
| `cohere` | `command-r-plus` | `COHERE_API_KEY` |
| `local` | `llama3` | `LOCAL_LLM_API_KEY`（可选，默认为 `ollama`） |

### `to-docx` — MHLW标准JSON → Word文档

```bash
# 单文件（内置布局）
sds-converter to-docx --input output.json --output result.docx --lang ja

# 批量模式（内置布局）
sds-converter to-docx --input-dir ./json/ --output-dir ./docx/ --lang en

# 填充Word模板（{{占位符}}替换）
sds-converter to-docx --input output.json --output result.docx \
  --template my_template.docx

# 批量模式 + 模板
sds-converter to-docx --input-dir ./json/ --output-dir ./docx/ \
  --template my_template.docx
```

#### Word模板格式

在 `.docx` 文件中使用 `{{字段名}}` 占位符，`字段名` 为MHLW JSON模式中的叶节点键名。也支持完整的点路径形式。

```
{{TradeNameJP}}          → 产品和名
{{CompanyName}}          → 公司名称
{{Phone}}                → 电话号码
{{IssueDate}}            → 发行日期
{{Identification.SupplierInformation.CompanyName}}  → 完整路径指定
```

占位符可出现在文档任意位置——段落、表格单元格、页眉和页脚均可。即使Word将文本分割为多个内部run，工具也会在替换前自动合并。

| 参数 | 默认值 | 说明 |
|---|---|---|
| `--input` | — | 输入JSON文件 |
| `--input-dir` | — | 输入目录（批量：处理 `.json`） |
| `--output` | — | 输出DOCX文件 |
| `--output-dir` | — | 输出目录（批量） |
| `--lang` | `ja` | 输出语言：`ja` / `en` / `zh-cn` / `zh-tw`（不使用 `--template` 时） |
| `--template` | — | 含 `{{字段名}}` 占位符的Word模板 |

### `extract-text` — 从PDF/DOCX提取原始文本

在不调用API的情况下提取LLM将接收的文本，用于检查提取质量或单独执行LLM步骤。

```bash
# 保存到文件
sds-converter extract-text --input input.pdf --output extracted.txt

# 输出到标准输出
sds-converter extract-text --input input.pdf

# 将提取结果传给to-json
sds-converter to-json --input extracted.txt --output output.json --lang ja
```

### `validate` — 检查JSON文件的结构问题

```bash
# 人类可读输出（退出码 0=正常，1=发现警告）
sds-converter validate --input output.json

# JSON数组输出（用于CI/脚本）
sds-converter validate --input output.json --json
```

检查关键节（Identification、HazardIdentification、ToxicologicalInformation等）是否已填充。发现问题时以退出码 `1` 退出。

---

## 语言支持

| 语言 | `--lang` | 源文档 | 输出DOCX标题 |
|---|---|---|---|
| 日语 | `ja` | JIS Z 7253标准SDS | JIS Z 7253 |
| 英语 | `en` | GHS/OSHA HazCom格式 | GHS Rev.10 / ISO 11014 |
| 简体中文 | `zh-cn` | GB/T 16483格式 | GB/T 16483-2012 |
| 繁体中文 | `zh-tw` | CNS 15030格式 | CNS 15030 |

---

## 环境要求

- Rust 1.75+
- LLM API密钥（仅 `to-json` 需要）— 设置提供商环境变量或通过 `--api-key` 传入
  - Anthropic: `ANTHROPIC_API_KEY`
  - OpenAI: `OPENAI_API_KEY`
  - Google Gemini: `GEMINI_API_KEY`
  - Mistral: `MISTRAL_API_KEY`
  - Groq: `GROQ_API_KEY`
  - Cohere: `COHERE_API_KEY`
  - 本地LLM（Ollama等）: 使用 `--provider local --base-url <url>`（无需API密钥）
- 输入文件必须是**基于文本**的PDF或DOCX
  - 不支持加密PDF
  - CID字体/Shift-JIS编码PDF（日语文档常见）：通过 `pdftotext`（poppler）回退处理
  - 扫描图像PDF：若已安装 `pdftoppm` + `tesseract` 则自动OCR重试，或使用Claude Vision API（`--provider anthropic` 时）
  - PDF三级回退：`pdf-extract` -> `pdftotext` -> OCR/Vision

---

## 更新日志

### 0.3.6 / 0.2.6 已完成
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
