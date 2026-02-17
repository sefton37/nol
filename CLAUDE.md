# CLAUDE.md — Instructions for Claude Code

## What This Project Is

NoLang is a programming language designed for LLM generation, not human authorship. It has:
- Fixed-width 64-bit instructions (no parsing ambiguity)
- De Bruijn indices instead of variable names (no naming decisions)
- Exactly one canonical representation for any computation (no style variance)
- Structural verification built into the format (hash integrity, exhaustive matching, type safety)

The human describes intent in natural language. The LLM generates canonical binary IR. The verifier confirms correctness. The VM executes it.

## Repository Structure

```
nolang/
├── CLAUDE.md              ← You are here
├── README.md              ← Project overview
├── Cargo.toml             ← Rust workspace root
├── docs/
│   ├── SPEC.md            ← Instruction set specification (THE source of truth)
│   ├── ARCHITECTURE.md    ← Component relationships and interfaces
│   ├── BUILD_ORDER.md     ← Sequenced build plan with gates (Phases 1-8)
│   ├── EXAMPLES.md        ← Example programs in assembly notation
│   └── SEMANTIC_VERIFICATION.md ← Layered verification architecture
├── crates/
│   ├── common/            ← Shared types: opcodes, type tags, instruction encoding
│   ├── vm/                ← Stack-based virtual machine
│   ├── verifier/          ← Static analysis and verification
│   └── assembler/         ← Text assembly ↔ binary translation
├── nolang-cli/            ← CLI binary: assemble, verify, run, hash, witness, generate
│   └── src/
│       ├── main.rs        ← CLI entry point (binary name: `nolang`)
│       ├── lib.rs         ← Exposes json, witness, catalog, generate modules
│       └── catalog/       ← 14 category modules for corpus generation
├── nolang-ml/             ← LLM integration + feedback loop (Python)
│   ├── configs/           ← LoRA configs (lora_7a.yaml, lora_7b.yaml, lora_8a.yaml)
│   ├── scripts/           ← 12 Python scripts (train, inference, evaluate, feedback)
│   ├── run_feedback_cycle.sh ← Phase 8 orchestration: collect → build → retrain → measure
│   ├── data/splits/       ← Train/val/test JSONL splits
│   ├── models/            ← Fine-tuned LoRA adapters
│   └── outputs/           ← Generations, metrics, feedback
└── tests/
    ├── programs/          ← .nol assembly files (19 hand-written + 220 generated)
    ├── witnesses/         ← Witness JSON files for Layer 3 verification
    └── corpus/            ← Verified (intent, binary) training pairs (.nolt)
```

## Build Phases (ALL COMPLETE)

All 8 phases are implemented and committed. 528 Rust tests passing.

1. **common** — Opcode enum (47 opcodes), TypeTag enum, Instruction struct, encode/decode
2. **vm** — Execute instruction streams, stack management, pattern matching, functions
3. **verifier** — Static checks: types, exhaustion, hashes, contracts, stack balance
4. **assembler** — Text ↔ binary bidirectional translation
5. **training** — CLI binary (`nolang`) + integration pipeline, corpus generation
6. **semantic layers** — Rich contracts (IMPLIES/FORALL), witness runner, 220 programs across 14 categories
7. **LLM integration** — LoRA fine-tuning (intent→assembly, assembly→description), comparison UI
8. **feedback loop** — Failure collection, error-aware retraining, improvement measurement

See `docs/BUILD_ORDER.md` for detailed acceptance criteria per phase.
See `docs/SEMANTIC_VERIFICATION.md` for the layered verification architecture.

## Coding Conventions

### Rust Style
- Use `#[derive(Debug, Clone, PartialEq, Eq)]` on all public types
- All public functions have doc comments
- No `unwrap()` except in tests — use `Result` or `Option` with `?`
- No `unsafe` blocks — correctness over performance at this stage
- Every module has unit tests in a `#[cfg(test)] mod tests` block

### Testing Requirements
- Every opcode has at least 3 tests: valid use, edge case, rejection case
- The verifier must never panic on any input — always return `Result`
- Use `proptest` for property-based testing of encode/decode roundtrips
- Assembly → binary → assembly roundtrip must be identity

### Architecture Rules
- The VM does NOT do verification. It assumes valid input. Separation of concerns.
- The verifier does NOT execute code. It only does static analysis.
- The assembler is a mechanical 1:1 translation. No optimization, no sugar.
- `common` has zero dependencies except `std`.

### Error Handling
- Define error types per crate: `VmError`, `VerifyError`, `AsmError`
- Errors carry source location (instruction index) for debugging
- Use `thiserror` for error derives

## Key Design Decisions (DO NOT CHANGE)

1. **Instructions are exactly 64 bits.** No variable-length encoding.
2. **De Bruijn indices, not names.** `REF n` means "the binding n levels up."
3. **One canonical form.** If two instruction streams compute the same thing, they are identical.
4. **Exhaustive pattern matching is the only control flow.** No if/else, no loops. Recursion + match.
5. **Contracts (PRE/POST) are structural, not comments.** They are part of the instruction stream.
6. **HASH fields are mandatory on FUNC blocks.** Computed as blake3 over the block body.

## Reading Order for Context

If you need to understand the project:
1. This file (CLAUDE.md)
2. `docs/SPEC.md` — The instruction set. This is the constitution.
3. `docs/ARCHITECTURE.md` — How components connect.
4. `docs/BUILD_ORDER.md` — What to build and when (Phases 1-8).
5. `docs/EXAMPLES.md` — Concrete programs that ground the abstractions.
6. `docs/SEMANTIC_VERIFICATION.md` — Layered verification architecture for Phases 6-8.

## When In Doubt

- Check SPEC.md. If the spec doesn't cover it, the answer is "don't implement it yet."
- Prefer rejection over silent acceptance. If input might be invalid, reject it.
- Prefer small PRs over big ones. One opcode at a time is fine.
- Write the test first. The test is the contract.
