# Plan: Phase 3 -- Verifier Crate (nolang-verifier)

## Context

Phase 1 (`nolang-common`) and Phase 2 (`nolang-vm`) are complete. The common crate provides `Opcode` (44 variants), `TypeTag` (13 variants), `Instruction`, `Program`, and `Value` with property-based encode/decode roundtrip proofs. The VM crate provides a stack-based execution engine with 141 integration tests passing across 2,338 lines of test code. The VM trusts the verifier -- it assumes valid input and may produce incorrect results or non-specific errors for invalid programs.

Phase 3 builds the static analysis component that stands between binary IR and execution. The verifier receives a `Program` and returns `Ok(())` or `Err(Vec<VerifyError>)`, collecting ALL errors rather than stopping at the first. This is the safety gatekeeper.

**Key invariant from ARCHITECTURE.md:**
> If the verifier returns Ok(()), the VM will never encounter a stack underflow, type mismatch, or structural error. Runtime errors (div by zero, precondition failure, recursion limit) are still possible.

**Existing patterns to follow:**
- Error types use `thiserror` with `#[derive(Debug, Clone, PartialEq, Eq, Error)]` (see `/home/kellogg/dev/nol/crates/vm/src/error.rs`)
- Public API is a single entry-point function re-exported from `lib.rs` (see `/home/kellogg/dev/nol/crates/vm/src/lib.rs` -- `pub fn run()`)
- Integration tests use helper functions for instruction construction (see `/home/kellogg/dev/nol/crates/vm/tests/vm_tests.rs` lines 14-73)
- `pub(crate)` visibility for internal state, `pub` for API types
- Module-level doc comments with `//!` at the top of each file
- Every module has `#[cfg(test)] mod tests` inline

## Approach (Recommended): Multi-Pass Architecture with Shared Context

Run 8 analysis passes in dependency order. Each pass takes `&[Instruction]` plus context from prior passes, and returns `Vec<VerifyError>`. A shared `ProgramContext` struct accumulates metadata discovered by the structural pass (function boundaries, MATCH blocks, consumed instruction indices) for use by later passes.

**Why this approach wins:**
- Clean separation of concerns -- each module handles one category of checks
- Passes can be tested independently with simple programs
- Later passes can skip analysis if structural pass found fatal errors (e.g., unmatched FUNC makes hash checking meaningless)
- Error collection is trivial -- concatenate each pass's errors
- Matches the file structure specified in BUILD_ORDER.md exactly
- Follows the pattern established by the VM's pre-scan approach (cf. `VM::scan_functions()` in `/home/kellogg/dev/nol/crates/vm/src/machine.rs` lines 76-120)

**Architecture diagram:**
```
Program
  |
  v
[limits check] ---------> Vec<VerifyError>
  |
  v
[structural pass] ------> ProgramContext + Vec<VerifyError>
  |                        (func boundaries, match blocks, consumed indices)
  v
[exhaustion pass] -------> Vec<VerifyError>  (uses match blocks)
  |
  v
[hash pass] -------------> Vec<VerifyError>  (uses func boundaries)
  |
  v
[type pass] -------------> Vec<VerifyError> + MatchTypeInfo  (uses everything)
  |
  v
[contract pass] ---------> Vec<VerifyError>  (uses structural + type logic)
  |
  v
[stack pass] ------------> Vec<VerifyError>  (uses structural + MatchTypeInfo)
  |
  v
[reachability pass] -----> Vec<VerifyError>  (uses structural)
  |
  v
Collect all errors --> Ok(()) or Err(all_errors)
```

## Alternatives Considered

### Alternative 1: Single-Pass Architecture (NOT CHOSEN)
**What:** Walk the instruction stream once, checking everything simultaneously.

**Why rejected:**
- Several checks have data dependencies. Type checking needs function boundaries (structural). Exhaustion checking needs MATCH block boundaries (structural). Hash checking needs FUNC boundaries.
- A single pass interleaves all check logic, creating a monolithic function that is difficult to test, maintain, and reason about.
- Error recovery is harder -- when structural errors are found, it is unclear whether to continue type-checking through a broken block.
- The VM itself uses a two-pass approach (scan_functions + execute), validating that multi-pass is natural for this instruction set.

### Alternative 2: Multi-Pass with Independent Contexts (NOT CHOSEN)
**What:** Each pass builds its own understanding of program structure from scratch.

**Why rejected:**
- Redundant work -- structural, type, stack, and exhaustion passes all need to know function and MATCH block boundaries.
- Inconsistency risk -- if passes disagree about boundaries, they produce contradictory errors.
- The shared `ProgramContext` is small (a few Vec fields), immutable after the structural pass, and eliminates redundancy.

### Alternative 3: Full Type Inference Engine (NOT CHOSEN)
**What:** Build a complete type inference engine that tracks precise types for every stack slot and binding through all control flow paths.

**Why rejected:**
- Overkill for Phase 3. The instruction set requires explicit types at CONST, VARIANT_NEW, TUPLE_NEW, ARRAY_NEW. Most types are knowable locally.
- Complex to implement correctly for MATCH/CASE branches with payloads, function calls, and recursion.
- High risk of spending weeks on type inference when simpler checking catches the errors that matter.

**Chosen compromise:** Track an `AbstractType` enum through the instruction stream. Use `Unknown` when type cannot be determined (e.g., after structural errors, complex control flow). Only emit `TypeMismatch` when both sides are concrete and different. This catches real errors while avoiding false positives from cascading inference failures.

## Implementation Steps

### Step 0: Create crate structure and Cargo.toml

