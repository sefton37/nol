# NoLang Architecture

## Component Overview

```
                    ┌─────────────────────┐
                    │   Natural Language   │
                    │      (Intent)        │
                    └──────────┬──────────┘
                               │
                    ┌──────────▼──────────┐
                    │   LLM Generator     │  (Future: Phase 5)
                    │  Intent → Binary IR │
                    └──────────┬──────────┘
                               │
              ┌────────────────▼────────────────┐
              │         Binary IR Stream         │
              │   (fixed-width 64-bit instrs)    │
              └──┬─────────────┬────────────┬───┘
                 │             │            │
        ┌────────▼──┐  ┌──────▼─────┐  ┌───▼──────────┐
        │ Assembler  │  │  Verifier  │  │     VM       │
        │ text ↔ bin │  │  static    │  │  execution   │
        └────────────┘  │  analysis  │  └──────────────┘
                        └────────────┘
```

**Data flow for execution:**
1. Binary IR enters the system (from LLM, from assembler, or from file)
2. Verifier checks the entire stream — type safety, exhaustion, hashes, contracts, canonical form
3. If VALID: VM executes the stream
4. If INVALID: error report with instruction indices

**Data flow for development/debugging:**
1. Human writes assembly text (.nol files)
2. Assembler converts text → binary
3. Verifier checks binary
4. VM executes binary
5. Disassembler converts binary → text for inspection

## Crate Dependencies

```
common ← verifier
common ← vm
common ← assembler
```

That's it. Three crates depend on `common`. No crate depends on another non-common crate. This is intentional — the components are independent and communicate only through the binary IR format defined in `common`.

## Crate Responsibilities

### `common` (the shared foundation)

**Defines:**
- `Opcode` enum — all opcodes from SPEC.md Section 4
- `TypeTag` enum — all type tags from SPEC.md Section 3
- `Instruction` struct — the 64-bit instruction representation
- `encode(Instruction) → [u8; 8]` — instruction to bytes
- `decode([u8; 8]) → Result<Instruction, DecodeError>` — bytes to instruction
- `Program` struct — a vector of instructions with metadata
- `Value` enum — runtime value representation (for VM use)
- `NolangError` trait — common error interface

