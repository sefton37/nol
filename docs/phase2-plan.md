# Plan: Phase 2 — VM Execution Engine

## Context

Phase 1 (`nolang-common`) is complete with 68 tests passing. The crate provides:
- `Opcode` enum with 44 opcodes
- `TypeTag` enum with 13 type tags
- `Instruction` struct with encode/decode
- `Value` enum for runtime representation
- `Instruction::const_value()` for extracting CONST values
- Property-based tests proving encode/decode roundtrip integrity

Phase 2 builds the execution engine (`nolang-vm`) that runs verified instruction streams. The VM assumes input is valid (verification happens in Phase 3). The VM must never panic on any input, but may produce incorrect results for unverified programs.

**Key principle from ARCHITECTURE.md:**
> The VM never panics. Any input (even unverified) produces either a Value or a RuntimeError. However, unverified input may produce incorrect results — the VM trusts the verifier.

## Approach (Recommended)

Build a stack-based virtual machine with strict separation of concerns:
- **Machine state**: stack, bindings, call stack, program counter
- **Execution loop**: fetch-decode-execute with opcode dispatch
- **Function discovery**: pre-scan pass before execution to build function table
- **Pattern matching**: jump-table based dispatch for MATCH/CASE
- **Contract execution**: inline PRE before body, defer POST to CallFrame
- **Float safety**: check after every F64 operation, halt on NaN/infinity

Implementation follows BUILD_ORDER.md Group A-F sequencing, with each group fully tested before proceeding.

## Alternatives Considered

### Alternative 1: Discover functions on-demand (NOT CHOSEN)
**What:** Skip pre-scan. When FUNC is encountered during execution, skip over it and record location.

**Why rejected:**
- CALL needs function locations immediately. If a CALL references a function defined later in the program, we'd need to scan forward on every call.
- Forward references would require lookahead, breaking linear execution model.
- Pre-scan is simple, happens once, and builds a complete function table upfront.

### Alternative 2: Store POST conditions separately from CallFrame (NOT CHOSEN)
**What:** Maintain a separate stack of POST condition locations.

**Why rejected:**
- CallFrame already exists for managing return state. Adding POST location to it is a natural extension.
- Separate stack introduces synchronization risk (call stack and POST stack getting out of sync).
- CallFrame approach keeps all per-call metadata in one place.

### Alternative 3: Pre-compute MATCH jump tables during function scan (NOT CHOSEN)
**What:** During pre-scan, analyze every MATCH block and build a Vec<usize> jump table mapping tag → body_start_pc.

**Why rejected:**
- Adds complexity to pre-scan phase.
- MATCH is not on hot path for simple programs (arithmetic, bindings).
- Linear scan of CASEs is simple, correct, and sufficient for Phase 2. Optimization is Phase 6+ territory.
- Jump tables only help when variant_count is large. Most real programs have 2-5 tags.

**Decision:** Use linear scan through CASEs. If profiling in Phase 5+ shows MATCH is a bottleneck, revisit.

## Implementation Steps

### Step 1: Create crate structure
```
crates/vm/
├── Cargo.toml        — Dependencies: nolang-common, thiserror
└── src/
    ├── lib.rs        — Public API: run(Program) -> Result<Value, RuntimeError>
    ├── machine.rs    — VM struct, state management
    ├── execute.rs    — Execution loop, opcode dispatch
    └── error.rs      — RuntimeError enum
```

### Step 2: Define error types (error.rs)
Implement all RuntimeError variants from ARCHITECTURE.md, plus float safety errors.

### Step 3: Define VM state (machine.rs)
Implement VM struct with initialization and helper methods.

### Step 4: Implement function pre-scan (machine.rs)
Scan program to locate all FUNC blocks, build function table.

### Step 5: Implement execution loop skeleton (execute.rs)
Main loop: fetch instruction, match opcode, dispatch to handler.

### Step 6-11: Implement opcodes in Groups A-F (execute.rs)
One group at a time per BUILD_ORDER.md. Each group fully tested before next.

## Module Structure and Responsibility Boundaries

### lib.rs — Public API
```rust
pub use error::RuntimeError;
pub use machine::VM;

/// Execute a NoLang program and return the result.
///
/// The program is assumed to have been verified. Unverified programs may
/// produce incorrect results but will not panic.
pub fn run(program: &Program) -> Result<Value, RuntimeError> {
    let mut vm = VM::new(program);
    vm.execute()
}
```

**Responsibility:** Single entry point. Constructs VM and delegates to execute().

### error.rs — RuntimeError enum
```rust
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuntimeError {
    #[error("Division by zero at instruction {at}")]
    DivisionByZero { at: usize },

    #[error("Recursion depth {current} exceeded limit {limit} at instruction {at}")]
    RecursionDepthExceeded { at: usize, current: u16, limit: u16 },

    #[error("Array index {index} out of bounds (length {length}) at instruction {at}")]
    ArrayIndexOutOfBounds { at: usize, index: u64, length: u64 },

    #[error("Precondition failed at instruction {at}")]
    PreconditionFailed { at: usize },

    #[error("Postcondition failed at instruction {at}")]
    PostconditionFailed { at: usize },

    #[error("Stack overflow (exceeds 4096 slots) at instruction {at}")]
    StackOverflow { at: usize },

    #[error("HALT with empty stack")]
    HaltWithEmptyStack,

    #[error("HALT with multiple values on stack (count: {count})")]
    HaltWithMultipleValues { count: usize },

    #[error("Float operation produced NaN at instruction {at}")]
    FloatNaN { at: usize },

    #[error("Float operation produced infinity at instruction {at}")]
    FloatInfinity { at: usize },

    #[error("ASSERT failed at instruction {at}")]
    AssertFailed { at: usize },

    #[error("Invalid CASE tag {tag} for MATCH at instruction {at}")]
    InvalidCaseTag { at: usize, tag: u16 },

    #[error("Unexpected end of program at instruction {at}")]
    UnexpectedEndOfProgram { at: usize },

    #[error("TYPEOF expected type {expected:?} but found {found:?} at instruction {at}")]
    TypeofMismatch { at: usize, expected: TypeTag, found: TypeTag },

    #[error("Stack underflow at instruction {at}")]
    StackUnderflow { at: usize },

    #[error("Binding underflow (REF to index {index} when depth is {depth}) at instruction {at}")]
    BindingUnderflow { at: usize, index: u16, depth: usize },

    #[error("PROJECT on non-tuple at instruction {at}")]
    ProjectOnNonTuple { at: usize },

    #[error("PROJECT field {field} out of bounds (tuple size {size}) at instruction {at}")]
    ProjectOutOfBounds { at: usize, field: u16, size: usize },

    #[error("ARRAY_GET on non-array at instruction {at}")]
    ArrayGetOnNonArray { at: usize },
}
```

