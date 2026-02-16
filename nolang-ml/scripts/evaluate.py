#!/usr/bin/env python3
"""Evaluation script for Phase 7a and 7b models.

7a metrics (intent → assembly):
  - Syntax validity: % that assemble without errors
  - Verification pass: % that pass verifier
  - Witness pass: % that pass all witness tests
  - Exact match: informational only

7b metrics (assembly → description):
  - BLEU-4 score vs ground truth
  - ROUGE-L score

Usage:
    # Full evaluation (requires trained models)
    python scripts/evaluate.py

    # Evaluate only 7a
    python scripts/evaluate.py --task 7a

    # Evaluate from pre-generated files
    python scripts/evaluate.py --7a-generations outputs/generations/test_7a.jsonl
    python scripts/evaluate.py --7b-descriptions outputs/descriptions/test_7b.jsonl
"""

import argparse
import json
import sys
import time
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
ML_ROOT = SCRIPT_DIR.parent

# Lazy imports for metrics (avoid import cost when not needed)
_nltk_ready = False


def _ensure_nltk():
    global _nltk_ready
    if not _nltk_ready:
        import nltk
        try:
            nltk.data.find("tokenizers/punkt_tab")
        except LookupError:
            nltk.download("punkt_tab", quiet=True)
        _nltk_ready = True


def compute_bleu(references: list[str], hypotheses: list[str]) -> float:
    """Compute corpus BLEU-4 score."""
    _ensure_nltk()
    from nltk.translate.bleu_score import SmoothingFunction, corpus_bleu

    refs = [[ref.split()] for ref in references]
    hyps = [hyp.split() for hyp in hypotheses]
    smoothing = SmoothingFunction().method1
    return corpus_bleu(refs, hyps, smoothing_function=smoothing)


def compute_rouge_l(references: list[str], hypotheses: list[str]) -> float:
    """Compute average ROUGE-L F1 score."""
    from rouge_score import rouge_scorer

    scorer = rouge_scorer.RougeScorer(["rougeL"], use_stemmer=True)
    scores = []
    for ref, hyp in zip(references, hypotheses):
        score = scorer.score(ref, hyp)
        scores.append(score["rougeL"].fmeasure)
    return sum(scores) / len(scores) if scores else 0.0


def evaluate_7a(generations_path: Path) -> dict:
    """Evaluate 7a generations using the validation pipeline."""
    sys.path.insert(0, str(SCRIPT_DIR))
    from validate import validate_assembly

    print("Evaluating 7a (intent → assembly)...")
    print(f"  Reading: {generations_path}")

    entries = []
    with open(generations_path) as f:
        for line in f:
            entries.append(json.loads(line.strip()))

    total = len(entries)
    assembled = 0
    verified = 0
    witness_pass = 0
    witness_total = 0
    exact_match = 0
    errors_by_type: dict[str, int] = {}

    for i, entry in enumerate(entries):
        gen_asm = entry["generated_assembly"]
        witnesses = entry.get("witnesses")

        print(f"  [{i+1}/{total}] ", end="", flush=True)

        result = validate_assembly(gen_asm, witnesses)

        if result.assembled:
            assembled += 1
        if result.verified:
            verified += 1
        if witnesses:
            witness_total += 1
            if result.witnesses_passed:
                witness_pass += 1

        # Exact match (ignoring HASH values and whitespace)
        ref_asm = entry.get("reference_assembly", "")
        if ref_asm:
            import re
            hash_re = re.compile(r"HASH\s+0x[0-9a-fA-F]+\s+0x[0-9a-fA-F]+\s+0x[0-9a-fA-F]+")
            gen_norm = hash_re.sub("HASH", gen_asm.strip())
            ref_norm = hash_re.sub("HASH", ref_asm.strip())
            if gen_norm == ref_norm:
                exact_match += 1

        # Track error types
        for err in result.errors:
            key = err.split(":")[0] if ":" in err else err[:40]
            errors_by_type[key] = errors_by_type.get(key, 0) + 1

        status = "OK" if result.fully_valid else "FAIL"
        print(status)

    metrics = {
        "total": total,
        "assembled": assembled,
        "assembled_pct": assembled / total * 100 if total else 0,
        "verified": verified,
        "verified_pct": verified / total * 100 if total else 0,
        "witness_total": witness_total,
        "witness_pass": witness_pass,
        "witness_pass_pct": witness_pass / witness_total * 100 if witness_total else 0,
        "exact_match": exact_match,
        "exact_match_pct": exact_match / total * 100 if total else 0,
        "error_types": errors_by_type,
    }

    return metrics


