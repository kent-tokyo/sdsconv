"""causasv_bridge — DAG-aware quality failure analysis for sdsconv.

Requires: pip install "sdsconv[analysis]"
  or:     pip install causasv>=0.8.2 scikit-learn pandas

Usage:
    from sdsconv.causasv_bridge import compute_asv, print_ranking

    df = compute_asv("runs/eval_001/causasv_features.csv")
    print_ranking("runs/eval_001/causasv_features.csv")
"""
from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    import pandas as pd

# ---------------------------------------------------------------------------
# DAG definition for sdsconv quality factors
# ---------------------------------------------------------------------------

# Causal edges: (cause, effect)
# Reading: "file_type → text extraction quality → source evidence → JSON coverage → score"
SDS_QUALITY_DAG_EDGES: list[tuple[str, str]] = [
    # File characteristics → extracted text
    ("file_type_pdf",           "text_length_chars"),
    ("file_type_docx",          "text_length_chars"),
    ("file_size_kb",            "text_length_chars"),

    # Language → section structure
    ("lang_ja",                 "populated_section_count"),
    ("lang_zh_cn",              "populated_section_count"),
    ("lang_zh_tw",              "populated_section_count"),
    ("lang_en",                 "populated_section_count"),

    # Extracted text → source evidence counts
    ("text_length_chars",       "cas_count_in_source"),
    ("text_length_chars",       "h_code_count_in_source"),
    ("text_length_chars",       "p_code_count_in_source"),
    ("text_length_chars",       "un_count_in_source"),
    ("text_length_chars",       "populated_section_count"),

    # Source evidence → JSON coverage
    ("cas_count_in_source",     "cas_coverage"),
    ("h_code_count_in_source",  "h_code_coverage"),
    ("p_code_count_in_source",  "p_code_coverage"),
    ("un_count_in_source",      "un_coverage"),

    # Section completeness
    ("populated_section_count", "empty_section_count"),

    # Coverage → finding counts
    ("cas_coverage",            "high_count"),
    ("h_code_coverage",         "high_count"),
    ("p_code_coverage",         "medium_count"),
    ("populated_section_count", "high_count"),
    ("empty_section_count",     "medium_count"),

    # Finding counts → overall score
    ("high_count",              "overall_score"),
    ("medium_count",            "overall_score"),
]

FEATURE_COLS = [
    "file_type_pdf", "file_type_docx", "file_size_kb", "text_length_chars",
    "lang_ja", "lang_zh_cn", "lang_zh_tw", "lang_en",
    "cas_count_in_source", "h_code_count_in_source",
    "p_code_count_in_source", "un_count_in_source",
    "cas_coverage", "h_code_coverage", "p_code_coverage", "un_coverage",
    "cas_recall", "cas_precision", "cas_hallucinated_count",
    "h_code_recall", "h_code_precision", "h_code_hallucinated_count",
    "p_code_recall", "p_code_precision", "p_code_hallucinated_count",
    "un_recall", "un_precision", "un_hallucinated_count",
    "populated_section_count", "empty_section_count",
    "high_count", "medium_count",
]

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def build_dag():
    """Build and return the sdsconv quality CausalDAG."""
    from causasv import CausalDAG
    return CausalDAG.from_edges(SDS_QUALITY_DAG_EDGES)


def _prepare_df(features_csv: str):
    import pandas as pd
    df = pd.read_csv(features_csv)
    df["file_type_pdf"]  = (df["file_type"] == "pdf").astype(float)
    df["file_type_docx"] = (df["file_type"] == "docx").astype(float)
    sl = df["source_language"].str.lower().fillna("")
    df["lang_ja"]    = sl.str.startswith("ja").astype(float)
    df["lang_zh_cn"] = sl.str.contains("zh-cn|zh_cn|zh-hans", regex=True).astype(float)
    df["lang_zh_tw"] = sl.str.contains("zh-tw|zh_tw|zh-hant", regex=True).astype(float)
    df["lang_en"]    = sl.str.startswith("en").astype(float)
    df["quality_fail"] = ((df["overall_score"] < 80) | (df["critical_count"] > 0)).astype(float)
    # Fill missing columns with 0
    for col in FEATURE_COLS:
        if col not in df.columns:
            df[col] = 0.0
    return df


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

