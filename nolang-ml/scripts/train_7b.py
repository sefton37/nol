#!/usr/bin/env python3
"""
LoRA fine-tuning script for NoLang assembly → description generation.

Trains Llama 3.1 8B Instruct (or CodeLlama 7B fallback) to generate plain English
descriptions of NoLang assembly programs using 4-bit quantization and LoRA.

Usage:
    python scripts/train_7b.py                    # use default config
    python scripts/train_7b.py --config custom.yaml
    python scripts/train_7b.py --dry-run          # print config and dataset info only
"""

import argparse
import json
import sys
from pathlib import Path
from typing import Dict, List, Any

import torch
import yaml
from datasets import Dataset
from transformers import (
    AutoModelForCausalLM,
    AutoTokenizer,
    BitsAndBytesConfig,
    Trainer,
    TrainingArguments,
)
from peft import (
    LoraConfig,
    get_peft_model,
    prepare_model_for_kbit_training,
)


# System prompt for assembly → description task
SYSTEM_PROMPT = (
    "You are a NoLang code explainer. Describe what this program does in plain English. "
    "Focus on function purpose, input/output behavior, and edge cases. "
    "Do NOT explain syntax. Describe what the code DOES, not what it was INTENDED to do."
)


def load_config(config_path: Path) -> Dict[str, Any]:
    """Load YAML configuration file."""
    if not config_path.exists():
        raise FileNotFoundError(f"Config file not found: {config_path}")

    with open(config_path) as f:
        return yaml.safe_load(f)


def load_jsonl(path: Path) -> List[Dict[str, str]]:
    """Load JSONL file as list of dicts."""
    if not path.exists():
        raise FileNotFoundError(f"Data file not found: {path}")

    data = []
    with open(path) as f:
        for line in f:
            line = line.strip()
            if line:
                data.append(json.loads(line))
    return data


def format_chat_prompt(example: Dict[str, str], tokenizer) -> str:
    """
    Format a single example as a chat completion using Llama 3.1 chat template.

    Args:
        example: Dict with 'assembly' and 'description' keys
        tokenizer: HuggingFace tokenizer with chat template

    Returns:
        Formatted string ready for tokenization
    """
    messages = [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": f"Assembly:\n{example['assembly']}"},
        {"role": "assistant", "content": example['description']},
    ]

    # Apply chat template with tokenize=False to get the formatted string
    return tokenizer.apply_chat_template(
        messages,
        tokenize=False,
        add_generation_prompt=False,
    )


def tokenize_dataset(
    examples: List[Dict[str, str]],
    tokenizer,
    max_length: int,
) -> Dataset:
    """
    Tokenize dataset for causal language modeling.

    Args:
        examples: List of dicts with 'assembly' and 'description'
        tokenizer: HuggingFace tokenizer
        max_length: Maximum sequence length

    Returns:
        HuggingFace Dataset ready for training
    """
    formatted_texts = [format_chat_prompt(ex, tokenizer) for ex in examples]

    # Tokenize all at once for efficiency
    tokenized = tokenizer(
        formatted_texts,
        truncation=True,
        max_length=max_length,
        padding=False,  # Dynamic padding in data collator
        return_tensors=None,  # Return lists, not tensors
    )

    # For causal LM, labels are the same as input_ids
    # Trainer will handle shifting and loss masking
    tokenized["labels"] = tokenized["input_ids"].copy()

    return Dataset.from_dict(tokenized)


def create_bnb_config(config: Dict[str, Any]) -> BitsAndBytesConfig:
    """Create BitsAndBytes quantization config from YAML config."""
    quant_cfg = config["quantization"]

    # Map string dtype to torch dtype
    compute_dtype = torch.bfloat16 if quant_cfg["bnb_4bit_compute_dtype"] == "bfloat16" else torch.float16

    return BitsAndBytesConfig(
        load_in_4bit=quant_cfg["load_in_4bit"],
        bnb_4bit_quant_type=quant_cfg["bnb_4bit_quant_type"],
        bnb_4bit_compute_dtype=compute_dtype,
        bnb_4bit_use_double_quant=quant_cfg["bnb_4bit_use_double_quant"],
    )


def create_lora_config(config: Dict[str, Any]) -> LoraConfig:
    """Create LoRA config from YAML config."""
    lora_cfg = config["lora"]

    return LoraConfig(
        r=lora_cfg["r"],
        lora_alpha=lora_cfg["lora_alpha"],
        lora_dropout=lora_cfg["lora_dropout"],
        target_modules=lora_cfg["target_modules"],
        bias=lora_cfg["bias"],
        task_type=lora_cfg["task_type"],
    )


