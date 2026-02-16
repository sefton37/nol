#!/usr/bin/env python3
"""Phase 7c: Interactive CLI comparison UI.

For each test example:
1. Generate assembly from intent (7a)
2. Validate through Rust CLI
3. Generate description from assembly (7b)
4. Display intent, validation status, and description
5. Collect human feedback (yes/no/skip/quit)

Logs feedback to outputs/human_feedback.jsonl for Phase 8.

Usage:
    # Interactive comparison on test set
    python scripts/compare.py

    # Use pre-generated files
    python scripts/compare.py \
        --7a-generations outputs/generations/test_7a.jsonl \
        --7b-descriptions outputs/descriptions/test_7b.jsonl

    # Limit number of examples
    python scripts/compare.py --limit 50
"""

import argparse
import json
import sys
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
ML_ROOT = SCRIPT_DIR.parent

SEPARATOR = "=" * 80


def load_test_data(test_path: Path) -> list[dict]:
    """Load test set entries."""
    entries = []
    with open(test_path) as f:
        for line in f:
            line = line.strip()
            if line:
                entries.append(json.loads(line))
    return entries


def load_pre_generated(gen_7a_path: Path | None, desc_7b_path: Path | None):
    """Load pre-generated 7a and 7b outputs."""
    gen_7a = {}
    desc_7b = {}

    if gen_7a_path and gen_7a_path.exists():
        with open(gen_7a_path) as f:
            for line in f:
                entry = json.loads(line.strip())
                gen_7a[entry["intent"]] = entry.get("generated_assembly", "")

    if desc_7b_path and desc_7b_path.exists():
        with open(desc_7b_path) as f:
            for line in f:
                entry = json.loads(line.strip())
                desc_7b[entry["assembly"]] = entry.get("generated_description", "")

    return gen_7a, desc_7b


def display_comparison(
    index: int,
    total: int,
    intent: str,
    assembly: str,
    validation_status: str,
    description: str,
):
    """Display a single comparison entry."""
    print(f"\n{SEPARATOR}")
    print(f"Program {index} of {total}")
    print(SEPARATOR)
    print(f"\nYOU SAID:")
    print(f"  {intent}")
    print(f"\nGENERATED ASSEMBLY:")
    for line in assembly.split("\n"):
        if line.strip():
            print(f"  {line}")
    print(f"\nVALIDATION: {validation_status}")
    print(f"\nTHIS CODE DOES:")
    print(f"  {description}")
    print(f"\n{SEPARATOR}")


def get_feedback() -> str:
    """Get user feedback: y/n/s/q."""
    while True:
        try:
            response = input("Does this match your intent? [y/n/s/q] (yes/no/skip/quit): ").strip().lower()
        except (EOFError, KeyboardInterrupt):
            return "q"
        if response in ("y", "yes"):
            return "y"
        elif response in ("n", "no"):
            return "n"
        elif response in ("s", "skip"):
            return "s"
        elif response in ("q", "quit", "exit"):
            return "q"
        else:
            print("  Please enter y, n, s, or q")


