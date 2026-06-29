"""Unit tests for coverage/recall/precision metrics in eval.py."""
from sdsconv.eval import _extract_cas, _extract_h_codes, _extract_p_codes, _extract_un_numbers, _recall, _precision


def test_recall_precision_partial():
    src = {"67-56-1", "75-09-2"}
    jsn = {"67-56-1", "999-99-9"}   # 1 match, 1 hallucinated, 1 missing
    assert _recall(src, jsn) == 0.5
    assert _precision(src, jsn) == 0.5


def test_recall_precision_perfect():
    src = {"H225", "H301"}
    jsn = {"H225", "H301"}
    assert _recall(src, jsn) == 1.0
    assert _precision(src, jsn) == 1.0


def test_recall_precision_hallucination():
    src = {"H225"}
    jsn = {"H225", "H302", "H303"}   # 2 hallucinated
    assert _recall(src, jsn) == 1.0        # source fully captured
    assert _precision(src, jsn) == 1/3     # only 1/3 of JSON is in source


def test_edge_empty_source():
    assert _recall(set(), {"H225"}) == 1.0   # source なし → recall 不問
    assert _recall(set(), set()) == 1.0


def test_edge_empty_json():
    assert _precision({"H225"}, set()) == 1.0  # json なし → precision 不問


def test_extract_cas():
    text = "成分: メタノール CAS 67-56-1 と 75-09-2"
    assert _extract_cas(text) == {"67-56-1", "75-09-2"}


def test_extract_h_codes():
    text = "H225、H301、P210（非GHSコード: H999は除外）"
    result = _extract_h_codes(text)
    assert "H225" in result
    assert "H301" in result
    assert "H999" not in result   # H9xx is not a valid GHS code range


def test_extract_p_codes():
    text = "P210 P260 P501"
    assert _extract_p_codes(text) == {"P210", "P260", "P501"}


def test_extract_un_normalized():
    text = "UN 1230 またはUN1230"
    result = _extract_un_numbers(text)
    assert result == {"UN1230"}   # normalized (space removed)


def test_missing_and_hallucinated_counts():
    src = {"67-56-1", "75-09-2", "71-43-2"}
    jsn = {"67-56-1", "999-99-9"}
    missing      = src - jsn   # {75-09-2, 71-43-2}
    hallucinated = jsn - src   # {999-99-9}
    assert len(missing) == 2
    assert len(hallucinated) == 1


if __name__ == "__main__":
    test_recall_precision_partial()
    test_recall_precision_perfect()
    test_recall_precision_hallucination()
    test_edge_empty_source()
    test_edge_empty_json()
    test_extract_cas()
    test_extract_h_codes()
    test_extract_p_codes()
    test_extract_un_normalized()
    test_missing_and_hallucinated_counts()
    print("all tests passed")
