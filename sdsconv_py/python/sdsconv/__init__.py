"""sdsconv — SDS document ↔ MHLW standard JSON converter (Python bindings)."""
from __future__ import annotations

import json
import os
from pathlib import Path
from typing import Any

from ._sdsconv import (
    extract_text as _extract_text,
    to_json_with_report as _to_json_with_report,
    to_json_bytes_with_report as _to_json_bytes_with_report,
    to_json_url_with_report as _to_json_url_with_report,
    validate_json as _validate_json,
)

__all__ = [
    "extract_text",
    "to_json",
    "to_json_with_report",
    "to_json_bytes",
    "to_json_bytes_with_report",
    "to_json_url",
    "to_json_url_with_report",
    "validate",
    "write_json",
]

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _resolve_api_key(api_key: str | None, backend: str) -> str:
    if api_key:
        return api_key
    env_map = {
        "anthropic": "ANTHROPIC_API_KEY",
        "openai":    "OPENAI_API_KEY",
        "gemini":    "GEMINI_API_KEY",
        "mistral":   "MISTRAL_API_KEY",
        "groq":      "GROQ_API_KEY",
        "cohere":    "COHERE_API_KEY",
        "local":     "LOCAL_LLM_API_KEY",
    }
    env_var = env_map.get(backend, "ANTHROPIC_API_KEY")
    key = os.environ.get(env_var, "")
    if not key and backend == "local":
        return "ollama"
    if not key:
        raise ValueError(
            f"API key not provided. Pass api_key= or set {env_var}."
        )
    return key


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

def extract_text(path: str | Path, *, max_chars: int = 80_000) -> str:
    """Extract raw text from a PDF/DOCX/XLSX/HTML/TXT file."""
    return _extract_text(str(path), max_chars)


def to_json_with_report(
    path: str | Path,
    *,
    backend: str = "anthropic",
    api_key: str | None = None,
    model: str = "claude-haiku-4-5-20251001",
    lang: str | None = None,
    country: str | None = None,
    max_chars: int = 80_000,
    max_tokens: int = 16_384,
    correct: bool = False,
    enrich: bool = False,
    strict_mhlw: bool = False,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Convert a file to MHLW standard JSON.

    Returns:
        (sds_data, conversion_report) as Python dicts.
    """
    key = _resolve_api_key(api_key, backend)
    sds_str, report_str = _to_json_with_report(
        str(path), backend, key, model, lang, country,
        max_chars, max_tokens, correct, enrich,
    )
    data, report = json.loads(sds_str), json.loads(report_str)
    if strict_mhlw:
        validate(data, strict_mhlw=True)
    return data, report


def to_json(
    path: str | Path,
    *,
    backend: str = "anthropic",
    api_key: str | None = None,
    model: str = "claude-haiku-4-5-20251001",
    lang: str | None = None,
    country: str | None = None,
    max_chars: int = 80_000,
    max_tokens: int = 16_384,
    correct: bool = False,
    enrich: bool = False,
    strict_mhlw: bool = False,
) -> dict[str, Any]:
    """Convert a file to MHLW standard JSON. Returns the SDS dict."""
    data, _ = to_json_with_report(
        path, backend=backend, api_key=api_key, model=model,
        lang=lang, country=country, max_chars=max_chars, max_tokens=max_tokens,
        correct=correct, enrich=enrich, strict_mhlw=strict_mhlw,
    )
    return data


def to_json_bytes_with_report(
    data: bytes,
    filename: str,
    *,
    backend: str = "anthropic",
    api_key: str | None = None,
    model: str = "claude-haiku-4-5-20251001",
    lang: str | None = None,
    country: str | None = None,
    max_chars: int = 80_000,
    max_tokens: int = 16_384,
    correct: bool = False,
    strict_mhlw: bool = False,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Convert in-memory bytes to MHLW standard JSON (for API/web use)."""
    key = _resolve_api_key(api_key, backend)
    sds_str, report_str = _to_json_bytes_with_report(
        data, filename, backend, key, model, lang, country,
        max_chars, max_tokens, correct,
    )
    sds_data, report = json.loads(sds_str), json.loads(report_str)
    if strict_mhlw:
        validate(sds_data, strict_mhlw=True)
    return sds_data, report


def to_json_bytes(data: bytes, filename: str, *, strict_mhlw: bool = False, **kwargs) -> dict[str, Any]:
    """Convert in-memory bytes to MHLW standard JSON."""
    result, _ = to_json_bytes_with_report(data, filename, strict_mhlw=strict_mhlw, **kwargs)
    return result


def to_json_url_with_report(
    url: str,
    *,
    backend: str = "anthropic",
    api_key: str | None = None,
    model: str = "claude-haiku-4-5-20251001",
    lang: str | None = None,
    country: str | None = None,
    max_chars: int = 80_000,
    max_tokens: int = 16_384,
    correct: bool = False,
    strict_mhlw: bool = False,
) -> tuple[dict[str, Any], dict[str, Any]]:
    """Fetch an SDS from a URL and convert to MHLW standard JSON."""
    key = _resolve_api_key(api_key, backend)
    sds_str, report_str = _to_json_url_with_report(
        url, backend, key, model, lang, country,
        max_chars, max_tokens, correct,
    )
    sds_data, report = json.loads(sds_str), json.loads(report_str)
    if strict_mhlw:
        validate(sds_data, strict_mhlw=True)
    return sds_data, report


def to_json_url(url: str, *, strict_mhlw: bool = False, **kwargs) -> dict[str, Any]:
    """Fetch an SDS from a URL and convert to MHLW standard JSON."""
    result, _ = to_json_url_with_report(url, strict_mhlw=strict_mhlw, **kwargs)
    return result


def validate(
    data: dict[str, Any] | str,
    *,
    strict_mhlw: bool = False,
) -> list[dict[str, Any]]:
    """Validate a MHLW standard JSON and return structured findings.

    Args:
        data: SDS dict or JSON string.
        strict_mhlw: If True, raises ValueError when any HIGH or CRIT finding exists.

    Returns:
        List of findings: [{"level": "HIGH", "rule": "S2-GHS-INCOMPLETE", "message": "..."}, ...]
    """
    json_text = json.dumps(data, ensure_ascii=False) if not isinstance(data, str) else data
    findings: list[dict] = json.loads(_validate_json(json_text))
    if strict_mhlw:
        bad = [f for f in findings if f.get("level") in ("HIGH", "CRIT")]
        if bad:
            msgs = "\n".join(f"  [{f['level']}][{f['rule']}] {f['message']}" for f in bad[:5])
            raise ValueError(f"strict_mhlw: {len(bad)} HIGH/CRIT findings:\n{msgs}")
    return findings


def write_json(data: dict[str, Any], path: str | Path) -> None:
    """Write MHLW standard JSON to a file (UTF-8, pretty-printed)."""
    Path(path).write_text(
        json.dumps(data, ensure_ascii=False, indent=2),
        encoding="utf-8",
    )