def compute_asv(
    features_csv: str,
    target: str = "overall_score",
    method: str = "auto",
    n_samples: int = 10_000,
    seed: int = 42,
) -> "pd.DataFrame":
    """Compute DAG-aware ASV for sdsconv quality features.

    Uses GradientBoostingRegressor + TabularExplainer (causasv).

    Args:
        features_csv: Path to causasv_features.csv from eval-corpus.
        target:       Column to explain ("overall_score" or "quality_fail").
        method:       causasv method ("auto", "exact", "approx").
        n_samples:    Approximation samples (used when method="approx").
        seed:         Random seed for reproducibility.

    Returns:
        DataFrame with columns [feature, mean_abs_asv] sorted descending.
    """
    import numpy as np
    import pandas as pd
    from causasv import CausalDAG, TabularExplainer
    from sklearn.ensemble import GradientBoostingRegressor

    df = _prepare_df(features_csv)
    avail = [c for c in FEATURE_COLS if c in df.columns]
    # Remove target and descendants if present to avoid leakage
    exclude = {target, "overall_score", "critical_count", "high_count", "medium_count",
               "quality_fail", "grade"}
    features = [c for c in avail if c not in exclude]

    X = df[features].fillna(0).astype(float)
    y = df[target].fillna(0).astype(float)

    if len(X) < 3:
        raise ValueError(f"Need at least 3 samples, got {len(X)}. Run eval-corpus with more files.")

    # Build DAG restricted to available features
    all_nodes = set(features)
    edges = [(s, t) for s, t in SDS_QUALITY_DAG_EDGES if s in all_nodes and t in all_nodes]
    dag = CausalDAG.from_edges(edges)

    import warnings
    X_np = X.values
    y_np = y.values
    model = GradientBoostingRegressor(n_estimators=100, max_depth=3, random_state=seed)
    model.fit(X_np, y_np)  # numpy array: no feature-name metadata → no sklearn warnings

    explainer = TabularExplainer.from_model(model, dag, X_np, features)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")  # suppress sklearn feature-name warnings during prediction
        asv_rows = [explainer.explain_instance(X_np[i], method=method) for i in range(len(X))]
    asv_matrix = np.array([[row.get(f, 0.0) for f in features] for row in asv_rows])

    mean_abs = np.abs(asv_matrix).mean(axis=0)
    result = pd.DataFrame({"feature": features, "mean_abs_asv": mean_abs})
    return result.sort_values("mean_abs_asv", ascending=False).reset_index(drop=True)


def print_ranking(features_csv: str, target: str = "overall_score") -> None:
    """Print ASV ranking to stdout."""
    ranking = compute_asv(features_csv, target=target)
    print(f"\n=== Quality factor ranking (DAG-aware ASV, target={target}) ===")
    print(f"{'Feature':<32} {'mean|ASV|':>10}")
    print("-" * 44)
    for _, row in ranking.iterrows():
        bar = "█" * max(1, int(row["mean_abs_asv"] * 200))
        print(f"{row['feature']:<32} {row['mean_abs_asv']:>10.4f}  {bar}")


def explain_stability(
    features_csv: str,
    target: str = "overall_score",
    seeds: list[int] | None = None,
) -> "pd.DataFrame":
    """Check ranking stability across multiple random seeds.

    Returns DataFrame with columns [feature, mean_abs_asv, std_abs_asv].
    """
    import numpy as np
    import pandas as pd

    if seeds is None:
        seeds = [42, 43, 44, 45, 46]

    frames = [compute_asv(features_csv, target=target, seed=s) for s in seeds]
    merged = frames[0].rename(columns={"mean_abs_asv": f"asv_{seeds[0]}"})
    for i, frame in enumerate(frames[1:], 1):
        merged = merged.merge(
            frame.rename(columns={"mean_abs_asv": f"asv_{seeds[i]}"}),
            on="feature", how="outer"
        )
    cols = [c for c in merged.columns if c.startswith("asv_")]
    merged["mean_abs_asv"] = merged[cols].mean(axis=1)
    merged["std_abs_asv"]  = merged[cols].std(axis=1)
    return merged[["feature", "mean_abs_asv", "std_abs_asv"]].sort_values(
        "mean_abs_asv", ascending=False
    ).reset_index(drop=True)
