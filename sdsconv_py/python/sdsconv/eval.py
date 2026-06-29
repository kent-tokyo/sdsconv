"""eval_corpus — large-scale SDS corpus evaluation in Python.

Usage:
    from sdsconv.eval import eval_corpus

    df = eval_corpus(
        "data/sds_raw",
        "runs/eval_001",
        api_key="sk-ant-...",
        lang=None,   # auto-detect
        jobs=8,
    )
    print(df[["filename", "overall_score", "grade", "high_count"]].head(20))
"""
from __future__ import annotations

import json
import os
import re
import time
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any

try:
    import pandas as pd
    _HAS_PANDAS = True
except ImportError:
    _HAS_PANDAS = False

try:
    from tqdm import tqdm as _tqdm
    def _progress(it, total, desc):
        return _tqdm(it, total=total, desc=desc)
except ImportError:
    def _progress(it, total, desc):
        return it

import sdsconv

SDS_EXTENSIONS = {".pdf", ".docx", ".xlsx", ".xls", ".txt", ".html", ".htm"}

# ---------------------------------------------------------------------------
# Score / grade helpers
# ---------------------------------------------------------------------------

def _compute_score(crit: int, high: int, med: int, low: int = 0) -> float:
    return max(0.0, 100.0 - crit * 35 - high * 8 - med * 1.5 - low * 0.3)

def _compute_grade(score: float, crit: int, high: int) -> str:
    if crit == 0 and high == 0  and score >= 90: return "A"
    if crit == 0 and high <= 3  and score >= 80: return "B"
    if crit == 0 and high <= 10 and score >= 65: return "C"
    return "D"

# ---------------------------------------------------------------------------
# Source-text feature extraction (regex, no LLM)
# ---------------------------------------------------------------------------

def _count_unique(pattern: str, text: str) -> int:
    return len(set(re.findall(pattern, text, re.IGNORECASE)))

def _count_cas(text: str) -> int:
    return _count_unique(r'\b\d{2,7}-\d{2}-\d\b', text)

def _count_h_codes(text: str) -> int:
    return _count_unique(r'\bH[23456]\d{2}\b', text)

def _count_p_codes(text: str) -> int:
    return _count_unique(r'\bP[123456]\d{2}\b', text)

def _count_un_numbers(text: str) -> int:
    return _count_unique(r'\bUN\s*\d{4}\b', text)

# ---------------------------------------------------------------------------
# Single-file evaluation
# ---------------------------------------------------------------------------