def evaluate_7b(descriptions_path: Path) -> dict:
    """Evaluate 7b descriptions using BLEU and ROUGE."""
    print("Evaluating 7b (assembly → description)...")
    print(f"  Reading: {descriptions_path}")

    references = []
    hypotheses = []

    with open(descriptions_path) as f:
        for line in f:
            entry = json.loads(line.strip())
            ref = entry.get("reference_description", "")
            hyp = entry.get("generated_description", "")
            if ref and hyp:
                references.append(ref)
                hypotheses.append(hyp)

    if not references:
        print("  WARNING: No reference/hypothesis pairs found")
        return {"total": 0, "bleu4": 0.0, "rouge_l": 0.0}

    print(f"  Computing metrics on {len(references)} pairs...")
    bleu = compute_bleu(references, hypotheses)
    rouge_l = compute_rouge_l(references, hypotheses)

    metrics = {
        "total": len(references),
        "bleu4": bleu,
        "rouge_l": rouge_l,
    }

    return metrics


def generate_7a_if_needed(test_path: Path, output_path: Path) -> Path:
    """Generate 7a outputs if not already present."""
    if output_path.exists():
        return output_path

    print("Generating 7a outputs (this requires a trained model)...")
    from inference_7a import generate_assembly, load_config, load_model

    config = load_config()
    model, tokenizer = load_model(config)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(test_path) as fin, open(output_path, "w") as fout:
        for line_num, line in enumerate(fin, 1):
            entry = json.loads(line.strip())
            intent = entry["intent"]
            print(f"  [{line_num}] {intent[:50]}...", end=" ", flush=True)
            gen = generate_assembly(model, tokenizer, intent, config)
            result = {
                "intent": intent,
                "generated_assembly": gen,
            }
            if "assembly" in entry:
                result["reference_assembly"] = entry["assembly"]
            if "witnesses" in entry:
                result["witnesses"] = entry["witnesses"]
            fout.write(json.dumps(result, ensure_ascii=False) + "\n")
            print("done")

    return output_path


def generate_7b_if_needed(test_path: Path, output_path: Path) -> Path:
    """Generate 7b outputs if not already present."""
    if output_path.exists():
        return output_path

    print("Generating 7b outputs (this requires a trained model)...")
    from inference_7b import generate_description, load_config, load_model

    config = load_config()
    model, tokenizer = load_model(config)

    output_path.parent.mkdir(parents=True, exist_ok=True)
    with open(test_path) as fin, open(output_path, "w") as fout:
        for line_num, line in enumerate(fin, 1):
            entry = json.loads(line.strip())
            assembly = entry["assembly"]
            print(f"  [{line_num}] ...", end=" ", flush=True)
            desc = generate_description(model, tokenizer, assembly, config)
            result = {
                "assembly": assembly,
                "generated_description": desc,
            }
            if "description" in entry:
                result["reference_description"] = entry["description"]
            fout.write(json.dumps(result, ensure_ascii=False) + "\n")
            print("done")

    return output_path


