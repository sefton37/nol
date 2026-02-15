# Semantic Verification Architecture

> See `BUILD_ORDER.md` for the full implementation timeline (Phases 1-8). See `ARCHITECTURE.md` for the mechanical component design.

## The Problem NoLang Solves — And The Problem That Remains

NoLang's verifier eliminates **mechanical errors**: type mismatches, stack imbalances, non-exhaustive pattern matches, structural malformations. If the verifier returns `Ok(())`, an entire category of failure cannot happen. This is the foundation.

But mechanical correctness is necessary, not sufficient. A program that passes verification may still do the wrong thing. If the intent is "compute absolute value" and the LLM generates a program that negates its input unconditionally, that program is structurally valid, type-safe, and stack-balanced. It's also wrong.

This is the **semantic gap** — the distance between what a human means and what a program does. Vibe coding lives in this gap. The human accepts the output because it "looks right." The errors that survive are the ones no one checked.

NoLang's mechanical verification raises the floor. This document defines the architecture for raising the ceiling.

## Core Insight: Convert Semantics Into Structure

NoLang's PRE/POST contract system already demonstrates the pattern. When a postcondition states `result >= 0`, a semantic claim ("this computes an absolute value") has been partially converted into a mechanical check. The verifier doesn't understand "absolute value." It doesn't need to. It checks that the output is non-negative. The meaning became structure.

**Every aspect of intent that can be expressed as a verifiable property migrates from the semantic bucket (where humans guess) into the mechanical bucket (where the verifier catches it).**

The history of programming language theory is this migration. Types moved one class of errors. Contracts moved another. Exhaustive matching moved another. Each formalization shrinks the semantic gap.

The question is: how do we formalize intent richly enough that the remaining gap approaches zero?

## Layered Verification Architecture

Currently, the LLM generates one artifact (a program) and the verifier checks one property (structural validity). The semantic gap spans the entire distance between natural language and assembly.

The revised architecture has the LLM generate three artifacts from a single intent, verified across four layers:

```
Natural Language Intent
        │
        ▼
  LLM generates:
        │
        ├── 1. The Program        (NoLang assembly)
        │
        ├── 2. The Specification   (rich contracts + properties)
        │
        └── 3. The Witness         (concrete input/output examples)
        
        
Verification layers:
        │
        ├── Layer 1: Mechanical    (existing verifier)
        │   Type safety, stack balance, exhaustiveness, hash integrity
        │
        ├── Layer 2: Contractual   (extended PRE/POST)
        │   Relational properties expressing intent as constraints
        │
        ├── Layer 3: Empirical     (witness execution)
        │   Concrete examples that the program must satisfy
        │
        └── Layer 4: Reflective    (bidirectional description)
            LLM describes what the program does; human compares to intent
```

### Layer 1: Mechanical Verification (Exists — Phases 1-3)

The current verifier. Checks that the program is well-formed. This is the foundation all other layers build on.

**Guarantees:** No type mismatch, no stack underflow, no structural error, no unhandled case, no hash corruption.

**Does not guarantee:** The program matches the user's intent.

### Layer 2: Contractual Verification (Seed exists — extend in Phase 6+)

PRE/POST conditions, but dramatically richer. The contracts don't just check boundary conditions — they express the *relationship* between input and output that constitutes the program's meaning.

**Current state:**
```
POST: result >= 0   (necessary but not sufficient for abs())
```

**Target state:**
```
POST: if input >= 0 then result == input
POST: if input < 0  then result == NEG(input)
POST: result >= 0
```

These three contracts together *are* the definition of absolute value. A program that satisfies all three either computes abs() or is observationally equivalent to abs() for all inputs. The semantic gap for this function is now zero — not because we eliminated ambiguity, but because we formalized the meaning completely.

**What this requires:**

