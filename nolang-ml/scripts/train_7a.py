#!/usr/bin/env python3
"""LoRA fine-tuning for Phase 7a: Intent → Assembly generation.

Fine-tunes Llama 3.1 8B Instruct to generate NoLang assembly from natural language intent.
Uses 4-bit quantization + LoRA for efficient training.

Usage:
    python train_7a.py [--config CONFIG] [--dry-run]
"""

import argparse
import json
import sys
from pathlib import Path
from typing import Any

import torch
import yaml
from datasets import Dataset
from peft import LoraConfig, get_peft_model, prepare_model_for_kbit_training
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
    BitsAndBytesConfig,
    Trainer,
    TrainingArguments,
)

# Path resolution relative to script location
SCRIPT_DIR = Path(__file__).resolve().parent
ML_ROOT = SCRIPT_DIR.parent  # nolang-ml/
PROJECT_ROOT = ML_ROOT.parent  # nol/

# System prompt for NoLang generation
SYSTEM_PROMPT = """You are a NoLang code generator. NoLang uses fixed 64-bit instructions, de Bruijn indices (REF 0 = most recent binding), exhaustive pattern matching, and mandatory HASH in function blocks. Use placeholder HASH 0x0000 0x0000 0x0000. Generate syntactically correct NoLang assembly for the given intent."""


def load_config(config_path: Path) -> dict[str, Any]:
    """Load YAML configuration file."""
    with open(config_path) as f:
        return yaml.safe_load(f)


def load_jsonl(path: Path) -> list[dict]:
    """Load JSON-lines dataset."""
    if not path.exists():
        print(f"ERROR: Dataset not found: {path}", file=sys.stderr)
        print("Run prepare_data.py first to generate training splits.", file=sys.stderr)
        sys.exit(1)

    entries = []
    with open(path) as f:
        for line_num, line in enumerate(f, 1):
            line = line.strip()
            if not line:
                continue
            try:
                entry = json.loads(line)
                if "intent" not in entry or "assembly" not in entry:
                    print(
                        f"WARNING: {path}:{line_num}: missing intent/assembly",
                        file=sys.stderr,
                    )
                    continue
                entries.append(entry)
            except json.JSONDecodeError as e:
                print(f"WARNING: {path}:{line_num}: {e}", file=sys.stderr)
                continue
    return entries


def format_chat_prompt(entry: dict, tokenizer) -> str:
    """Format entry as Llama 3.1 chat template."""
    messages = [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": f"Intent: {entry['intent']}"},
        {"role": "assistant", "content": entry["assembly"]},
    ]
    # Apply chat template with tokenization disabled (we'll tokenize separately)
    return tokenizer.apply_chat_template(messages, tokenize=False, add_generation_prompt=False)


def tokenize_function(examples: dict, tokenizer, max_length: int) -> dict:
    """Tokenize chat-formatted examples."""
    # Batch tokenization
    outputs = tokenizer(
        examples["text"],
        truncation=True,
        max_length=max_length,
        padding="max_length",
        return_tensors=None,  # Return lists, not tensors
    )
    # For causal LM, labels are input_ids (shifted internally by model)
    outputs["labels"] = outputs["input_ids"].copy()
    return outputs


def create_datasets(config: dict, tokenizer) -> tuple[Dataset, Dataset]:
    """Load and tokenize train/val datasets."""
    train_path = ML_ROOT / "data" / "splits" / "train_7a.jsonl"
    val_path = ML_ROOT / "data" / "splits" / "val_7a.jsonl"

    print(f"Loading train data from {train_path}...")
    train_entries = load_jsonl(train_path)
    print(f"  Loaded {len(train_entries)} training examples")

    print(f"Loading validation data from {val_path}...")
    val_entries = load_jsonl(val_path)
    print(f"  Loaded {len(val_entries)} validation examples")

    # Format as chat prompts
    train_texts = [format_chat_prompt(e, tokenizer) for e in train_entries]
    val_texts = [format_chat_prompt(e, tokenizer) for e in val_entries]

    # Create HuggingFace datasets
    train_dataset = Dataset.from_dict({"text": train_texts})
    val_dataset = Dataset.from_dict({"text": val_texts})

    # Tokenize
    max_length = config["model"]["max_length"]
    print(f"\nTokenizing (max_length={max_length})...")
    train_dataset = train_dataset.map(
        lambda x: tokenize_function(x, tokenizer, max_length),
        batched=True,
        remove_columns=["text"],
        desc="Tokenizing train",
    )
    val_dataset = val_dataset.map(
        lambda x: tokenize_function(x, tokenizer, max_length),
        batched=True,
        remove_columns=["text"],
        desc="Tokenizing val",
    )

    return train_dataset, val_dataset


def load_model_and_tokenizer(config: dict):
    """Load base model with 4-bit quantization and tokenizer."""
    model_name = config["model"]["base"]
    print(f"\nLoading model: {model_name}")

    # 4-bit quantization config
    quant_config = BitsAndBytesConfig(
        load_in_4bit=config["quantization"]["load_in_4bit"],
        bnb_4bit_quant_type=config["quantization"]["bnb_4bit_quant_type"],
        bnb_4bit_compute_dtype=getattr(torch, config["quantization"]["bnb_4bit_compute_dtype"]),
        bnb_4bit_use_double_quant=config["quantization"]["bnb_4bit_use_double_quant"],
    )

    # Load model
    model = AutoModelForCausalLM.from_pretrained(
        model_name,
        quantization_config=quant_config,
        device_map="auto",
        trust_remote_code=False,
    )

    # Load tokenizer
    tokenizer = AutoTokenizer.from_pretrained(model_name, trust_remote_code=False)

    # Set pad token if not present (required for batch training)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
        model.config.pad_token_id = model.config.eos_token_id

    return model, tokenizer


