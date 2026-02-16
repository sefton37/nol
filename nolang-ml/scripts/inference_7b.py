#!/usr/bin/env python3
"""Inference for Phase 7b: Assembly → Description generation.

Loads base model + LoRA adapter and generates English descriptions of NoLang assembly.

Usage:
    # Single assembly
    python scripts/inference_7b.py --assembly "CONST I64 0x0000 0x002a\nHALT"

    # Batch from file
    python scripts/inference_7b.py --input data/splits/test_7b.jsonl --output outputs/descriptions/test_7b.jsonl

    # From assembly file
    python scripts/inference_7b.py --file tests/programs/ex001.nol
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
    "You are a NoLang code explainer. Describe what this program does in plain "
    "English. Focus on function purpose, input/output behavior, and edge cases. "
    "Do NOT explain syntax. Describe what the code DOES, not what it was INTENDED to do."
)


def load_config(config_path: Path | None = None) -> dict:
    """Load LoRA config from YAML."""
    if config_path is None:
        config_path = ML_ROOT / "configs" / "lora_7b.yaml"
    with open(config_path) as f:
        return yaml.safe_load(f)


def load_model(config: dict, adapter_path: Path | None = None):
    """Load base model with quantization and LoRA adapter."""
    model_name = config["model"]["base"]

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

    if adapter_path is None:
        adapter_path = ML_ROOT / config["output_dir"] / "final"
    if adapter_path.exists():
        print(f"Loading LoRA adapter: {adapter_path}")
        model = PeftModel.from_pretrained(model, str(adapter_path))
    else:
        print(f"WARNING: No adapter found at {adapter_path}, using base model")

    model.eval()
    return model, tokenizer


def build_prompt(assembly: str) -> list[dict]:
    """Build chat messages for the assembly → description task."""
    return [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": f"Assembly:\n{assembly}"},
    ]


def generate_description(
    model,
    tokenizer,
    assembly: str,
    config: dict,
) -> str:
    """Generate description from assembly."""
    messages = build_prompt(assembly)
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

    generated = outputs[0][inputs["input_ids"].shape[1]:]
    description = tokenizer.decode(generated, skip_special_tokens=True).strip()
    return description


def main():
    parser = argparse.ArgumentParser(description="Describe NoLang assembly in English")
    parser.add_argument("--config", type=Path, help="Path to LoRA config YAML")
    parser.add_argument("--adapter", type=Path, help="Path to LoRA adapter directory")
    parser.add_argument("--assembly", type=str, help="Assembly string (use \\n for newlines)")
    parser.add_argument("--file", type=Path, help="Assembly .nol file to describe")
    parser.add_argument("--input", type=Path, help="Input JSONL file with assemblies")
    parser.add_argument("--output", type=Path, help="Output JSONL file for descriptions")
    args = parser.parse_args()

    config = load_config(args.config)
    model, tokenizer = load_model(config, args.adapter)

    if args.assembly:
        # Unescape literal \n in command-line arg
        assembly = args.assembly.replace("\\n", "\n")
        desc = generate_description(model, tokenizer, assembly, config)
        print(desc)

    elif args.file:
        with open(args.file) as f:
            assembly = f.read()
        desc = generate_description(model, tokenizer, assembly, config)
        print(desc)

    elif args.input:
        if args.output is None:
            print("ERROR: --output required with --input", file=sys.stderr)
            sys.exit(1)
        args.output.parent.mkdir(parents=True, exist_ok=True)

        with open(args.input) as fin, open(args.output, "w") as fout:
            for line_num, line in enumerate(fin, 1):
                entry = json.loads(line.strip())
                assembly = entry["assembly"]
                print(f"[{line_num}] {assembly[:50].replace(chr(10), ' / ')}...", end=" ", flush=True)
                desc = generate_description(model, tokenizer, assembly, config)
                result = {
                    "assembly": assembly,
                    "generated_description": desc,
                }
                if "description" in entry:
                    result["reference_description"] = entry["description"]
                fout.write(json.dumps(result, ensure_ascii=False) + "\n")
                print("done")

        print(f"\nGenerated {line_num} descriptions → {args.output}")

    else:
        parser.print_help()
        sys.exit(1)


if __name__ == "__main__":
    main()