**Responsibility:** All runtime error variants. Each includes `at: usize` (instruction index) for debugging.

### machine.rs — VM State Management

```rust
use nolang_common::{Instruction, Program, Value, Opcode};
use crate::error::RuntimeError;
use std::collections::HashMap;

/// A call frame for function invocation.
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// Instruction index to return to after RET.
    pub return_pc: usize,
    /// Binding depth before this call (for restoring environment).
    pub saved_binding_depth: usize,
    /// Location of the function body start (after FUNC instruction).
    pub function_start: usize,
    /// Current recursion depth for this function.
    pub recursion_depth: u16,
    /// Location of POST condition blocks (if any).
    /// Each entry is (post_start_pc, post_len).
    pub post_conditions: Vec<(usize, u16)>,
}

/// Function metadata discovered during pre-scan.
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    /// Instruction index of the FUNC instruction.
    pub func_pc: usize,
    /// Number of parameters (from FUNC arg1).
    pub param_count: u16,
    /// Total body length (from FUNC arg2).
    pub body_len: u16,
    /// Instruction index where function body starts (after all PRE/POST blocks).
    pub body_start_pc: usize,
    /// PRE condition blocks: (pre_start_pc, pre_len).
    pub pre_conditions: Vec<(usize, u16)>,
    /// POST condition blocks: (post_start_pc, post_len).
    pub post_conditions: Vec<(usize, u16)>,
}

/// The NoLang virtual machine.
pub struct VM<'a> {
    /// The program being executed.
    program: &'a Program,
    /// Operand stack (max 4096 slots per SPEC.md Section 9).
    stack: Vec<Value>,
    /// Binding environment (de Bruijn indexed values).
    bindings: Vec<Value>,
    /// Call stack for function invocation.
    call_stack: Vec<CallFrame>,
    /// Program counter (instruction index).
    pc: usize,
    /// Function table: maps binding index → FunctionInfo.
    /// Built during pre-scan. Functions are "bound" in the order they appear.
    functions: HashMap<usize, FunctionInfo>,
    /// Current binding index for the next function definition.
    next_function_binding: usize,
}

impl<'a> VM<'a> {
    /// Create a new VM for the given program.
    pub fn new(program: &'a Program) -> Self {
        Self {
            program,
            stack: Vec::new(),
            bindings: Vec::new(),
            call_stack: Vec::new(),
            pc: 0,
            functions: HashMap::new(),
            next_function_binding: 0,
        }
    }

    /// Pre-scan the program to locate all function definitions.
    ///
    /// Functions are discovered in order and assigned binding indices.
    /// This must be called before execute().
    fn scan_functions(&mut self) -> Result<(), RuntimeError> {
        let mut pc = 0;
        while pc < self.program.instructions().len() {
            let instr = &self.program.instructions()[pc];

            if instr.opcode == Opcode::Func {
                let param_count = instr.arg1;
                let body_len = instr.arg2;
                let func_pc = pc;

                // Scan through the function body to find PRE/POST blocks
                let mut pre_conditions = Vec::new();
                let mut post_conditions = Vec::new();
                let mut body_start_pc = pc + 1; // Start after FUNC
                let mut scan_pc = pc + 1;

                // Scan for PRE/POST blocks at the start of the function
                while scan_pc < self.program.instructions().len()
                      && scan_pc < pc + body_len as usize {
                    let scan_instr = &self.program.instructions()[scan_pc];

                    match scan_instr.opcode {
                        Opcode::Pre => {
                            let pre_len = scan_instr.arg1;
                            pre_conditions.push((scan_pc + 1, pre_len));
                            scan_pc += 1 + pre_len as usize;
                            body_start_pc = scan_pc;
                        }
                        Opcode::Post => {
                            let post_len = scan_instr.arg1;
                            post_conditions.push((scan_pc + 1, post_len));
                            scan_pc += 1 + post_len as usize;
                            body_start_pc = scan_pc;
                        }
                        _ => break, // End of PRE/POST section
                    }
                }

                let func_info = FunctionInfo {
                    func_pc,
                    param_count,
                    body_len,
                    body_start_pc,
                    pre_conditions,
                    post_conditions,
                };

                self.functions.insert(self.next_function_binding, func_info);
                self.next_function_binding += 1;

                // Skip to end of function
                pc += body_len as usize + 1; // +1 for ENDFUNC
            } else {
                pc += 1;
            }
        }

        Ok(())
    }

    /// Push a value onto the stack.
    fn push(&mut self, value: Value) -> Result<(), RuntimeError> {
        if self.stack.len() >= 4096 {
            return Err(RuntimeError::StackOverflow { at: self.pc });
        }
        self.stack.push(value);
        Ok(())
    }

    /// Pop a value from the stack.
    fn pop(&mut self) -> Result<Value, RuntimeError> {
        self.stack.pop().ok_or(RuntimeError::StackUnderflow { at: self.pc })
    }

    /// Peek at the top of the stack without removing it.
    fn peek(&self) -> Result<&Value, RuntimeError> {
        self.stack.last().ok_or(RuntimeError::StackUnderflow { at: self.pc })
    }

    /// Check if a float value is NaN or infinity.
    fn check_float(&self, val: f64) -> Result<(), RuntimeError> {
        if val.is_nan() {
            Err(RuntimeError::FloatNaN { at: self.pc })
        } else if val.is_infinite() {
            Err(RuntimeError::FloatInfinity { at: self.pc })
        } else {
            Ok(())
        }
    }

    /// Fetch the current instruction and advance pc.
    fn fetch(&mut self) -> Result<&Instruction, RuntimeError> {
        if self.pc >= self.program.instructions().len() {
            return Err(RuntimeError::UnexpectedEndOfProgram { at: self.pc });
        }
        let instr = &self.program.instructions()[self.pc];
        Ok(instr)
    }
}
```

