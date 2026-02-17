#!/bin/bash
# Phase 8: Orchestrate a feedback cycle.
#
# Collects failures from Phase 7 evaluation, builds error-aware training data,
# retrains with conservative LoRA, and measures improvement.
#
# Usage:
#   ./run_feedback_cycle.sh          # Run cycle 1
#   ./run_feedback_cycle.sh 2        # Run cycle 2
#   ./run_feedback_cycle.sh 1 --dry-run  # Dry run (show stats only)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CYCLE="${1:-1}"
SHIFT_ARGS=1
DRY_RUN=""

# Check for --dry-run in any position
for arg in "$@"; do
    if [ "$arg" = "--dry-run" ]; then
        DRY_RUN="--dry-run"
    fi
done

echo "=============================================="
echo "Phase 8: Feedback Cycle ${CYCLE}"
echo "=============================================="
echo ""

# Step 1: Collect failures
echo "--- Step 1/4: Collecting failures ---"
python "${SCRIPT_DIR}/scripts/collect_failures.py" \
    --output "${SCRIPT_DIR}/outputs/feedback/failures_v${CYCLE}.jsonl"
echo ""

# Step 2: Build feedback dataset
echo "--- Step 2/4: Building feedback dataset ---"
python "${SCRIPT_DIR}/scripts/build_feedback_dataset.py" \
    --failures "${SCRIPT_DIR}/outputs/feedback/failures_v${CYCLE}.jsonl" \
    --output-7a "${SCRIPT_DIR}/data/splits/feedback_7a.jsonl" \
    --output-7b "${SCRIPT_DIR}/data/splits/feedback_7b.jsonl"
echo ""

# Step 3: Retrain
echo "--- Step 3/4: Retraining 7a ---"
python "${SCRIPT_DIR}/scripts/retrain.py" \
    --task 7a \
    --cycle "${CYCLE}" \
    ${DRY_RUN}
echo ""

# Step 4: Measure improvement
if [ -z "${DRY_RUN}" ]; then
    echo "--- Step 4/4: Measuring improvement ---"
    python "${SCRIPT_DIR}/scripts/measure_improvement.py" \
        --baseline "${SCRIPT_DIR}/outputs/metrics/metrics_7a.json" \
        --improved "${SCRIPT_DIR}/outputs/metrics/metrics_8a_v${CYCLE}.json" \
        --output "${SCRIPT_DIR}/outputs/metrics/improvement_v${CYCLE}.json" \
        --cycle "${CYCLE}"
else
    echo "--- Step 4/4: Skipped (dry run) ---"
fi

echo ""
echo "=============================================="
echo "Feedback cycle ${CYCLE} complete."
echo "=============================================="
