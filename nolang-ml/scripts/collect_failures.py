#!/usr/bin/env python3
"""Phase 8: Collect failures from evaluation and human feedback.

Harvests failures from two sources:
1. 7a generation results (outputs/generations/test_7a.jsonl) — validates each
   entry through the Rust CLI and classifies failures into Layers 1-3.
2. Human feedback (outputs/human_feedback.jsonl) — Layer 4 semantic rejections.

Outputs a unified failures.jsonl with layer classification for downstream
feedback dataset building.

Usage:
    # Collect from all available sources
    python scripts/collect_failures.py

    # Specify sources explicitly
    python scripts/collect_failures.py \
        --generations outputs/generations/test_7a.jsonl \
        --human-feedback outputs/human_feedback.jsonl \
        --output outputs/feedback/failures.jsonl

    # Also collect 7b failures (description quality)
    python scripts/collect_failures.py --include-7b
"""

import argparse
import json
import sys
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
ML_ROOT = SCRIPT_DIR.parent


def classify_validation(validation: dict) -> tuple[int, str]:
    """Classify a validation result into failure layer and type.

    Returns (layer, failure_type) or (0, "success") if no failure.
    """
    if not validation.get("assembled", False):
        return 1, "assembly_syntax"
    if not validation.get("verified", False):
        return 2, "verification"
    if validation.get("witnesses_total", 0) > 0 and not validation.get("witnesses_passed", False):
        return 3, "witness_mismatch"
    return 0, "success"


def collect_from_generations(gen_path: Path) -> list[dict]:
    """Collect Layer 1-3 failures from generation results.

    Reads the generations file and runs validation on each entry via
    the validate module.
    """
    if not gen_path.exists():
        print(f"  Generations file not found: {gen_path}", file=sys.stderr)
        return []

    sys.path.insert(0, str(SCRIPT_DIR))
    from validate import validate_assembly

    failures = []
    total = 0
    skipped = 0

    with open(gen_path) as f:
        for line_num, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                print(f"  WARNING: {gen_path}:{line_num}: invalid JSON", file=sys.stderr)
                continue

            total += 1
            gen_asm = entry.get("generated_assembly", "")
            if not gen_asm:
                skipped += 1
                continue

            intent = entry.get("intent", "")
            witnesses = entry.get("witnesses")

            # Validate through Rust CLI
            result = validate_assembly(gen_asm, witnesses)
            layer, failure_type = classify_validation(result.to_dict())

            if layer == 0:
                # No failure — skip
                continue

            error_msg = "; ".join(result.errors) if result.errors else f"Layer {layer} failure"

            failures.append({
                "intent": intent,
                "generated_assembly": gen_asm,
                "failure_layer": layer,
                "failure_type": failure_type,
                "error_message": error_msg,
                "source": "eval_7a",
                "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
            })

    print(f"  Generations: {total} total, {len(failures)} failures, {skipped} skipped")
    return failures


def collect_from_human_feedback(feedback_path: Path) -> list[dict]:
    """Collect Layer 4 failures from human feedback.

    Reads human_feedback.jsonl and extracts entries where feedback == "n".
    Also re-classifies structural failures if the validation data shows
    Layer 1-3 issues (human may have rejected for the wrong reason).
    """
    if not feedback_path.exists():
        print(f"  Human feedback file not found: {feedback_path}", file=sys.stderr)
        return []

    failures = []
    total = 0
    rejected = 0

    with open(feedback_path) as f:
        for line_num, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                print(f"  WARNING: {feedback_path}:{line_num}: invalid JSON", file=sys.stderr)
                continue

            total += 1
            feedback = entry.get("feedback", "")
            if feedback != "n":
                continue

            rejected += 1
            intent = entry.get("intent", "")
            gen_asm = entry.get("generated_assembly", "")
            validation = entry.get("validation", {})

            # Check if the rejection is actually a structural failure
            layer, failure_type = classify_validation(validation)
            if layer == 0:
                # Structurally valid but human rejected → Layer 4
                layer = 4
                failure_type = "semantic_mismatch"

            errors = validation.get("errors", [])
            if layer == 4:
                error_msg = "Human rejected: assembly does not match intent"
            else:
                error_msg = "; ".join(errors) if errors else f"Layer {layer} failure"

            failures.append({
                "intent": intent,
                "generated_assembly": gen_asm,
                "failure_layer": layer,
                "failure_type": failure_type,
                "error_message": error_msg,
                "source": "human_feedback",
                "timestamp": entry.get("timestamp", time.strftime("%Y-%m-%dT%H:%M:%S")),
            })

    print(f"  Human feedback: {total} total, {rejected} rejected")
    return failures