**Responsibility:** State management, stack operations, float safety, function pre-scan.

**Design decision — Function table:**
- Functions are "bound" in order of appearance. First FUNC = binding index 0, second FUNC = binding index 1.
- CALL 0 means "call the function at binding index 0" (the first function defined).
- Pre-scan builds this table before execution starts.

**Design decision — CallFrame POST storage:**
- CallFrame stores POST condition locations so they can be checked at RET time.
- POST runs with the return value at binding index 0.

### execute.rs — Execution Loop and Opcode Dispatch

```rust
use crate::machine::{VM, CallFrame};
use crate::error::RuntimeError;
use nolang_common::{Instruction, Opcode, TypeTag, Value};

impl<'a> VM<'a> {
    /// Execute the program until HALT.
    pub fn execute(&mut self) -> Result<Value, RuntimeError> {
        // Pre-scan to locate functions
        self.scan_functions()?;

        // Find the entry point (first instruction after last ENDFUNC)
        self.pc = self.find_entry_point();

        // Main execution loop
        loop {
            let instr = *self.fetch()?;
            self.pc += 1; // Advance pc after fetch

            match instr.opcode {
                Opcode::Halt => return self.exec_halt(),
                Opcode::Nop => {} // Do nothing

                // Group A: Foundation
                Opcode::Const => self.exec_const(&instr)?,
                Opcode::ConstExt => self.exec_const_ext(&instr)?,
                Opcode::Bind => self.exec_bind()?,
                Opcode::Ref => self.exec_ref(&instr)?,
                Opcode::Drop => self.exec_drop()?,

                // Group B: Arithmetic
                Opcode::Add => self.exec_add()?,
                Opcode::Sub => self.exec_sub()?,
                Opcode::Mul => self.exec_mul()?,
                Opcode::Div => self.exec_div()?,
                Opcode::Mod => self.exec_mod()?,
                Opcode::Neg => self.exec_neg()?,

                // Comparison
                Opcode::Eq => self.exec_eq()?,
                Opcode::Neq => self.exec_neq()?,
                Opcode::Lt => self.exec_lt()?,
                Opcode::Gt => self.exec_gt()?,
                Opcode::Lte => self.exec_lte()?,
                Opcode::Gte => self.exec_gte()?,

                // Logic & Bitwise
                Opcode::And => self.exec_and()?,
                Opcode::Or => self.exec_or()?,
                Opcode::Not => self.exec_not()?,
                Opcode::Xor => self.exec_xor()?,
                Opcode::Shl => self.exec_shl()?,
                Opcode::Shr => self.exec_shr()?,

                // Group C: Pattern matching
                Opcode::Match => self.exec_match(&instr)?,
                Opcode::Case => {} // Handled by MATCH
                Opcode::Exhaust => {} // End of MATCH block, continue

                // Group D: Functions
                Opcode::Func => self.exec_func(&instr)?, // Skip during execution
                Opcode::EndFunc => {} // End of function definition, continue
                Opcode::Pre => self.exec_pre(&instr)?, // Skip (handled at call time)
                Opcode::Post => self.exec_post(&instr)?, // Skip (handled at return time)
                Opcode::Call => self.exec_call(&instr)?,
                Opcode::Recurse => self.exec_recurse(&instr)?,
                Opcode::Ret => self.exec_ret()?,

                // Group E: Data structures
                Opcode::VariantNew => self.exec_variant_new(&instr)?,
                Opcode::TupleNew => self.exec_tuple_new(&instr)?,
                Opcode::Project => self.exec_project(&instr)?,
                Opcode::ArrayNew => self.exec_array_new(&instr)?,
                Opcode::ArrayGet => self.exec_array_get()?,
                Opcode::ArrayLen => self.exec_array_len()?,

                // Group F: Verification & Meta
                Opcode::Hash => {} // NOP during execution (verification only)
                Opcode::Assert => self.exec_assert()?,
                Opcode::Typeof => self.exec_typeof(&instr)?,
            }
        }
    }

    /// Find the program entry point (first instruction after last ENDFUNC).
    fn find_entry_point(&self) -> usize {
        let mut last_endfunc = None;
        for (i, instr) in self.program.instructions().iter().enumerate() {
            if instr.opcode == Opcode::EndFunc {
                last_endfunc = Some(i);
            }
        }
        last_endfunc.map(|i| i + 1).unwrap_or(0)
    }

    // --- Opcode implementations (detailed below) ---
}
```

**Responsibility:** Main execution loop, opcode dispatch, entry point discovery.

