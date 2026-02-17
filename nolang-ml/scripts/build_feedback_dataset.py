#!/usr/bin/env python3
"""Phase 8: Build error-aware feedback dataset for retraining.

Pairs each failure with the correct reference assembly from the corpus,
creating error-aware SFT training examples where the system prompt includes
error context from the previous failed attempt.

Layer 1-3 failures (structural) get error message context.
Layer 4 failures (semantic) get the incorrect assembly as negative example.

Usage:
    python scripts/build_feedback_dataset.py

    # Specify paths explicitly
    python scripts/build_feedback_dataset.py \
        --failures outputs/feedback/failures.jsonl \
        --corpus-dir ../../tests/corpus \
        --output-7a data/splits/feedback_7a.jsonl \
        --output-7b data/splits/feedback_7b.jsonl
"""

import argparse
import json
import sys
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
ML_ROOT = SCRIPT_DIR.parent
PROJECT_ROOT = ML_ROOT.parent


def load_corpus(corpus_dir: Path) -> dict[str, str]:
    """Load reference assemblies from corpus files, indexed by intent.

    Returns dict mapping normalized intent → assembly.
    If an intent appears multiple times, keeps the first occurrence.
    """
    corpus: dict[str, str] = {}

    for corpus_file in sorted(corpus_dir.glob("*.nolt")):
        with open(corpus_file) as f:
            for line_num, line in enumerate(f, 1):
                line = line.strip()
                if not line:
                    continue
                try:
                    entry = json.loads(line)
                except json.JSONDecodeError:
                    print(f"  WARNING: {corpus_file}:{line_num}: invalid JSON", file=sys.stderr)
                    continue

                intent = entry.get("intent", "").strip()
                assembly = entry.get("assembly", "").strip()
                if intent and assembly and intent not in corpus:
                    corpus[intent] = assembly

    return corpus


def load_training_data(splits_dir: Path) -> dict[str, str]:
    """Load reference assemblies from training splits as fallback.

    Checks train_7a.jsonl, val_7a.jsonl, test_7a.jsonl.
    """
    refs: dict[str, str] = {}
    for split in ["train_7a.jsonl", "val_7a.jsonl", "test_7a.jsonl"]:
        path = splits_dir / split
        if not path.exists():
            continue
        with open(path) as f:
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    entry = json.loads(line)
                except json.JSONDecodeError:
                    continue
                intent = entry.get("intent", "").strip()
                assembly = entry.get("assembly", "").strip()
                if intent and assembly and intent not in refs:
                    refs[intent] = assembly
    return refs


def build_7a_entry(failure: dict, reference_assembly: str) -> dict:
    """Build an error-aware SFT entry for 7a (intent → assembly) task.

    Layer 1-3: System prompt includes the error message.
    Layer 4: System prompt includes the incorrect assembly as negative example.
    """
    layer = failure.get("failure_layer", 0)
    error_msg = failure.get("error_message", "")
    gen_asm = failure.get("generated_assembly", "")

    if layer <= 3:
        # Structural failure — include error context
        system_suffix = (
            f"\nA previous attempt produced an error: {error_msg}\n"
            "Generate correct assembly."
        )
    else:
        # Semantic failure — include incorrect assembly as negative example
        system_suffix = (
            "\nA previous attempt was syntactically valid but did not match the user's intent.\n"
            f"The incorrect assembly was:\n{gen_asm}\n"
            "Generate the correct assembly instead."
        )

    return {
        "intent": failure["intent"],
        "assembly": reference_assembly,
        "system_suffix": system_suffix,
        "failure_layer": layer,
        "failure_type": failure.get("failure_type", ""),
        "source": "feedback",
    }


def build_7b_entry(failure: dict, reference_description: str | None = None) -> dict | None:
    """Build an error-aware SFT entry for 7b (assembly → description) task.

    Only for Layer 4 failures where the description didn't match the intent.
    """
    if failure.get("failure_type") != "description_mismatch":
        return None

    # For 7b, the "correct" description is the original intent
    description = reference_description or failure.get("intent", "")
    if not description:
        return None

    return {
        "assembly": failure.get("generated_assembly", ""),
        "description": description,
        "system_suffix": (
            "\nA previous description of this code did not accurately capture its behavior.\n"
            "Provide an accurate description."
        ),
        "failure_layer": 4,
        "source": "feedback",
    }


