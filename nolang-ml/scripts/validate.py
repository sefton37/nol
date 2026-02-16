#!/usr/bin/env python3
"""Validation pipeline for LLM-generated NoLang assembly.

Calls the Rust CLI to:
1. Normalize HASH placeholders
2. Compute correct hashes via `nolang hash`
3. Patch hashes into assembly
4. Assemble text → binary
5. Verify binary
6. Optionally run witness tests

Usage:
    from validate import validate_assembly
    result = validate_assembly("FUNC 1 3\nPARAM I64\nREF 0\nRET\nHASH 0x0000 0x0000 0x0000\nENDFUNC\nCONST I64 0x0000 0x002a\nCALL 0\nHALT")
"""

import json
import os
import re
import subprocess
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent.parent  # nolang-ml/../ = nol/

# Find the nolang binary — prefer release build
_NOLANG_BIN = None
for candidate in [
    PROJECT_ROOT / "target" / "release" / "nolang",
    PROJECT_ROOT / "target" / "debug" / "nolang",
]:
    if candidate.exists():
        _NOLANG_BIN = str(candidate)
        break

# Fall back to PATH
if _NOLANG_BIN is None:
    import shutil
    _NOLANG_BIN = shutil.which("nolang")


def get_nolang_bin() -> str:
    """Return path to nolang CLI binary, raising if not found."""
    if _NOLANG_BIN is None:
        raise FileNotFoundError(
            "nolang CLI binary not found. Build with: "
            "cargo build --release -p nolang-cli"
        )
    return _NOLANG_BIN


# Regex to match HASH lines with any values
HASH_LINE_RE = re.compile(r"^HASH\s+0x[0-9a-fA-F]+\s+0x[0-9a-fA-F]+\s+0x[0-9a-fA-F]+$", re.MULTILINE)
HASH_PLACEHOLDER = "HASH 0x0000 0x0000 0x0000"


@dataclass
class ValidationResult:
    """Result of validating a generated assembly program."""
    assembly: str  # final assembly (with corrected hashes)
    hash_patched: bool = False
    assembled: bool = False
    verified: bool = False
    witnesses_passed: bool = False
    witnesses_total: int = 0
    witnesses_ok: int = 0
    errors: list[str] = field(default_factory=list)
    binary_path: str | None = None

    @property
    def fully_valid(self) -> bool:
        return self.assembled and self.verified

    def to_dict(self) -> dict:
        return {
            "assembled": self.assembled,
            "verified": self.verified,
            "witnesses_passed": self.witnesses_passed,
            "witnesses_total": self.witnesses_total,
            "witnesses_ok": self.witnesses_ok,
            "errors": self.errors,
        }


def _run_cmd(args: list[str], timeout: float = 30.0) -> subprocess.CompletedProcess:
    """Run a subprocess with timeout."""
    return subprocess.run(
        args,
        capture_output=True,
        text=True,
        timeout=timeout,
    )


def normalize_hashes(assembly: str) -> str:
    """Replace all HASH lines with placeholder values."""
    return HASH_LINE_RE.sub(HASH_PLACEHOLDER, assembly)


def compute_hashes(assembly: str) -> str | None:
    """Run `nolang hash` on assembly, return output with correct hash lines.

    The `nolang hash` command reads a .nol file and outputs one HASH line
    per FUNC block (in order), with correct values computed via blake3.
    """
    nolang = get_nolang_bin()
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".nol", delete=False
    ) as f:
        f.write(assembly)
        tmp_path = f.name

    try:
        result = _run_cmd([nolang, "hash", tmp_path])
        if result.returncode != 0:
            return None
        return result.stdout
    finally:
        os.unlink(tmp_path)


def patch_hashes(original: str, hashed_output: str) -> str:
    """Patch correct HASH values from nolang hash output into the assembly.

    Strategy: find all HASH lines in original and hashed_output, replace
    in order (FUNC blocks are in the same order).
    """
    original_hashes = HASH_LINE_RE.findall(original)
    correct_hashes = HASH_LINE_RE.findall(hashed_output)

    if len(original_hashes) != len(correct_hashes):
        # Mismatched FUNC count — return hashed output directly
        return hashed_output

    result = original
    for old_hash, new_hash in zip(original_hashes, correct_hashes):
        result = result.replace(old_hash, new_hash, 1)
    return result


def assemble(assembly: str) -> tuple[bool, str | None, str]:
    """Assemble text to binary, return (success, binary_path, error_msg)."""
    nolang = get_nolang_bin()
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".nol", delete=False
    ) as f:
        f.write(assembly)
        nol_path = f.name

    nolb_path = nol_path.replace(".nol", ".nolb")

    try:
        result = _run_cmd([nolang, "assemble", nol_path, "-o", nolb_path])
        if result.returncode == 0 and os.path.exists(nolb_path):
            return True, nolb_path, ""
        error = result.stderr.strip() or result.stdout.strip()
        return False, None, error
    finally:
        os.unlink(nol_path)


