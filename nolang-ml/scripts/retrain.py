#!/usr/bin/env python3
"""Phase 8: Retrain with feedback-augmented data.

Loads original training data, appends feedback examples (optionally upsampled),
and retrains from the Phase 7 adapter checkpoint using conservative LoRA config.

Reuses tokenization and prompt formatting from train_7a.py / train_7b.py.

Usage:
    # Retrain 7a from Phase 7 checkpoint with feedback
    python scripts/retrain.py --task 7a --cycle 1

    # Retrain 7b
    python scripts/retrain.py --task 7b --cycle 1

    # Upsample feedback examples 3x
    python scripts/retrain.py --task 7a --cycle 1 --feedback-weight 3

    # Dry run — show data stats without training
    python scripts/retrain.py --task 7a --cycle 1 --dry-run
"""

import argparse
import json
import sys
from pathlib import Path
from typing import Any

SCRIPT_DIR = Path(__file__).resolve().parent
ML_ROOT = SCRIPT_DIR.parent

# Add scripts dir to path for imports
sys.path.insert(0, str(SCRIPT_DIR))


def load_config(config_path: Path) -> dict[str, Any]:
    """Load YAML configuration file."""
    import yaml
    with open(config_path) as f:
        return yaml.safe_load(f)


def load_jsonl(path: Path) -> list[dict]:
    """Load JSON-lines file."""
    if not path.exists():
        return []
    entries = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                try:
                    entries.append(json.loads(line))
                except json.JSONDecodeError:
                    continue
    return entries


def format_7a_feedback_prompt(entry: dict, tokenizer) -> str:
    """Format a feedback entry for 7a using error-aware system prompt.

    Uses the standard train_7a system prompt plus the error context
    from the feedback entry's system_suffix field.
    """
    from train_7a import SYSTEM_PROMPT

    system_content = SYSTEM_PROMPT
    suffix = entry.get("system_suffix", "")
    if suffix:
        system_content = system_content + suffix

    messages = [
        {"role": "system", "content": system_content},
        {"role": "user", "content": f"Intent: {entry['intent']}"},
        {"role": "assistant", "content": entry["assembly"]},
    ]
    return tokenizer.apply_chat_template(
        messages, tokenize=False, add_generation_prompt=False
    )


def format_7b_feedback_prompt(entry: dict, tokenizer) -> str:
    """Format a feedback entry for 7b using error-aware system prompt."""
    from train_7b import SYSTEM_PROMPT

    system_content = SYSTEM_PROMPT
    suffix = entry.get("system_suffix", "")
    if suffix:
        system_content = system_content + suffix

    messages = [
        {"role": "system", "content": system_content},
        {"role": "user", "content": f"Assembly:\n{entry['assembly']}"},
        {"role": "assistant", "content": entry["description"]},
    ]
    return tokenizer.apply_chat_template(
        messages, tokenize=False, add_generation_prompt=False
    )


def prepare_augmented_data(
    task: str,
    feedback_weight: int,
    tokenizer,
    max_length: int,
) -> tuple:
    """Load original + feedback data, return augmented HuggingFace datasets.

    Returns (train_dataset, val_dataset, stats_dict).
    """
    from datasets import Dataset

    splits_dir = ML_ROOT / "data" / "splits"
    train_path = splits_dir / f"train_{task}.jsonl"
    val_path = splits_dir / f"val_{task}.jsonl"
    feedback_path = splits_dir / f"feedback_{task}.jsonl"

    # Load original training data
    original_train = load_jsonl(train_path)
    val_data = load_jsonl(val_path)
    feedback_data = load_jsonl(feedback_path)

    print(f"  Original training: {len(original_train)} examples")
    print(f"  Validation:        {len(val_data)} examples")
    print(f"  Feedback:          {len(feedback_data)} examples")
    print(f"  Feedback weight:   {feedback_weight}x")

    # Format original data
    if task == "7a":
        from train_7a import format_chat_prompt
        original_texts = [format_chat_prompt(e, tokenizer) for e in original_train]
        feedback_texts = [format_7a_feedback_prompt(e, tokenizer) for e in feedback_data]
        val_texts = [format_chat_prompt(e, tokenizer) for e in val_data]
    else:
        from train_7b import format_chat_prompt
        original_texts = [format_chat_prompt(e, tokenizer) for e in original_train]
        feedback_texts = [format_7b_feedback_prompt(e, tokenizer) for e in feedback_data]
        val_texts = [format_chat_prompt(e, tokenizer) for e in val_data]

    # Upsample feedback examples
    augmented_texts = original_texts + feedback_texts * feedback_weight
    total_train = len(augmented_texts)

    print(f"  Augmented training: {total_train} examples "
          f"({len(original_texts)} original + {len(feedback_texts) * feedback_weight} feedback)")

    # Tokenize
    def tokenize_batch(texts: list[str]) -> Dataset:
        dataset = Dataset.from_dict({"text": texts})
        return dataset.map(
            lambda x: _tokenize(x, tokenizer, max_length),
            batched=True,
            remove_columns=["text"],
            desc="Tokenizing",
        )

    train_dataset = tokenize_batch(augmented_texts)
    val_dataset = tokenize_batch(val_texts)

    stats = {
        "original_train": len(original_train),
        "feedback": len(feedback_data),
        "feedback_weight": feedback_weight,
        "augmented_train": total_train,
        "val": len(val_data),
    }

    return train_dataset, val_dataset, stats


