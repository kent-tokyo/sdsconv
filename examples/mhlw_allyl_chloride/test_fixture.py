"""Golden fixture test — MHLW allyl chloride SDS (no API key needed)."""
import json
import sys
from pathlib import Path

HERE = Path(__file__).parent


def test():
    data = json.loads((HERE / "expected.json").read_text(encoding="utf-8"))

    import sdsconv

    findings = sdsconv.validate(data)
    crit = [f for f in findings if f["level"] == "CRIT"]
    assert not crit, f"CRIT findings in fixture: {crit}"

    trade = data.get("Identification", {}).get("TradeProductIdentity", {})
    assert trade.get("TradeNameEN") or trade.get("TradeNameJP"), \
        "Identification.TradeProductIdentity.TradeName* missing"

    import re
    text = json.dumps(data, ensure_ascii=False)
    assert "107-05-1" in text, "Allyl chloride CAS 107-05-1 not found in output JSON"

    print(f"fixture OK — {len(findings)} findings, CAS OK, TradeName OK")


if __name__ == "__main__":
    test()
