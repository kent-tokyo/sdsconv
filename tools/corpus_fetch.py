#!/usr/bin/env python3
"""SDS corpus collector — downloads public SDS documents with legal compliance checks.

Usage:
    python3 tools/corpus_fetch.py fetch <url> [--out corpus/raw] [--lang ja] [--notes "..."]
    python3 tools/corpus_fetch.py fetch-list urls.txt [--out corpus/raw] [--lang ja] [--delay 3] [--jobs 1]
    python3 tools/corpus_fetch.py audit [--manifest corpus/manifest.jsonl]
    python3 tools/corpus_fetch.py stats [--manifest corpus/manifest.jsonl]
    python3 tools/corpus_fetch.py remove <sha256> [--manifest corpus/manifest.jsonl]

Legal design:
  - robots.txt is checked before every download.
  - rate limit (--delay) is enforced between requests.
  - raw SDS files are saved to corpus/raw/ which is .gitignored.
  - manifest.jsonl (committed) tracks only URL/sha256/metadata, never file content.
  - redistribution_allowed defaults to False — caller must explicitly set if known.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import sys
import time
import urllib.parse
import urllib.request
import urllib.robotparser
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

MANIFEST_PATH  = Path("corpus/manifest.jsonl")
RAW_DIR        = Path("corpus/raw")
MAX_BYTES      = 50 * 1024 * 1024  # 50 MB
DEFAULT_DELAY  = 3.0
USER_AGENT     = "sdsconv-corpus-collector/1.0 (research; +https://github.com/kent-tokyo/sdsconv)"

ALLOWED_TYPES  = {
    "application/pdf": "pdf",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document": "docx",
    "application/msword": "doc",
    "application/vnd.ms-excel": "xls",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet": "xlsx",
    "text/html": "html",
    "text/plain": "txt",
}

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()

def _sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()

def _load_manifest(path: Path) -> list[dict]:
    if not path.exists():
        return []
    entries = []
    with path.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if line:
                try:
                    entries.append(json.loads(line))
                except json.JSONDecodeError:
                    pass
    return entries

def _append_manifest(path: Path, entry: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(entry, ensure_ascii=False) + "\n")

def _rewrite_manifest(path: Path, entries: list[dict]) -> None:
    with path.open("w", encoding="utf-8") as f:
        for e in entries:
            f.write(json.dumps(e, ensure_ascii=False) + "\n")

def _check_robots(url: str) -> bool:
    parsed = urllib.parse.urlparse(url)
    robots_url = f"{parsed.scheme}://{parsed.netloc}/robots.txt"
    rp = urllib.robotparser.RobotFileParser()
    rp.set_url(robots_url)
    try:
        rp.read()
        return rp.can_fetch(USER_AGENT, url)
    except Exception:
        return True  # be conservative: if robots.txt unreachable, allow

def _guess_ext(content_type: str, url: str) -> str:
    ct = content_type.split(";")[0].strip().lower()
    if ct in ALLOWED_TYPES:
        return ALLOWED_TYPES[ct]
    # fallback: infer from URL
    path = urllib.parse.urlparse(url).path.lower()
    for ext in ("pdf", "docx", "xlsx", "xls", "html", "htm", "txt"):
        if path.endswith(f".{ext}"):
            return ext
    return "bin"

def _make_filename(sha256: str, url: str, ext: str) -> str:
    base = Path(urllib.parse.urlparse(url).path).stem[:40]
    base = re.sub(r"[^\w\-]", "_", base).strip("_") or "sds"
    return f"{sha256[:8]}_{base}.{ext}"

# ---------------------------------------------------------------------------
# Commands
# ---------------------------------------------------------------------------

def cmd_fetch(url: str, out_dir: Path, lang: str, notes: str,
              manifest_path: Path, delay: float) -> Optional[dict]:
    """Fetch a single URL and add to corpus."""
    print(f"[fetch] {url}")

    # robots.txt
    allowed = _check_robots(url)
    if not allowed:
        print(f"  [SKIP] robots.txt disallows: {url}")
        return None

    # HEAD to check size + content type
    req = urllib.request.Request(url, method="HEAD", headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            content_type = resp.headers.get("Content-Type", "")
            content_len  = int(resp.headers.get("Content-Length", 0) or 0)
    except Exception as e:
        print(f"  [ERROR] HEAD failed: {e}")
        return None

    if content_len > MAX_BYTES:
        print(f"  [SKIP] file too large ({content_len / 1e6:.1f} MB > 50 MB)")
        return None

    ext = _guess_ext(content_type, url)
    ct_base = content_type.split(";")[0].strip().lower()
    if ct_base not in ALLOWED_TYPES and ext == "bin":
        print(f"  [SKIP] unsupported content-type: {content_type}")
        return None

    # Check for duplicates by URL
    existing = _load_manifest(manifest_path)
    if any(e["source_url"] == url for e in existing):
        print(f"  [SKIP] already in manifest")
        return None

    # Download
    time.sleep(delay)
    req2 = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    try:
        with urllib.request.urlopen(req2, timeout=60) as resp:
            data = resp.read(MAX_BYTES + 1)
    except Exception as e:
        print(f"  [ERROR] download failed: {e}")
        return None

    if len(data) > MAX_BYTES:
        print(f"  [SKIP] actual size exceeds 50 MB")
        return None

    sha256 = _sha256_bytes(data)

    # Check sha256 duplicate
    if any(e.get("sha256") == sha256 for e in existing):
        print(f"  [SKIP] duplicate content (sha256 match)")
        return None

    filename = _make_filename(sha256, url, ext)
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / filename).write_bytes(data)

    entry = {
        "source_url":            url,
        "filename":              filename,
        "sha256":                sha256,
        "downloaded_at":         datetime.now(timezone.utc).isoformat(),
        "file_type":             ext,
        "lang":                  lang,
        "file_size_bytes":       len(data),
        "robots_allowed":        allowed,
        "access_type":           "public_web",
        "redistribution_allowed": False,
        "public_export_allowed": False,
        "notes":                 notes,
    }
    _append_manifest(manifest_path, entry)
    print(f"  [OK]   saved → {filename}  ({len(data)/1024:.0f} KB)")
    return entry


def cmd_fetch_list(list_path: Path, out_dir: Path, lang: str,
                   manifest_path: Path, delay: float, jobs: int) -> None:
    """Fetch all URLs from a text file (one URL per line, # = comment)."""
    urls = [
        line.strip() for line in list_path.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.strip().startswith("#")
    ]
    print(f"fetch-list: {len(urls)} URLs from {list_path}")
    for i, url in enumerate(urls, 1):
        print(f"\n[{i}/{len(urls)}]")
        cmd_fetch(url, out_dir, lang, "", manifest_path, delay if i > 1 else 0)


def cmd_audit(manifest_path: Path) -> int:
    """Audit manifest for legal/compliance issues."""
    entries = _load_manifest(manifest_path)
    if not entries:
        print("manifest is empty.")
        return 0

    warnings = 0
    pii_patterns = [
        (r"[a-zA-Z0-9._%+\-]+@[a-zA-Z0-9.\-]+\.[a-zA-Z]{2,}", "email"),
        (r"\b0\d{1,4}[-\s]\d{2,4}[-\s]\d{4}\b", "JP phone"),
    ]

    for e in entries:
        issues = []
        if not e.get("robots_allowed", True):
            issues.append("robots.txt disallowed at fetch time")
        if e.get("access_type") == "auth_required":
            issues.append("auth_required — may violate terms")
        if e.get("redistribution_allowed"):
            issues.append("redistribution_allowed=true — double-check license")

        # Check notes for PII patterns
        notes = e.get("notes", "") + " " + e.get("source_url", "")
        for pattern, label in pii_patterns:
            if re.search(pattern, notes):
                issues.append(f"possible PII in notes ({label})")

        if issues:
            print(f"WARN [{e['filename']}]: " + "; ".join(issues))
            warnings += len(issues)

    print(f"\naudit: {len(entries)} entries, {warnings} warnings")
    return warnings


def cmd_stats(manifest_path: Path) -> None:
    """Show corpus statistics."""
    entries = _load_manifest(manifest_path)
    if not entries:
        print("manifest is empty.")
        return

    by_lang  = {}
    by_type  = {}
    total_kb = 0
    for e in entries:
        by_lang[e.get("lang", "?")] = by_lang.get(e.get("lang", "?"), 0) + 1
        by_type[e.get("file_type", "?")] = by_type.get(e.get("file_type", "?"), 0) + 1
        total_kb += e.get("file_size_bytes", 0) / 1024

    print(f"Total entries : {len(entries)}")
    print(f"Total size    : {total_kb/1024:.1f} MB")
    print(f"By lang       : {dict(sorted(by_lang.items(), key=lambda x: -x[1]))}")
    print(f"By file type  : {dict(sorted(by_type.items(), key=lambda x: -x[1]))}")


def cmd_remove(sha256_prefix: str, manifest_path: Path) -> None:
    """Remove an entry from manifest by sha256 prefix (file stays on disk)."""
    entries = _load_manifest(manifest_path)
    before = len(entries)
    entries = [e for e in entries if not e.get("sha256", "").startswith(sha256_prefix)]
    after = len(entries)
    _rewrite_manifest(manifest_path, entries)
    print(f"Removed {before - after} entry/entries matching sha256 prefix '{sha256_prefix}'")


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------

def main() -> None:
    ap = argparse.ArgumentParser(description="SDS corpus collector")
    ap.add_argument("--manifest", default=str(MANIFEST_PATH), help="Path to manifest.jsonl")
    sub = ap.add_subparsers(dest="cmd", required=True)

    # fetch
    p_fetch = sub.add_parser("fetch", help="Fetch a single URL")
    p_fetch.add_argument("url")
    p_fetch.add_argument("--out", default=str(RAW_DIR))
    p_fetch.add_argument("--lang", default="ja", choices=["ja", "en", "zh-cn", "zh-tw"])
    p_fetch.add_argument("--notes", default="")
    p_fetch.add_argument("--delay", type=float, default=DEFAULT_DELAY)

    # fetch-list
    p_list = sub.add_parser("fetch-list", help="Fetch URLs from a text file")
    p_list.add_argument("list_file")
    p_list.add_argument("--out", default=str(RAW_DIR))
    p_list.add_argument("--lang", default="ja", choices=["ja", "en", "zh-cn", "zh-tw"])
    p_list.add_argument("--delay", type=float, default=DEFAULT_DELAY)
    p_list.add_argument("--jobs", type=int, default=1)

    # audit
    sub.add_parser("audit", help="Audit manifest for legal issues")

    # stats
    sub.add_parser("stats", help="Show corpus statistics")

    # remove
    p_rm = sub.add_parser("remove", help="Remove entry by sha256 prefix")
    p_rm.add_argument("sha256")

    args = ap.parse_args()
    manifest = Path(args.manifest)

    if args.cmd == "fetch":
        cmd_fetch(args.url, Path(args.out), args.lang, args.notes, manifest, args.delay)
    elif args.cmd == "fetch-list":
        cmd_fetch_list(Path(args.list_file), Path(args.out), args.lang, manifest, args.delay, args.jobs)
    elif args.cmd == "audit":
        warnings = cmd_audit(manifest)
        sys.exit(1 if warnings else 0)
    elif args.cmd == "stats":
        cmd_stats(manifest)
    elif args.cmd == "remove":
        cmd_remove(args.sha256, manifest)


if __name__ == "__main__":
    main()
