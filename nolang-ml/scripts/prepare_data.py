#!/usr/bin/env python3
"""Prepare training data from .nolt corpus files.

Reads tests/corpus/generated.nolt and tests/corpus/examples.nolt,
creates stratified 80/10/10 train/val/test splits for:
  - 7a: intent → assembly
  - 7b: assembly → description (intent as ground-truth description)

Outputs JSON-lines files to data/splits/.
"""

import json
import os
import re
import sys
from collections import defaultdict
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent  # nolang-ml/../ = nol/
ML_ROOT = SCRIPT_DIR.parent  # nolang-ml/

CORPUS_FILES = [
    PROJECT_ROOT / "tests" / "corpus" / "generated.nolt",
    PROJECT_ROOT / "tests" / "corpus" / "examples.nolt",
]
OUTPUT_DIR = ML_ROOT / "data" / "splits"

# Category inference from intent text (heuristic, for stratification)
CATEGORY_PATTERNS = [
    ("arithmetic", re.compile(r"add|subtract|multiply|divide|negate|absolute|sum|square|cube|factorial|fibonacci|modulo|remainder|power|increment|decrement", re.I)),
    ("boolean", re.compile(r"boolean|true|false|and |or |not |xor|implies|logical", re.I)),
    ("comparison", re.compile(r"compare|greater|less|equal|minimum|maximum|clamp|between|sign|positive|negative|zero", re.I)),
    ("matching", re.compile(r"match|case|pattern|variant|unwrap|option|some|none", re.I)),
    ("tuple", re.compile(r"tuple|pair|first|second|element|project|swap|struct", re.I)),
    ("array", re.compile(r"array|length|index|new array|element.*array", re.I)),
    ("binding", re.compile(r"bind|drop|reference|ref|de bruijn", re.I)),
    ("function", re.compile(r"function|call|return|param|recursive|recursion|contract|pre|post|precondition|postcondition", re.I)),
    ("character", re.compile(r"character|char|letter|digit|space|newline|uppercase|lowercase", re.I)),
    ("conversion", re.compile(r"convert|cast|to integer|to boolean|to float", re.I)),
    ("float", re.compile(r"float|f64|decimal|floating", re.I)),
    ("forall", re.compile(r"forall|for all|every|each element|all elements", re.I)),
    ("identity", re.compile(r"identity|return.*constant|return.*value|halt", re.I)),
]


def infer_category(intent: str) -> str:
    """Infer a category from intent text for stratification purposes."""
    for name, pattern in CATEGORY_PATTERNS:
        if pattern.search(intent):
            return name
    return "other"


def load_corpus() -> list[dict]:
    """Load all .nolt entries from corpus files."""
    entries = []
    for path in CORPUS_FILES:
        if not path.exists():
            print(f"WARNING: Corpus file not found: {path}", file=sys.stderr)
            continue
        with open(path) as f:
            for line_num, line in enumerate(f, 1):
                line = line.strip()
                if not line:
                    continue
                try:
                    entry = json.loads(line)
                except json.JSONDecodeError as e:
                    print(f"WARNING: {path}:{line_num}: {e}", file=sys.stderr)
                    continue
                if "intent" not in entry or "assembly" not in entry:
                    print(f"WARNING: {path}:{line_num}: missing intent/assembly", file=sys.stderr)
                    continue
                entry["_source"] = path.name
                entry["_category"] = infer_category(entry["intent"])
                entries.append(entry)
    return entries