def main():
    parser = argparse.ArgumentParser(
        description="Phase 8: Build error-aware feedback dataset for retraining"
    )
    parser.add_argument(
        "--failures", type=Path,
        default=ML_ROOT / "outputs" / "feedback" / "failures.jsonl",
        help="Path to failures JSONL from collect_failures.py",
    )
    parser.add_argument(
        "--corpus-dir", type=Path,
        default=PROJECT_ROOT / "tests" / "corpus",
        help="Directory containing .nolt corpus files",
    )
    parser.add_argument(
        "--output-7a", type=Path,
        default=ML_ROOT / "data" / "splits" / "feedback_7a.jsonl",
        help="Output path for 7a feedback dataset",
    )
    parser.add_argument(
        "--output-7b", type=Path,
        default=ML_ROOT / "data" / "splits" / "feedback_7b.jsonl",
        help="Output path for 7b feedback dataset",
    )
    args = parser.parse_args()

    if not args.failures.exists():
        print(f"ERROR: Failures file not found: {args.failures}", file=sys.stderr)
        print("Run collect_failures.py first.", file=sys.stderr)
        sys.exit(1)

    # Load reference assemblies
    print("Loading reference assemblies...")
    corpus = load_corpus(args.corpus_dir)
    print(f"  Corpus: {len(corpus)} unique intents")

    # Also check training splits as fallback
    splits_dir = ML_ROOT / "data" / "splits"
    training_refs = load_training_data(splits_dir)
    print(f"  Training splits: {len(training_refs)} unique intents")

    # Merge (corpus takes priority)
    all_refs = {**training_refs, **corpus}
    print(f"  Combined: {len(all_refs)} unique intents")

    # Load failures
    failures = []
    with open(args.failures) as f:
        for line in f:
            line = line.strip()
            if line:
                failures.append(json.loads(line))
    print(f"\nLoaded {len(failures)} failures")

    if not failures:
        print("No failures to process. Writing empty output files.")
        args.output_7a.parent.mkdir(parents=True, exist_ok=True)
        args.output_7a.write_text("")
        args.output_7b.parent.mkdir(parents=True, exist_ok=True)
        args.output_7b.write_text("")
        return

    # Build feedback entries
    entries_7a = []
    entries_7b = []
    matched = 0
    unmatched = 0
    seen_intents: set[str] = set()

    for failure in failures:
        intent = failure.get("intent", "").strip()
        if not intent:
            continue

        # Dedup by intent within this run
        if intent in seen_intents:
            continue
        seen_intents.add(intent)

        # Find reference assembly
        ref_asm = all_refs.get(intent)
        if ref_asm is None:
            unmatched += 1
            continue
        matched += 1

        # 7a entry
        entry_7a = build_7a_entry(failure, ref_asm)
        entries_7a.append(entry_7a)

        # 7b entry (only for description failures)
        entry_7b = build_7b_entry(failure)
        if entry_7b is not None:
            entries_7b.append(entry_7b)

    # Write outputs
    args.output_7a.parent.mkdir(parents=True, exist_ok=True)
    with open(args.output_7a, "w") as f:
        for entry in entries_7a:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")

    args.output_7b.parent.mkdir(parents=True, exist_ok=True)
    with open(args.output_7b, "w") as f:
        for entry in entries_7b:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")

    # Summary
    by_layer: dict[int, int] = {}
    for e in entries_7a:
        layer = e.get("failure_layer", 0)
        by_layer[layer] = by_layer.get(layer, 0) + 1

    print(f"\nFeedback dataset built:")
    print(f"  Matched with reference: {matched}")
    print(f"  Unmatched (skipped):    {unmatched}")
    print(f"\n  7a entries: {len(entries_7a)}")
    for layer in sorted(by_layer):
        label = {1: "Syntax", 2: "Verification", 3: "Witness", 4: "Semantic"}.get(layer, f"L{layer}")
        print(f"    Layer {layer} ({label}): {by_layer[layer]}")
    print(f"  7b entries: {len(entries_7b)}")
    print(f"\n  Written to: {args.output_7a}")
    if entries_7b:
        print(f"  Written to: {args.output_7b}")


if __name__ == "__main__":
    main()
