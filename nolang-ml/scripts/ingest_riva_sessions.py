#!/usr/bin/env python3
"""Ingest RIVA session logs into .nolt training pairs.

Reads RIVA session JSON files (from Talking Rock's SessionLogger) and
extracts (intent, nol_assembly) pairs. Valid pairs are written as JSONL
in .nolt format compatible with the existing training pipeline.

Usage:
    python ingest_riva_sessions.py --sessions-dir /path/to/sessions --output data/riva_pairs.nolt [--nol-binary /path/to/nolang]
"""

import argparse
import base64
import json
import logging
import subprocess
import sys
import tempfile
from pathlib import Path

logger = logging.getLogger(__name__)

SCRIPT_DIR = Path(__file__).resolve().parent
ML_ROOT = SCRIPT_DIR.parent
PROJECT_ROOT = ML_ROOT.parent


def find_nol_pairs(session_data: dict) -> list[tuple[str, str]]:
    """Extract (intent, assembly) pairs from a RIVA session.

    Looks for cycles where the action has nol_assembly set.
    The intent comes from the intention's 'what' field.

    Returns list of (intent_string, assembly_string) tuples.
    """
    pairs = []

    # Session structure: intentions -> cycles -> action
    intentions = session_data.get("intentions", [])
    for intention in intentions:
        intent_text = intention.get("what", "")
        if not intent_text:
            continue

        cycles = intention.get("cycles", [])
        for cycle in cycles:
            action = cycle.get("action", {})
            nol_assembly = action.get("nol_assembly")
            if nol_assembly and nol_assembly.strip():
                pairs.append((intent_text, nol_assembly.strip()))

    return pairs


def validate_assembly(assembly: str, nol_binary: Path) -> tuple[bool, bytes | None]:
    """Validate assembly text by running it through the nolang assembler.

    Returns (is_valid, binary_bytes_or_none).
    """
    with tempfile.NamedTemporaryFile(suffix=".nol", mode="w", delete=False) as f:
        f.write(assembly)
        input_path = Path(f.name)

    output_path = input_path.with_suffix(".nolb")

    try:
        result = subprocess.run(
            [str(nol_binary), "assemble", str(input_path), "-o", str(output_path)],
            capture_output=True,
            text=True,
            timeout=10,
        )

        if result.returncode == 0 and output_path.exists():
            binary = output_path.read_bytes()
            return True, binary
        else:
            logger.warning("Assembly failed: %s", result.stderr.strip())
            return False, None
    except subprocess.TimeoutExpired:
        logger.warning("Assembly timed out")
        return False, None
    finally:
        input_path.unlink(missing_ok=True)
        output_path.unlink(missing_ok=True)


def ingest_sessions(
    sessions_dir: Path,
    output_path: Path,
    nol_binary: Path,
) -> dict[str, int]:
    """Process all session files and write training pairs.

    Returns stats dict with counts.
    """
    stats = {
        "sessions_processed": 0,
        "pairs_found": 0,
        "pairs_valid": 0,
        "pairs_invalid": 0,
    }

    session_files = sorted(sessions_dir.glob("*.json"))
    if not session_files:
        logger.warning("No session files found in %s", sessions_dir)
        return stats

    output_path.parent.mkdir(parents=True, exist_ok=True)

    with open(output_path, "w") as out:
        for session_file in session_files:
            stats["sessions_processed"] += 1

            try:
                with open(session_file) as f:
                    session_data = json.load(f)
            except (json.JSONDecodeError, OSError) as e:
                logger.warning("Skipping %s: %s", session_file.name, e)
                continue

            pairs = find_nol_pairs(session_data)
            stats["pairs_found"] += len(pairs)

            for intent, assembly in pairs:
                is_valid, binary = validate_assembly(assembly, nol_binary)

                if is_valid and binary:
                    stats["pairs_valid"] += 1
                    b64 = base64.b64encode(binary).decode("ascii")
                    record = {
                        "intent": intent,
                        "assembly": assembly,
                        "binary_b64": b64,
                        "source": session_file.name,
                    }
                    out.write(json.dumps(record) + "\n")
                else:
                    stats["pairs_invalid"] += 1
                    logger.info(
                        "Invalid pair from %s: %s",
                        session_file.name,
                        intent[:60],
                    )

    return stats


def _find_nol_binary() -> Path | None:
    """Auto-detect the nolang binary from common locations."""
    candidates = [
        PROJECT_ROOT / "target" / "release" / "nolang",
        PROJECT_ROOT / "target" / "debug" / "nolang",
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    return None


def main():
    parser = argparse.ArgumentParser(
        description="Ingest RIVA session logs into .nolt training pairs"
    )
    parser.add_argument(
        "--sessions-dir",
        type=Path,
        required=True,
        help="Directory containing RIVA session JSON files",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=ML_ROOT / "data" / "riva_pairs.nolt",
        help="Output .nolt file path (default: data/riva_pairs.nolt)",
    )
    parser.add_argument(
        "--nol-binary",
        type=Path,
        default=None,
        help="Path to nolang binary (default: auto-detect)",
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Enable verbose logging",
    )

    args = parser.parse_args()

    logging.basicConfig(
        level=logging.DEBUG if args.verbose else logging.INFO,
        format="%(levelname)s: %(message)s",
    )

    # Find nolang binary
    nol_binary = args.nol_binary
    if nol_binary is None:
        nol_binary = _find_nol_binary()

    if nol_binary is None or not nol_binary.exists():
        logger.error("nolang binary not found. Use --nol-binary to specify path.")
        sys.exit(1)

    if not args.sessions_dir.is_dir():
        logger.error("Sessions directory not found: %s", args.sessions_dir)
        sys.exit(1)

    logger.info("Using nolang binary: %s", nol_binary)
    logger.info("Sessions directory:  %s", args.sessions_dir)
    logger.info("Output path:         %s", args.output)

    stats = ingest_sessions(args.sessions_dir, args.output, nol_binary)

    print(f"Sessions processed: {stats['sessions_processed']}")
    print(f"Pairs found:        {stats['pairs_found']}")
    print(f"Pairs valid:        {stats['pairs_valid']}")
    print(f"Pairs invalid:      {stats['pairs_invalid']}")

    if stats["pairs_valid"] > 0:
        print(f"Output written to:  {args.output}")
    else:
        print("No valid pairs found.")


if __name__ == "__main__":
    main()
