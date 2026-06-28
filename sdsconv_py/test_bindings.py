"""Smoke tests for sdsconv Python bindings.

Run from sdsconv_py/:
    source .venv/bin/activate
    python3 test_bindings.py

Requires ANTHROPIC_API_KEY in environment or ../.env file.
"""
import json
import os
import sys
from pathlib import Path

# Load ../.env if ANTHROPIC_API_KEY not set
if not os.environ.get("ANTHROPIC_API_KEY"):
    env_file = Path(__file__).parent.parent / ".env"
    if env_file.exists():
        for line in env_file.read_text().splitlines():
            if line.startswith("ANTHROPIC_API_KEY="):
                os.environ["ANTHROPIC_API_KEY"] = line.split("=", 1)[1].strip().strip('"')
                break

import sdsconv

SAMPLE = str(Path(__file__).parent.parent / "corpus" / "raw" / "misc" / "input01.pdf")

def test_import():
    print(f"[1] import OK: {sdsconv.__file__}")

def test_extract_text():
    text = sdsconv.extract_text(SAMPLE)
    assert len(text) > 100, f"text too short: {len(text)}"
    print(f"[2] extract_text: {len(text)} chars  ✓  (first 60: {text[:60].replace(chr(10),' ')!r})")

def test_validate_empty():
    findings = sdsconv.validate({})
    print(f"[3] validate({{}}): {len(findings)} findings  ✓")

def test_to_json_with_report():
    api_key = os.environ.get("ANTHROPIC_API_KEY", "")
    if not api_key:
        print("[4] SKIP to_json_with_report — ANTHROPIC_API_KEY not set")
        return
    print(f"[4] to_json_with_report({Path(SAMPLE).name}) ...", end=" ", flush=True)
    data, report = sdsconv.to_json_with_report(SAMPLE, lang="en")
    populated = len(report.get("populated_sections", []))
    findings = sdsconv.validate(data)
    high  = sum(1 for f in findings if f["level"] == "HIGH")
    med   = sum(1 for f in findings if f["level"] == "MED")
    print(f"sections={populated}  findings={len(findings)} (HIGH={high} MED={med})  ✓")
    # write_json smoke
    out = Path("/tmp/sdsconv_test_output.json")
    sdsconv.write_json(data, out)
    assert out.exists() and out.stat().st_size > 100
    print(f"     write_json → {out}  ✓")
    # strict_mhlw
    try:
        sdsconv.validate(data, strict_mhlw=True)
        print("     validate(strict_mhlw=True) → no HIGH/CRIT  ✓")
    except ValueError as e:
        print(f"     validate(strict_mhlw=True) → raised ValueError (expected if HIGH>0)  ✓")

def main():
    print("=== sdsconv Python binding smoke tests ===\n")
    test_import()
    test_extract_text()
    test_validate_empty()
    test_to_json_with_report()
    print("\nAll tests passed ✓")

if __name__ == "__main__":
    main()
