# sds-converter

用于**双向转换**安全数据表（SDS）文档（Word/PDF）与日本厚生劳动省（MHLW）标准JSON格式的Rust工作区。

支持**日语、英语、简体中文、繁体中文**的SDS文档处理。

---

## 包结构

| 包 | 说明 |
|---|---|
| [`sds-converter-core`](./sds_converter_core/) | Rust库 — 基于LLM的提取、DOCX生成、MHLW模式 |
| [`sds-converter`](./sds_converter/) | CLI工具 — `to-json`、`to-docx`、`validate`、`extract-text` 子命令 |

---

## 功能特点

- **SDS文档 → JSON**: 从PDF/DOCX/XLSX/TXT中提取文本，并转换为符合MHLW SDS数据交换标准格式v1.0的JSON。支持并行提取与自动重试。
- **JSON → DOCX**: 从标准JSON生成符合JIS Z 7253规范的16节Word文档，支持多语言节标题。
- **多语言支持**: 支持 `ja` / `en` / `zh-CN` / `zh-TW` 的输入和输出。
- **可扩展LLM后端**: 内置Anthropic Claude、OpenAI GPT、Google Gemini、Mistral、Groq、Cohere实现。通过实现 `LlmBackend` trait可接入任意LLM。
- **库 + CLI**: 可作为Rust库嵌入使用，也可作为独立命令行工具使用。

---

## 为何使用LLM？

SDS文档是**非结构化的自然语言文本**，而非电子表格。即使遵循同一标准，不同文档之间也存在以下差异：

- **章节顺序不同** — 各厂商对16节的排列顺序各有不同
- **表述方式多样** — 同一数据可能写作"≥99.5%"、"99.5%以上"或"含量约100%"等不同形式
- **标题名称各异** — JIS Z 7253、GHS/OSHA HazCom、GB/T 16483、CNS 15030对同一概念使用不同标签
- **多语言混用** — 日语SDS中常混有英语化学品名和CAS编号

MHLW标准JSON格式包含**约200个深度嵌套的字段**。为每种文档格式编写基于规则的解析器几乎不可行。LLM能像人类一样阅读文档，无论格式如何，都能将自由文本映射到正确的模式字段，并原生支持多语言文档。

通过`LlmBackend` trait，LLM后端可灵活替换，支持Claude、GPT-4o、Gemini或未来的任何新模型。

---

## 快速开始

```bash
# 安装CLI工具
cargo install sds-converter

# PDF → MHLW标准JSON
export ANTHROPIC_API_KEY=sk-ant-...
sds-converter to-json --input input.pdf --output output.json

# JSON → Word文档
sds-converter to-docx --input output.json --output result.docx --lang zh-cn
```

完整CLI参考请查看 [`sds-converter` README](./sds_converter/README.md)，库API请查看 [`sds-converter-core` README](./sds_converter_core/README.md)。

---

## 语言支持

| 语言 | `--lang` | 源文档格式 | 输出DOCX标题 |
|---|---|---|---|
| 日语 | `ja` | JIS Z 7253标准SDS | JIS Z 7253 |
| 英语 | `en` | GHS/OSHA HazCom格式 | GHS Rev.10 / ISO 11014 |
| 简体中文 | `zh-cn` | GB/T 16483格式 | GB/T 16483-2012 |
| 繁体中文 | `zh-tw` | CNS 15030格式 | CNS 15030 |

---

## 与同类产品对比

### 开源工具

| | **sds-converter**（本工具） | [sds_parser](https://github.com/astepe/sds_parser) | [tungsten](https://github.com/CrucibleSDS/tungsten) |
|---|---|---|---|
| 语言 | Rust | Python | Python |
| AI/LLM | 有（可替换） | 无（正则表达式） | 无（规则驱动） |
| MHLW JSON | 有 | 无 | 无 |
| 双向转换 | 有（↔ DOCX） | 无 | 无 |
| 多语言 | ja / en / zh-CN / zh-TW | 有限 | 仅英文 |

### 商业产品（日本）

| | **sds-converter**（本工具） | [SDS Meister](https://www.kcs.co.jp/ja/service/ind/general/chemical/sds.html) | [SmartSDS](https://smartsds.jp/) | [Dr.EHS Chemical](https://www.iad.co.jp/drehs/chemical2/) |
|---|---|---|---|---|
| 提供商 | — | さくらケーシーエス | テクノヒル | アイアンドディー |
| AI | 有（自备API密钥） | 无 | 有（翻译） | AI-OCR |
| MHLW JSON | 有 | 有 | 有 | 有 |
| PDF→JSON | 有 | 无（仅创作） | 部分（仅日语） | 有 |
| 开源 | 有（MIT/Apache-2.0） | 无 | 无 | 无 |

### 商业产品（全球）

| | **sds-converter**（本工具） | [Affinda](https://www.affinda.com/documents/material-safety-data-sheet) | [SDS Manager API](https://sdsmanager.com/) | [safetydatasheetapi.com](https://safetydatasheetapi.com/) | [EcoOnline](https://www.ecoonline.com/) |
|---|---|---|---|---|---|
| AI/LLM | 可替换LLM | LLM（自适应） | NLP/ML | ML + OCR | AI/NLP |
| 输入 | PDF / DOCX | PDF / Word | PDF | PDF（含扫描件） | PDF |
| 输出 | MHLW JSON + DOCX | 自定义JSON | JSON / XML | JSON / XML / CSV | 仅内部数据 |
| 开源 | 有 | 无 | 无 | 无 | 无 |

**本工具的核心优势**：唯一支持MHLW标准JSON、双向转换（JSON→DOCX）、无需云订阅的本地运行以及可替换LLM后端的开源解决方案。

---

## 参考链接

- [厚生劳动省 — SDS信息交换标准格式发布页面](https://www.mhlw.go.jp/stf/newpage_56484.html)（日语）
- [SDS数据交换格式开发者手册（PDF）](https://www.mhlw.go.jp/content/11305000/001467068.pdf)（日语）

---

## 许可证

以下两种许可证任选其一：
- Apache License, Version 2.0
- MIT License
