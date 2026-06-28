#!/usr/bin/env python3
"""Download the MHLW allyl chloride sample SDS and convert with sdsconv."""
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

HERE = Path(__file__).parent
URL = (HERE / "source.url").read_text().strip()
PDF = HERE / "source.pdf"
EXPECTED_SHA256 = (HERE / "source.sha256").read_text().strip()
OUT_JSON = HERE / "expected.json"
OUT_REPORT = HERE / "expected_report.json"


def download():
    if PDF.exists():
        return
    print(f"Downloading {URL} ...")
    urllib.request.urlretrieve(URL, PDF)
    print(f"Saved {PDF.stat().st_size // 1024} KB → {PDF.name}")


def verify():
    sha = hashlib.sha256(PDF.read_bytes()).hexdigest()
    if sha != EXPECTED_SHA256:
        print(f"WARNING: sha256 mismatch\n  expected: {EXPECTED_SHA256}\n  got:      {sha}")
    else:
        print(f"sha256 OK ({sha[:16]}...)")


def convert():
    import sdsconv

    print("Converting ...")
    # ponytail: sonnet needed — haiku's 8k output limit truncates this 16-section SDS
    data, report = sdsconv.to_json_bytes_with_report(
        PDF.read_bytes(), PDF.name, lang="ja",
        model="claude-sonnet-4-6", max_tokens=16384,
    )
    findings = sdsconv.validate(data)

    OUT_JSON.write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    OUT_REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    n_high = sum(1 for f in findings if f["level"] == "HIGH")
    n_med = sum(1 for f in findings if f["level"] == "MED")
    print(f"Sections: {len(report.get('populated_sections', []))}")
    print(f"Findings: {len(findings)} (HIGH={n_high}, MED={n_med})")
    print(f"Saved → {OUT_JSON.name}, {OUT_REPORT.name}")


if __name__ == "__main__":
    try:
        import sdsconv  # noqa: F401
    except ImportError:
        sys.exit("sdsconv not installed. Run: pip install sdsconv")

    download()
    verify()
    convert()
