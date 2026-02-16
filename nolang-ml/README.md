# NoLang ML — Phase 7: LLM Integration

Fine-tunes a local LLM (Llama 3.1 8B Instruct) to generate NoLang assembly from
natural language intent (7a), describe assembly in English (7b), and provides a
CLI for human confirmation (7c).

## Prerequisites

- Python 3.10+
- NVIDIA GPU with 16GB+ VRAM
- CUDA toolkit installed
- Rust toolchain with `nolang` CLI built (`cargo build --release` from workspace root)

## Setup

```bash
cd nolang-ml
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

Verify GPU access:
```bash
python -c "import torch; print(torch.cuda.is_available())"
```

## Pipeline

### 1. Prepare Data
```bash
python scripts/prepare_data.py
```
Reads `tests/corpus/generated.nolt` and `tests/corpus/examples.nolt`, creates
stratified 80/10/10 train/val/test splits in `data/splits/`.

### 2. Train Intent → Assembly (7a)
```bash
python scripts/train_7a.py [--config configs/lora_7a.yaml]
```
Saves LoRA adapter to `models/7a_intent_to_asm/`.

### 3. Train Assembly → Description (7b)
```bash
python scripts/train_7b.py [--config configs/lora_7b.yaml]
```
Saves LoRA adapter to `models/7b_asm_to_desc/`.

### 4. Evaluate
```bash
python scripts/evaluate.py
```
Runs inference on test set, validates through Rust CLI, computes metrics.

### 5. Interactive Comparison (7c)
```bash
python scripts/compare.py
```
Shows intent → generated assembly → description side by side, collects human
feedback to `outputs/human_feedback.jsonl` for Phase 8.

## Validation Pipeline

The validation script (`scripts/validate.py`) calls the Rust CLI:
1. Normalizes HASH placeholders in generated assembly
2. Runs `nolang hash` to compute correct hashes
3. Patches hashes into assembly
4. Runs `nolang assemble` → `nolang verify`
5. Optionally runs `nolang witness` with test cases

This ensures generated programs are structurally and semantically correct.

## Metrics Targets

| Sub-phase | Metric | Target |
|-----------|--------|--------|
| 7a | Syntax validity (assembles) | ≥80% |
| 7a | Verification pass | ≥70% |
| 7a | Witness pass | ≥60% |
| 7b | BLEU-4 | ≥0.4 |
| 7b | Human accuracy | ≥80% |
| 7c | Match rate | ≥75% |

## Directory Structure

```
nolang-ml/
├── configs/          # LoRA hyperparameter configs
├── scripts/          # All Python scripts
├── data/splits/      # Train/val/test JSONL splits (gitignored)
├── models/           # Saved LoRA adapters (gitignored)
└── outputs/          # Generations, metrics, feedback (gitignored)
```