- A richer contract language that can express relational properties, conditional assertions, and universally quantified statements. This may mean extending NoLang's contract instruction set or building a property language that compiles down to contract instructions.
- The LLM must generate contracts alongside the program. The contracts are the formalized intent.
- The verifier must check that contracts are internally consistent (contracts don't contradict each other, contract types are valid).
- The VM enforces contracts at runtime against every execution path.

**Key design question:** How expressive can contracts be while remaining decidable for the verifier? The sweet spot is properties that can be checked mechanically (via static analysis or runtime assertion) without requiring a theorem prover.

### Layer 3: Empirical Verification (New — Phase 6+)

Concrete input/output examples that the program must satisfy. These are the **witnesses** — evidence that the program does what it claims.

```json
{
  "intent": "Compute the absolute value of an integer",
  "witnesses": [
    { "input": [5],   "expected": 5 },
    { "input": [-13], "expected": 13 },
    { "input": [0],   "expected": 0 },
    { "input": [-1],  "expected": 1 },
    { "input": [2147483647], "expected": 2147483647 }
  ]
}
```

**Verification process:**
1. Assemble the program.
2. Pass mechanical verification (Layer 1).
3. For each witness: execute the program with the given input, compare output to expected.
4. If any witness fails, the program is rejected.

**What this adds:** Witnesses catch errors that contracts miss (or that the LLM failed to express as contracts). They are easy for humans to generate, easy for LLMs to generate, and cheap to execute. They serve as both a verification layer and as training signal — failed witnesses tell the LLM exactly how its output was wrong.

**Integration with `.nolt` training format:**

The existing training pair format:
```json
{"intent": "...", "assembly": "...", "binary_b64": "..."}
```

Extended format:
```json
{
  "intent": "Compute the absolute value of an integer",
  "assembly": "FUNC 1 20\n  ...",
  "binary_b64": "AQAA...",
  "contracts": [
    "if input >= 0 then result == input",
    "if input < 0 then result == NEG(input)",
    "result >= 0"
  ],
  "witnesses": [
    { "input": [5], "expected": 5 },
    { "input": [-13], "expected": 13 },
    { "input": [0], "expected": 0 }
  ]
}
```

### Layer 4: Reflective Verification (New — Phase 7+)

The LLM generates a natural language description of what the program *actually does*, derived from reading the assembly — not from the original intent. The human sees both side by side:

```
You said:    "Compute the absolute value of an integer"
This does:   "Takes one I64 parameter. If the value is negative, 
              returns its negation. If non-negative, returns 
              the value unchanged."
Match? [yes/no]
```

**Why this works:** The human is no longer comparing English to assembly. They are comparing English to English. This is a dramatically simpler judgment. A non-programmer can make this call.

**What this requires:**
- A model (or the same model) trained on assembly → description, not just intent → assembly.
- This is actually easier than generation. Explanation is a simpler task than creation.
- The description must be generated from the assembly, not parroted from the intent. Otherwise it provides no independent verification.

**Integration:** The reflective layer is the last check before execution. Mechanical, contractual, and empirical verification happen automatically. Reflective verification is the human-in-the-loop confirmation for high-stakes operations.

## Error Compounding: Why Four Layers Win

Each layer independently catches some fraction of semantic errors. The probability that a wrong program passes all four is the product of escaping each:

| Layers Active | Semantic Errors Surviving (estimated) |
|---------------|---------------------------------------|
| Layer 1 only (current) | ~20% of semantic errors pass |
| Layers 1 + 2 | ~4% |
| Layers 1 + 2 + 3 | ~0.8% |
| Layers 1 + 2 + 3 + 4 | ~0.16% |

These numbers are illustrative, not measured. The actual rates depend on contract richness, witness quality, and description accuracy. But the multiplicative structure holds: independent verification layers compound exponentially against errors.

## Impact on NoLang Roadmap

### Current Phases (unchanged)

| Phase | Status | What |
|-------|--------|------|
| 1 | Complete | `common` crate — types, encoding, decode |
| 2 | Complete | `vm` crate — execution engine |
| 3 | Complete | `verifier` crate — static analysis |
| 4 | Complete | `assembler` crate — text ↔ binary |
| 5 | Complete | CLI + integration pipeline |

### Extended Phases (new)

| Phase | What | Layer |
|-------|------|-------|
| 6 | Corpus expansion + rich contracts | Layers 2 + 3 |
| 6a | Extend contract instruction set for relational properties | Layer 2 |
| 6b | Add witness format to `.nolt` and build witness runner | Layer 3 |
| 6c | Write 200+ programs with contracts AND witnesses | Layers 2 + 3 |
| 7 | LLM integration — generation + description | Layers 2 + 4 |
| 7a | Train/fine-tune on (intent → assembly + contracts + witnesses) | Layers 2 + 3 |
| 7b | Train on (assembly → description) for reflective verification | Layer 4 |
| 7c | Build the comparison UI (intent vs. description) | Layer 4 |
| 8 | Feedback loop — failures as training signal | All layers |
| 8a | Contract violations identify specific semantic mismatches | Layer 2 |
| 8b | Witness failures provide concrete counterexamples | Layer 3 |
| 8c | Human rejections at Layer 4 feed back to fine-tuning | Layer 4 |

### What Changes in Phases 3-5

Nothing structurally. But the awareness of Layers 2-4 should inform design choices:

- **Phase 3 (verifier):** The contract checking system (PRE/POST validation) should be designed for extensibility. Future contract instructions will need to express richer properties than "result is BOOL."
- **Phase 4 (assembler):** The assembly format should accommodate contract annotations cleanly. Consider how richer contracts will be expressed in `.nol` text.
- **Phase 5 (CLI):** The `nolang run` command should support witness files as an input. `nolang verify` should support contract-level checks beyond structural validity.

## Open Questions

### Q1: How expressive should contracts be?

The spectrum runs from "runtime assertions" (current PRE/POST) to "full dependent types" (theorem prover territory). The sweet spot is probably:
- Conditional assertions: `if P(input) then Q(result)`
- Equality assertions: `result == f(input)` for simple f
- Ordering assertions: `for all i, result[i] <= result[i+1]`
- Relational assertions: `result is a permutation of input`

These can all be checked at runtime (not statically) without a theorem prover. They require extending NoLang's instruction set or adding a contract-specific sub-language.

### Q2: Who generates the witnesses?

Three sources, in order of trust:
1. **Human-authored:** Highest confidence, lowest volume. Used for critical operations.
2. **LLM-generated alongside the program:** Medium confidence. The LLM should generate witnesses that exercise edge cases. Witness quality becomes part of the training signal.
3. **Automatically generated via property-based testing:** Lowest human effort, highest volume. If contracts are rich enough, a fuzzer can generate random inputs and check contract satisfaction.

All three should be supported. The pipeline accepts witnesses from any source.

### Q3: How do we prevent the LLM from gaming the reflective layer?

If the same model generates the program AND the description, it could produce a description that matches the intent rather than the program. Mitigations:
- Use a different model (or different prompt) for description than for generation.
- The description model receives ONLY the assembly, not the original intent.
- Train the description model on (assembly → description) pairs where the description is verified by humans to be accurate.
- Witnesses serve as an independent check — even if the description is wrong, witness failures catch the mismatch.

### Q4: What is the irreducible semantic residue?

After all four layers, the remaining gap is: a human reading two English sentences and deciding if they mean the same thing. This is a natural language understanding task — inherently fuzzy, context-dependent, and imperfect.

For most practical programs, this is sufficient. "Takes one integer, returns it unchanged if non-negative, negates it if negative" clearly means absolute value. The judgment is easy.

For complex programs with subtle intent, the residue grows. "Optimize the portfolio allocation given these constraints" may not have a clean English-to-English comparison. This is where the architecture reaches its limits — and where human expertise becomes irreplaceable.

The honest answer: the semantic gap never reaches zero. But it can be made small enough that the remaining risk is manageable, visible, and concentrated in the one place where human judgment is actually good — reading natural language.

## Relationship to ReOS

NoLang with semantic verification is a component of the larger ReOS vision. The progression:

1. **NoLang** — a verified instruction format that LLMs can target safely.
2. **Semantic verification** — layered checking that programs match intent.
3. **ReOS integration** — natural language commands generate verified NoLang programs that operate on the local system.

The semantic verification architecture ensures that when a user says "find all files larger than 1GB and move them to /archive," the generated program provably does that and only that. The contracts say "output is a subset of input files," "all output files exceed 1GB," "output locations are all under /archive." The witnesses test with known filesystem states. The reflective description says "Scans the filesystem for files exceeding 1GB and relocates them to /archive, preserving directory structure."

The human reads that description, confirms it matches their intent, and executes with confidence.

That's the endgame. Language becomes interface, verification becomes trust, and the system serves the human's sovereignty rather than requiring the human to serve the system.