def _tokenize(examples: dict, tokenizer, max_length: int) -> dict:
    """Tokenize for causal LM."""
    outputs = tokenizer(
        examples["text"],
        truncation=True,
        max_length=max_length,
        padding="max_length",
        return_tensors=None,
    )
    outputs["labels"] = outputs["input_ids"].copy()
    return outputs


def retrain(
    task: str,
    cycle: int,
    config: dict,
    train_dataset,
    val_dataset,
):
    """Run retraining from Phase 7 adapter with augmented data."""
    import torch
    from peft import PeftModel, LoraConfig, get_peft_model, prepare_model_for_kbit_training
    from transformers import (
        AutoModelForCausalLM,
        AutoTokenizer,
        BitsAndBytesConfig,
        Trainer,
        TrainingArguments,
    )

    model_name = config["model"]["base"]
    base_adapter = ML_ROOT / config.get("base_adapter_path", f"models/{task}_intent_to_asm/final")
    output_dir = ML_ROOT / config["output_dir"] / f"feedback_v{cycle}"

    print(f"\nLoading base model: {model_name}")
    quant_config = BitsAndBytesConfig(
        load_in_4bit=config["quantization"]["load_in_4bit"],
        bnb_4bit_quant_type=config["quantization"]["bnb_4bit_quant_type"],
        bnb_4bit_compute_dtype=getattr(torch, config["quantization"]["bnb_4bit_compute_dtype"]),
        bnb_4bit_use_double_quant=config["quantization"]["bnb_4bit_use_double_quant"],
    )

    model = AutoModelForCausalLM.from_pretrained(
        model_name,
        quantization_config=quant_config,
        device_map="auto",
        trust_remote_code=False,
    )

    tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=False)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
        model.config.pad_token_id = model.config.eos_token_id

    # Load Phase 7 adapter if it exists
    if base_adapter.exists():
        print(f"Loading Phase 7 adapter from: {base_adapter}")
        model = PeftModel.from_pretrained(model, str(base_adapter))
        # Merge adapter weights so we can apply new LoRA on top
        model = model.merge_and_unload()
        print("  Merged Phase 7 adapter into base model")

    # Prepare for new LoRA training
    print("Preparing model for k-bit training...")
    model = prepare_model_for_kbit_training(model)

    lora_config = LoraConfig(
        r=config["lora"]["r"],
        lora_alpha=config["lora"]["lora_alpha"],
        lora_dropout=config["lora"]["lora_dropout"],
        target_modules=config["lora"]["target_modules"],
        bias=config["lora"]["bias"],
        task_type=config["lora"]["task_type"],
    )

    print("Applying fresh LoRA...")
    model = get_peft_model(model, lora_config)

    trainable_params = sum(p.numel() for p in model.parameters() if p.requires_grad)
    total_params = sum(p.numel() for p in model.parameters())
    print(f"  Trainable: {trainable_params:,} / {total_params:,} "
          f"({100 * trainable_params / total_params:.2f}%)")

    # Training arguments — conservative
    training_cfg = config["training"]
    training_args = TrainingArguments(
        output_dir=str(output_dir),
        num_train_epochs=training_cfg["num_epochs"],
        per_device_train_batch_size=training_cfg["per_device_train_batch_size"],
        per_device_eval_batch_size=training_cfg["per_device_train_batch_size"],
        gradient_accumulation_steps=training_cfg["gradient_accumulation_steps"],
        learning_rate=training_cfg["learning_rate"],
        lr_scheduler_type=training_cfg["lr_scheduler_type"],
        warmup_ratio=training_cfg["warmup_ratio"],
        weight_decay=training_cfg["weight_decay"],
        max_grad_norm=training_cfg["max_grad_norm"],
        eval_strategy="steps",
        eval_steps=training_cfg["eval_steps"],
        save_strategy="steps",
        save_steps=training_cfg["save_steps"],
        logging_steps=training_cfg["logging_steps"],
        save_total_limit=training_cfg["save_total_limit"],
        load_best_model_at_end=training_cfg["load_best_model_at_end"],
        metric_for_best_model=training_cfg["metric_for_best_model"],
        greater_is_better=training_cfg["greater_is_better"],
        fp16=training_cfg["fp16"],
        bf16=training_cfg["bf16"],
        dataloader_num_workers=training_cfg["dataloader_num_workers"],
        remove_unused_columns=False,
        report_to="none",
    )

    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=train_dataset,
        eval_dataset=val_dataset,
    )

    print("\n" + "=" * 70)
    print(f"Starting feedback retraining (cycle {cycle})...")
    print("=" * 70 + "\n")

    trainer.train()

    # Save
    final_dir = output_dir / "final"
    print(f"\nSaving adapter to {final_dir}...")
    trainer.model.save_pretrained(str(final_dir))
    tokenizer.save_pretrained(str(final_dir))

    # Report
    print("\n" + "=" * 70)
    print(f"Retraining Complete (cycle {cycle})")
    print("=" * 70)

    metrics = trainer.state.log_history
    if metrics:
        train_losses = [m["loss"] for m in metrics if "loss" in m]
        eval_losses = [m["eval_loss"] for m in metrics if "eval_loss" in m]
        if train_losses:
            print(f"\n  Final train loss: {train_losses[-1]:.4f}")
        if eval_losses:
            print(f"  Final eval loss:  {eval_losses[-1]:.4f}")
            print(f"  Best eval loss:   {min(eval_losses):.4f}")

    print(f"\n  Adapter saved to: {final_dir}")
    print("=" * 70)

    return final_dir


