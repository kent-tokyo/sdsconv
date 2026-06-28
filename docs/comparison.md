# Comparison with Alternatives

## Open-source tools

| | **sdsconv** (this) | [sds_parser](https://github.com/astepe/sds_parser) | [tungsten](https://github.com/CrucibleSDS/tungsten) |
|---|---|---|---|
| Language | Rust + Python | Python | Python |
| AI/LLM | Yes (pluggable) | No (regex) | No (rule-based) |
| MHLW JSON | Yes | No | No |
| Bidirectional | Yes (DOCX + HTML + PDF) | No | No |
| HTML/URL input | Yes | No | No |
| GHS/CAS validation | Yes | No | No |
| Multilingual | ja / en / zh-CN / zh-TW | Limited | English only |
| Corpus evaluation | Yes (`eval_corpus`) | No | No |

## Commercial products (Japan)

| | **sdsconv** (this) | [SDS Meister](https://www.kcs.co.jp/ja/service/ind/general/chemical/sds.html) | [SmartSDS](https://smartsds.jp/) | [Dr.EHS Chemical](https://www.iad.co.jp/drehs/chemical2/) |
|---|---|---|---|---|
| Provider | — | さくらケーシーエス | テクノヒル | アイアンドディー |
| AI | Yes (your API key) | No | Yes (translation) | AI-OCR |
| MHLW JSON | Yes | Yes | Yes | Yes |
| Source PDF → JSON | Yes | No (authoring only) | Partial (JP only) | Yes |
| Open-source | Yes (MIT/Apache-2.0) | No | No | No |

## Commercial products (Global)

| | **sdsconv** (this) | [Affinda](https://www.affinda.com/documents/material-safety-data-sheet) | [SDS Manager API](https://sdsmanager.com/) | [safetydatasheetapi.com](https://safetydatasheetapi.com/) | [EcoOnline](https://www.ecoonline.com/) |
|---|---|---|---|---|---|
| AI/LLM | Pluggable LLM | LLM (adaptive) | NLP/ML | ML + OCR | AI/NLP |
| Input | PDF / DOCX | PDF / Word | PDF | PDF (incl. scanned) | PDF |
| Output | MHLW JSON + DOCX | Custom JSON | JSON / XML | JSON / XML / CSV | Internal only |
| Open-source | Yes | No | No | No | No |

**Key advantages:** the only open-source solution that supports the MHLW standard JSON, bidirectional conversion (JSON → DOCX/HTML/PDF), local execution without cloud subscriptions, GHS Rev.10 validation, PubChem enrichment, corpus-scale quality evaluation, and a pluggable LLM backend.
