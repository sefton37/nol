#!/usr/bin/env python3
"""Phase 8: Measure improvement after feedback retraining.

Compares evaluation metrics between the Phase 7 baseline and Phase 8 retrained
model. Reports side-by-side metrics, per-layer breakdown, and regression check.

Usage:
    # Compare baseline vs retrained (uses evaluate.py under the hood)
    python scripts/measure_improvement.py \
        --baseline outputs/metrics/metrics_7a.json \
        --improved outputs/metrics/metrics_8a_v1.json

    # Full evaluation: generate + evaluate + compare
    python scripts/measure_improvement.py \
        --baseline-adapter models/7a_intent_to_asm/final \
        --improved-adapter models/7a_intent_to_asm/feedback_v1/final \
        --run-eval

    # Compare from pre-computed generation files
    python scripts/measure_improvement.py \
        --baseline-generations outputs/generations/test_7a.jsonl \
        --improved-generations outputs/generations/test_7a_feedback_v1.jsonl

    # Specify output path
    python scripts/measure_improvement.py \
        --baseline outputs/metrics/metrics_7a.json \
        --improved outputs/metrics/metrics_8a_v1.json \
        --output outputs/metrics/improvement_v1.json
"""

import argparse
import json
import sys
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
ML_ROOT = SCRIPT_DIR.parent


def load_metrics(path: Path) -> dict:
    """Load metrics JSON file."""
    if not path.exists():
        print(f"ERROR: Metrics file not found: {path}", file=sys.stderr)
        sys.exit(1)
    with open(path) as f:
        return json.load(f)


def evaluate_generations(gen_path: Path) -> dict:
    """Run evaluation on a generations file and return metrics."""
    sys.path.insert(0, str(SCRIPT_DIR))
    from evaluate import evaluate_7a
    return evaluate_7a(gen_path)


def compute_deltas(baseline: dict, improved: dict) -> dict:
    """Compute metric deltas between baseline and improved."""
    metrics = ["assembled_pct", "verified_pct", "witness_pass_pct"]
    labels = ["Syntax validity", "Verification pass", "Witness pass"]

    deltas = {}
    for metric, label in zip(metrics, labels):
        base_val = baseline.get(metric, 0.0)
        imp_val = improved.get(metric, 0.0)
        delta = imp_val - base_val
        deltas[metric] = {
            "label": label,
            "baseline": base_val,
            "improved": imp_val,
            "delta": delta,
        }

    return deltas


def check_gates(deltas: dict) -> tuple[bool, list[str]]:
    """Check acceptance gates.

    Gates:
    - At least 2 of 3 7a metrics improve
    - No metric regresses by more than 2%

    Returns (passed, messages).
    """
    messages = []
    improved_count = 0
    regression_fail = False

    metrics_order = ["assembled_pct", "verified_pct", "witness_pass_pct"]

    for metric in metrics_order:
        info = deltas.get(metric)
        if info is None:
            continue
        delta = info["delta"]
        label = info["label"]

        if delta > 0:
            improved_count += 1
        if delta < -2.0:
            regression_fail = True
            messages.append(f"REGRESSION: {label} dropped {abs(delta):.1f}% (> 2% threshold)")

    gate_improved = improved_count >= 2
    if gate_improved:
        messages.append(f"PASS: {improved_count}/3 metrics improved (need >= 2)")
    else:
        messages.append(f"FAIL: Only {improved_count}/3 metrics improved (need >= 2)")

    if regression_fail:
        messages.append("FAIL: Regression exceeds 2% threshold")
    else:
        messages.append("PASS: No metric regressed > 2%")

    passed = gate_improved and not regression_fail
    return passed, messages


def find_regressions(
    baseline_gen_path: Path | None,
    improved_gen_path: Path | None,
) -> list[dict]:
    """Find examples that passed in baseline but fail in improved.

    Only available when generation files are provided.
    """
    if baseline_gen_path is None or improved_gen_path is None:
        return []
    if not baseline_gen_path.exists() or not improved_gen_path.exists():
        return []

    sys.path.insert(0, str(SCRIPT_DIR))
    from validate import validate_assembly

    # Load both generation files
    baseline_entries = {}
    with open(baseline_gen_path) as f:
        for line in f:
            entry = json.loads(line.strip())
            baseline_entries[entry["intent"]] = entry

    improved_entries = {}
    with open(improved_gen_path) as f:
        for line in f:
            entry = json.loads(line.strip())
            improved_entries[entry["intent"]] = entry

    regressions = []
    for intent, base_entry in baseline_entries.items():
        imp_entry = improved_entries.get(intent)
        if imp_entry is None:
            continue

        # Validate both
        base_result = validate_assembly(
            base_entry["generated_assembly"],
            base_entry.get("witnesses"),
        )
        imp_result = validate_assembly(
            imp_entry["generated_assembly"],
            imp_entry.get("witnesses"),
        )

        # Check for regression: base passed, improved failed
        if base_result.fully_valid and not imp_result.fully_valid:
            regressions.append({
                "intent": intent,
                "baseline_status": "valid",
                "improved_status": "invalid",
                "improved_errors": imp_result.errors,
            })

    return regressions