def main():
    parser = argparse.ArgumentParser(
        description="Phase 8: Retrain with feedback-augmented data"
    )
    parser.add_argument(
        "--task", choices=["7a", "7b"], required=True,
        help="Task to retrain: 7a (intent→assembly) or 7b (assembly→description)",
    )
    parser.add_argument(
        "--cycle", type=int, default=1,
        help="Feedback cycle number (for output directory naming)",
    )
    parser.add_argument(
        "--feedback-weight", type=int, default=2,
        help="How many times to upsample feedback examples (default: 2)",
    )
    parser.add_argument(
        "--config", type=Path, default=None,
        help="Override config path (default: configs/lora_8a.yaml for 7a)",
    )
    parser.add_argument(
        "--dry-run", action="store_true",
        help="Show data stats without training",
    )
    args = parser.parse_args()

    # Select config
    if args.config:
        config_path = args.config
    else:
        config_path = ML_ROOT / "configs" / "lora_8a.yaml"
    print(f"Loading config: {config_path}")
    config = load_config(config_path)

    # Load tokenizer for formatting
    print("Loading tokenizer...")
    from transformers import AutoTokenizer
    model_name = config["model"]["base"]
    tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=False)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    max_length = config["model"]["max_length"]

    # Prepare augmented data
    print(f"\nPreparing augmented data for {args.task}...")
    train_dataset, val_dataset, stats = prepare_augmented_data(
        args.task, args.feedback_weight, tokenizer, max_length
    )

    if args.dry_run:
        print("\n" + "=" * 70)
        print("DRY RUN — Data Summary")
        print("=" * 70)
        print(f"  Task:              {args.task}")
        print(f"  Cycle:             {args.cycle}")
        print(f"  Original train:    {stats['original_train']}")
        print(f"  Feedback examples: {stats['feedback']}")
        print(f"  Feedback weight:   {stats['feedback_weight']}x")
        print(f"  Augmented train:   {stats['augmented_train']}")
        print(f"  Validation:        {stats['val']}")
        print(f"  Config:            {config_path}")
        print(f"  Learning rate:     {config['training']['learning_rate']}")
        print(f"  Epochs:            {config['training']['num_epochs']}")
        print(f"  Base adapter:      {config.get('base_adapter_path', 'N/A')}")
        print("=" * 70)
        return

    if stats["feedback"] == 0:
        print("\nWARNING: No feedback examples found. Training on original data only.")
        print("This is equivalent to another epoch of Phase 7 training with conservative LR.")

    retrain(args.task, args.cycle, config, train_dataset, val_dataset)


if __name__ == "__main__":
    main()
