#!/usr/bin/env bash
# Run eval-corpus with settings from .env
# Usage: ./tools/run_eval.sh [--jobs N] [--max-files N] [extra sdsconv flags...]
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$SCRIPT_DIR/.."
cd "$ROOT"

# Load .env if present
if [ -f .env ]; then
  set -a; source .env; set +a
fi

RUN_ID="eval_$(date +%Y%m%d_%H%M%S)"
OUTPUT_DIR="runs/$RUN_ID"
mkdir -p "$OUTPUT_DIR"

# Input dir: prefer corpus/raw if populated, fall back to data/sds_raw
if [ -n "${INPUT_DIR:-}" ]; then
  : # caller set it explicitly
elif [ -d "corpus/raw" ] && [ "$(ls -A corpus/raw 2>/dev/null)" ]; then
  INPUT_DIR="corpus/raw"
else
  INPUT_DIR="data/sds_raw"
fi

echo "=== eval-corpus: $RUN_ID ==="
echo "input:  $INPUT_DIR/"
echo "output: $OUTPUT_DIR/"
echo ""

./target/release/sdsconv eval-corpus \
  --input-dir  "$INPUT_DIR" \
  --output-dir "$OUTPUT_DIR" \
  --jobs       4 \
  --correct \
  --qc-script  tools/quality_check.py \
  "$@" \
  2>&1 | tee "$OUTPUT_DIR/run.log"

echo ""
echo "=== Results ==="
cat "$OUTPUT_DIR/summary.md" 2>/dev/null || echo "(summary.md not generated)"
echo ""
echo "causasv_features.csv:"
head -3 "$OUTPUT_DIR/causasv_features.csv" 2>/dev/null || echo "(not generated)"