def print_report(metrics_7a: dict | None, metrics_7b: dict | None):
    """Print formatted evaluation report."""
    print("\n" + "=" * 70)
    print("EVALUATION REPORT")
    print("=" * 70)

    if metrics_7a:
        print("\n--- Phase 7a: Intent → Assembly ---")
        print(f"  Total programs:     {metrics_7a['total']}")
        print(f"  Syntax valid:       {metrics_7a['assembled']:4d} / {metrics_7a['total']}  ({metrics_7a['assembled_pct']:.1f}%)  target ≥80%  {'PASS' if metrics_7a['assembled_pct'] >= 80 else 'FAIL'}")
        print(f"  Verification pass:  {metrics_7a['verified']:4d} / {metrics_7a['total']}  ({metrics_7a['verified_pct']:.1f}%)  target ≥70%  {'PASS' if metrics_7a['verified_pct'] >= 70 else 'FAIL'}")
        if metrics_7a['witness_total'] > 0:
            print(f"  Witness pass:       {metrics_7a['witness_pass']:4d} / {metrics_7a['witness_total']}  ({metrics_7a['witness_pass_pct']:.1f}%)  target ≥60%  {'PASS' if metrics_7a['witness_pass_pct'] >= 60 else 'FAIL'}")
        print(f"  Exact match:        {metrics_7a['exact_match']:4d} / {metrics_7a['total']}  ({metrics_7a['exact_match_pct']:.1f}%)  (informational)")

        if metrics_7a.get("error_types"):
            print("\n  Error distribution:")
            for err, count in sorted(metrics_7a["error_types"].items(), key=lambda x: -x[1])[:10]:
                print(f"    {err}: {count}")

    if metrics_7b:
        print("\n--- Phase 7b: Assembly → Description ---")
        print(f"  Total pairs:  {metrics_7b['total']}")
        print(f"  BLEU-4:       {metrics_7b['bleu4']:.4f}  target ≥0.4  {'PASS' if metrics_7b['bleu4'] >= 0.4 else 'FAIL'}")
        print(f"  ROUGE-L:      {metrics_7b['rouge_l']:.4f}")

    print("\n" + "=" * 70)


def main():
    parser = argparse.ArgumentParser(description="Evaluate Phase 7 models")
    parser.add_argument("--task", choices=["7a", "7b", "both"], default="both")
    parser.add_argument("--7a-generations", type=Path, help="Pre-generated 7a JSONL")
    parser.add_argument("--7b-descriptions", type=Path, help="Pre-generated 7b JSONL")
    parser.add_argument("--output", type=Path, default=ML_ROOT / "outputs" / "metrics",
                        help="Directory to save metrics JSON")
    args = parser.parse_args()

    args.output.mkdir(parents=True, exist_ok=True)

    metrics_7a = None
    metrics_7b = None

    if args.task in ("7a", "both"):
        gen_path = args.__dict__["7a_generations"]
        if gen_path is None:
            gen_path = generate_7a_if_needed(
                ML_ROOT / "data" / "splits" / "test_7a.jsonl",
                ML_ROOT / "outputs" / "generations" / "test_7a.jsonl",
            )
        metrics_7a = evaluate_7a(gen_path)
        with open(args.output / "metrics_7a.json", "w") as f:
            json.dump(metrics_7a, f, indent=2)

    if args.task in ("7b", "both"):
        desc_path = args.__dict__["7b_descriptions"]
        if desc_path is None:
            desc_path = generate_7b_if_needed(
                ML_ROOT / "data" / "splits" / "test_7b.jsonl",
                ML_ROOT / "outputs" / "descriptions" / "test_7b.jsonl",
            )
        metrics_7b = evaluate_7b(desc_path)
        with open(args.output / "metrics_7b.json", "w") as f:
            json.dump(metrics_7b, f, indent=2)

    print_report(metrics_7a, metrics_7b)

    # Save combined report
    combined = {"timestamp": time.strftime("%Y-%m-%dT%H:%M:%S")}
    if metrics_7a:
        combined["7a"] = metrics_7a
    if metrics_7b:
        combined["7b"] = metrics_7b
    with open(args.output / "evaluation_report.json", "w") as f:
        json.dump(combined, f, indent=2)
    print(f"\nMetrics saved to: {args.output}")


if __name__ == "__main__":
    main()