def collect_from_7b_feedback(feedback_path: Path) -> list[dict]:
    """Collect Layer 4 failures for 7b task (description quality).

    Human feedback entries where the assembly was valid but the description
    didn't match the intent are 7b failures.
    """
    if not feedback_path.exists():
        return []

    failures = []

    with open(feedback_path) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
            except json.JSONDecodeError:
                continue

            if entry.get("feedback") != "n":
                continue

            validation = entry.get("validation", {})
            # Only include if assembly was structurally valid
            if not validation.get("assembled") or not validation.get("verified"):
                continue

            failures.append({
                "intent": entry.get("intent", ""),
                "generated_assembly": entry.get("generated_assembly", ""),
                "generated_description": entry.get("description", ""),
                "failure_layer": 4,
                "failure_type": "description_mismatch",
                "error_message": "Human rejected: description does not match intent",
                "source": "human_feedback_7b",
                "timestamp": entry.get("timestamp", time.strftime("%Y-%m-%dT%H:%M:%S")),
            })

    if failures:
        print(f"  7b description failures: {len(failures)}")
    return failures


def dedup_failures(failures: list[dict]) -> list[dict]:
    """Deduplicate failures by intent, keeping the most recent."""
    seen: dict[str, dict] = {}
    for f in failures:
        intent = f.get("intent", "")
        if not intent:
            continue
        existing = seen.get(intent)
        if existing is None or f.get("timestamp", "") >= existing.get("timestamp", ""):
            seen[intent] = f
    return list(seen.values())


def print_summary(failures: list[dict]):
    """Print a summary of collected failures by layer."""
    by_layer: dict[int, int] = {}
    by_source: dict[str, int] = {}
    for f in failures:
        layer = f.get("failure_layer", 0)
        source = f.get("source", "unknown")
        by_layer[layer] = by_layer.get(layer, 0) + 1
        by_source[source] = by_source.get(source, 0) + 1

    print(f"\nCollected {len(failures)} unique failures:")
    for layer in sorted(by_layer):
        label = {1: "Syntax", 2: "Verification", 3: "Witness", 4: "Semantic"}.get(layer, f"Layer {layer}")
        print(f"  Layer {layer} ({label}): {by_layer[layer]}")
    print("  By source:")
    for source, count in sorted(by_source.items()):
        print(f"    {source}: {count}")


def main():
    parser = argparse.ArgumentParser(
        description="Phase 8: Collect failures from evaluation and human feedback"
    )
    parser.add_argument(
        "--generations", type=Path,
        default=ML_ROOT / "outputs" / "generations" / "test_7a.jsonl",
        help="Path to 7a generation results JSONL",
    )
    parser.add_argument(
        "--human-feedback", type=Path,
        default=ML_ROOT / "outputs" / "human_feedback.jsonl",
        help="Path to human feedback JSONL",
    )
    parser.add_argument(
        "--output", type=Path,
        default=ML_ROOT / "outputs" / "feedback" / "failures.jsonl",
        help="Output path for unified failures JSONL",
    )
    parser.add_argument(
        "--include-7b", action="store_true",
        help="Also collect 7b description failures from human feedback",
    )
    args = parser.parse_args()

    print("Phase 8: Collecting failures...")

    all_failures = []

    # Layer 1-3: From generation results
    print("\nSource 1: Generation results")
    gen_failures = collect_from_generations(args.generations)
    all_failures.extend(gen_failures)

    # Layer 1-4: From human feedback
    print("\nSource 2: Human feedback")
    human_failures = collect_from_human_feedback(args.human_feedback)
    all_failures.extend(human_failures)

    # Layer 4 (7b): Description quality failures
    if args.include_7b:
        print("\nSource 3: 7b description failures")
        desc_failures = collect_from_7b_feedback(args.human_feedback)
        all_failures.extend(desc_failures)

    # Deduplicate by intent (keep most recent)
    unique_failures = dedup_failures(all_failures)

    # Write output
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with open(args.output, "w") as f:
        for failure in unique_failures:
            f.write(json.dumps(failure, ensure_ascii=False) + "\n")

    print_summary(unique_failures)
    print(f"\nWritten to: {args.output}")


if __name__ == "__main__":
    main()
