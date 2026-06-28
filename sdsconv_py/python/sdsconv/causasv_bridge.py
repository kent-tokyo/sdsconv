"""causasv_bridge — DAG-aware quality failure analysis.

Usage:
    from sdsconv.causasv_bridge import compute_asv, print_ranking

    compute_asv("runs/eval_001/summary.csv")
"""
from __future__ import annotations

DAG: dict[str, list[str]] = {
    "file_type_pdf":           [],
    "file_type_docx":          [],
    "file_size_kb":            ["file_type_pdf", "file_type_docx"],
    "text_length_chars":       ["file_size_kb"],
    "lang_ja":                 [],
    "lang_zh_cn":              [],
    "lang_zh_tw":              [],
    "lang_en":                 [],
    "populated_section_count": ["text_length_chars", "lang_ja", "lang_zh_cn", "lang_zh_tw"],
    "empty_section_count":     ["populated_section_count"],
    "critical_count":          ["text_length_chars", "lang_ja", "lang_zh_cn"],
    "high_count":              ["text_length_chars", "lang_ja", "lang_zh_cn"],
    "medium_count":            ["populated_section_count"],
}

FEATURE_COLS = list(DAG.keys())


def _prepare_df(features_csv: str):
    import pandas as pd
    df = pd.read_csv(features_csv)
    df["file_type_pdf"]  = (df["file_type"] == "pdf").astype(int)
    df["file_type_docx"] = (df["file_type"] == "docx").astype(int)
    df["lang_ja"]   = (df["source_language"] == "ja").astype(int)
    df["lang_zh_cn"]= (df["source_language"].str.lower().str.contains("zh-cn|zh_cn", na=False)).astype(int)
    df["lang_zh_tw"]= (df["source_language"].str.lower().str.contains("zh-tw|zh_tw", na=False)).astype(int)
    df["lang_en"]   = (df["source_language"] == "en").astype(int)
    df["quality_fail"] = ((df.get("overall_score", 100) < 80) | (df.get("critical_count", 0) > 0)).astype(int)
    return df


def compute_asv(features_csv: str, target: str = "quality_fail"):
    """Compute DAG-aware ASV using causasv and return a ranked DataFrame.

    Args:
        features_csv: Path to summary.csv from eval_corpus.
        target:       Binary target column name.

    Returns:
        pandas.DataFrame with columns [feature, mean_abs_asv] sorted descending.
    """
    import numpy as np
    import pandas as pd
    try:
        import causasv
    except ImportError:
        raise ImportError("pip install causasv")
    from sklearn.ensemble import GradientBoostingClassifier

    df = _prepare_df(features_csv)
    X = df[[c for c in FEATURE_COLS if c in df.columns]].fillna(0)
    y = df[target]
    used_features = X.columns.tolist()
    used_dag = {k: [v for v in vs if v in used_features] for k, vs in DAG.items() if k in used_features}

    model = GradientBoostingClassifier(n_estimators=100, max_depth=3, random_state=42)
    model.fit(X, y)

    asv_values = causasv.compute(
        model=model, X=X, dag=used_dag,
        feature_names=used_features,
        n_samples=min(300, len(X)),
    )
    mean_asv = np.abs(asv_values).mean(axis=0)
    result = pd.DataFrame({"feature": used_features, "mean_abs_asv": mean_asv})
    return result.sort_values("mean_abs_asv", ascending=False).reset_index(drop=True)


def print_ranking(features_csv: str) -> None:
    """Print ASV ranking to stdout."""
    ranking = compute_asv(features_csv)
    print("\n=== Quality failure causal ranking (mean |ASV|) ===")
    print(f"{'Feature':<30} {'ASV':>8}")
    print("-" * 40)
    for _, row in ranking.iterrows():
        bar = "█" * int(row["mean_abs_asv"] * 300)
        print(f"{row['feature']:<30} {row['mean_abs_asv']:>8.4f}  {bar}")