def print_report(deltas: dict, gates_passed: bool, gate_messages: list[str],
                 regressions: list[dict], cycle: int):
    """Print formatted improvement report."""
    print("\n" + "=" * 70)
    print(f"IMPROVEMENT REPORT (Cycle {cycle})")
    print("=" * 70)

    # Side-by-side table
    print(f"\n{'Metric':<22} {'Phase 7':>10} {'Phase 8':>10} {'Delta':>10}")
    print("-" * 54)

    for info in deltas.values():
        label = info["label"]
        baseline = info["baseline"]
        improved = info["improved"]
        delta = info["delta"]
        sign = "+" if delta >= 0 else ""
        print(f"  {label:<20} {baseline:>8.1f}%  {improved:>8.1f}%  {sign}{delta:>7.1f}%")

    # Gates
    print(f"\n{'Gates':}")
    print("-" * 54)
    for msg in gate_messages:
        print(f"  {msg}")
    print(f"\n  Overall: {'PASS' if gates_passed else 'FAIL'}")

    # Regressions
    if regressions:
        print(f"\nRegressions ({len(regressions)} examples):")
        print("-" * 54)
        for reg in regressions[:10]:  # Show first 10
            print(f"  {reg['intent'][:60]}")
            if reg.get("improved_errors"):
                print(f"    Errors: {'; '.join(reg['improved_errors'][:2])}")
        if len(regressions) > 10:
            print(f"  ... and {len(regressions) - 10} more")

    print("\n" + "=" * 70)


def main():
    parser = argparse.ArgumentParser(
        description="Phase 8: Measure improvement after feedback retraining"
    )
    parser.add_argument(
        "--baseline", type=Path,
        help="Path to baseline metrics JSON (from evaluate.py)",
    )
    parser.add_argument(
        "--improved", type=Path,
        help="Path to improved metrics JSON",
    )
    parser.add_argument(
        "--baseline-generations", type=Path,
        help="Path to baseline generation JSONL (for regression analysis)",
    )
    parser.add_argument(
        "--improved-generations", type=Path,
        help="Path to improved generation JSONL (for regression analysis)",
    )
    parser.add_argument(
        "--output", type=Path,
        default=None,
        help="Output path for improvement report JSON",
    )
    parser.add_argument(
        "--cycle", type=int, default=1,
        help="Feedback cycle number",
    )
    args = parser.parse_args()

    # Determine metrics sources
    baseline_metrics = None
    improved_metrics = None

    if args.baseline and args.baseline.exists():
        baseline_metrics = load_metrics(args.baseline)
    elif args.baseline_generations and args.baseline_generations.exists():
        print("Evaluating baseline generations...")
        baseline_metrics = evaluate_generations(args.baseline_generations)
    else:
        # Try default locations
        default_baseline = ML_ROOT / "outputs" / "metrics" / "metrics_7a.json"
        if default_baseline.exists():
            baseline_metrics = load_metrics(default_baseline)
        else:
            print("ERROR: No baseline metrics found. Provide --baseline or --baseline-generations.",
                  file=sys.stderr)
            sys.exit(1)

    if args.improved and args.improved.exists():
        improved_metrics = load_metrics(args.improved)
    elif args.improved_generations and args.improved_generations.exists():
        print("Evaluating improved generations...")
        improved_metrics = evaluate_generations(args.improved_generations)
    else:
        default_improved = ML_ROOT / "outputs" / "metrics" / f"metrics_8a_v{args.cycle}.json"
        if default_improved.exists():
            improved_metrics = load_metrics(default_improved)
        else:
            print("ERROR: No improved metrics found. Provide --improved or --improved-generations.",
                  file=sys.stderr)
            sys.exit(1)

    # Compute deltas
    deltas = compute_deltas(baseline_metrics, improved_metrics)

    # Check gates
    gates_passed, gate_messages = check_gates(deltas)

    # Find regressions (if generation files available)
    regressions = find_regressions(
        args.baseline_generations, args.improved_generations
    )

    # Print report
    print_report(deltas, gates_passed, gate_messages, regressions, args.cycle)

    # Save report
    output_path = args.output
    if output_path is None:
        output_path = ML_ROOT / "outputs" / "metrics" / f"improvement_v{args.cycle}.json"
    output_path.parent.mkdir(parents=True, exist_ok=True)

    report = {
        "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "cycle": args.cycle,
        "baseline": baseline_metrics,
        "improved": improved_metrics,
        "deltas": {k: v for k, v in deltas.items()},
        "gates_passed": gates_passed,
        "gate_messages": gate_messages,
        "regressions_count": len(regressions),
        "regressions": regressions[:20],  # Cap at 20 for file size
    }
    with open(output_path, "w") as f:
        json.dump(report, f, indent=2)

    print(f"\nReport saved to: {output_path}")

    # Exit code reflects gate status
    sys.exit(0 if gates_passed else 1)


if __name__ == "__main__":
    main()