Create the file tree specified in BUILD_ORDER.md:

```
crates/verifier/
  Cargo.toml
  src/
    lib.rs           -- Public API + orchestration
    error.rs         -- VerifyError enum (21 variants)
    structural.rs    -- Block matching, ordering, unused fields
    types.rs         -- Type checking and inference
    exhaustion.rs    -- Pattern match completeness
    hashing.rs       -- Hash verification (blake3)
    contracts.rs     -- PRE/POST validation
    reachability.rs  -- Dead code detection
    stack.rs         -- Stack balance analysis
```

**Cargo.toml** (follow pattern from `/home/kellogg/dev/nol/crates/vm/Cargo.toml`):
```toml
[package]
name = "nolang-verifier"
version.workspace = true
edition.workspace = true
license.workspace = true
description = "Static analysis and verification for NoLang instruction streams"

[dependencies]
nolang-common = { path = "../common" }
thiserror = "1"
blake3 = "1"

[dev-dependencies]
proptest = "1"
```

Add `"crates/verifier"` to the workspace members list in `/home/kellogg/dev/nol/Cargo.toml`.

### Step 1: error.rs -- VerifyError enum (21 variants)

Implement the exact enum from ARCHITECTURE.md. All 21 variants, exactly as specified. Use `thiserror` for Display derives.

```rust
//! Verification errors for NoLang instruction streams.

use nolang_common::TypeTag;
use thiserror::Error;

/// Errors discovered during static verification.
///
/// The verifier collects ALL errors found in a program, not just the first.
/// Each variant includes an instruction index (`at`) for debugging.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VerifyError {
    // Structural
    #[error("program does not end with HALT")]
    MissingHalt,

    #[error("unmatched FUNC at instruction {at}")]
    UnmatchedFunc { at: usize },

    #[error("unmatched MATCH at instruction {at}")]
    UnmatchedMatch { at: usize },

    #[error("nested FUNC at instruction {at}")]
    NestedFunc { at: usize },

    #[error("CASE tag order violation at instruction {at}: expected tag {expected_tag}, found {found_tag}")]
    CaseOrderViolation { at: usize, expected_tag: u16, found_tag: u16 },

    #[error("non-zero unused field at instruction {at}")]
    NonZeroUnusedField { at: usize },

    // Type safety
    #[error("type mismatch at instruction {at}: expected {expected:?}, found {found:?}")]
    TypeMismatch { at: usize, expected: TypeTag, found: TypeTag },

    #[error("unresolvable REF at instruction {at}: index {index} exceeds binding depth {binding_depth}")]
    UnresolvableRef { at: usize, index: u16, binding_depth: u16 },

    // Exhaustion
    #[error("non-exhaustive MATCH at instruction {at}: expected {expected} cases, found {found}")]
    NonExhaustiveMatch { at: usize, expected: u16, found: u16 },

    #[error("duplicate CASE tag {tag} at instruction {at}")]
    DuplicateCase { at: usize, tag: u16 },

    // Hash
    #[error("hash mismatch at instruction {at}: expected {expected:02x?}, computed {computed:02x?}")]
    HashMismatch { at: usize, expected: [u8; 6], computed: [u8; 6] },

    #[error("missing HASH in FUNC block starting at instruction {func_at}")]
    MissingHash { func_at: usize },

    // Contracts
    #[error("PRE condition does not produce BOOL at instruction {at}")]
    PreConditionNotBool { at: usize },

    #[error("POST condition does not produce BOOL at instruction {at}")]
    PostConditionNotBool { at: usize },

    // Reachability
    #[error("unreachable instruction at {at}")]
    UnreachableInstruction { at: usize },

    // Stack
    #[error("stack underflow at instruction {at}")]
    StackUnderflow { at: usize },

    #[error("unbalanced stack at HALT (instruction {at_halt}): depth {depth}, expected 1")]
    UnbalancedStack { at_halt: usize, depth: usize },

    // Limits
    #[error("program too large: {size} instructions (max 65536)")]
    ProgramTooLarge { size: usize },

    #[error("REF index {index} too deep at instruction {at} (max 4096)")]
    RefTooDeep { at: usize, index: u16 },

    #[error("RECURSE depth limit {limit} too high at instruction {at} (max 1024)")]
    RecursionLimitTooHigh { at: usize, limit: u16 },
}
```

Include display tests for each variant (follow VM error.rs pattern at lines 92-111).

### Step 2: lib.rs -- Public API and ProgramContext

**Public API:**
```rust
//! NoLang verifier -- static analysis for instruction streams.
//!
//! The verifier checks a program for structural validity, type safety,
//! exhaustive pattern matching, hash integrity, contract validity,
//! reachability, and stack balance.
//!
//! # Usage
//!
//! ```
//! use nolang_common::Program;
//! use nolang_verifier::verify;
//!
//! let program = Program::new(vec![/* ... */]);
//! match verify(&program) {
//!     Ok(()) => println!("Program is valid"),
//!     Err(errors) => {
//!         for e in &errors {
//!             eprintln!("  {}", e);
//!         }
//!     }
//! }
//! ```

pub mod error;
mod structural;
mod types;
mod exhaustion;
mod hashing;
mod contracts;
mod reachability;
mod stack;

pub use error::VerifyError;

use nolang_common::Program;