## Resolution of Design Questions

### Q1: Function registration
**Answer:** Pre-scan pass before execution. `scan_functions()` iterates through the program, records FUNC locations, and builds a `HashMap<usize, FunctionInfo>`. Functions are assigned binding indices in order of appearance.

**Rationale:** Simple, deterministic, enables forward references.

### Q2: Function calling convention
**Answer:** CallFrame contains:
```rust
pub struct CallFrame {
    pub return_pc: usize,              // Where to jump after RET
    pub saved_binding_depth: usize,    // Restore bindings on return
    pub function_start: usize,         // For RECURSE
    pub recursion_depth: u16,          // Track recursion depth
    pub post_conditions: Vec<(usize, u16)>, // POST blocks to check at RET
}
```

**Stack discipline:**
- Before CALL: arguments are on stack (last pushed = param 0)
- CALL pops arguments, binds them (last popped = binding index 0)
- CALL pushes CallFrame, jumps to function body
- RET pops return value, checks POST conditions, pops CallFrame, restores bindings, pushes return value

### Q3: RECURSE mechanics
**Answer:** CallFrame stores `function_start` (location of current function body). RECURSE:
1. Increments `recursion_depth` in current CallFrame
2. Checks if depth exceeds arg1 (depth limit)
3. Pushes new CallFrame with same `function_start`, depth = current + 1
4. Jumps to `function_start`

**Rationale:** RECURSE is syntactic sugar for "call the current function". No need to track "which function is enclosing" — it's in the current CallFrame.

### Q4: PRE/POST contract execution
**Answer:**
- **PRE:** Executed inline at CALL time, after binding arguments, before jumping to function body. PRE body must produce BOOL. If false, RuntimeError::PreconditionFailed.
- **POST:** Deferred. POST condition locations stored in CallFrame. At RET time, bind return value at index 0, execute each POST body. POST must produce BOOL. If false, RuntimeError::PostconditionFailed.

**Implementation detail:**
- `scan_functions()` records PRE/POST locations in FunctionInfo
- `exec_call()` runs all PRE blocks before jumping to body
- `exec_ret()` runs all POST blocks before returning to caller

### Q5: MATCH/CASE/EXHAUST execution flow
**Answer:** Linear scan through CASEs.

**Algorithm:**
```rust
fn exec_match(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
    let variant_count = instr.arg1;
    let matched_value = self.pop()?;

    // Extract tag from matched value
    let tag = match &matched_value {
        Value::Bool(b) => if *b { 1 } else { 0 },
        Value::Variant { tag, .. } => *tag,
        _ => return Err(RuntimeError::InvalidCaseTag { at: self.pc, tag: 0 }),
    };

    // Scan through CASE instructions to find matching tag
    let mut cases_seen = 0;
    while cases_seen < variant_count {
        let case_instr = self.fetch()?;
        if case_instr.opcode != Opcode::Case {
            return Err(RuntimeError::InvalidCaseTag { at: self.pc, tag });
        }

        let case_tag = case_instr.arg1;
        let body_len = case_instr.arg2;
        self.pc += 1; // Advance past CASE

        if case_tag == tag {
            // Match found. If variant has payload, bind it.
            if let Value::Variant { payload, .. } = matched_value {
                self.bindings.push(*payload);
            }

            // Execute case body (body_len instructions).
            // Body executes normally, leaving one value on stack.
            // After body, skip remaining CASEs and EXHAUST.
            let end_pc = self.pc + body_len as usize;
            // (Body executes via main loop)
            // Need to skip to EXHAUST after body completes.
            // Store end_pc in VM state? Or scan for EXHAUST after body?

            // Decision: After body executes (detected when pc == end_pc),
            // scan forward to EXHAUST and skip.
            // This requires tracking "we're in a CASE body" state.

            return Ok(()); // Body will execute in main loop
        } else {
            // Not a match, skip this case body
            self.pc += body_len as usize;
        }

        cases_seen += 1;
    }

    // Should never reach here if program is verified (exhaustive match)
    Err(RuntimeError::InvalidCaseTag { at: self.pc, tag })
}
```

**Complication:** After executing a CASE body, we need to skip remaining CASEs and EXHAUST. This requires tracking execution mode.

**Refined approach:** Add VM state to track "skip to EXHAUST" mode.

```rust
pub struct VM<'a> {
    // ... existing fields ...
    /// If Some(n), skip the next n instructions (for skipping unmatched CASEs).
    skip_mode: Option<usize>,
}
```

When MATCH finds matching CASE:
1. If variant has payload, push it to stack (not bindings — BIND is explicit in CASE body)
2. Set `skip_mode = None` (execute body normally)
3. After body completes (detected by body_len), set `skip_mode = Some(skip_remaining_cases_count)`

**Actually simpler approach:** MATCH scans all CASEs, finds the right one, jumps to its body, executes body, then jumps past EXHAUST.

```rust
fn exec_match(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
    let variant_count = instr.arg1;
    let matched_value = self.pop()?;

    let tag = extract_tag(&matched_value)?;

    // Scan through CASEs to find match
    let match_start_pc = self.pc; // Points to first CASE
    let mut scan_pc = match_start_pc;
    let mut found = false;

    for _ in 0..variant_count {
        let case_instr = &self.program.instructions()[scan_pc];
        if case_instr.opcode != Opcode::Case {
            return Err(RuntimeError::InvalidCaseTag { at: scan_pc, tag });
        }

        let case_tag = case_instr.arg1;
        let body_len = case_instr.arg2 as usize;

        if case_tag == tag && !found {
            // This is the match. Set pc to body start.
            self.pc = scan_pc + 1; // Skip CASE instruction

            // Push payload if variant
            if let Value::Variant { payload, .. } = matched_value {
                self.push(*payload)?;
            }

            found = true;
            // Continue scanning to find EXHAUST location
        }

        scan_pc += 1 + body_len; // Skip CASE + body
    }

    // scan_pc now points to EXHAUST
    // Store this location so we can jump there after body executes
    // Problem: we don't know when body finishes (it's executed by main loop)

    // Need to track "after executing N instructions, jump to pc X"
    // This is getting complex. Simpler: just let the main loop continue,
    // and when we hit the next CASE, skip it.

    Ok(())
}
```

