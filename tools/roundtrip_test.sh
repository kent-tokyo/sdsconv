#!/bin/zsh
# Round-trip conversion test: PDF → JSON → DOCX with r23 quality analysis
# Usage: ./tools/roundtrip_test.sh [N=30]
#
# Randomly selects PDFs balanced across languages (ja/en/zh-cn/zh-tw),
# runs to-json + quality_check.py + validate + to-docx, then prints a summary.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$REPO_ROOT/target/debug/sds-converter"
QC="$REPO_ROOT/tools/quality_check.py"
REFS="$REPO_ROOT/references/sds"
OUT_DIR="/tmp/sds_roundtrip_$(date +%Y%m%d_%H%M%S)"
ENV_FILE="$REPO_ROOT/.env"
N="${1:-30}"

if [[ -f "$ENV_FILE" ]]; then
  # export all variables defined in .env so child processes inherit them
  set -a
  source "$ENV_FILE"
  set +a
fi
if [[ -z "${ANTHROPIC_API_KEY:-}" ]]; then
  echo "ERROR: ANTHROPIC_API_KEY not set (add it to .env or export it)" >&2
  exit 1
fi

mkdir -p "$OUT_DIR"/{json,docx,logs,qc}

echo "================================================================"
echo " SDS Round-trip Test (n=$N) — QC r23"
echo " Output : $OUT_DIR"
echo " Binary : $BIN"
echo " Date   : $(date)"
echo "================================================================"
echo ""

