# NoLang

**A programming language designed for LLM generation, not human authorship.**

## The Problem

Every programming language alive today optimizes for human readability. Variable names, syntactic sugar, multiple ways to express the same logic — these are features for humans, but error sources for LLMs.

When an LLM generates code, every naming decision is a coin flip. Every style choice is a probability distribution across conventions. Every implicit behavior is invisible context that must be tracked in the attention window.

NoLang eliminates all of it.

## The Design

- **Fixed-width 64-bit instructions** — no parsing ambiguity, no variable-length tokens
- **De Bruijn indices** — positional references instead of names. `REF 0` means "the most recent binding." No naming decisions.
- **One canonical form** — any computation has exactly ONE valid representation. No style variance.
- **Exhaustive pattern matching** — the only control flow. No if/else, no loops. Recursion + match.
- **Structural verification** — hash integrity, type safety, and contract checking built into the format
- **Inline contracts** — preconditions and postconditions are part of the instruction stream, not comments

## How It Works

```
Human Intent → LLM → Binary IR → Verifier → VM → Result
                         ↑            |
                         └── reject ──┘
```

1. A human describes what they want in natural language
2. An LLM generates canonical binary IR (fixed-width instructions)
3. The verifier statically checks the IR for correctness
4. If valid, the VM executes it
5. If invalid, the error feeds back to improve the next generation

Because the IR is canonical, verification is cheap. Because verification is cheap, it can run locally on modest hardware. Because it runs locally, retries are free.

## Current State

Foundation complete (Phases 1-5): common types, VM, verifier, assembler, and CLI/training pipeline. Next: semantic verification layers — contracts, witnesses, LLM integration, and feedback loops (Phases 6-8). See [BUILD_ORDER.md](docs/BUILD_ORDER.md) for the full sequenced plan.

## Documentation

- [SPEC.md](docs/SPEC.md) — Instruction set specification (the source of truth)
- [ARCHITECTURE.md](docs/ARCHITECTURE.md) — Component design and interfaces
- [BUILD_ORDER.md](docs/BUILD_ORDER.md) — What to build and when (Phases 1-8)
- [EXAMPLES.md](docs/EXAMPLES.md) — Example programs in assembly notation
- [SEMANTIC_VERIFICATION.md](docs/SEMANTIC_VERIFICATION.md) — Layered verification architecture (rationale for Phases 6-8)

## License

MIT