**Even simpler:** After MATCH, execution continues normally through the matched CASE body. When we hit the next CASE (or EXHAUST), we're done. But we need to skip unmatched CASEs.

**Final approach:**
- MATCH determines which CASE to execute
- VM state tracks "we're in CASE body N, skip all other CASEs"
- When we encounter CASE and we're in skip mode, skip body
- When we encounter EXHAUST, clear skip mode

Actually, **clearest approach:** MATCH computes jump target and directly sets pc to the target CASE body. After body executes (tracked by instruction count), jump to EXHAUST.

This requires tracking case execution state. Let's add:

```rust
pub struct VM<'a> {
    // ... existing fields ...
    /// If Some((body_end_pc, exhaust_pc)), we're in a CASE body.
    /// When pc reaches body_end_pc, jump to exhaust_pc.
    case_context: Option<(usize, usize)>,
}
```

In main loop:
```rust
// At start of loop iteration
if let Some((body_end_pc, exhaust_pc)) = self.case_context {
    if self.pc == body_end_pc {
        self.pc = exhaust_pc;
        self.case_context = None;
        continue;
    }
}
```

### Q6: CONST_EXT mechanics
**Answer:** CONST_EXT consumes two instruction slots.

```rust
fn exec_const_ext(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
    // instr.arg1 is high 16 bits
    // Next instruction's (type_tag, arg1, arg2, arg3) are low 48 bits
    let next_instr = &self.program.instructions()[self.pc];
    self.pc += 1; // Consume next instruction

    let high16 = instr.arg1 as u64;
    let low48 = {
        let bytes = next_instr.encode();
        // Extract 48 bits from bytes[1..7] (skip opcode byte)
        let mut low_bytes = [0u8; 8];
        low_bytes[0..6].copy_from_slice(&bytes[1..7]);
        u64::from_le_bytes(low_bytes)
    };

    let full_value = (high16 << 48) | low48;

    match instr.type_tag {
        TypeTag::I64 => self.push(Value::I64(full_value as i64))?,
        TypeTag::U64 => self.push(Value::U64(full_value))?,
        TypeTag::F64 => {
            let float_val = f64::from_bits(full_value);
            self.check_float(float_val)?;
            self.push(Value::F64(float_val))?;
        }
        _ => return Err(RuntimeError::UnexpectedEndOfProgram { at: self.pc }),
    }

    Ok(())
}
```

### Q7: TYPEOF semantics
**Answer:** Pop value, check type, push value back, push BOOL.

```rust
fn exec_typeof(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
    let expected_tag = TypeTag::try_from(instr.arg1 as u8)
        .map_err(|_| RuntimeError::TypeofMismatch {
            at: self.pc,
            expected: TypeTag::None,
            found: TypeTag::None
        })?;

    let value = self.pop()?;
    let actual_tag = value.type_tag();
    let matches = actual_tag == expected_tag;

    self.push(value)?;  // Push value back (non-destructive)
    self.push(Value::Bool(matches))?;

    Ok(())
}
```