# ── 1. Balanced random selection ─────────────────────────────────────────
# Associative array (requires bash 4+ or zsh; shebang is /bin/zsh)
typeset -A LANG_COUNTS
LANG_COUNTS=([ja]=10 [zh-cn]=10 [en]=7 [zh-tw]=3)
SELECTED=()
for lang in ja zh-cn en zh-tw; do
  count=${LANG_COUNTS[$lang]}
  candidates=()
  while IFS= read -r f; do candidates+=("$f"); done < <(find "$REFS/$lang" -name "*.pdf" 2>/dev/null)
  if [[ ${#candidates[@]} -eq 0 ]]; then
    echo "  WARN: no PDFs found for lang=$lang"
    continue
  fi
  picked=()
  while IFS= read -r f; do picked+=("$f"); done < <(printf '%s\n' "${candidates[@]}" | sort -R | head -n "$count")
  SELECTED+=("${picked[@]}")
  echo "  Selected ${#picked[@]}  lang=$lang"
done
echo "  Total: ${#SELECTED[@]} PDFs"
echo ""

# ── 2. Per-file processing ────────────────────────────────────────────────
PASS=0; FAIL_JSON=0; FAIL_DOCX=0
TOTAL_CRIT=0; TOTAL_HIGH=0; TOTAL_MED=0
REPORT_ROWS=()
JSONL_FILE="$OUT_DIR/results.jsonl"

for pdf in "${SELECTED[@]}"; do
  name=$(basename "$pdf" .pdf)
  lang=$(echo "$pdf" | sed 's|.*/sds/\([^/]*\)/.*|\1|')
  json_out="$OUT_DIR/json/${lang}_${name}.json"
  docx_out="$OUT_DIR/docx/${lang}_${name}.docx"
  log_out="$OUT_DIR/logs/${lang}_${name}.log"
  qc_out="$OUT_DIR/qc/${lang}_${name}.txt"

  echo "────────────────────────────────────────────────────────────"
  echo "  FILE : $(basename "$pdf")"
  echo "  LANG : $lang"

  # ── Step A: PDF → JSON ──────────────────────────────────────────────────
  t0=$SECONDS
  if "$BIN" to-json \
      --input "$pdf" \
      --output "$json_out" \
      --lang "$lang" \
      --quality medium \
      --correct \
      > "$log_out" 2>&1; then
    t1=$SECONDS
    echo "  to-json : OK  ($(( t1 - t0 ))s)"
  else
    t1=$SECONDS
    echo "  to-json : FAIL  ($(( t1 - t0 ))s)"
    tail -10 "$log_out" | sed 's/^/    /'
    FAIL_JSON=$(( FAIL_JSON + 1 ))
    REPORT_ROWS+=("FAIL_JSON | $lang | $name | - | - | - | -")
    echo ""
    continue
  fi

  # ── Step B: Built-in validator ─────────────────────────────────────────
  val_json=$("$BIN" validate --input "$json_out" --json 2>/dev/null || echo "[]")
  val_err=$(python3 -c "
import sys,json
items=json.loads(sys.stdin.read())
if not isinstance(items, list): items=[]
e=sum(1 for i in items if isinstance(i,dict) and i.get('level','').lower()=='error')
w=sum(1 for i in items if isinstance(i,str) or (isinstance(i,dict) and i.get('level','').lower()=='warning'))
print(f'err={e} warn={w}')" <<< "$val_json" 2>/dev/null || echo "err=? warn=?")
  echo "  validate: $val_err"
  # Print first 5 built-in issues
  python3 -c "
import sys,json
items=json.loads(sys.stdin.read())
if not isinstance(items, list): items=[]
for i in items[:5]:
    msg = i if isinstance(i,str) else i.get('message',str(i))
    lvl = 'WARN' if isinstance(i,str) else i.get('level','?').upper()
    print(f'    [{lvl}] {msg}')" <<< "$val_json" 2>/dev/null || true

  # ── Step C: QC r23 analysis ────────────────────────────────────────────
  qc_summary=$(python3 "$QC" "$json_out" "$lang" --jsonl 2>/dev/null | tee "$qc_out" || true)
  qc_text=$(head -n -1 "$qc_out" 2>/dev/null || cat "$qc_out")
  qc_jsonl=$(tail -1 "$qc_out" 2>/dev/null || echo '{}')

  crit=$(printf '%s' "$qc_jsonl"  | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(d.get('crit',0))"  2>/dev/null || echo 0)
  high=$(printf '%s' "$qc_jsonl"  | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(d.get('high',0))"  2>/dev/null || echo 0)
  med=$(printf '%s'  "$qc_jsonl"  | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(d.get('med',0))"   2>/dev/null || echo 0)
  total=$(printf '%s' "$qc_jsonl" | python3 -c "import json,sys; d=json.loads(sys.stdin.read()); print(d.get('total',0))" 2>/dev/null || echo 0)

  echo "  QC r23 : CRIT=$crit HIGH=$high MED=$med TOTAL=$total"
  # Print CRIT/HIGH lines
  grep -E "^QC-(CRIT|HIGH):" "$qc_out" 2>/dev/null | sed 's/^/    /' || true

  TOTAL_CRIT=$(( TOTAL_CRIT + crit ))
  TOTAL_HIGH=$(( TOTAL_HIGH + high ))
  TOTAL_MED=$(( TOTAL_MED + med ))

  # Append JSONL
  echo "$qc_jsonl" >> "$JSONL_FILE"

  # ── Step D: JSON → DOCX ────────────────────────────────────────────────
  docx_size=0
  DOCX_STATUS="SKIP"
  if "$BIN" to-docx \
      --input "$json_out" \
      --output "$docx_out" \
      >> "$log_out" 2>&1; then
    docx_size=$(wc -c < "$docx_out" 2>/dev/null || echo 0)
    echo "  to-docx : OK  (${docx_size} bytes)"
    DOCX_STATUS="OK"
  else
    echo "  to-docx : FAIL"
    tail -5 "$log_out" | sed 's/^/    /'
    FAIL_DOCX=$(( FAIL_DOCX + 1 ))
    DOCX_STATUS="FAIL"
  fi

  PASS=$(( PASS + 1 ))
  REPORT_ROWS+=("OK | $lang | $name | CRIT=$crit HIGH=$high MED=$med | $val_err | $DOCX_STATUS | ${docx_size}B")
  echo ""
done

# ── 3. Summary ────────────────────────────────────────────────────────────
total_files=${#SELECTED[@]}
echo "================================================================"
echo " FINAL SUMMARY"
echo "================================================================"
printf "  to-json    : %d / %d succeeded\n" "$PASS" "$total_files"
printf "  FAIL_JSON  : %d\n" "$FAIL_JSON"
printf "  FAIL_DOCX  : %d\n" "$FAIL_DOCX"
printf "  QC issues  : CRIT=%d  HIGH=%d  MED=%d\n" "$TOTAL_CRIT" "$TOTAL_HIGH" "$TOTAL_MED"
echo ""
echo "  Per-file results:"
printf "  %-8s | %-7s | %-38s | %-22s | %-18s | %-6s | %s\n" \
  "STATUS" "LANG" "FILE" "QC (r23)" "VALIDATOR" "DOCX" "SIZE"
echo "  $(printf '%.s─' {1..115})"
for row in "${REPORT_ROWS[@]}"; do
  printf "  %s\n" "$row"
done

# ── 4. Rule frequency analysis ────────────────────────────────────────────
if [[ -f "$JSONL_FILE" ]]; then
  echo ""
  echo "  Top 10 most frequent QC issues:"
  python3 - "$JSONL_FILE" << 'PYEOF'
import json, sys
from collections import Counter

counts = Counter()
for line in open(sys.argv[1]):
    line = line.strip()
    if not line: continue
    try:
        d = json.loads(line)
        for iss in d.get("issues", []):
            counts[f"[{iss['level']}] {iss['rule']}"] += 1
    except: pass

for rule, cnt in counts.most_common(10):
    print(f"    {cnt:>4}x  {rule}")
PYEOF
fi

echo ""
echo "  Output     : $OUT_DIR"
echo "  JSONL log  : $JSONL_FILE"
echo "Done."