def stratified_split(entries: list[dict], train_ratio=0.8, val_ratio=0.1):
    """Split entries into train/val/test preserving category distribution.

    Groups by intent to prevent data leakage: all entries sharing the same
    intent string go into the same split.
    """
    import random
    random.seed(42)

    # Group entries by intent to prevent leakage
    intent_groups: dict[str, list[dict]] = defaultdict(list)
    for entry in entries:
        intent_groups[entry["intent"]].append(entry)

    # Categorize each unique intent
    by_category: dict[str, list[str]] = defaultdict(list)
    for intent, group in intent_groups.items():
        cat = group[0]["_category"]
        by_category[cat].append(intent)

    train, val, test = [], [], []

    for cat, intents in sorted(by_category.items()):
        random.shuffle(intents)
        n = len(intents)
        n_train = max(1, int(n * train_ratio))
        n_val = max(1, int(n * val_ratio))
        # Ensure at least 1 in each split for small categories
        if n <= 3:
            train.extend(intent_groups[intents[0]])
            if n >= 2:
                val.extend(intent_groups[intents[1]])
            if n >= 3:
                test.extend(intent_groups[intents[2]])
            continue

        for intent in intents[:n_train]:
            train.extend(intent_groups[intent])
        for intent in intents[n_train:n_train + n_val]:
            val.extend(intent_groups[intent])
        for intent in intents[n_train + n_val:]:
            test.extend(intent_groups[intent])

    # Shuffle within splits
    random.shuffle(train)
    random.shuffle(val)
    random.shuffle(test)

    return train, val, test


def format_7a(entry: dict) -> dict:
    """Format entry for intent → assembly task."""
    result = {"intent": entry["intent"], "assembly": entry["assembly"]}
    if "witnesses" in entry:
        result["witnesses"] = entry["witnesses"]
    return result


def format_7b(entry: dict) -> dict:
    """Format entry for assembly → description task."""
    return {"assembly": entry["assembly"], "description": entry["intent"]}


def write_jsonl(entries: list[dict], path: Path):
    """Write entries as JSON-lines."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "w") as f:
        for entry in entries:
            f.write(json.dumps(entry, ensure_ascii=False) + "\n")


def main():
    print("Loading corpus...")
    entries = load_corpus()
    print(f"  Loaded {len(entries)} entries from {len(CORPUS_FILES)} files")

    # Report category distribution
    cats = defaultdict(int)
    for e in entries:
        cats[e["_category"]] += 1
    print("\n  Category distribution:")
    for cat, count in sorted(cats.items(), key=lambda x: -x[1]):
        print(f"    {cat:15s}: {count:4d}")

    print("\nSplitting 80/10/10...")
    train, val, test = stratified_split(entries)
    print(f"  Train: {len(train)}, Val: {len(val)}, Test: {len(test)}")

    # Write 7a splits
    print("\nWriting 7a splits (intent → assembly)...")
    write_jsonl([format_7a(e) for e in train], OUTPUT_DIR / "train_7a.jsonl")
    write_jsonl([format_7a(e) for e in val], OUTPUT_DIR / "val_7a.jsonl")
    write_jsonl([format_7a(e) for e in test], OUTPUT_DIR / "test_7a.jsonl")

    # Write 7b splits
    print("Writing 7b splits (assembly → description)...")
    write_jsonl([format_7b(e) for e in train], OUTPUT_DIR / "train_7b.jsonl")
    write_jsonl([format_7b(e) for e in val], OUTPUT_DIR / "val_7b.jsonl")
    write_jsonl([format_7b(e) for e in test], OUTPUT_DIR / "test_7b.jsonl")

    # Validate: check opcode coverage in test set
    all_opcodes = set()
    for e in test:
        for word in e["assembly"].split():
            if word.isupper() and word.isalpha():
                all_opcodes.add(word)
    print(f"\n  Unique opcodes in test set: {len(all_opcodes)}")
    print(f"  Opcodes: {', '.join(sorted(all_opcodes))}")

    # Check for data leakage (same intent in train and test)
    train_intents = {e["intent"] for e in train}
    test_intents = {e["intent"] for e in test}
    leaked = train_intents & test_intents
    if leaked:
        print(f"\n  WARNING: {len(leaked)} intents appear in both train and test!")
        for intent in list(leaked)[:5]:
            print(f"    - {intent[:80]}")
    else:
        print("\n  No data leakage detected (train ∩ test = ∅)")

    print("\nDone. Splits written to:", OUTPUT_DIR)


if __name__ == "__main__":
    main()
