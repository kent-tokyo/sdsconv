"""causasv quality analysis — SDS JSON 品質劣化の因果寄与分析

Usage:
    cd sdsconv_py
    pip install causasv scikit-learn pandas
    python3 ../examples/causasv_quality_analysis.py ../runs/eval_YYYYMMDD/causasv_features.csv

または最新の runs/ を自動検出:
    python3 ../examples/causasv_quality_analysis.py
"""
from __future__ import annotations

import sys
from pathlib import Path

# ── 依存確認 ──────────────────────────────────────────────────────────────────
for pkg in ("causasv", "sklearn", "pandas", "numpy"):
    try:
        __import__(pkg)
    except ImportError:
        print(f"[ERROR] {pkg} not found. pip install causasv scikit-learn pandas")
        sys.exit(1)

# ── CSV パス解決 ──────────────────────────────────────────────────────────────
if len(sys.argv) >= 2:
    csv_path = Path(sys.argv[1])
else:
    # sdsconv_py/ または repo root から実行された場合に最新 run を自動検出
    for candidate in [Path("runs"), Path("../runs")]:
        if candidate.exists():
            runs = sorted(candidate.iterdir(), reverse=True)
            for run in runs:
                p = run / "causasv_features.csv"
                if p.exists():
                    csv_path = p
                    break
            else:
                continue
            break
    else:
        print("Usage: python3 causasv_quality_analysis.py <causasv_features.csv>")
        sys.exit(1)

print(f"Loading: {csv_path}")

# ── sdsconv_py パスを sys.path に追加 ─────────────────────────────────────────
for sp in [Path(__file__).parent.parent / "sdsconv_py" / "python",
           Path("python"), Path("../sdsconv_py/python")]:
    if (sp / "sdsconv").exists():
        sys.path.insert(0, str(sp))
        break

from sdsconv.causasv_bridge import compute_asv, print_ranking, explain_stability

import pandas as pd

df = pd.read_csv(csv_path)
print(f"\nDataset: {len(df)} files")
print(f"  json_ok: {df['json_ok'].sum() if 'json_ok' in df.columns else '?'}")
if "overall_score" in df.columns:
    print(f"  avg score: {df['overall_score'].mean():.1f}")
    print(f"  grades:    {df['grade'].value_counts().to_dict() if 'grade' in df.columns else '?'}")

# ── ASV ランキング ─────────────────────────────────────────────────────────────
print_ranking(str(csv_path), target="overall_score")

# ── 安定性チェック（サンプルが5件以上あれば）────────────────────────────────────
if len(df) >= 5:
    print("\n=== Ranking stability (5 seeds) ===")
    stab = explain_stability(str(csv_path))
    print(stab.head(10).to_string(index=False))
else:
    print(f"\n(stability check skipped — need ≥5 samples, got {len(df)})")

# ── DAG 可視化 ────────────────────────────────────────────────────────────────
try:
    from sdsconv.causasv_bridge import build_dag
    dag = build_dag()
    print(f"\nDAG: {len(dag.nodes())} nodes, {len(dag.edges())} edges")
    try:
        print(dag.to_dot()[:300], "...")
    except Exception:
        pass
except Exception as e:
    print(f"\n(DAG info unavailable: {e})")
