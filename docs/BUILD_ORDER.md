# NoLang Build Order

## Rules

1. **Do not start Phase N+1 until Phase N passes ALL acceptance tests.**
2. **Write tests before implementation.** The test defines the contract.
3. **One opcode at a time.** Don't implement all arithmetic at once. Implement ADD, test it, then SUB.
4. **Keep it small.** If a crate exceeds 2,000 lines (excluding tests), something is wrong.
5. **Reject over accept.** When in doubt, reject the input. False negatives are better than false positives.

---

## Phase 1: `common` crate

**Goal:** Define the shared types and prove that encode/decode is lossless.

### What to build

```
crates/common/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── opcode.rs      — Opcode enum with u8 values from SPEC.md
    ├── type_tag.rs     — TypeTag enum with u8 values from SPEC.md
    ├── instruction.rs  — Instruction struct + encode/decode
    ├── program.rs      — Program struct (Vec<Instruction> + helpers)
    ├── value.rs        — Value enum for runtime representation
    └── error.rs        — DecodeError
```

### Implementation details

**Opcode enum:**
```rust
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Bind = 0x01,
    Ref = 0x02,
    Drop = 0x03,
    Const = 0x04,
    ConstExt = 0x05,
    Add = 0x10,
    Sub = 0x11,
    // ... every opcode from SPEC.md Section 4
    Halt = 0xFE,
    Nop = 0xFF,
}
```

Include `TryFrom<u8>` impl that rejects reserved/illegal values.

**Instruction struct:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub opcode: Opcode,
    pub type_tag: TypeTag,  // TypeTag::None for N/A
    pub arg1: u16,
    pub arg2: u16,
    pub arg3: u16,
}
```

**Encode:** `Instruction → [u8; 8]` little-endian.

**Decode:** `[u8; 8] → Result<Instruction, DecodeError>`.

**Value enum:**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    I64(i64),
    U64(u64),
    F64(f64),
    Bool(bool),
    Char(char),
    Unit,
    Variant { tag_count: u16, tag: u16, payload: Box<Value> },
    Tuple(Vec<Value>),
    Array(Vec<Value>),
}
```

### Acceptance tests (ALL must pass)

- [ ] Every opcode from SPEC.md has a corresponding enum variant
- [ ] Every type tag from SPEC.md has a corresponding enum variant
- [ ] `Opcode::try_from(0x00)` returns `Err` (illegal)
- [ ] `Opcode::try_from(0x06)` returns `Err` (reserved range)
- [ ] `decode(encode(instr)) == Ok(instr)` for every valid opcode (exhaustive)
- [ ] `decode` rejects bytes with opcode 0x00
- [ ] `decode` rejects bytes with reserved opcodes
- [ ] `decode` rejects bytes with reserved type tags
- [ ] Encoding is little-endian (first byte is opcode)
- [ ] `proptest`: random valid Instructions roundtrip through encode/decode
- [ ] `proptest`: random [u8; 8] either decode successfully or return a specific error

### Phase 1 gate
Run `cargo test -p nolang-common`. All tests pass. Zero warnings.

---

## Phase 2: `vm` crate

**Goal:** Execute verified instruction streams and produce results.

### What to build

```
crates/vm/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── machine.rs   — The VM: stack, bindings, call stack, pc
    ├── execute.rs   — Main execution loop + opcode dispatch
    └── error.rs     — RuntimeError
```

### Implementation order (build and test each group before the next)

**Group A: Foundation**
1. HALT — create VM, load program, execute until HALT, return top of stack
2. CONST — push constants, verify they appear on stack
3. NOP — skip and continue
4. BIND/REF/DROP — binding and reference mechanics

Test: A program that pushes a constant and halts returns that constant.
Test: A program that binds a value and refs it gets the same value back.
Test: De Bruijn index shifting works correctly with multiple binds.

**Group B: Arithmetic**
5. ADD — pop two, push sum
6. SUB, MUL, DIV, MOD — same pattern
7. NEG — unary
8. Comparisons: EQ, NEQ, LT, GT, LTE, GTE
9. Logic: AND, OR, NOT, XOR, SHL, SHR

