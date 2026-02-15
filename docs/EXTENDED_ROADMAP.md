# Extended Roadmap: Phases 6-8

> **Prerequisite:** Phases 1-5 must be complete before any work in this document begins.
> See `BUILD_ORDER.md` for Phases 1-5. See `SEMANTIC_VERIFICATION.md` for the full architectural rationale.

## Overview

Phases 1-5 build the foundation: encode, execute, verify, assemble, integrate. The system can take assembly text, prove it's mechanically valid, and run it.

Phases 6-8 close the semantic gap: the distance between what a human means and what a program does. This is achieved through layered verification — contracts, witnesses, and reflective description — that progressively formalize intent until the only remaining judgment is a human comparing two English sentences.

---

## Phase 6: Corpus Expansion + Semantic Layers

**Goal:** Extend the training pair format with contracts and witnesses. Build 200+ programs that exercise all opcodes and include rich semantic annotations.

### 6a: Extend Contract Instruction Set

**What:** Design and implement richer contract primitives that express relational properties.

**Target contract expressiveness:**
- Conditional assertions: `if input >= 0 then result == input`
- Equality with expressions: `result == input * 2`
- Ordering: `for all i in 0..len-1: result[i] <= result[i+1]`
- Set properties: `result is a permutation of input`

**Deliverables:**
- Specification addendum to SPEC.md for new contract instructions (or contract sub-language)
- Updates to `common` (new opcodes or contract representation)
- Updates to `verifier` (validate richer contracts)
- Updates to `vm` (enforce richer contracts at runtime)

**Acceptance:** 20+ programs with rich contracts that pass verification and enforcement.

### 6b: Witness Format + Runner

**What:** Extend `.nolt` training pair format with witnesses. Build a witness runner in the CLI.

**Extended `.nolt` format:**
```json
{
  "intent": "Compute absolute value of an integer",
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

**CLI extension:**
- `nolang witness input.nolb witnesses.json` — run program against all witnesses, report pass/fail

**Acceptance:** Witness runner executes all example programs against their witnesses with correct results.

### 6c: Corpus Building

**What:** Write 200+ programs covering:
- Sorting algorithms (insertion, merge, selection)
- Data structure operations (stack, queue, linked list via arrays)
- String/character operations (once CHAR support is mature)
- Mathematical functions (abs, max, min, clamp, gcd, fibonacci, power)
- Array operations (map, filter, reduce, zip, reverse, contains)
- Validation functions (is_sorted, is_palindrome, all_positive)

Each program includes: intent, assembly, contracts, and 5+ witnesses.

**Acceptance:** All 200+ programs assemble, verify, execute, and pass all witnesses.

---

## Phase 7: LLM Integration — Generation + Description

**Goal:** Train a model on the corpus. Build the bidirectional pipeline: intent → program and program → description.

### 7a: Intent → Program + Contracts + Witnesses

**What:** Fine-tune or LoRA-adapt a small model on the Phase 6 corpus.

**Training format:** (intent → assembly + contracts + witnesses)

**Pipeline:**
1. Human provides natural language intent.
2. Model generates assembly, contracts, and witnesses.
3. Assembler converts to binary.
4. Verifier checks mechanical validity (Layer 1).
5. Verifier checks contract consistency (Layer 2).
6. Witness runner checks all examples (Layer 3).
7. If any layer rejects: failure becomes training signal (Phase 8).

**Acceptance:** Model generates valid, verified programs for 80%+ of held-out intents on first attempt.

### 7b: Program → Description (Reflective Layer)

**What:** Train a model (or prompt strategy) to read NoLang assembly and produce an accurate natural language description.

**Critical constraint:** The description model receives ONLY the assembly. It does not see the original intent. This ensures independence — the description is derived from what the program does, not what the human asked for.

**Training format:** (assembly → human-verified description)

**Acceptance:** Human evaluators rate descriptions as accurate for 90%+ of test programs.

### 7c: Comparison Interface

**What:** Build the human-in-the-loop confirmation UI.

```
You said:    "Compute the absolute value of an integer"
This does:   "Takes one I64 parameter. Returns the input unchanged
              if non-negative. Negates the input if negative."
Match? [yes/no]
```

**Integration with ReOS:** This is the confirmation step before execution in the natural language → verified execution pipeline.

**Acceptance:** End-to-end flow from intent to confirmation works for all corpus programs.

---

## Phase 8: Feedback Loop

**Goal:** Failures at any layer become structured training signal that improves the model.

### 8a: Contract Violation Signal

When a generated program violates its own contracts:
- The contract identifies *which* semantic property was violated.
- The violation provides a specific, structured error (not just "wrong").
- This (intent, failed_program, violated_contract) triple becomes a negative training example.

### 8b: Witness Failure Signal

When a generated program fails a witness:
- The witness provides a concrete counterexample: "For input [-13], expected 13, got -13."
- This is the most actionable training signal — the model learns from specific cases.
- Failed witnesses can be automatically augmented (generate more witnesses near the failure).

### 8c: Human Rejection Signal

When a human rejects at the reflective layer:
- The (intent, description, "no match") triple indicates the model generated a valid program that does the wrong thing.
- This is the most valuable signal — it catches errors that passed all automated layers.
- Over time, these rejections should decrease as the model improves.

### Feedback Architecture

```
Intent
  │
  ▼
LLM generates program + contracts + witnesses
  │
  ├─ Layer 1 fail → structural error → retrain on error type
  ├─ Layer 2 fail → contract violation → retrain on (intent, violation)
  ├─ Layer 3 fail → witness failure → retrain on (intent, counterexample)
  ├─ Layer 4 fail → human rejection → retrain on (intent, description, "no")
  │
  └─ All pass → (intent, program, contracts, witnesses) → positive training example
```

**Acceptance:** Demonstrate measurable improvement in first-attempt success rate after one feedback cycle on 50+ failure cases.

---

## Success Criteria for the Full Stack

When Phases 1-8 are complete, the system satisfies:

1. **Mechanical safety:** No program executes without passing structural verification.
2. **Semantic coverage:** Intent is expressed through contracts, witnesses, and description — not just code.
3. **Independent verification:** Four layers check correctness independently. A wrong program must fool all four.
4. **Human sovereignty:** The final confirmation is a natural language comparison that any human can make.
5. **Self-improvement:** Every failure makes the system better through structured training signal.

The irreducible gap — a human comparing two English sentences — is as small as the semantic gap gets. For practical purposes, it's small enough.