def create_training_args(config: Dict[str, Any], output_dir: Path) -> TrainingArguments:
    """Create training arguments from YAML config."""
    train_cfg = config["training"]

    return TrainingArguments(
        output_dir=str(output_dir),
        num_train_epochs=train_cfg["num_epochs"],
        per_device_train_batch_size=train_cfg["per_device_train_batch_size"],
        per_device_eval_batch_size=train_cfg["per_device_train_batch_size"],
        gradient_accumulation_steps=train_cfg["gradient_accumulation_steps"],
        learning_rate=train_cfg["learning_rate"],
        lr_scheduler_type=train_cfg["lr_scheduler_type"],
        warmup_ratio=train_cfg["warmup_ratio"],
        weight_decay=train_cfg["weight_decay"],
        max_grad_norm=train_cfg["max_grad_norm"],
        eval_strategy="steps",
        eval_steps=train_cfg["eval_steps"],
        save_strategy="steps",
        save_steps=train_cfg["save_steps"],
        logging_steps=train_cfg["logging_steps"],
        save_total_limit=train_cfg["save_total_limit"],
        load_best_model_at_end=train_cfg["load_best_model_at_end"],
        metric_for_best_model=train_cfg["metric_for_best_model"],
        greater_is_better=train_cfg["greater_is_better"],
        fp16=train_cfg["fp16"],
        bf16=train_cfg["bf16"],
        dataloader_num_workers=train_cfg["dataloader_num_workers"],
        remove_unused_columns=False,  # Keep all columns
        report_to="none",  # Disable wandb/tensorboard for now
    )


def print_dry_run_info(
    config: Dict[str, Any],
    train_data: List[Dict[str, str]],
    val_data: List[Dict[str, str]],
    output_dir: Path,
):
    """Print configuration and dataset info for dry run."""
    print("=" * 80)
    print("DRY RUN MODE - Configuration Summary")
    print("=" * 80)
    print()

    print(f"Model: {config['model']['base']}")
    print(f"Fallback: {config['model']['fallback']}")
    print(f"Max Length: {config['model']['max_length']}")
    print()

    print("Quantization:")
    print(f"  4-bit: {config['quantization']['load_in_4bit']}")
    print(f"  Type: {config['quantization']['bnb_4bit_quant_type']}")
    print(f"  Compute dtype: {config['quantization']['bnb_4bit_compute_dtype']}")
    print(f"  Double quant: {config['quantization']['bnb_4bit_use_double_quant']}")
    print()

    print("LoRA:")
    print(f"  Rank: {config['lora']['r']}")
    print(f"  Alpha: {config['lora']['lora_alpha']}")
    print(f"  Dropout: {config['lora']['lora_dropout']}")
    print(f"  Target modules: {', '.join(config['lora']['target_modules'])}")
    print()

    print("Training:")
    print(f"  Epochs: {config['training']['num_epochs']}")
    print(f"  Batch size: {config['training']['per_device_train_batch_size']}")
    print(f"  Gradient accumulation: {config['training']['gradient_accumulation_steps']}")
    print(f"  Learning rate: {config['training']['learning_rate']}")
    print(f"  LR scheduler: {config['training']['lr_scheduler_type']}")
    print(f"  Warmup ratio: {config['training']['warmup_ratio']}")
    print()

    print("Dataset:")
    print(f"  Training examples: {len(train_data)}")
    print(f"  Validation examples: {len(val_data)}")
    print()

    print(f"Output directory: {output_dir}")
    print(f"Final adapter will be saved to: {output_dir}/final/")
    print()

    if train_data:
        print("Sample training example:")
        print("-" * 80)
        print(f"Assembly:\n{train_data[0]['assembly'][:200]}...")
        print()
        print(f"Description:\n{train_data[0]['description'][:200]}...")
        print("-" * 80)

    print()
    print("Dry run complete. Remove --dry-run to start training.")
    print("=" * 80)