**Does NOT contain:**
- Execution logic (that's `vm`)
- Validation logic (that's `verifier`)
- Parsing logic (that's `assembler`)

**Key property:** `decode(encode(instr)) == Ok(instr)` for all valid instructions. This is a property-based test target.

### `vm` (the execution engine)

**Inputs:** A `Program` (vector of `Instruction`). Assumes the program has been verified.

**Outputs:** A `Value` (the top of stack at HALT) or a `RuntimeError`.

**Internal state:**
- `stack: Vec<Value>` — the operand stack
- `bindings: Vec<Value>` — the de Bruijn binding environment
- `call_stack: Vec<CallFrame>` — return addresses and saved binding depths
- `pc: usize` — program counter (instruction index)

**RuntimeErrors (these are NOT verification errors — they can only happen at runtime):**
- Division by zero
- Recursion depth exceeded
- Array index out of bounds
- Precondition failed
- Postcondition failed
- Stack overflow (exceeds 4096)

**Key property:** The VM never panics. Any input (even unverified) produces either a `Value` or a `RuntimeError`. However, unverified input may produce *incorrect* results — the VM trusts the verifier.

### `verifier` (the static analyzer)

**Inputs:** A `Program` (vector of `Instruction`).

**Outputs:** `Ok(())` or `Err(Vec<VerifyError>)` — all errors found, not just the first.

**Checks performed (in this order):**

1. **Structural validity**
   - Program ends with HALT
   - All FUNC blocks have matching ENDFUNC
   - All MATCH blocks have matching EXHAUST
   - No nested FUNC blocks
   - CASE branches in ascending tag order
   - Unused arg fields are zero
   - PARAM count equals FUNC param_count
   - PARAMs appear before PRE/POST in function body

2. **Type safety**
   - Every REF resolves to a valid binding
   - Arithmetic operands have matching numeric types
   - MATCH subjects are matchable types
   - All CASE bodies produce the same type
   - Function arguments match parameter types

3. **Exhaustion**
   - Every MATCH has exactly the right number of CASE branches
   - No duplicate CASE tags
   - No missing CASE tags

4. **Hash integrity**
   - Every FUNC block has a HASH instruction
   - Recomputed hash matches stored hash

5. **Contract validity**
   - PRE/POST blocks produce BOOL
   - PRE blocks only reference function parameters
   - POST blocks may reference the return value (at index 0)

6. **Reachability**
   - No dead code (every instruction reachable from entry or from a function entry)

7. **Stack balance**
   - Stack depth computable at every instruction
   - No underflow possible
   - Stack has exactly 1 value at HALT

8. **Limits**
   - Program size ≤ 65,536 instructions
   - No REF to index > 4,096
   - No RECURSE with depth > 1,024

**Key property:** If the verifier returns `Ok(())`, the VM will never encounter a stack underflow, type mismatch, or structural error. Runtime errors (div by zero, precondition failure, recursion limit) are still possible.

### `assembler` (the text ↔ binary translator)

**Two functions:**
- `assemble(text: &str) → Result<Program, AsmError>` — text to binary
- `disassemble(program: &Program) → String` — binary to text

**Assembly text format:**

```nol
; Comments start with semicolons
; Function: add two I64 values
FUNC 2 8                        ; 2 params, 8 instructions in body
  PRE 3                         ; precondition: 3 instructions
    REF 0
    TYPEOF I64
    ASSERT
  POST 3                        ; postcondition: 3 instructions
    REF 0                       ; return value
    TYPEOF I64
    ASSERT
  REF 1                         ; second param (deeper binding)
  REF 0                         ; first param (most recent)
  ADD
  RET
  HASH 0xA3F2 0x1B4C 0x7D9E    ; 48-bit truncated blake3
ENDFUNC

; Entry point
CONST I64 0x0000 0x0005         ; push 5
CONST I64 0x0000 0x0003         ; push 3
REF 0                           ; the add function
CALL 0                          ; call it
HALT
```

**Text format rules:**
- One instruction per line
- Opcode is uppercase
- Type tags are uppercase
- Numeric arguments are hexadecimal with 0x prefix for arg fields, decimal for semantic values (like param counts)
- Wait — to keep it unambiguous: ALL numeric values in assembly are **decimal** unless they represent raw bit patterns (HASH values, CONST values), which are **hexadecimal with 0x prefix**
- Comments from `;` to end of line
- Blank lines allowed
- Indentation is whitespace (ignored, but convention is 2 spaces inside blocks)

**Key property:** `disassemble(assemble(text)) ≈ text` — they produce semantically identical output (whitespace/comment differences are acceptable). `assemble(disassemble(program)) == program` — this direction is exact.

## Error Types

```rust
// common
pub enum DecodeError {
    InvalidOpcode(u8),
    ReservedOpcode(u8),
    InvalidTypeTag(u8),
    NonZeroUnusedField { opcode: Opcode, field: &'static str },
}

// vm
pub enum RuntimeError {
    DivisionByZero { at: usize },
    RecursionDepthExceeded { at: usize, limit: u16 },
    ArrayIndexOutOfBounds { at: usize, index: u64, length: u64 },
    PreconditionFailed { at: usize },
    PostconditionFailed { at: usize },
    StackOverflow { at: usize },
    HaltWithEmptyStack,
    HaltWithMultipleValues { count: usize },
}

// verifier
pub enum VerifyError {
    // Structural
    MissingHalt,
    UnmatchedFunc { at: usize },
    UnmatchedMatch { at: usize },
    NestedFunc { at: usize },
    CaseOrderViolation { at: usize, expected_tag: u16, found_tag: u16 },
    NonZeroUnusedField { at: usize },

    // Type safety
    TypeMismatch { at: usize, expected: TypeTag, found: TypeTag },
    UnresolvableRef { at: usize, index: u16, binding_depth: u16 },

    // Exhaustion
    NonExhaustiveMatch { at: usize, expected: u16, found: u16 },
    DuplicateCase { at: usize, tag: u16 },

    // Hash
    HashMismatch { at: usize, expected: [u8; 6], computed: [u8; 6] },
    MissingHash { func_at: usize },

    // Contracts
    PreConditionNotBool { at: usize },
    PostConditionNotBool { at: usize },

    // Reachability
    UnreachableInstruction { at: usize },

    // Stack
    StackUnderflow { at: usize },
    UnbalancedStack { at_halt: usize, depth: usize },

    // Limits
    ProgramTooLarge { size: usize },
    RefTooDeep { at: usize, index: u16 },
    RecursionLimitTooHigh { at: usize, limit: u16 },
}

// assembler
pub enum AsmError {
    UnknownOpcode { line: usize, token: String },
    UnknownTypeTag { line: usize, token: String },
    MissingArgument { line: usize, opcode: &'static str, expected: usize },
    InvalidNumber { line: usize, token: String },
    UnexpectedToken { line: usize, token: String },
}
```

## Data Formats

### Binary files (.nolb)
Raw concatenation of 8-byte instructions. File size must be a multiple of 8. No header, no metadata — the instructions ARE the format.

### Assembly files (.nol)
UTF-8 text in the assembly format described above.

### Training pair files (.nolt)
JSON lines format:
```json
{"intent": "Add two integers and return the sum", "assembly": "FUNC 2 8\n  ...", "binary_b64": "AQAA..."}
```
One pair per line. `binary_b64` is base64-encoded binary.