/// Verify a program, returning all errors found.
///
/// Returns `Ok(())` if the program is valid, or `Err(Vec<VerifyError>)`
/// containing every error discovered. Errors are collected across all
/// analysis passes, not just the first.
pub fn verify(program: &Program) -> Result<(), Vec<VerifyError>> {
    let instrs = &program.instructions;
    let mut errors = Vec::new();

    // Phase 0: Limits (cheap, run first)
    errors.extend(check_limits(instrs));

    // Phase 1: Structural analysis (produces ProgramContext)
    let (context, structural_errors) = structural::analyze(instrs);
    errors.extend(structural_errors);

    // If structural analysis found fatal errors (unmatched blocks),
    // skip passes that depend on correct block boundaries.
    if !context.has_fatal_structural_errors {
        // Phase 2: Exhaustion
        errors.extend(exhaustion::check(&context, instrs));

        // Phase 3: Hash integrity
        errors.extend(hashing::check(&context, instrs));

        // Phase 4: Type safety (produces MatchTypeInfo for stack pass)
        let (type_errors, match_type_info) = types::check(&context, instrs);
        errors.extend(type_errors);

        // Phase 5: Contract validity
        errors.extend(contracts::check(&context, instrs));

        // Phase 6: Stack balance (uses match type info for payload tracking)
        errors.extend(stack::check(&context, instrs, &match_type_info));

        // Phase 7: Reachability
        errors.extend(reachability::check(&context, instrs));
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
```

**ProgramContext** (defined in structural.rs, re-exported for other modules):

```rust
/// Metadata about a function block discovered during structural analysis.
#[derive(Debug, Clone)]
pub(crate) struct FuncBlock {
    /// Index of the FUNC instruction.
    pub func_at: usize,
    /// Index of the ENDFUNC instruction.
    pub endfunc_at: usize,
    /// Parameter count (FUNC arg1).
    pub param_count: u16,
    /// Body length (FUNC arg2).
    pub body_len: u16,
    /// PRE condition blocks: Vec<(pre_instr_at, condition_len)>.
    pub pre_blocks: Vec<(usize, u16)>,
    /// POST condition blocks: Vec<(post_instr_at, condition_len)>.
    pub post_blocks: Vec<(usize, u16)>,
    /// Index of the HASH instruction (if found).
    pub hash_at: Option<usize>,
    /// Index of the RET instruction (if found).
    pub ret_at: Option<usize>,
}

/// Metadata about a MATCH block discovered during structural analysis.
#[derive(Debug, Clone)]
pub(crate) struct MatchBlock {
    /// Index of the MATCH instruction.
    pub match_at: usize,
    /// Index of the EXHAUST instruction (if found).
    pub exhaust_at: Option<usize>,
    /// Expected variant count (MATCH arg1).
    pub variant_count: u16,
    /// CASE branches: Vec<(case_instr_at, tag, body_len)>.
    pub cases: Vec<(usize, u16, u16)>,
}

/// Shared context built by the structural pass, consumed by all later passes.
#[derive(Debug, Clone)]
pub(crate) struct ProgramContext {
    /// All function blocks, in order of appearance.
    pub functions: Vec<FuncBlock>,
    /// All MATCH blocks, in order of appearance.
    pub matches: Vec<MatchBlock>,
    /// Instruction indices consumed by CONST_EXT (the data instruction after it).
    pub consumed_indices: Vec<usize>,
    /// The entry point: first instruction after the last ENDFUNC, or 0.
    pub entry_point: usize,
    /// Whether the structural pass found errors so severe that later passes
    /// should skip analysis within broken regions.
    pub has_fatal_structural_errors: bool,
}
```

### Step 3: structural.rs -- Group A: Structural Validity

The foundation pass. Produces `ProgramContext` and checks 6 structural rules.

**Checks implemented:**

1. **Program ends with HALT** -- `instrs.last()?.opcode == Opcode::Halt`. Empty program emits MissingHalt.

2. **FUNC/ENDFUNC matching** -- Linear scan with a depth counter. Push FUNC index on encountering Func; pop on EndFunc and create FuncBlock. Leftover Func entries produce `UnmatchedFunc`. Orphan EndFunc also produces `UnmatchedFunc`. Set `has_fatal_structural_errors = true` on any mismatch.

3. **No nested FUNC** -- If Func is encountered while depth > 0, emit `NestedFunc`. Set fatal flag.

4. **MATCH/EXHAUST matching** -- Same pattern. Scanned separately from FUNC (MATCH blocks can appear inside function bodies). Set fatal flag on mismatch.

5. **CASE tag ordering** -- Within each matched MATCH block, verify CASE tags appear in strictly ascending order. Emit `CaseOrderViolation { at, expected_tag, found_tag }`.

6. **Unused field validation** -- For every instruction, check the per-opcode unused field rules. Emit `NonZeroUnusedField { at }`.

**Additionally, structural analysis produces:**
- `consumed_indices` for CONST_EXT data instructions
- `entry_point` calculation (first instruction after last ENDFUNC, or 0)
- `FuncBlock` internal structure (PRE/POST blocks, HASH location, RET location) by scanning within each matched FUNC..ENDFUNC range

**The per-opcode unused field rules (exhaustive, from prompt specification):**

All 30 opcodes with their required-zero fields are encoded as a match statement. This is the most tedious part of structural.rs but is entirely mechanical. The full table is reproduced in the code sketch in the Appendix.

**Algorithm for FUNC body scanning (after FUNC/ENDFUNC match):**

```rust
// For each matched FuncBlock:
// 1. Scan from func_at+1 forward for PRE blocks
// 2. After PREs, scan for POST blocks
// 3. Scan remaining body for RET and HASH locations
// 4. Verify HASH is second-to-last before ENDFUNC
// 5. Verify exactly one RET exists
```

### Step 4: exhaustion.rs -- Group C: Pattern Match Completeness

Uses `ProgramContext::matches` to check each MATCH block. This is one of the simpler passes.

**Checks:**

1. **Correct count** -- `match_block.cases.len() as u16 == match_block.variant_count`. If not, emit `NonExhaustiveMatch { at: match_at, expected: variant_count, found: cases.len() }`.

2. **No duplicates** -- Check that no two CASEs share a tag. Emit `DuplicateCase { at: case_at, tag }`.

3. **No gaps** -- Implied by correct count + no duplicates + ascending order (checked in structural). Verify explicitly: tags should form the sequence 0, 1, ..., variant_count-1.

### Step 5: hashing.rs -- Group D: Hash Integrity

Uses `ProgramContext::functions` to verify each function's HASH instruction.

**Checks:**

1. **HASH presence** -- `func_block.hash_at.is_some()`. If None, emit `MissingHash { func_at }`.

2. **HASH value verification:**
   - Collect all instruction bytes from `func_at` (inclusive) through `hash_at - 1` (inclusive).
   - Encode each instruction to `[u8; 8]`, concatenate into a byte vector.
   - Compute `blake3::hash(&bytes)`.
   - Take first 6 bytes of the 32-byte hash output.
   - Compare against stored hash: `arg1.to_be_bytes()`, `arg2.to_be_bytes()`, `arg3.to_be_bytes()` concatenated.
   - If mismatch, emit `HashMismatch { at: hash_at, expected: stored, computed: actual }`.

**Code sketch for hash extraction:**
```rust
fn extract_stored_hash(instr: &Instruction) -> [u8; 6] {
    let mut hash = [0u8; 6];
    hash[0..2].copy_from_slice(&instr.arg1.to_be_bytes());
    hash[2..4].copy_from_slice(&instr.arg2.to_be_bytes());
    hash[4..6].copy_from_slice(&instr.arg3.to_be_bytes());
    hash
}

fn compute_func_hash(instrs: &[Instruction], func_at: usize, hash_at: usize) -> [u8; 6] {
    let mut bytes = Vec::with_capacity((hash_at - func_at) * 8);
    for i in func_at..hash_at {
        bytes.extend_from_slice(&instrs[i].encode());
    }
    let full_hash = blake3::hash(&bytes);
    let mut truncated = [0u8; 6];
    truncated.copy_from_slice(&full_hash.as_bytes()[..6]);
    truncated
}
```

### Step 6: types.rs -- Group B: Type Safety

The most complex pass. Simulates an abstract stack and binding environment to track types through the instruction stream.

**AbstractType enum:**
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AbstractType {
    I64, U64, F64, Bool, Char, Unit,
    Variant { tag_count: u16 },
    Tuple { fields: Vec<AbstractType> },
    Array { element: Box<AbstractType>, length: u16 },
    Func { param_count: u16 },
    Maybe,
    Result,
    /// Type cannot be determined. No TypeMismatch errors are emitted
    /// when either side is Unknown.
    Unknown,
}
```

**Key method -- `conflicts_with`:**
Returns true only when BOTH types are concrete (not Unknown) and are different. This prevents cascading false positives.

**Type-checks performed:**

| Check | What | Error |
|-------|------|-------|
| REF resolves | `arg1 < binding_depth` | UnresolvableRef |
| Arithmetic types match | Both operands same numeric type | TypeMismatch |
| MOD not F64 | MOD operands are I64 or U64 | TypeMismatch |
| NEG valid type | I64 or F64 only | TypeMismatch |
| Logic valid types | Bool, I64, or U64 | TypeMismatch |
| Shift valid types | I64 or U64 | TypeMismatch |
| MATCH subject matchable | Bool, Variant, Maybe, Result | TypeMismatch |
| CASE bodies same type | All branches produce same type | TypeMismatch |

**Output:** In addition to `Vec<VerifyError>`, produces `MatchTypeInfo` -- a map from match instruction index to the abstract type of the matched value. This tells the stack pass whether each CASE has a payload.

**Strategy for function bodies:** Type-check each function body independently. Parameters are bound as `Unknown` (FUNC does not declare parameter types). Within the body, type checks still catch:
- Arithmetic with mismatched known types (e.g., adding a CONST I64 and a CONST F64)
- REF beyond binding depth
- MATCH on a non-matchable type

**CONST_EXT handling:** Determine type from `type_tag` field, push onto abstract stack, mark next instruction as consumed.

**MATCH/CASE type tracking (code sketch):**
```rust
fn check_match(&mut self, mb: &MatchBlock) {
    // Pop matched value type
    let matched_type = self.pop_abstract_type(mb.match_at);

    // Record for stack pass
    self.match_type_info.insert(mb.match_at, matched_type.clone());

    // Check matchable
    if !matched_type.is_matchable() && matched_type != AbstractType::Unknown {
        self.errors.push(VerifyError::TypeMismatch {
            at: mb.match_at,
            expected: TypeTag::Variant,
            found: matched_type.to_type_tag(),
        });
    }

    let depth_after_pop = self.stack.len();
    let mut result_types: Vec<AbstractType> = Vec::new();

    for &(case_at, tag, body_len) in &mb.cases {
        // Save stack, reset to match-entry depth
        let saved = self.stack.clone();
        self.stack.truncate(depth_after_pop);

        // Push payload if applicable
        if matched_type.has_payload_for_tag(tag) {
            self.stack.push(AbstractType::Unknown); // payload type unknown
        }

        // Type-check body
        let body_start = case_at + 1;
        for i in body_start..(body_start + body_len as usize) {
            if !self.is_consumed(i) { self.check_instruction(i); }
        }

        // Record result type
        result_types.push(self.stack.last().cloned().unwrap_or(AbstractType::Unknown));

        // Restore stack
        self.stack = saved;
    }

    // Verify all branches produce same type
    let concrete: Vec<_> = result_types.iter()
        .filter(|t| **t != AbstractType::Unknown)
        .collect();
    if concrete.len() >= 2 {
        for i in 1..concrete.len() {
            if concrete[i].conflicts_with(concrete[0]) {
                self.errors.push(VerifyError::TypeMismatch {
                    at: mb.cases[i].0,
                    expected: concrete[0].to_type_tag(),
                    found: concrete[i].to_type_tag(),
                });
            }
        }
    }

    // Push match result type onto stack
    let result = concrete.first().map(|t| (*t).clone())
        .unwrap_or(AbstractType::Unknown);
    self.stack.push(result);
}
```

### Step 7: contracts.rs -- Group E: Contract Validity

Uses `ProgramContext::functions` to validate PRE/POST blocks.

**Checks:**

1. **PRE produces Bool** -- Run a mini type simulation on each PRE block. The type-checking logic from `types.rs` is reused (extract the core type-simulation into a shared helper, or call it directly). If the result type is known and not Bool, emit `PreConditionNotBool { at }`.

2. **POST produces Bool** -- Same for POST blocks. Emit `PostConditionNotBool { at }`.

3. **PRE only references parameters** -- In a PRE block, any REF with `index >= param_count` references something outside function parameters. This is a verification error. (Note: the VerifyError enum does not have a specific variant for this. Emit `UnresolvableRef` with `binding_depth = param_count`.)

4. **POST references return value at index 0** -- POST bindings are: [return_value, param_0, param_1, ...]. REF 0 is the return value. REF 1..param_count are parameters. REF beyond param_count + 1 is invalid.

**Implementation approach:** Create a mini `TypeChecker` with a constrained binding environment:
- For PRE: `bindings = [Unknown; param_count]`
- For POST: `bindings = [Unknown; param_count + 1]` (return value at index 0)

### Step 8: stack.rs -- Group G: Stack Balance

Compute stack depth at every instruction. Verify no underflow and exactly 1 value at HALT.

**Stack delta table (pops, pushes):**

| Opcode | Pops | Pushes | Notes |
|--------|------|--------|-------|
| CONST | 0 | 1 | |
| CONST_EXT | 0 | 1 | Consumes next instruction |
| BIND | 1 | 0 | Pops from stack to bindings |
| REF | 0 | 1 | |
| DROP | 0 | 0 | Bindings only |
| ADD..MOD | 2 | 1 | Binary arithmetic |
| NEG | 1 | 1 | Unary |
| EQ..GTE | 2 | 1 | Comparison |
| AND, OR, XOR | 2 | 1 | Binary logic |
| NOT | 1 | 1 | Unary logic |
| SHL, SHR | 2 | 1 | Shift |
| MATCH | 1 | 0 | Pops matched value |
| CASE | 0 or 0 | 0 or 1 | Payload push handled per-case |
| EXHAUST | 0 | 0 | |
| VARIANT_NEW | 1 | 1 | Pop payload, push variant |
| TUPLE_NEW | arg1 | 1 | Pop N, push tuple |
| PROJECT | 1 | 1 | Pop tuple, push field |
| ARRAY_NEW | arg1 | 1 | Pop N, push array |
| ARRAY_GET | 2 | 1 | Pop index + array, push element |
| ARRAY_LEN | 1 | 1 | Pop array, push U64 |
| ASSERT | 1 | 0 | Pop Bool |
| TYPEOF | 1 | 2 | Pop value, push value + Bool |
| HASH | 0 | 0 | |
| NOP | 0 | 0 | |
| HALT | 0 | 0 | Depth check handled separately |
| CALL | N | 1 | Pops param_count args, pushes return value |
| RECURSE | N | 1 | Same as CALL |
| RET | 1 | 0 | Within function scope |
| FUNC | 0 | 0 | Skipped (structural context) |
| ENDFUNC | 0 | 0 | |
| PRE, POST | 0 | 0 | Skipped (structural context) |

**Algorithm for entry-point code:**
1. Start at `context.entry_point` with depth 0.
2. Walk forward, applying deltas.
3. At FUNC: skip to `endfunc_at + 1` (function definitions are skipped during entry-point execution). But note: per SPEC Section 5, functions appear BEFORE the entry point, so the entry-point walker should never encounter FUNC.
4. At CALL: `depth -= func.param_count; depth += 1`.
5. At MATCH: save depth, `depth -= 1`. Walk each CASE body independently (with payload adjustment using `MatchTypeInfo`). After all cases, `depth = saved_depth` (matched value consumed, result produced, net zero change to depth).
6. If `depth < 0` at any point, emit `StackUnderflow { at }`.
7. At HALT: if `depth != 1`, emit `UnbalancedStack { at_halt, depth }`.

**Algorithm for function bodies:**
1. Start at body_start_pc with depth 0 (parameters are in bindings, not stack).
2. Walk body instructions, applying deltas.
3. At RET: depth should be 1 (the return value).
4. Skip PRE/POST blocks (they are checked by contracts pass and have their own stack scope).

**MATCH payload handling (uses MatchTypeInfo from type pass):**

The `MatchTypeInfo` map tells us the abstract type of the matched value. From this:
- `Bool` -- no CASE has a payload
- `Variant { tag_count }` -- all CASEs have a payload (VARIANT_NEW always wraps a payload)
- `Maybe` -- CASE 0 (SOME) has payload, CASE 1 (NONE) does not
- `Result` -- CASE 0 (OK) has payload, CASE 1 (ERR) has payload
- `Unknown` -- conservatively assume no payload (check body-internal consistency only)

### Step 9: reachability.rs -- Group F: Dead Code Detection

Mark all reachable instructions. Anything unmarked (and not a CONST_EXT data instruction) is unreachable.

**Simplified range-marking approach** (avoids full CFG construction):

1. Mark the entry-point range as reachable:
   - Walk from `context.entry_point` forward
   - At MATCH: mark all CASE bodies within the MATCH block as reachable (exhaustive matching means all branches are taken)
   - At HALT: stop
   - Skip FUNC..ENDFUNC ranges (they are entered via CALL, not linear execution)

2. Mark each function's contents as reachable:
   - FUNC instruction itself
   - All PRE block instructions
   - All POST block instructions
   - Function body from after PRE/POST to RET (inclusive)
   - HASH instruction
   - ENDFUNC instruction

3. Mark CONST_EXT consumed indices as "consumed" (not unreachable, not reachable -- they are data).

4. Any instruction index not marked reachable and not consumed emits `UnreachableInstruction { at }`.

**Edge cases:**
- Instructions after HALT are unreachable (unless inside a function)
- NOP instructions that are reachable should NOT be flagged
- Instructions between function definitions are unreachable unless they are part of the entry point
- Nested MATCH blocks: MATCH inside a CASE body. The outer CASE body is reachable, so the inner MATCH's CASEs are also reachable.

### Step 10: Integration tests

Write `crates/verifier/tests/verifier_tests.rs` following the VM test pattern.

**Helper functions** (reuse the same pattern from `/home/kellogg/dev/nol/crates/vm/tests/vm_tests.rs` lines 14-73):
```rust
fn instr(op: Opcode, tt: TypeTag, a1: u16, a2: u16, a3: u16) -> Instruction
fn halt() -> Instruction
fn const_i64(val: i32) -> Instruction
fn const_bool(val: bool) -> Instruction
fn nop() -> Instruction
fn bind() -> Instruction
fn ref_idx(index: u16) -> Instruction
```

**Test categories:**

1. **Acceptance criteria** (from BUILD_ORDER.md -- 12 required tests)
2. **Multi-error collection** (programs with 3+ deliberate errors)
3. **Positive tests** (examples from EXAMPLES.md pass after hash computation)
4. **Per-variant negative tests** (21 tests, one per VerifyError variant)
5. **Property-based fuzzing** (proptest: 10,000+ random programs, zero panics)

## Files Affected

### New files to create:
| File | Est. Lines | Purpose |
|------|-----------|---------|
| `/home/kellogg/dev/nol/crates/verifier/Cargo.toml` | 15 | Crate config |
| `/home/kellogg/dev/nol/crates/verifier/src/lib.rs` | 120 | Public API + orchestration |
| `/home/kellogg/dev/nol/crates/verifier/src/error.rs` | 120 | VerifyError enum |
| `/home/kellogg/dev/nol/crates/verifier/src/structural.rs` | 400 | Block matching, unused fields |
| `/home/kellogg/dev/nol/crates/verifier/src/types.rs` | 450 | Type checking + AbstractType |
| `/home/kellogg/dev/nol/crates/verifier/src/exhaustion.rs` | 100 | Match completeness |
| `/home/kellogg/dev/nol/crates/verifier/src/hashing.rs` | 120 | blake3 verification |
| `/home/kellogg/dev/nol/crates/verifier/src/contracts.rs` | 150 | PRE/POST validation |
| `/home/kellogg/dev/nol/crates/verifier/src/reachability.rs` | 200 | Dead code detection |
| `/home/kellogg/dev/nol/crates/verifier/src/stack.rs` | 300 | Stack balance analysis |
| `/home/kellogg/dev/nol/crates/verifier/tests/verifier_tests.rs` | 1500+ | Integration tests |

**Total estimated production code: ~1,975 lines** (under the 2,000-line limit from BUILD_ORDER.md rule 4).

### Files to modify:
| File | Change |
|------|--------|
| `/home/kellogg/dev/nol/Cargo.toml` | Add `"crates/verifier"` to workspace members |

## Implementation Order

Build and test each step fully before moving to the next. Within each step, write tests first (TDD).

| Order | File | Check Group | Depends On | Est. Hours |
|-------|------|------------|------------|-----------|
| 1 | error.rs | -- | nothing | 1 |
| 2 | structural.rs | A (structural) | error.rs | 4 |
| 3 | exhaustion.rs | C (exhaustion) | structural.rs | 1 |
| 4 | hashing.rs | D (hash) | structural.rs | 2 |
| 5 | types.rs | B (types) | structural.rs | 6 |
| 6 | contracts.rs | E (contracts) | structural.rs, types.rs | 2 |
| 7 | stack.rs | G (stack) | structural.rs, types.rs | 4 |
| 8 | reachability.rs | F (reachability) | structural.rs | 3 |
| 9 | lib.rs | orchestration | all modules | 1 |
| 10 | verifier_tests.rs | integration | all modules | 4 |

**Total: ~28 hours**

**Why this order:**
1. `error.rs` first because every module imports it.
2. `structural.rs` second because it produces `ProgramContext` consumed by all other passes.
3. `exhaustion.rs` and `hashing.rs` early because they are self-contained and build confidence.
4. `types.rs` is the most complex -- build after simpler passes are proven.
5. `contracts.rs` reuses type-checking logic.
6. `stack.rs` uses `MatchTypeInfo` from `types.rs`.
7. `reachability.rs` is independent but ordered late -- less critical for correctness.
8. `lib.rs` assembles parts.
9. Integration tests verify end-to-end.

## Risks and Mitigations

### Risk 1: Type inference complexity explodes
**Likelihood:** Medium | **Impact:** High (schedule slip, incorrect errors)

**What could go wrong:** Tracking types through MATCH/CASE branches, function calls, and recursion creates combinatorial complexity. Edge cases in payload type tracking and function return type inference produce subtle bugs.

**Mitigation:** Use `AbstractType::Unknown` aggressively. Only emit `TypeMismatch` when BOTH sides are concrete and different. Test type inference incrementally: scalars first, then variants, then functions. The `conflicts_with` method is the safety valve -- it returns false whenever either type is Unknown.

**Validation:** Write 20+ type-error test cases before implementing. If any case is ambiguous, default to Unknown.

### Risk 2: Stack balance incorrect for MATCH/CASE with payloads
**Likelihood:** Medium | **Impact:** High (false positives or missed underflows)

**What could go wrong:** Whether a CASE body starts with a payload on the stack depends on the matched value's type, which is not encoded in the MATCH instruction. Wrong payload assumptions cause phantom underflow reports or miss real ones.

**Mitigation:** Run type pass before stack pass. The type pass records `MatchTypeInfo` (matched-value type per MATCH). The stack pass uses this. If matched type is Unknown, skip detailed CASE depth analysis and only verify overall MATCH balance.

**Validation:** Test with Example 3 (Bool match, no payloads), Example 5 (Maybe match, SOME has payload, NONE does not), and a Variant match where every tag has a payload.

### Risk 3: blake3 hash computation mismatch
**Likelihood:** Low | **Impact:** High (all programs with functions fail verification)

**What could go wrong:** Off-by-one in which instructions are included. Big-endian vs little-endian confusion in hash extraction/storage. Including the wrong bytes.

**Mitigation:** SPEC.md Section 6 is precise: "from FUNC (inclusive) through the instruction before HASH (inclusive)." Encode each instruction to `[u8; 8]`, concatenate. Use blake3 standard API. Extract first 6 bytes. Compare using big-endian storage per SPEC.

**Validation:** Write a test that constructs a known function, manually computes blake3 of the raw bytes, and verifies the verifier accepts the correct hash. Then corrupt one byte and verify `HashMismatch`.

### Risk 4: Reachability false positives
**Likelihood:** Medium | **Impact:** Medium (valid programs rejected)

**What could go wrong:** Instructions inside CASE bodies, PRE/POST blocks, or function bodies not marked reachable because the walker does not follow all control flow paths. CONST_EXT data instructions falsely flagged as unreachable.

**Mitigation:** Use the simplified range-marking approach. Mark entire CASE bodies as reachable when the MATCH is reachable (exhaustive matching means all branches are live). Explicitly exclude `consumed_indices` from unreachability checks.

**Validation:** All 9 examples from EXAMPLES.md must pass without `UnreachableInstruction`. Then add dead code after HALT and verify it is caught.

### Risk 5: The verifier panics on malformed input
**Likelihood:** Medium | **Impact:** High (violates Phase 3 gate requirement)

**What could go wrong:** Index-out-of-bounds when arg values point beyond program boundaries. Infinite loops from circular body_len values. Arithmetic overflow from large u16 values.

**Mitigation:** Every array access uses `.get()` with bounds checking. Never trust instruction arg values -- they could be any u16. Use `usize::checked_add` and `usize::saturating_add` when computing indices from args. Use iterative algorithms only (no recursion in the verifier itself).

**Validation:** Fuzz with 10,000 random programs (proptest). Zero panics required. Also test specifically: empty program, all-NOP program, FUNC without ENDFUNC, body_len = u16::MAX, REF with index = u16::MAX.

### Risk 6: Unused field validation table has errors
**Likelihood:** Medium | **Impact:** Low (wrong acceptance/rejection of edge cases)

**What could go wrong:** A typo in the per-opcode table causes the verifier to accept invalid programs or reject valid ones.

**Mitigation:** Cross-reference every rule against SPEC.md Section 4. Write two tests per opcode: (1) valid instruction with all unused fields zero passes, (2) instruction with each unused field set to a non-zero value fails. That is 44 opcodes x 2 = 88 tests just for this sub-check.

## Appendix: Unused Field Rules (Complete Table)

For reference during implementation. Each row shows which fields MUST have specific values.

| Opcode | type_tag | arg1 | arg2 | arg3 |
|--------|----------|------|------|------|
| BIND | None | 0 | 0 | 0 |
| REF | None | *used* | 0 | 0 |
| DROP | None | 0 | 0 | 0 |
| CONST | *used* | *used* | *used* | 0 |
| CONST_EXT | *used* | *used* | 0 | 0 |
| ADD..NEG | None | 0 | 0 | 0 |
| EQ..GTE | None | 0 | 0 | 0 |
| AND..SHR | None | 0 | 0 | 0 |
| MATCH | None | *used* | 0 | 0 |
| CASE | None | *used* | *used* | 0 |
| EXHAUST | None | 0 | 0 | 0 |
| FUNC | None | *used* | *used* | 0 |
| PRE | None | *used* | 0 | 0 |
| POST | None | *used* | 0 | 0 |
| RET | None | 0 | 0 | 0 |
| CALL | None | *used* | 0 | 0 |
| RECURSE | None | *used* | 0 | 0 |
| ENDFUNC | None | 0 | 0 | 0 |
| VARIANT_NEW | Variant | *used* | *used* | 0 |
| TUPLE_NEW | Tuple | *used* | 0 | 0 |
| PROJECT | None | *used* | 0 | 0 |
| ARRAY_NEW | Array | *used* | 0 | 0 |
| ARRAY_GET | None | 0 | 0 | 0 |
| ARRAY_LEN | None | 0 | 0 | 0 |
| HASH | None | *used* | *used* | *used* |
| ASSERT | None | 0 | 0 | 0 |
| TYPEOF | None | *used* | 0 | 0 |
| HALT | None | 0 | 0 | 0 |
| NOP | None | 0 | 0 | 0 |

## Testing Strategy

### Per-module unit tests

Each source file has a `#[cfg(test)] mod tests` block with:
- At least 3 tests per major check (valid case, edge case, error case)
- Per-opcode unused field tests (88 tests in structural.rs)
- Per-opcode stack delta tests (in stack.rs)

### Integration tests (`verifier_tests.rs`)

**Acceptance criteria tests (required by BUILD_ORDER.md):**
```
valid_program_passes_verification()
missing_halt_error()
unmatched_func_error()
case_order_violation_error()
ref_beyond_binding_depth_error()
mixed_type_arithmetic_error()
non_exhaustive_match_error()
wrong_hash_error()
missing_hash_error()
unreachable_code_error()
stack_underflow_error()
unbalanced_stack_at_halt_error()
multiple_errors_collected()       // 3+ errors in one program
```

**Positive tests (examples from EXAMPLES.md):**
```
example_1_constant_return_passes()
example_2_addition_passes()
example_3_boolean_match_passes()
example_7_tuple_passes()
example_8_array_passes()
// Examples 4, 5, 6, 9 need correct HASH values computed
```

**Property-based fuzzing:**
```rust
proptest! {
    #[test]
    fn verifier_never_panics(instrs in vec(arb_instruction(), 0..200)) {
        let program = Program::new(instrs);
        let _ = verify(&program);
        // If we get here without panic, test passes
    }
}
```

Run for 60 seconds with 10,000+ cases.

## Definition of Done

- [ ] All 8 check groups implemented (limits, structural, types, exhaustion, hash, contracts, reachability, stack)
- [ ] All 21 VerifyError variants can be triggered and are correctly formatted
- [ ] Valid programs from EXAMPLES.md pass verification (after HASH computation)
- [ ] Missing HALT produces `MissingHalt`
- [ ] Unmatched FUNC produces `UnmatchedFunc`
- [ ] CASE out of order produces `CaseOrderViolation`
- [ ] REF beyond depth produces `UnresolvableRef`
- [ ] I64 + F64 arithmetic produces `TypeMismatch`
- [ ] Non-exhaustive MATCH produces `NonExhaustiveMatch`
- [ ] Wrong hash produces `HashMismatch`
- [ ] Missing hash produces `MissingHash`
- [ ] Unreachable code produces `UnreachableInstruction`
- [ ] Stack underflow produces `StackUnderflow`
- [ ] Multiple values at HALT produces `UnbalancedStack`
- [ ] Programs with 3+ errors collect ALL errors (not just the first)
- [ ] Fuzzer runs 60 seconds with zero panics on 10,000+ random programs
- [ ] `cargo test -p nolang-verifier` passes with zero warnings
- [ ] No `unwrap()` in production code (only in tests)
- [ ] All public types and functions have doc comments
- [ ] All modules have `#[cfg(test)] mod tests` blocks
- [ ] Production code is under 2,000 lines (BUILD_ORDER.md rule 4)
- [ ] Phase 3 gate: `cargo test -p nolang-verifier` all pass; fuzzer 60s no panics

## Confidence Assessment

**Overall: 78%**

| Area | Confidence | Notes |
|------|-----------|-------|
| Error enum | 95% | Mechanical, well-specified by ARCHITECTURE.md |
| Structural pass | 90% | Linear scan, clear rules, follows VM pre-scan pattern |
| Exhaustion pass | 95% | Simple counting on precomputed data |
| Hash verification | 85% | blake3 API is clean; risk is byte ordering |
| Limits checking | 99% | Trivial comparisons |
| Unused field validation | 85% | Tedious but mechanical; risk is table typos |
| Reachability | 70% | Simplified approach avoids CFG; risk is false positives |
| Contract validation | 75% | Mini type-checker scope; depends on types.rs quality |
| Type inference | 60% | Most complex; Unknown fallback limits damage |
| Stack balance | 65% | Depends on MatchTypeInfo accuracy |
| Overall orchestration | 90% | Simple assembly of independent passes |

**Unknowns requiring validation before or during implementation:**

1. Does SPEC.md intend MATCH on I64/U64 to be rejected? Section 4.6 says "not directly matchable." **Decision:** Reject per spec.

2. How to handle CONST_EXT at program end (no next instruction)? **Decision:** Structural error (implicit MissingHalt -- program cannot end with CONST_EXT because it must end with HALT).

3. Are functions without PRE/POST valid? **Decision:** Yes. SPEC says "0 or more PRE blocks."

4. When FUNC body_len disagrees with actual FUNC..ENDFUNC distance, what happens? **Decision:** Emit a structural error. Trust the actual ENDFUNC position for block boundary, note the inconsistency. This is not a named error variant -- use `UnmatchedFunc` as the closest match, or add a comment that this edge case maps to structural rejection.

5. Can a function body contain MATCH blocks? **Decision:** Yes. MATCH blocks can appear anywhere in executable code. The structural pass handles them independently from FUNC blocks.