def main():
    parser = argparse.ArgumentParser(
        description="Fine-tune Llama 3.1 8B for NoLang assembly → description"
    )
    parser.add_argument(
        "--config",
        type=Path,
        default=Path("configs/lora_7b.yaml"),
        help="Path to YAML config file",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print config and dataset info without training",
    )
    args = parser.parse_args()

    # Resolve paths relative to nolang-ml/ directory
    script_dir = Path(__file__).parent
    ml_root = script_dir.parent  # nolang-ml/

    config_path = ml_root / args.config
    train_path = ml_root / "data/splits/train_7b.jsonl"
    val_path = ml_root / "data/splits/val_7b.jsonl"

    print(f"Loading config from: {config_path}")
    config = load_config(config_path)

    print(f"Loading training data from: {train_path}")
    train_data = load_jsonl(train_path)
    print(f"Loaded {len(train_data)} training examples")

    print(f"Loading validation data from: {val_path}")
    val_data = load_jsonl(val_path)
    print(f"Loaded {len(val_data)} validation examples")

    output_dir = ml_root / config["output_dir"]

    if args.dry_run:
        print_dry_run_info(config, train_data, val_data, output_dir)
        return

    print()
    print("=" * 80)
    print("Starting training pipeline")
    print("=" * 80)
    print()

    # Step 1: Load tokenizer
    print("Step 1/6: Loading tokenizer...")
    model_name = config["model"]["base"]
    tokenizer = AutoTokenizer.from_pretrained(model_name)

    # Ensure pad token is set (Llama models don't have one by default)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
        tokenizer.pad_token_id = tokenizer.eos_token_id

    # Step 2: Tokenize datasets
    print("Step 2/6: Tokenizing datasets...")
    max_length = config["model"]["max_length"]
    train_dataset = tokenize_dataset(train_data, tokenizer, max_length)
    val_dataset = tokenize_dataset(val_data, tokenizer, max_length)
    print(f"  Train: {len(train_dataset)} examples")
    print(f"  Val: {len(val_dataset)} examples")

    # Step 3: Load base model with quantization
    print("Step 3/6: Loading base model with 4-bit quantization...")
    bnb_config = create_bnb_config(config)

    try:
        model = AutoModelForCausalLM.from_pretrained(
            model_name,
            quantization_config=bnb_config,
            device_map="auto",
            trust_remote_code=True,
        )
    except Exception as e:
        print(f"Failed to load {model_name}: {e}")
        fallback = config["model"]["fallback"]
        print(f"Falling back to {fallback}")
        model = AutoModelForCausalLM.from_pretrained(
            fallback,
            quantization_config=bnb_config,
            device_map="auto",
            trust_remote_code=True,
        )

    # Step 4: Prepare model for k-bit training
    print("Step 4/6: Preparing model for k-bit training...")
    model = prepare_model_for_kbit_training(model)

    # Step 5: Add LoRA adapters
    print("Step 5/6: Adding LoRA adapters...")
    lora_config = create_lora_config(config)
    model = get_peft_model(model, lora_config)

    # Print trainable parameters
    trainable_params = sum(p.numel() for p in model.parameters() if p.requires_grad)
    total_params = sum(p.numel() for p in model.parameters())
    print(f"  Trainable params: {trainable_params:,} ({100 * trainable_params / total_params:.2f}%)")
    print(f"  Total params: {total_params:,}")

    # Step 6: Train
    print("Step 6/6: Starting training...")
    training_args = create_training_args(config, output_dir)

    trainer = Trainer(
        model=model,
        args=training_args,
        train_dataset=train_dataset,
        eval_dataset=val_dataset,
        tokenizer=tokenizer,
    )

    # Train and save
    trainer.train()

    # Save final adapter
    final_dir = output_dir / "final"
    print(f"\nSaving final adapter to: {final_dir}")
    trainer.save_model(str(final_dir))
    tokenizer.save_pretrained(str(final_dir))

    # Print training summary
    print()
    print("=" * 80)
    print("Training Complete")
    print("=" * 80)
    print()
    print(f"Best model saved to: {final_dir}")
    print(f"Checkpoints saved to: {output_dir}")
    print()

    # Print final metrics if available
    if trainer.state.log_history:
        final_metrics = trainer.state.log_history[-1]
        print("Final metrics:")
        for key, value in final_metrics.items():
            if isinstance(value, float):
                print(f"  {key}: {value:.4f}")
            else:
                print(f"  {key}: {value}")

    print()
    print("To use the fine-tuned model:")
    print(f"  from peft import PeftModel")
    print(f"  base_model = AutoModelForCausalLM.from_pretrained('{model_name}')")
    print(f"  model = PeftModel.from_pretrained(base_model, '{final_dir}')")


if __name__ == "__main__":
    main()