def run_comparison(
    test_entries: list[dict],
    gen_7a: dict[str, str],
    desc_7b: dict[str, str],
    feedback_path: Path,
    model_7a=None,
    tokenizer_7a=None,
    config_7a=None,
    model_7b=None,
    tokenizer_7b=None,
    config_7b=None,
    limit: int | None = None,
):
    """Run the interactive comparison loop."""
    sys.path.insert(0, str(SCRIPT_DIR))
    from validate import validate_assembly

    feedback_path.parent.mkdir(parents=True, exist_ok=True)

    total = min(len(test_entries), limit) if limit else len(test_entries)
    yes_count = 0
    no_count = 0
    skip_count = 0
    reviewed = 0

    with open(feedback_path, "a") as fout:
        for i, entry in enumerate(test_entries[:total], 1):
            intent = entry["intent"]

            # Get or generate assembly
            if intent in gen_7a:
                assembly = gen_7a[intent]
            elif model_7a is not None:
                from inference_7a import generate_assembly
                assembly = generate_assembly(model_7a, tokenizer_7a, intent, config_7a)
            else:
                print(f"\n[{i}/{total}] Skipping (no 7a model/generation): {intent[:50]}...")
                skip_count += 1
                continue

            # Validate
            witnesses = entry.get("witnesses")
            vr = validate_assembly(assembly, witnesses)
            if vr.fully_valid:
                if vr.witnesses_passed:
                    validation_status = "PASS (assembled + verified + witnesses)"
                elif vr.witnesses_total > 0:
                    validation_status = f"PARTIAL (assembled + verified, witnesses: {vr.witnesses_ok}/{vr.witnesses_total})"
                else:
                    validation_status = "PASS (assembled + verified)"
            elif vr.assembled:
                validation_status = f"PARTIAL (assembled, verification failed: {'; '.join(vr.errors)})"
            else:
                validation_status = f"FAIL ({'; '.join(vr.errors)})"

            # Get or generate description
            asm_key = assembly
            if asm_key in desc_7b:
                description = desc_7b[asm_key]
            elif model_7b is not None:
                from inference_7b import generate_description
                description = generate_description(model_7b, tokenizer_7b, assembly, config_7b)
            else:
                description = "(no 7b model/generation available)"

            # Display
            display_comparison(i, total, intent, assembly, validation_status, description)

            # Collect feedback
            feedback = get_feedback()
            if feedback == "q":
                print("\nQuitting...")
                break
            elif feedback == "y":
                yes_count += 1
                reviewed += 1
            elif feedback == "n":
                no_count += 1
                reviewed += 1
            elif feedback == "s":
                skip_count += 1
                continue

            # Log feedback
            record = {
                "timestamp": time.strftime("%Y-%m-%dT%H:%M:%S"),
                "intent": intent,
                "generated_assembly": assembly,
                "description": description,
                "validation": vr.to_dict(),
                "feedback": feedback,
            }
            fout.write(json.dumps(record, ensure_ascii=False) + "\n")
            fout.flush()

    # Summary
    print(f"\n{SEPARATOR}")
    print("COMPARISON SUMMARY")
    print(SEPARATOR)
    print(f"  Reviewed:  {reviewed}")
    print(f"  Matched:   {yes_count}")
    print(f"  Mismatched: {no_count}")
    print(f"  Skipped:   {skip_count}")
    if reviewed > 0:
        match_rate = yes_count / reviewed * 100
        print(f"  Match rate: {match_rate:.1f}%  target ≥75%  {'PASS' if match_rate >= 75 else 'FAIL'}")
    print(f"\n  Feedback logged to: {feedback_path}")
    print(SEPARATOR)


def main():
    parser = argparse.ArgumentParser(description="Phase 7c: Interactive comparison UI")
    parser.add_argument("--test-data", type=Path,
                        default=ML_ROOT / "data" / "splits" / "test_7a.jsonl",
                        help="Test data JSONL file")
    parser.add_argument("--7a-generations", type=Path,
                        help="Pre-generated 7a assemblies JSONL")
    parser.add_argument("--7b-descriptions", type=Path,
                        help="Pre-generated 7b descriptions JSONL")
    parser.add_argument("--feedback", type=Path,
                        default=ML_ROOT / "outputs" / "human_feedback.jsonl",
                        help="Path to save feedback JSONL")
    parser.add_argument("--limit", type=int, help="Max examples to review")
    parser.add_argument("--use-models", action="store_true",
                        help="Load trained models for live generation (slow)")
    args = parser.parse_args()

    if not args.test_data.exists():
        print(f"ERROR: Test data not found: {args.test_data}", file=sys.stderr)
        print("Run 'python scripts/prepare_data.py' first.", file=sys.stderr)
        sys.exit(1)

    test_entries = load_test_data(args.test_data)
    print(f"Loaded {len(test_entries)} test entries")

    gen_7a, desc_7b = load_pre_generated(
        args.__dict__.get("7a_generations"),
        args.__dict__.get("7b_descriptions"),
    )

    model_7a = tokenizer_7a = config_7a = None
    model_7b = tokenizer_7b = config_7b = None

    if args.use_models:
        print("Loading 7a model...")
        from inference_7a import load_config as load_config_7a
        from inference_7a import load_model as load_model_7a
        config_7a = load_config_7a()
        model_7a, tokenizer_7a = load_model_7a(config_7a)

        print("Loading 7b model...")
        from inference_7b import load_config as load_config_7b
        from inference_7b import load_model as load_model_7b
        config_7b = load_config_7b()
        model_7b, tokenizer_7b = load_model_7b(config_7b)

    if not gen_7a and model_7a is None:
        print("WARNING: No 7a generations or model. Use --7a-generations or --use-models.")
        print("Will skip examples without pre-generated assembly.")

    run_comparison(
        test_entries=test_entries,
        gen_7a=gen_7a,
        desc_7b=desc_7b,
        feedback_path=args.feedback,
        model_7a=model_7a,
        tokenizer_7a=tokenizer_7a,
        config_7a=config_7a,
        model_7b=model_7b,
        tokenizer_7b=tokenizer_7b,
        config_7b=config_7b,
        limit=args.limit,
    )


if __name__ == "__main__":
    main()
