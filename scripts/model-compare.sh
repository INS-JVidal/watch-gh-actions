#!/usr/bin/env bash
set -euo pipefail

SCOPE="${1:-crates/ciw-core}"
MODELS=("sonnet" "opus")
RESULTS_DIR="results/$(date +%Y%m%d-%H%M%S)"
mkdir -p "$RESULTS_DIR"

# Unset CLAUDECODE to allow launching claude subprocesses from within a session
unset CLAUDECODE

echo "=== Model Comparison Experiment ==="
echo "Scope: $SCOPE"
echo "Output: $RESULTS_DIR/"
echo ""

# Phase 1: Run analyses in parallel (each in its own worktree for full isolation)
pids=()
for model in "${MODELS[@]}"; do
  echo "Starting $model analysis..."
  claude -w "${model}-run" -p \
    --model "$model" \
    --dangerously-skip-permissions \
    --no-session-persistence \
    "/code-quality $SCOPE" \
    > "$RESULTS_DIR/${model}.md" 2>"$RESULTS_DIR/${model}.err" &
  pids+=($!)
done

echo "Waiting for both analyses to complete..."
failures=0
for i in "${!pids[@]}"; do
  wait "${pids[$i]}" || {
    rc=$?
    echo "ERROR: ${MODELS[$i]} analysis failed (exit $rc). See $RESULTS_DIR/${MODELS[$i]}.err"
    ((failures++))
  }
done

if [ "$failures" -gt 0 ]; then
  echo "Aborting comparison — $failures analysis run(s) failed."
  exit 1
fi

echo "Both analyses complete."
echo ""

# Phase 2: Generate comparison report
echo "Generating comparison report..."
claude -p --model opus \
  --dangerously-skip-permissions \
  --no-session-persistence \
  "Read these two code quality reports and produce a structured comparison:
   - $RESULTS_DIR/sonnet.md
   - $RESULTS_DIR/opus.md

   Compare across these dimensions:
   1. **Overlap**: Findings flagged by both models at the same file:line
   2. **Unique to opus**: Issues only opus found — are they real or false positives?
   3. **Unique to sonnet**: Issues only sonnet found — same question
   4. **Severity agreement**: Where both find the same issue, do they rate severity the same?
   5. **Depth**: Which model provides more actionable recommendations?
   6. **False positives**: Which model is more prone to flagging non-issues?
   7. **Cost-value**: Is opus's analysis meaningfully better for the ~5x cost difference?

   For each disagreement, state which model you believe is correct and why.
   Output a structured markdown comparison report." \
  > "$RESULTS_DIR/comparison.md"

echo ""
echo "=== Done ==="
echo "Results:"
echo "  $RESULTS_DIR/sonnet.md"
echo "  $RESULTS_DIR/opus.md"
echo "  $RESULTS_DIR/comparison.md"