Test: `CONST 5, CONST 3, ADD, HALT` → `I64(8)`
Test: `CONST 10, CONST 0, DIV` → `RuntimeError::DivisionByZero`
Test: Type mismatch (I64 + F64) — VM behavior undefined here (verifier's job), but should not panic.

**Group C: Pattern matching**
10. MATCH/CASE/EXHAUST with BOOL
11. MATCH/CASE/EXHAUST with VARIANT
12. MATCH/CASE/EXHAUST with MAYBE and RESULT

Test: Match on BOOL true → executes correct branch, returns correct value.
Test: Match on VARIANT with 3 tags → correct dispatch.
Test: Payload extraction in CASE body.

**Group D: Functions**
13. FUNC/ENDFUNC — function definition (skip during execution, record location)
14. CALL — push call frame, jump to function body
15. RET — pop call frame, return to caller
16. RECURSE — recursive call with depth tracking
17. PRE/POST — contract checking at call/return time

Test: Define add function, call it, get correct result.
Test: Recursive factorial with depth limit 20, compute fact(10).
Test: RECURSE exceeding depth limit → RuntimeError.
Test: PRE condition false → RuntimeError::PreconditionFailed.

**Group E: Data structures**
18. VARIANT_NEW — construct variants
19. TUPLE_NEW/PROJECT — construct and destructure tuples
20. ARRAY_NEW/ARRAY_GET/ARRAY_LEN — array operations

Test: Construct a tuple of (I64(1), Bool(true)), project field 0 → I64(1).
Test: Array out of bounds → RuntimeError.

**Group F: Verification & Meta**
21. HASH — skip during execution (verification only, VM treats as NOP)
22. ASSERT — pop bool, halt if false
23. TYPEOF — type check

### Acceptance tests (ALL must pass)

- [ ] Empty program (just HALT) → RuntimeError::HaltWithEmptyStack
- [ ] `CONST I64 0 5, HALT` → Ok(Value::I64(5))
- [ ] Every arithmetic opcode tested with valid inputs
- [ ] Division by zero produces RuntimeError, not panic
- [ ] Pattern match dispatches to correct branch
- [ ] All CASE bodies tested
- [ ] Function call and return preserves stack correctly
- [ ] Recursive factorial(10) = 3628800
- [ ] Recursion depth exceeded → RuntimeError
- [ ] Precondition failure → RuntimeError
- [ ] Postcondition failure → RuntimeError
- [ ] Array out of bounds → RuntimeError
- [ ] **The VM never panics on any input.** Fuzz with 1000 random programs.

### Phase 2 gate
Run `cargo test -p nolang-vm`. All tests pass. Run the fuzzer for 60 seconds with no panics.

---

## Phase 3: `verifier` crate

**Goal:** Statically analyze instruction streams and reject all invalid programs.

### What to build

```
crates/verifier/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── structural.rs   — Block matching, ordering, unused fields
    ├── types.rs         — Type checking and inference
    ├── exhaustion.rs    — Pattern match completeness
    ├── hashing.rs       — Hash verification (blake3)
    ├── contracts.rs     — PRE/POST validation
    ├── reachability.rs  — Dead code detection
    ├── stack.rs         — Stack balance analysis
    └── error.rs         — VerifyError
```

### Implementation order

**Group A: Structural**
1. Program ends with HALT
2. FUNC/ENDFUNC matching
3. MATCH/EXHAUST matching
4. No nested FUNC
5. CASE ordering (ascending tags)
6. Unused fields are zero

**Group B: Types**
7. REF resolves to valid binding depth
8. Arithmetic type matching
9. MATCH subject is matchable
10. CASE bodies produce same type

**Group C: Exhaustion**
11. Correct number of CASE branches
12. No duplicate tags
13. No missing tags

**Group D: Hashing**
14. HASH instruction present in every FUNC
15. Recompute blake3 and compare

**Group E: Contracts**
16. PRE blocks produce BOOL
17. POST blocks produce BOOL

**Group F: Reachability**
18. Build control flow graph
19. Mark unreachable instructions

**Group G: Stack balance**
20. Compute stack depth at each instruction
21. Verify no underflow
22. Verify exactly 1 value at HALT

### Acceptance tests (ALL must pass)

- [ ] Valid programs pass verification
- [ ] Missing HALT → VerifyError::MissingHalt
- [ ] Unmatched FUNC → VerifyError::UnmatchedFunc
- [ ] CASE out of order → VerifyError::CaseOrderViolation
- [ ] REF beyond binding depth → VerifyError::UnresolvableRef
- [ ] I64 + F64 arithmetic → VerifyError::TypeMismatch
- [ ] Non-exhaustive MATCH → VerifyError::NonExhaustiveMatch
- [ ] Wrong hash → VerifyError::HashMismatch
- [ ] Missing hash → VerifyError::MissingHash
- [ ] Unreachable code → VerifyError::UnreachableInstruction
- [ ] Stack underflow → VerifyError::StackUnderflow
- [ ] Multiple values at HALT → VerifyError::UnbalancedStack
- [ ] **All errors are collected, not just the first.** Verify with programs having 3+ errors.
- [ ] **The verifier never panics.** Fuzz with 10,000 random programs.

### Phase 3 gate
Run `cargo test -p nolang-verifier`. All tests pass. Run the fuzzer for 60 seconds with no panics.

---

## Phase 4: `assembler` crate

**Goal:** Bidirectional translation between text assembly and binary.

### What to build

```
crates/assembler/
├── Cargo.toml
└── src/
    ├── lib.rs
    ├── lexer.rs        — Tokenize assembly text
    ├── parser.rs       — Tokens → Instructions
    ├── assembler.rs    — Text → Program
    ├── disassembler.rs — Program → Text
    └── error.rs        — AsmError
```

### Implementation details

The assembly format is defined in ARCHITECTURE.md. Key points:
- One instruction per line
- Opcodes uppercase
- Numeric values: decimal by default, hex with 0x prefix for raw bit patterns
- Comments with `;`
- Whitespace indentation ignored

### Acceptance tests (ALL must pass)

- [ ] Assemble every example from EXAMPLES.md
- [ ] Disassemble → reassemble roundtrip produces identical binary
- [ ] Assemble → disassemble → reassemble roundtrip produces identical binary
- [ ] Unknown opcode → AsmError::UnknownOpcode with line number
- [ ] Missing argument → AsmError::MissingArgument with line number
- [ ] Invalid number → AsmError::InvalidNumber with line number
- [ ] Comments are stripped, blank lines are handled
- [ ] Assembled programs pass verification

### Phase 4 gate
Run `cargo test -p nolang-assembler`. All tests pass. All example programs assemble, verify, and execute correctly.

---

## Phase 5: Integration & Training Pipeline

**Goal:** End-to-end pipeline from assembly to execution, plus training pair generation.

### What to build

```
nolang-cli/          (binary crate, separate from library crates)
├── Cargo.toml
└── src/
    └── main.rs      — CLI: assemble, verify, run, disassemble, hash
```

**CLI commands:**
- `nolang assemble input.nol -o output.nolb` — assemble text to binary
- `nolang verify input.nolb` — run verifier, print result
- `nolang run input.nolb` — verify then execute, print result
- `nolang disassemble input.nolb` — binary to text
- `nolang hash input.nol` — compute hashes for all FUNC blocks (helper for writing assembly)

### Training pair workflow

1. Write assembly programs in `tests/programs/`
2. Use CLI to assemble + verify
3. Write intent descriptions
4. Package as `.nolt` (JSON lines)
5. Accumulate corpus in `tests/corpus/`

### Acceptance tests

- [ ] CLI assembles, verifies, and runs all example programs
- [ ] `nolang run` on a valid program prints the result value
- [ ] `nolang verify` on an invalid program prints all errors
- [ ] `nolang hash` computes correct hashes (test against manually computed)
- [ ] Full pipeline: write .nol → assemble → verify → run → get expected output

### Phase 5 gate
All example programs from EXAMPLES.md execute correctly through the CLI.

---

## Extended Phases: Semantic Verification

Phases 1-5 build the foundation: encode, execute, verify, assemble, integrate. The system can take assembly text, prove it's mechanically valid, and run it.

Phases 6-8 close the **semantic gap**: the distance between what a human means and what a program does. This is achieved through layered verification — contracts, witnesses, and reflective description — that progressively formalize intent until the only remaining judgment is a human comparing two English sentences.

See `SEMANTIC_VERIFICATION.md` for the full architectural rationale behind the layered verification approach.

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