def setup_lora(model, config: dict):
    """Apply LoRA configuration to model."""
    print("\nPreparing model for k-bit training...")
    model = prepare_model_for_kbit_training(model)

    lora_config = LoraConfig(
        r=config["lora"]["r"],
        lora_alpha=config["lora"]["lora_alpha"],
        lora_dropout=config["lora"]["lora_dropout"],
        target_modules=config["lora"]["target_modules"],
        bias=config["lora"]["bias"],
        task_type=config["lora"]["task_type"],
    )

    print("Applying LoRA...")
    model = get_peft_model(model, lora_config)

    # Print trainable parameters
    trainable_params = sum(p.numel() for p in model.parameters() if p.requires_grad)
    total_params = sum(p.numel() for p in model.parameters())
    print(f"  Trainable params: {trainable_params:,} / {total_params:,} ({100 * trainable_params / total_params:.2f}%)")

    return model


def create_training_args(config: dict) -> TrainingArguments:
    """Create TrainingArguments from config."""
    output_dir = ML_ROOT / config["output_dir"]
    training_cfg = config["training"]

    return TrainingArguments(
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
        remove_unused_columns=False,  # We handle columns explicitly
        report_to="none",  # Disable wandb/tensorboard for now
    )


def print_dry_run_info(config: dict, train_dataset, val_dataset):
    """Print configuration and dataset info for dry run."""
    print("\n" + "=" * 80)
    print("DRY RUN - Configuration Summary")
    print("=" * 80)

    print(f"\nModel: {config['model']['base']}")
    print(f"Max length: {config['model']['max_length']}")

    print("\nQuantization:")
    print(f"  4-bit: {config['quantization']['load_in_4bit']}")
    print(f"  Type: {config['quantization']['bnb_4bit_quant_type']}")
    print(f"  Compute dtype: {config['quantization']['bnb_4bit_compute_dtype']}")

    print("\nLoRA:")
    print(f"  Rank: {config['lora']['r']}")
    print(f"  Alpha: {config['lora']['lora_alpha']}")
    print(f"  Dropout: {config['lora']['lora_dropout']}")
    print(f"  Targets: {', '.join(config['lora']['target_modules'])}")

    print("\nTraining:")
    print(f"  Epochs: {config['training']['num_epochs']}")
    print(f"  Batch size: {config['training']['per_device_train_batch_size']}")
    print(f"  Gradient accumulation: {config['training']['gradient_accumulation_steps']}")
    print(f"  Effective batch size: {config['training']['per_device_train_batch_size'] * config['training']['gradient_accumulation_steps']}")
    print(f"  Learning rate: {config['training']['learning_rate']}")
    print(f"  LR scheduler: {config['training']['lr_scheduler_type']}")

    print("\nDatasets:")
    print(f"  Train: {len(train_dataset)} examples")
    print(f"  Val: {len(val_dataset)} examples")

    print(f"\nOutput: {ML_ROOT / config['output_dir']}")

    print("\n" + "=" * 80)
    print("Dry run complete. Remove --dry-run flag to start training.")
    print("=" * 80)


def main():
    parser = argparse.ArgumentParser(description="Train intent→assembly LoRA adapter")
    parser.add_argument(
        "--config",
        type=Path,
        default=ML_ROOT / "configs" / "lora_7a.yaml",
        help="Path to config YAML (default: configs/lora_7a.yaml)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print config and dataset info without training",
    )
    args = parser.parse_args()

    # Load config
    print(f"Loading config from {args.config}...")
    config = load_config(args.config)

    # Load model and tokenizer
    model, tokenizer = load_model_and_tokenizer(config)

    # Create datasets
    train_dataset, val_dataset = create_datasets(config, tokenizer)

    # Dry run: print info and exit
    if args.dry_run:
        print_dry_run_info(config, train_dataset, val_dataset)
        return

    # Setup LoRA
    model = setup_lora(model, config)

    # Create training arguments
    training_args = create_training_args(config)

    # Create trainer
    print("\nInitializing trainer...")
    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=train_dataset,
        eval_dataset=val_dataset,
    )

    # Train
    print("\n" + "=" * 80)
    print("Starting training...")
    print("=" * 80 + "\n")

    trainer.train()

    # Save final adapter
    final_dir = ML_ROOT / config["output_dir"] / "final"
    print(f"\nSaving final adapter to {final_dir}...")
    trainer.model.save_pretrained(final_dir)
    tokenizer.save_pretrained(final_dir)

    # Print training summary
    print("\n" + "=" * 80)
    print("Training Complete")
    print("=" * 80)

    metrics = trainer.state.log_history
    if metrics:
        # Get final train and eval losses
        train_losses = [m["loss"] for m in metrics if "loss" in m]
        eval_losses = [m["eval_loss"] for m in metrics if "eval_loss" in m]

        if train_losses:
            print(f"\nFinal train loss: {train_losses[-1]:.4f}")
        if eval_losses:
            print(f"Final eval loss: {eval_losses[-1]:.4f}")
            print(f"Best eval loss: {min(eval_losses):.4f}")

    print(f"\nAdapter saved to: {final_dir}")
    print("=" * 80)


if __name__ == "__main__":
    main()