def eval_one(
    path: str | Path,
    output_dir: str | Path,
    *,
    api_key: str | None = None,
    backend: str = "anthropic",
    model: str = "claude-haiku-4-5-20251001",
    lang: str | None = None,
    country: str | None = None,
    correct: bool = False,
    enrich: bool = False,
    max_chars: int = 80_000,
) -> dict[str, Any]:
    """Evaluate a single SDS file. Returns a record dict."""
    path = Path(path)
    out  = Path(output_dir)
    stem = path.stem

    record: dict[str, Any] = {
        "filename": path.name,
        "file_type": path.suffix.lstrip(".").lower(),
        "file_size_kb": round(path.stat().st_size / 1024, 1),
    }

    t0 = time.monotonic()
    try:
        try:
            raw_text = sdsconv.extract_text(str(path))
        except Exception:
            raw_text = ""
        record["text_length_chars"]      = len(raw_text)
        record["cas_count_in_source"]    = _count_cas(raw_text)
        record["h_code_count_in_source"] = _count_h_codes(raw_text)
        record["p_code_count_in_source"] = _count_p_codes(raw_text)
        record["un_count_in_source"]     = _count_un_numbers(raw_text)

        data, report = sdsconv.to_json_with_report(
            path,
            backend=backend, api_key=api_key, model=model,
            lang=lang, country=country, correct=correct, enrich=enrich,
            max_chars=max_chars,
        )
        record["json_ok"] = True
        record["extraction_time_ms"] = int((time.monotonic() - t0) * 1000)
        record["source_language"]       = report.get("source_language", "")
        record["populated_section_count"] = len(report.get("populated_sections", []))
        record["empty_section_count"]     = len(report.get("empty_sections", []))

        # Write outputs
        (out / "generated").mkdir(parents=True, exist_ok=True)
        (out / "reports").mkdir(parents=True, exist_ok=True)
        (out / "findings").mkdir(parents=True, exist_ok=True)

        sdsconv.write_json(data, out / "generated" / f"{stem}.json")
        (out / "reports"  / f"{stem}.json").write_text(
            json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

        findings = sdsconv.validate(data)
        (out / "findings" / f"{stem}.json").write_text(
            json.dumps(findings, ensure_ascii=False, indent=2), encoding="utf-8")

        crit = sum(1 for f in findings if f.get("level") == "CRIT")
        high = sum(1 for f in findings if f.get("level") == "HIGH")
        med  = sum(1 for f in findings if f.get("level") == "MED")

        record["critical_count"] = crit
        record["high_count"]     = high
        record["medium_count"]   = med
        record["overall_score"]  = _compute_score(crit, high, med)
        record["grade"]          = _compute_grade(record["overall_score"], crit, high)
        record["error"]          = ""

    except Exception as exc:
        record["json_ok"]              = False
        record["extraction_time_ms"]   = int((time.monotonic() - t0) * 1000)
        record["source_language"]      = ""
        record["populated_section_count"] = 0
        record["empty_section_count"]  = 0
        record["critical_count"]       = 0
        record["high_count"]           = 0
        record["medium_count"]         = 0
        record["overall_score"]        = 0.0
        record["grade"]                = "D"
        record["error"]                = str(exc)
        for k in ("text_length_chars", "cas_count_in_source",
                  "h_code_count_in_source", "p_code_count_in_source",
                  "un_count_in_source"):
            record.setdefault(k, 0)

    return record

# ---------------------------------------------------------------------------
# Corpus evaluation
# ---------------------------------------------------------------------------

def eval_corpus(
    input_dir: str | Path,
    output_dir: str | Path,
    *,
    api_key: str | None = None,
    backend: str = "anthropic",
    model: str = "claude-haiku-4-5-20251001",
    lang: str | None = None,
    country: str | None = None,
    correct: bool = False,
    enrich: bool = False,
    jobs: int = 4,
    limit: int | None = None,
    max_chars: int = 80_000,
):
    """Evaluate a directory of SDS files.

    Args:
        input_dir:  Directory containing SDS files (PDF/DOCX/XLSX/…).
        output_dir: Where to write generated JSON, reports, findings, CSV.
        api_key:    LLM API key. Falls back to env var if not provided.
        backend:    LLM backend ("anthropic", "openai", "gemini", …).
        model:      Model name.
        lang:       Source language code ("ja", "en", "zh-cn", "zh-tw") or None for auto.
        country:    Regulatory country ("jp", "cn", "tw", "kr") or None for auto.
        correct:    Run validation-driven correction pass.
        enrich:     Enrich CAS numbers via PubChem.
        jobs:       Parallel worker threads.
        limit:      Max number of files to process (useful for smoke tests).
        max_chars:  Max chars to extract from each file.

    Returns:
        pandas.DataFrame with one row per file, or list[dict] if pandas unavailable.
    """
    input_dir  = Path(input_dir)
    output_dir = Path(output_dir)
    output_dir.mkdir(parents=True, exist_ok=True)

    files = sorted(
        p for p in input_dir.rglob("*")
        if p.is_file() and p.suffix.lower() in SDS_EXTENSIONS
    )
    if limit:
        files = files[:limit]

    if not files:
        print(f"No SDS files found in {input_dir}")
        return [] if not _HAS_PANDAS else pd.DataFrame()

    print(f"eval_corpus: {len(files)} files, {jobs} workers → {output_dir}")

    records: list[dict] = []
    kwargs = dict(
        api_key=api_key, backend=backend, model=model,
        lang=lang, country=country, correct=correct, enrich=enrich,
        max_chars=max_chars,
    )

    with ThreadPoolExecutor(max_workers=jobs) as pool:
        futures = {
            pool.submit(eval_one, f, output_dir, **kwargs): f
            for f in files
        }
        for fut in _progress(as_completed(futures), total=len(files), desc="eval"):
            path = futures[fut]
            try:
                rec = fut.result()
            except Exception as exc:
                rec = {"filename": path.name, "error": str(exc),
                       "json_ok": False, "overall_score": 0.0, "grade": "D"}
            records.append(rec)

    # Write manifest
    manifest_path = output_dir / "manifest.jsonl"
    with manifest_path.open("w", encoding="utf-8") as mf:
        for rec in records:
            mf.write(json.dumps(rec, ensure_ascii=False) + "\n")

    # Write CSV
    if _HAS_PANDAS:
        df = pd.DataFrame(records)
        df.to_csv(output_dir / "summary.csv", index=False)
        _write_causasv_features(output_dir, df)
        _write_summary_md(output_dir, df)
        _write_failures_csv(output_dir)
        print(f"\n=== Summary ===")
        print(df[["filename", "json_ok", "overall_score", "grade",
                   "critical_count", "high_count", "medium_count"]].to_string(index=False))
        return df
    else:
        print(f"Done. {sum(r.get('json_ok', False) for r in records)}/{len(records)} ok")
        return records


_CAUSASV_COLS = [
    "filename", "file_type", "file_size_kb", "text_length_chars",
    "source_language", "populated_section_count", "empty_section_count",
    "cas_count_in_source", "h_code_count_in_source",
    "p_code_count_in_source", "un_count_in_source",
    "critical_count", "high_count", "medium_count",
    "overall_score", "grade",
]

def _write_causasv_features(output_dir: Path, df) -> None:
    cols = [c for c in _CAUSASV_COLS if c in df.columns]
    df[cols].to_csv(output_dir / "causasv_features.csv", index=False)


def _write_summary_md(output_dir: Path, df) -> None:
    n = len(df)
    ok = df["json_ok"].sum() if "json_ok" in df else 0
    avg = df["overall_score"].mean() if "overall_score" in df else 0
    grades = df["grade"].value_counts().to_dict() if "grade" in df else {}
    crit = df["critical_count"].sum() if "critical_count" in df else 0
    high = df["high_count"].sum() if "high_count" in df else 0
    med  = df["medium_count"].sum() if "medium_count" in df else 0
    grades_str = " / ".join(f"{g}: {grades.get(g, 0)}" for g in ["A", "B", "C", "D"])

    md = (
        "# eval_corpus summary\n\n"
        "| Metric | Value |\n"
        "|--------|-------|\n"
        f"| Files | {n} |\n"
        f"| JSON ok | {ok} |\n"
        f"| Avg score | {avg:.1f} |\n"
        f"| Grades | {grades_str} |\n"
        f"| CRIT | {crit} |\n"
        f"| HIGH | {high} |\n"
        f"| MED  | {med} |\n"
    )
    (output_dir / "summary.md").write_text(md, encoding="utf-8")


def _write_failures_csv(output_dir: Path) -> None:
    """Aggregate finding frequencies from findings/*.json into failures_by_rule.csv."""
    import csv
    findings_dir = output_dir / "findings"
    rule_map: dict[str, dict] = {}

    for p in findings_dir.glob("*.json"):
        stem = p.stem
        try:
            findings = json.loads(p.read_text(encoding="utf-8"))
        except Exception:
            continue
        for f in findings:
            rule  = f.get("rule", "UNKNOWN")
            level = f.get("level", "?")
            if rule not in rule_map:
                rule_map[rule] = {"level": level, "count": 0, "files": set()}
            rule_map[rule]["count"] += 1
            rule_map[rule]["files"].add(stem)

    rows = sorted(rule_map.items(), key=lambda x: x[1]["count"], reverse=True)
    with (output_dir / "failures_by_rule.csv").open("w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["rule_id", "level", "count", "affected_files"])
        for rule, info in rows:
            w.writerow([rule, info["level"], info["count"], len(info["files"])])