def verify(binary_path: str) -> tuple[bool, str]:
    """Verify a binary program, return (success, error_msg)."""
    nolang = get_nolang_bin()
    result = _run_cmd([nolang, "verify", binary_path])
    if result.returncode == 0:
        return True, ""
    error = result.stderr.strip() or result.stdout.strip()
    return False, error


def run_witnesses(
    binary_path: str, witnesses: list[dict]
) -> tuple[int, int, list[str]]:
    """Run witness tests, return (total, passed, errors)."""
    if not witnesses:
        return 0, 0, []

    nolang = get_nolang_bin()

    # Write witness file
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".json", delete=False
    ) as f:
        json.dump(witnesses, f)
        wit_path = f.name

    try:
        result = _run_cmd([nolang, "witness", binary_path, wit_path])
        total = len(witnesses)
        if result.returncode == 0:
            return total, total, []
        # Parse partial results from output
        errors = [result.stderr.strip() or result.stdout.strip()]
        # Try to count passes from output
        passed = 0
        for line in result.stdout.split("\n"):
            if "PASS" in line.upper():
                passed += 1
        return total, passed, errors
    finally:
        os.unlink(wit_path)


def validate_assembly(
    assembly: str,
    witnesses: list[dict] | None = None,
) -> ValidationResult:
    """Full validation pipeline for generated assembly.

    1. Normalize HASH placeholders
    2. Compute correct hashes via nolang hash
    3. Patch hashes
    4. Assemble
    5. Verify
    6. Optionally run witnesses
    """
    result = ValidationResult(assembly=assembly)

    # Step 1: Normalize hashes
    normalized = normalize_hashes(assembly)

    # Step 2: Compute correct hashes (only if HASH lines present)
    if HASH_PLACEHOLDER in normalized:
        hashed_output = compute_hashes(normalized)
        if hashed_output is None:
            result.errors.append("hash computation failed (nolang hash returned error)")
            result.assembly = normalized
            return result
        # Step 3: Patch
        patched = patch_hashes(normalized, hashed_output)
        result.assembly = patched
        result.hash_patched = True
    else:
        result.assembly = normalized

    # Step 4: Assemble
    ok, binary_path, error = assemble(result.assembly)
    result.assembled = ok
    if not ok:
        result.errors.append(f"assembly failed: {error}")
        return result
    result.binary_path = binary_path

    # Step 5: Verify
    ok, error = verify(binary_path)
    result.verified = ok
    if not ok:
        result.errors.append(f"verification failed: {error}")

    # Step 6: Witnesses
    if witnesses and binary_path:
        total, passed, wit_errors = run_witnesses(binary_path, witnesses)
        result.witnesses_total = total
        result.witnesses_ok = passed
        result.witnesses_passed = (passed == total)
        result.errors.extend(wit_errors)

    # Cleanup binary
    if binary_path and os.path.exists(binary_path):
        os.unlink(binary_path)
        result.binary_path = None

    return result


def validate_batch(
    items: list[dict],
    max_workers: int = 4,
) -> list[ValidationResult]:
    """Validate a batch of generated assemblies in parallel."""
    from concurrent.futures import ProcessPoolExecutor, as_completed

    results = [None] * len(items)

    def _validate_one(idx: int, item: dict) -> tuple[int, ValidationResult]:
        witnesses = item.get("witnesses")
        return idx, validate_assembly(item["assembly"], witnesses)

    with ProcessPoolExecutor(max_workers=max_workers) as executor:
        futures = {
            executor.submit(_validate_one, i, item): i
            for i, item in enumerate(items)
        }
        for future in as_completed(futures):
            idx, vr = future.result()
            results[idx] = vr

    return results


if __name__ == "__main__":
    import sys

    if len(sys.argv) < 2:
        print("Usage: python validate.py <assembly_file.nol>")
        print("       python validate.py --test")
        sys.exit(1)

    if sys.argv[1] == "--test":
        # Quick self-test with a known-good program
        test_asm = "CONST I64 0x0000 0x002a\nHALT\n"
        print("Testing with: CONST I64 0x0000 0x002a / HALT")
        r = validate_assembly(test_asm)
        print(f"  Assembled: {r.assembled}")
        print(f"  Verified:  {r.verified}")
        print(f"  Errors:    {r.errors}")

        # Test with HASH placeholder (body_len=4: PARAM, REF, RET, HASH)
        test_func = (
            "FUNC 1 4\nPARAM I64\nREF 0\nRET\n"
            "HASH 0x0000 0x0000 0x0000\nENDFUNC\n"
            "CONST I64 0x0000 0x002a\nCALL 0\nHALT\n"
        )
        print("\nTesting with FUNC + HASH placeholder:")
        r = validate_assembly(test_func)
        print(f"  Hash patched: {r.hash_patched}")
        print(f"  Assembled:    {r.assembled}")
        print(f"  Verified:     {r.verified}")
        print(f"  Errors:       {r.errors}")
    else:
        with open(sys.argv[1]) as f:
            asm = f.read()
        r = validate_assembly(asm)
        print(json.dumps(r.to_dict(), indent=2))
