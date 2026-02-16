#!/usr/bin/env python3
"""Inference for Phase 7a: Intent → Assembly generation.

Loads base model + LoRA adapter and generates NoLang assembly from intent.

Usage:
    # Single intent
    python scripts/inference_7a.py --intent "Compute the absolute value of an integer"

    # Batch from file
    python scripts/inference_7a.py --input data/splits/test_7a.jsonl --output outputs/generations/test_7a.jsonl

    # Interactive mode
    python scripts/inference_7a.py --interactive
"""

import argparse
import json
import sys
from pathlib import Path

import torch
import yaml
from peft import PeftModel
from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig

SCRIPT_DIR = Path(__file__).resolve().parent
ML_ROOT = SCRIPT_DIR.parent

SYSTEM_PROMPT = (
    "You are a NoLang code generator. NoLang uses fixed 64-bit instructions, "
    "de Bruijn indices (REF 0 = most recent binding), exhaustive pattern matching, "
    "and mandatory HASH in function blocks. Use placeholder HASH 0x0000 0x0000 0x0000. "
    "Generate syntactically correct NoLang assembly for the given intent."
)


def load_config(config_path: Path | None = None) -> dict:
    """Load LoRA config from YAML."""
    if config_path is None:
        config_path = ML_ROOT / "configs" / "lora_7a.yaml"
    with open(config_path) as f:
        return yaml.safe_load(f)


def load_model(config: dict, adapter_path: Path | None = None):
    """Load base model with quantization and LoRA adapter."""
    model_name = config["model"]["base"]

    # Quantization config
    qcfg = config["quantization"]
    bnb_config = BitsAndBytesConfig(
        load_in_4bit=qcfg["load_in_4bit"],
        bnb_4bit_quant_type=qcfg["bnb_4bit_quant_type"],
        bnb_4bit_compute_dtype=getattr(torch, qcfg["bnb_4bit_compute_dtype"]),
        bnb_4bit_use_double_quant=qcfg["bnb_4bit_use_double_quant"],
    )

    print(f"Loading base model: {model_name}")
    tokenizer = AutoTokenizer.from_pretrained(model_name)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    model = AutoModelForCausalLM.from_pretrained(
        model_name,
        quantization_config=bnb_config,
        device_map="auto",
        torch_dtype=torch.bfloat16,
    )

    # Load LoRA adapter
    if adapter_path is None:
        adapter_path = ML_ROOT / config["output_dir"] / "final"
    if adapter_path.exists():
        print(f"Loading LoRA adapter: {adapter_path}")
        model = PeftModel.from_pretrained(model, str(adapter_path))
    else:
        print(f"WARNING: No adapter found at {adapter_path}, using base model")

    model.eval()
    return model, tokenizer


def build_prompt(intent: str) -> list[dict]:
    """Build chat messages for the intent → assembly task."""
    return [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": f"Intent: {intent}"},
    ]


def generate_assembly(
    model,
    tokenizer,
    intent: str,
    config: dict,
) -> str:
    """Generate assembly from a single intent."""
    messages = build_prompt(intent)
    input_text = tokenizer.apply_chat_template(
        messages, tokenize=False, add_generation_prompt=True
    )
    inputs = tokenizer(input_text, return_tensors="pt").to(model.device)

    inf_cfg = config["inference"]
    with torch.no_grad():
        outputs = model.generate(
            **inputs,
            max_new_tokens=inf_cfg["max_new_tokens"],
            temperature=inf_cfg["temperature"] if inf_cfg["do_sample"] else 1.0,
            top_p=inf_cfg["top_p"] if inf_cfg["do_sample"] else 1.0,
            do_sample=inf_cfg["do_sample"],
            pad_token_id=tokenizer.pad_token_id,
        )

    # Extract only the generated tokens (after input)
    generated = outputs[0][inputs["input_ids"].shape[1]:]
    assembly = tokenizer.decode(generated, skip_special_tokens=True).strip()
    return assembly


def main():
    parser = argparse.ArgumentParser(description="Generate NoLang assembly from intent")
    parser.add_argument("--config", type=Path, help="Path to LoRA config YAML")
    parser.add_argument("--adapter", type=Path, help="Path to LoRA adapter directory")
    parser.add_argument("--intent", type=str, help="Single intent to generate from")
    parser.add_argument("--input", type=Path, help="Input JSONL file with intents")
    parser.add_argument("--output", type=Path, help="Output JSONL file for generations")
    parser.add_argument("--interactive", action="store_true", help="Interactive mode")
    args = parser.parse_args()

    config = load_config(args.config)
    model, tokenizer = load_model(config, args.adapter)

    if args.intent:
        assembly = generate_assembly(model, tokenizer, args.intent, config)
        print(assembly)

    elif args.input:
        if args.output is None:
            print("ERROR: --output required with --input", file=sys.stderr)
            sys.exit(1)
        args.output.parent.mkdir(parents=True, exist_ok=True)

        with open(args.input) as fin, open(args.output, "w") as fout:
            for line_num, line in enumerate(fin, 1):
                entry = json.loads(line.strip())
                intent = entry["intent"]
                print(f"[{line_num}] {intent[:60]}...", end=" ", flush=True)
                assembly = generate_assembly(model, tokenizer, intent, config)
                result = {
                    "intent": intent,
                    "generated_assembly": assembly,
                }
                if "assembly" in entry:
                    result["reference_assembly"] = entry["assembly"]
                if "witnesses" in entry:
                    result["witnesses"] = entry["witnesses"]
                fout.write(json.dumps(result, ensure_ascii=False) + "\n")
                print("done")

        print(f"\nGenerated {line_num} assemblies → {args.output}")

    elif args.interactive:
        print("NoLang Assembly Generator (type 'quit' to exit)")
        print("=" * 60)
        while True:
            try:
                intent = input("\nIntent: ").strip()
            except (EOFError, KeyboardInterrupt):
                break
            if intent.lower() in ("quit", "exit", "q"):
                break
            if not intent:
                continue
            assembly = generate_assembly(model, tokenizer, intent, config)
            print(f"\nAssembly:\n{assembly}")

    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