**Wait, re-reading SPEC.md:**
> TYPEOF arg1=expected_tag — Pop value, push BOOL (1 if value's type tag matches arg1). Non-destructive: value is pushed back.

So it's: pop, push back, push bool. Net effect: +1 stack depth.

**Correction:** arg1 is stored in instr.arg1 (u16), but TypeTag is u8. Need to handle:
```rust
fn exec_typeof(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
    // instr.arg1 contains the expected type tag (u16, but really u8)
    let expected = TypeTag::try_from((instr.arg1 & 0xFF) as u8)
        .map_err(|_| RuntimeError::TypeofMismatch {
            at: self.pc,
            expected: TypeTag::None,
            found: TypeTag::None
        })?;

    let value = self.pop()?;
    let actual = value.type_tag();

    self.push(value)?;  // Non-destructive
    self.push(Value::Bool(actual == expected))?;

    Ok(())
}
```

### Q8: Stack overflow check
**Answer:** Check on every push. Max 4096 slots per SPEC.md Section 9.

```rust
fn push(&mut self, value: Value) -> Result<(), RuntimeError> {
    if self.stack.len() >= 4096 {
        return Err(RuntimeError::StackOverflow { at: self.pc });
    }
    self.stack.push(value);
    Ok(())
}
```

### Q9: Float safety
**Answer:** Check after every F64 arithmetic and comparison op.

```rust
fn check_float(&self, val: f64) -> Result<(), RuntimeError> {
    if val.is_nan() {
        Err(RuntimeError::FloatNaN { at: self.pc })
    } else if val.is_infinite() {
        Err(RuntimeError::FloatInfinity { at: self.pc })
    } else {
        Ok(())
    }
}

fn exec_add(&mut self) -> Result<(), RuntimeError> {
    let b = self.pop()?;
    let a = self.pop()?;

    let result = match (a, b) {
        (Value::I64(x), Value::I64(y)) => Value::I64(x.wrapping_add(y)),
        (Value::U64(x), Value::U64(y)) => Value::U64(x.wrapping_add(y)),
        (Value::F64(x), Value::F64(y)) => {
            let sum = x + y;
            self.check_float(sum)?;
            Value::F64(sum)
        }
        _ => return Err(RuntimeError::StackUnderflow { at: self.pc }), // Type mismatch
    };

    self.push(result)?;
    Ok(())
}
```

**Design choice:** Wrapping arithmetic for integers (no overflow errors). This matches common VM behavior and avoids making overflow a runtime error.

### Q10: HASH during execution
**Answer:** Treat as NOP. Just skip it.

```rust
Opcode::Hash => {} // NOP during execution
```

## Code Sketches for Tricky Opcodes

### MATCH/CASE/EXHAUST (Refined Final Approach)

```rust
fn exec_match(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
    let variant_count = instr.arg1;
    let matched_value = self.pop()?;

    // Extract tag from matched value
    let tag = match &matched_value {
        Value::Bool(b) => if *b { 1 } else { 0 },
        Value::Variant { tag, .. } => *tag,
        _ => return Err(RuntimeError::InvalidCaseTag { at: self.pc - 1, tag: 0 }),
    };

    // Scan through all CASEs to find:
    // 1. The matching CASE body start
    // 2. The EXHAUST location
    let mut scan_pc = self.pc;
    let mut match_body_start = None;
    let mut match_body_end = None;

    for _ in 0..variant_count {
        let case_instr = &self.program.instructions()[scan_pc];
        if case_instr.opcode != Opcode::Case {
            return Err(RuntimeError::InvalidCaseTag { at: scan_pc, tag });
        }

        let case_tag = case_instr.arg1;
        let body_len = case_instr.arg2 as usize;
        let body_start = scan_pc + 1;
        let body_end = body_start + body_len;

        if case_tag == tag {
            match_body_start = Some(body_start);
            match_body_end = Some(body_end);
        }

        scan_pc = body_end; // Move to next CASE or EXHAUST
    }

    // scan_pc now points to EXHAUST
    let exhaust_pc = scan_pc;

    let body_start = match_body_start.ok_or(RuntimeError::InvalidCaseTag {
        at: self.pc - 1,
        tag
    })?;
    let body_end = match_body_end.unwrap();

    // If variant has payload, push it onto stack (not bindings)
    if let Value::Variant { payload, .. } = matched_value {
        self.push(*payload)?;
    }

    // Jump to matched CASE body
    self.pc = body_start;

    // Track that we're in a CASE body. When pc reaches body_end, jump to exhaust_pc + 1.
    self.case_context = Some((body_end, exhaust_pc + 1));

    Ok(())
}
```

In main loop, before fetching next instruction:
```rust
// Check if we've finished a CASE body
if let Some((body_end_pc, exhaust_pc)) = self.case_context {
    if self.pc == body_end_pc {
        self.pc = exhaust_pc;
        self.case_context = None;
    }
}
```

### CALL (with PRE execution)

```rust
fn exec_call(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
    let func_binding_index = instr.arg1 as usize;

    let func_info = self.functions.get(&func_binding_index)
        .ok_or(RuntimeError::BindingUnderflow {
            at: self.pc - 1,
            index: instr.arg1,
            depth: self.bindings.len()
        })?
        .clone();

    // Pop arguments from stack and bind them (last popped = binding index 0)
    let mut args = Vec::new();
    for _ in 0..func_info.param_count {
        args.push(self.pop()?);
    }
    args.reverse(); // First arg should be at binding index (param_count - 1)

    let saved_binding_depth = self.bindings.len();
    for arg in args {
        self.bindings.push(arg);
    }

    // Execute PRE conditions
    for (pre_start, pre_len) in &func_info.pre_conditions {
        let saved_pc = self.pc;
        self.pc = *pre_start;

        // Execute pre_len instructions
        for _ in 0..*pre_len {
            let pre_instr = *self.fetch()?;
            self.pc += 1;
            self.execute_single_instruction(&pre_instr)?;
        }

        // PRE must leave a BOOL on stack
        let condition = self.pop()?;
        if let Value::Bool(true) = condition {
            // Continue
        } else {
            return Err(RuntimeError::PreconditionFailed { at: saved_pc });
        }

        self.pc = saved_pc; // Restore
    }

    // Push call frame
    let call_frame = CallFrame {
        return_pc: self.pc,
        saved_binding_depth,
        function_start: func_info.body_start_pc,
        recursion_depth: 0,
        post_conditions: func_info.post_conditions.clone(),
    };
    self.call_stack.push(call_frame);

    // Jump to function body
    self.pc = func_info.body_start_pc;

    Ok(())
}
```

**Problem:** `execute_single_instruction` doesn't exist. We'd need to refactor the dispatch logic into a separate function.

**Simpler approach for PRE:** Jump to PRE block, execute until PRE block ends, then continue with function call setup.

Actually, PRE blocks are embedded in the function body. We could:
1. Jump to each PRE block
2. Execute it (main loop handles it)
3. Check result

But this breaks the linear execution model. **Cleaner:** Extract opcode dispatch into a method.

```rust
fn execute_instruction(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
    match instr.opcode {
        // ... (same dispatch as in execute())
    }
}
```

Then `execute()` becomes:
```rust
pub fn execute(&mut self) -> Result<Value, RuntimeError> {
    self.scan_functions()?;
    self.pc = self.find_entry_point();

    loop {
        // Check CASE context
        if let Some((body_end_pc, exhaust_pc)) = self.case_context {
            if self.pc == body_end_pc {
                self.pc = exhaust_pc;
                self.case_context = None;
            }
        }

        let instr = *self.fetch()?;
        self.pc += 1;

        match instr.opcode {
            Opcode::Halt => return self.exec_halt(),
            _ => self.execute_instruction(&instr)?,
        }
    }
}
```

And `exec_call` can use a helper to execute a range of instructions:

```rust
fn execute_range(&mut self, start_pc: usize, len: usize) -> Result<(), RuntimeError> {
    let saved_pc = self.pc;
    self.pc = start_pc;

    for _ in 0..len {
        let instr = *self.fetch()?;
        self.pc += 1;
        self.execute_instruction(&instr)?;
    }

    self.pc = saved_pc;
    Ok(())
}
```

### RET (with POST execution)

```rust
fn exec_ret(&mut self) -> Result<(), RuntimeError> {
    let return_value = self.pop()?;

    let call_frame = self.call_stack.pop()
        .ok_or(RuntimeError::StackUnderflow { at: self.pc - 1 })?;

    // Bind return value for POST conditions
    self.bindings.push(return_value.clone());

    // Execute POST conditions
    for (post_start, post_len) in &call_frame.post_conditions {
        self.execute_range(*post_start, *post_len as usize)?;

        let condition = self.pop()?;
        if let Value::Bool(true) = condition {
            // Continue
        } else {
            return Err(RuntimeError::PostconditionFailed { at: self.pc - 1 });
        }
    }

    // Unbind return value (it was only for POST)
    self.bindings.pop();

    // Restore binding environment
    self.bindings.truncate(call_frame.saved_binding_depth);

    // Return to caller
    self.pc = call_frame.return_pc;

    // Push return value
    self.push(return_value)?;

    Ok(())
}
```

### RECURSE (with depth tracking)

```rust
fn exec_recurse(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
    let depth_limit = instr.arg1;

    let current_frame = self.call_stack.last()
        .ok_or(RuntimeError::StackUnderflow { at: self.pc - 1 })?;

    let current_depth = current_frame.recursion_depth;
    if current_depth + 1 > depth_limit {
        return Err(RuntimeError::RecursionDepthExceeded {
            at: self.pc - 1,
            current: current_depth + 1,
            limit: depth_limit
        });
    }

    let function_start = current_frame.function_start;
    let post_conditions = current_frame.post_conditions.clone();

    // Get function info to know param count
    // Problem: we don't know which function we're recursing into.
    // Need to track function info in CallFrame.

    // Solution: Add func_binding_index to CallFrame
    let func_binding_index = current_frame.func_binding_index;
    let func_info = self.functions.get(&func_binding_index).unwrap().clone();

    // Pop arguments
    let mut args = Vec::new();
    for _ in 0..func_info.param_count {
        args.push(self.pop()?);
    }
    args.reverse();

    let saved_binding_depth = self.bindings.len();
    for arg in args {
        self.bindings.push(arg);
    }

    // Execute PRE conditions (same as CALL)
    for (pre_start, pre_len) in &func_info.pre_conditions {
        self.execute_range(*pre_start, *pre_len as usize)?;
        let condition = self.pop()?;
        if let Value::Bool(true) = condition {
            // OK
        } else {
            return Err(RuntimeError::PreconditionFailed { at: self.pc - 1 });
        }
    }

    // Push new call frame with incremented depth
    let new_frame = CallFrame {
        return_pc: self.pc,
        saved_binding_depth,
        function_start,
        recursion_depth: current_depth + 1,
        post_conditions,
        func_binding_index,
    };
    self.call_stack.push(new_frame);

    // Jump to function body
    self.pc = function_start;

    Ok(())
}
```

**Refinement needed:** CallFrame must track which function it's calling (for RECURSE to know param_count).

Updated CallFrame:
```rust
pub struct CallFrame {
    pub return_pc: usize,
    pub saved_binding_depth: usize,
    pub function_start: usize,
    pub recursion_depth: u16,
    pub post_conditions: Vec<(usize, u16)>,
    pub func_binding_index: usize, // NEW: which function we're in
}
```

## Files Affected

### New files to create:
- `/home/kellogg/dev/nol/crates/vm/Cargo.toml`
- `/home/kellogg/dev/nol/crates/vm/src/lib.rs`
- `/home/kellogg/dev/nol/crates/vm/src/error.rs`
- `/home/kellogg/dev/nol/crates/vm/src/machine.rs`
- `/home/kellogg/dev/nol/crates/vm/src/execute.rs`

### Files to modify:
- `/home/kellogg/dev/nol/Cargo.toml` — Add `vm` to workspace members

## Implementation Order (Groups A-F)

### Group A: Foundation (HALT, CONST, NOP, BIND, REF, DROP)
**Tests:**
- Empty program (just HALT) → HaltWithEmptyStack
- `CONST I64 0 42, HALT` → Ok(I64(42))
- `CONST I64 0 5, BIND, REF 0, HALT` → Ok(I64(5))
- Multiple binds with correct de Bruijn indexing

**Example programs executable:** Example 1 (Constant Return)

### Group B: Arithmetic (ADD, SUB, MUL, DIV, MOD, NEG, comparisons, logic/bitwise)
**Tests:**
- ADD: `CONST I64 0 5, CONST I64 0 3, ADD, HALT` → I64(8)
- DIV by zero: RuntimeError::DivisionByZero
- F64 NaN: RuntimeError::FloatNaN
- All comparison opcodes with I64, U64, F64
- All logic opcodes with BOOL and integers

**Example programs executable:** Example 2 (Addition)

### Group C: Pattern Matching (MATCH, CASE, EXHAUST)
**Tests:**
- Boolean match (both branches)
- Variant match with 3 tags
- Payload extraction

**Example programs executable:** Example 3 (Boolean Match), Example 5 (Maybe Type Handling)

### Group D: Functions (FUNC, CALL, RET, RECURSE, PRE, POST)
**Tests:**
- Simple function call
- Function with PRE that passes
- Function with PRE that fails → PreconditionFailed
- Function with POST that fails → PostconditionFailed
- Recursive factorial(5) → 120
- Recursion depth exceeded → RecursionDepthExceeded

**Example programs executable:** Example 4 (Simple Function), Example 6 (Recursive Factorial), Example 9 (Function with Contracts)

### Group E: Data Structures (VARIANT_NEW, TUPLE_NEW, PROJECT, ARRAY_NEW, ARRAY_GET, ARRAY_LEN)
**Tests:**
- Tuple construction and projection
- Array construction, get, and length
- Array out of bounds → ArrayIndexOutOfBounds

**Example programs executable:** Example 7 (Tuple), Example 8 (Array)

### Group F: Verification & Meta (HASH, ASSERT, TYPEOF)
**Tests:**
- HASH is NOP (no effect)
- ASSERT with true → continue
- ASSERT with false → AssertFailed
- TYPEOF matches → true
- TYPEOF mismatch → false

**All example programs executable at this point.**

## Testing Strategy

### Unit tests (per opcode)
Each opcode has at least 3 tests:
1. Valid use (expected behavior)
2. Edge case (boundary values, empty collections)
3. Error case (div by zero, out of bounds, etc.)

### Integration tests (per example program)
Each example from EXAMPLES.md is tested end-to-end:
```rust
#[test]
fn example_1_constant_return() {
    let program = assemble("CONST I64 0 42\nHALT");
    let result = run(&program).unwrap();
    assert_eq!(result, Value::I64(42));
}
```

### Property-based tests (fuzzing)
```rust
proptest! {
    #[test]
    fn vm_never_panics(instrs in vec(arb_instruction(), 1..100)) {
        let program = Program::new(instrs);
        let result = run(&program);
        // Either Ok or Err, never panic
        match result {
            Ok(_) | Err(_) => {}
        }
    }
}
```

Run fuzzer for 60 seconds with no panics as Phase 2 gate.

### Acceptance criteria checklist (from BUILD_ORDER.md)

Phase 2 gate requirements:
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
- [ ] The VM never panics on any input (fuzz with 1000 random programs)

## Risks & Mitigations

### Risk 1: CASE body execution breaks linear execution model
**Likelihood:** Medium
**Impact:** High (main loop complexity)

**Mitigation:** Use `case_context` state tracking. Main loop checks context before each instruction fetch. Adds small overhead but keeps execution model clean.

**Validation:** Test nested MATCH (not in spec, so should fail during verification, but VM should handle gracefully if encountered).

### Risk 2: PRE/POST execution requires instruction re-execution
**Likelihood:** High
**Impact:** Medium (code complexity)

**Mitigation:** Extract `execute_instruction()` and `execute_range()` helpers. PRE/POST blocks execute via the same dispatch logic as main loop.

**Validation:** Test function with multiple PRE and POST blocks. Verify binding environment is correct for each.

### Risk 3: Float NaN/infinity checking misses edge cases
**Likelihood:** Medium
**Impact:** High (violates spec guarantee)

**Mitigation:** Check after every F64 arithmetic and comparison op. Use exhaustive test matrix: +0/-0, max/min finite values, operations that could overflow to infinity.

**Validation:** Property-based test with random F64 inputs. Any NaN or infinity result is an error.

### Risk 4: Stack or binding underflow not caught
**Likelihood:** Low (verification should prevent this)
**Impact:** High (panic)

**Mitigation:** Explicit bounds checks on pop, peek, and REF. Return RuntimeError, never panic.

**Validation:** Fuzzing with 1000 random programs. Zero panics required.

### Risk 5: CONST_EXT off-by-one in bit extraction
**Likelihood:** Medium
**Impact:** High (wrong constant values)

**Mitigation:** Careful bit manipulation. Extract low 48 bits from next instruction's bytes[1..7]. Test with known 64-bit values.

**Validation:** Test with i64::MIN, i64::MAX, u64::MAX, specific F64 values. Roundtrip through assembler (Phase 4) to verify.

## Definition of Done

- [ ] All Group A-F opcodes implemented
- [ ] All 9 example programs from EXAMPLES.md execute correctly
- [ ] Every opcode has 3+ tests (valid, edge, error)
- [ ] Fuzzer runs for 60 seconds with zero panics
- [ ] `cargo test -p nolang-vm` passes with zero warnings
- [ ] RuntimeError includes instruction index for all error types
- [ ] Float NaN/infinity never exist as values (all caught and rejected)
- [ ] Stack overflow checked on every push
- [ ] No `unwrap()` in production code (only in tests)
- [ ] All public types have doc comments
- [ ] Phase 2 gate from BUILD_ORDER.md: all acceptance tests pass

## Confidence Assessment

**Overall confidence: High (85%)**

**High confidence areas:**
- Foundation opcodes (CONST, BIND, REF, HALT): straightforward stack operations
- Arithmetic/comparison/logic: standard VM operations, well-understood
- Error handling: comprehensive RuntimeError enum, explicit checks everywhere

**Medium confidence areas:**
- MATCH/CASE execution flow: requires state tracking, but approach is sound
- PRE/POST execution: refactoring dispatch into helpers adds complexity but is manageable
- CONST_EXT bit manipulation: needs careful testing but logic is clear

**Low confidence areas:**
- None. All design questions have clear resolutions.

**Unknowns requiring validation:**
- Does wrapping arithmetic for integers match intended semantics? (Spec doesn't specify overflow behavior)
- Are there edge cases in TYPEOF where type tags don't match cleanly? (e.g., MAYBE vs VARIANT)

**Recommendation:** Proceed with implementation. The Group A-F sequencing minimizes risk by validating foundation before building on it. If MATCH/CASE state tracking proves too complex, we can revisit with a simpler (but less efficient) approach of linear scanning without jump-ahead optimization.
