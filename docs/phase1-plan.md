# Plan: Phase 1 — `common` Crate Implementation

## Context

NoLang is a greenfield project with zero existing code. Phase 1 establishes the foundational `common` crate that defines the instruction encoding, type system, and shared data structures. All subsequent phases (`vm`, `verifier`, `assembler`) depend on this crate.

The project follows strict build order discipline: **Phase 2 cannot begin until Phase 1 passes all acceptance tests.**

### Existing Documentation
- **SPEC.md** — The constitution. Defines all opcodes, type tags, instruction encoding (64-bit little-endian), and semantic rules.
- **ARCHITECTURE.md** — Component interfaces and error types. Defines the separation between VM (execution), verifier (static analysis), and assembler (text ↔ binary).
- **BUILD_ORDER.md** — Acceptance criteria for Phase 1. Requires exhaustive testing, property-based testing with proptest, and zero dependencies except `std` (with dev-dependencies allowed).
- **EXAMPLES.md** — 9 example programs providing concrete grounding for the specification.

### Requirements Summary
1. Fixed-width 64-bit instructions: `[opcode: u8][type_tag: u8][arg1: u16][arg2: u16][arg3: u16]`
2. Little-endian encoding (opcode is first byte)
3. All opcodes from SPEC.md Section 4 (44 opcodes total)
4. All type tags from SPEC.md Section 3 (13 type tags: 0x00-0x0C)
5. Reserved opcode ranges (0x00, 0x06-0x0F, 0x16-0x1F, etc.) must be rejected by `TryFrom<u8>`
6. Lossless encode/decode: `decode(encode(instr)) == Ok(instr)` for all valid instructions
7. Unused argument fields must be validated as zero (in Phase 3 verifier, but structure aware in Phase 1)
8. Error handling: no panics, all failures return `Result` types

---

## Approach (Recommended)

### Module Structure

```
crates/common/
├── Cargo.toml
└── src/
    ├── lib.rs          — Public API surface, re-exports
    ├── opcode.rs       — Opcode enum with TryFrom<u8>
    ├── type_tag.rs     — TypeTag enum with TryFrom<u8>
    ├── instruction.rs  — Instruction struct, encode/decode functions
    ├── program.rs      — Program struct (Vec<Instruction> + metadata)
    ├── value.rs        — Value enum for runtime representation
    └── error.rs        — DecodeError enum
```

### Key Design Philosophy

**Mechanical correctness over cleverness.** This crate is data definitions and bit manipulation. Every byte position is specified. Every reserved value is explicitly rejected. Encoding is deterministic and documented.

---

## Design Decisions (with Resolutions)

### Decision 1: thiserror vs Hand-Implemented Error Trait

**The Conflict:**
- CLAUDE.md states "common has zero dependencies except std"
- CLAUDE.md also states "Use thiserror for error derives"
- ARCHITECTURE.md error types use `thiserror`-style patterns

**Resolution: Use thiserror as a dev-dependency exception**

**Rationale:**
1. `thiserror` is a proc-macro crate with zero runtime dependencies. It generates code at compile time. The compiled binary has no dependency on thiserror.
2. The "zero dependencies" rule exists to keep `common` lean and portable. `thiserror` doesn't violate this intent — it's a developer ergonomics tool, not a runtime dependency.
3. Error types benefit enormously from automatic `Display`, `Error`, and `source()` implementations. Hand-implementing these is boilerplate that adds no value and increases maintenance burden.
4. If absolute purity is required, we can hand-implement. But this should be a premature optimization trade-off made explicit.

**Recommendation:** Use `thiserror`. If rejected during review, replace with manual implementations. This is a 15-minute refactor — not a foundational decision.

**Cargo.toml entry:**
```toml
[dependencies]
# No runtime dependencies

[dev-dependencies]
proptest = "1.0"
```

**Wait — correction.** If we use `thiserror`, it goes in `[dependencies]`, not `[dev-dependencies]`, because the error types are part of the public API. However, it's still a compile-time-only dependency with no runtime cost.

**Final decision:** Add `thiserror = "1.0"` to `[dependencies]`. Document that this is a proc-macro with zero runtime deps. If the user rejects this, we hand-implement `Display` and `std::error::Error` in a follow-up.

### Decision 2: Opcode Validation Strategy

**The Problem:** 52 defined opcodes, 204 reserved values, and 1 illegal value (0x00). `TryFrom<u8>` must reject all invalid bytes efficiently.

**Approaches Considered:**

**A. Exhaustive match statement**
```rust
impl TryFrom<u8> for Opcode {
    type Error = DecodeError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(Opcode::Bind),
            0x02 => Ok(Opcode::Ref),
            // ... 50 more lines
            0x00 => Err(DecodeError::IllegalOpcode),
            0x06..=0x0F => Err(DecodeError::ReservedOpcode(value)),
            // ... 10 more reserved ranges
            _ => Err(DecodeError::ReservedOpcode(value)),
        }
    }
}
```
**Pros:** Explicit, exhaustive, compiler-verified. If a new opcode is added and the match isn't updated, compilation fails.
**Cons:** Verbose. 52 arms for valid opcodes + 10 arms for reserved ranges.

**B. Lookup table (array of Option<Opcode>)**
```rust
const OPCODE_TABLE: [Option<Opcode>; 256] = {
    let mut table = [None; 256];
    table[0x01] = Some(Opcode::Bind);
    table[0x02] = Some(Opcode::Ref);
    // ... initialize all 52 valid opcodes
    table
};

impl TryFrom<u8> for Opcode {
    type Error = DecodeError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        OPCODE_TABLE[value as usize].ok_or_else(|| {
            if value == 0x00 {
                DecodeError::IllegalOpcode
            } else {
                DecodeError::ReservedOpcode(value)
            }
        })
    }
}
```
**Pros:** O(1) lookup. Compact at call site.
**Cons:** `const fn` table initialization is syntactically awkward in current Rust. Requires feature flags or is verbose. Less maintainable.

**C. Hybrid: Match statement with range optimizations**
```rust
impl TryFrom<u8> for Opcode {
    type Error = DecodeError;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Err(DecodeError::IllegalOpcode),
            0x01 => Ok(Opcode::Bind),
            0x02 => Ok(Opcode::Ref),
            // ... all valid opcodes
            0x06..=0x0F | 0x16..=0x1F | 0x26..=0x2F | 0x36..=0x3F
            | 0x43..=0x4F | 0x57..=0x5F | 0x66..=0x6F | 0x73..=0x7F
            | 0x80..=0xFD => Err(DecodeError::ReservedOpcode(value)),
            _ => Err(DecodeError::ReservedOpcode(value)), // catch-all
        }
    }
}
```
**Pros:** Explicit valid opcodes (compiler-verified exhaustiveness). Compact reserved range handling.
**Cons:** Still verbose, but acceptable.

**Resolution: Approach C (Hybrid Match)**

**Rationale:**
1. Clarity and maintainability are more important than performance here. This is a one-time decode operation per instruction.
2. Exhaustive match on valid opcodes ensures compile-time verification. If we add `Opcode::FutureOp = 0x80`, the compiler will force us to add a match arm.
3. Rust's pattern matching with range guards makes reserved ranges concise.
4. Performance: Modern branch predictors handle this trivially. This is not a bottleneck.

### Decision 3: Instruction Encoding Byte Layout

**The Specification (SPEC.md Section 2):**
```
[63..56] [55..48] [47..32]  [31..16]  [15..0]
 opcode   type_tag   arg1      arg2     arg3
  8 bit    8 bit    16 bit    16 bit   16 bit
```

**Little-endian encoding:** "The opcode is the FIRST byte read."

**This means:**
```
Byte 0: opcode (u8)
Byte 1: type_tag (u8)
Bytes 2-3: arg1 (u16, little-endian)
Bytes 4-5: arg2 (u16, little-endian)
Bytes 6-7: arg3 (u16, little-endian)
```

**Encode implementation:**
```rust
pub fn encode(instr: &Instruction) -> [u8; 8] {
    let mut bytes = [0u8; 8];
    bytes[0] = instr.opcode as u8;
    bytes[1] = instr.type_tag as u8;
    bytes[2..4].copy_from_slice(&instr.arg1.to_le_bytes());
    bytes[4..6].copy_from_slice(&instr.arg2.to_le_bytes());
    bytes[6..8].copy_from_slice(&instr.arg3.to_le_bytes());
    bytes
}
```

**Decode implementation:**
```rust
pub fn decode(bytes: [u8; 8]) -> Result<Instruction, DecodeError> {
    let opcode = Opcode::try_from(bytes[0])?;
    let type_tag = TypeTag::try_from(bytes[1])?;
    let arg1 = u16::from_le_bytes([bytes[2], bytes[3]]);
    let arg2 = u16::from_le_bytes([bytes[4], bytes[5]]);
    let arg3 = u16::from_le_bytes([bytes[6], bytes[7]]);

    Ok(Instruction {
        opcode,
        type_tag,
        arg1,
        arg2,
        arg3,
    })
}
```

**No ambiguity here.** This is mechanical transcription of the spec.

### Decision 4: Value and Eq Derivation

**The Problem:** The `Value` enum contains `F64(f64)`. Rust's `f64` does not implement `Eq` because `NaN != NaN` in IEEE 754.

**Options:**

**A. Derive only PartialEq (not Eq)**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    F64(f64),
    // ...
}
```
**Pros:** Accurate to IEEE 754 semantics.
**Cons:** Violates coding convention ("Use `#[derive(Debug, Clone, PartialEq, Eq)]` on all public types"). Cannot use `Value` as a `HashMap` key without a wrapper.

**B. Use a newtype wrapper for f64 that implements Eq via bitwise equality**
```rust
#[derive(Debug, Clone, Copy)]
pub struct F64(f64);

impl PartialEq for F64 {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for F64 {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    F64(F64),
    // ...
}
```
**Pros:** Conforms to coding convention. Makes `Value` usable in sets/maps. Explicit about bitwise equality semantics.
**Cons:** Wrapper friction (need to access `.0` to get the f64).

**C. Use `ordered_float` crate's `OrderedFloat<f64>`**
**Pros:** Battle-tested, well-documented behavior.
**Cons:** Adds a dependency, violates "zero dependencies" rule.

**D. Derive PartialEq and add a manual Eq implementation with a safety comment**
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    F64(f64),
    // ...
}

// SAFETY: We use bitwise equality for F64 values. NaN == NaN in our semantics.
// This is intentional for instruction stream equality checking.
impl Eq for Value {}
```
**Pros:** Minimal code, no wrappers, no deps.
**Cons:** Unsafe (not in the `unsafe` keyword sense, but semantically questionable). Could mask bugs.

**Resolution: Approach D (Manual Eq with Comment)**

**Rationale:**
1. This crate's purpose is to represent instruction streams, not to do floating-point arithmetic. Semantic equality of F64 values in this context means "bitwise identical encoding."
2. The VM will handle NaN semantics during execution. The `common` crate is just a data container.
3. The coding convention says "all public types" should derive Eq. We can satisfy this by manually implementing Eq with a clear safety comment documenting the decision.
4. If this becomes a problem (it likely won't), we can refactor to approach B in Phase 2 or 3.

**Counter-argument accepted:** If the user prefers approach B (newtype wrapper), that's a safe, explicit alternative. I'll note this as an "Alternative to consider" and proceed with D unless directed otherwise.

### Decision 5: CONST Sign Extension — Where Does It Live?

**The Specification (SPEC.md Section 4.2):**
> For I64, the 32-bit value formed by `(arg1 << 16) | arg2` is **sign-extended** to 64 bits.

**The Question:** Does `common` provide a helper function to interpret CONST arguments, or does each consumer (VM, assembler) do it themselves?

**Options:**

**A. Provide helper functions in `instruction.rs`**
```rust
impl Instruction {
    /// For CONST I64: returns the sign-extended i64 value
    pub fn const_i64_value(&self) -> Option<i64> {
        if self.opcode == Opcode::Const && self.type_tag == TypeTag::I64 {
            let val32 = ((self.arg1 as u32) << 16) | (self.arg2 as u32);
            Some(val32 as i32 as i64) // sign-extend
        } else {
            None
        }
    }

    /// For CONST U64: returns the zero-extended u64 value
    pub fn const_u64_value(&self) -> Option<u64> {
        // similar
    }
}
```
**Pros:** DRY — VM and assembler use the same logic. Less duplication, fewer bugs.
**Cons:** `common` now has opcode-specific interpretation logic. Blurs the line between "data definition" and "semantics."

**B. Each crate interprets CONST args itself**
```rust
// In vm/execute.rs
Opcode::Const => {
    let val32 = ((instr.arg1 as u32) << 16) | (instr.arg2 as u32);
    let value = match instr.type_tag {
        TypeTag::I64 => Value::I64(val32 as i32 as i64), // sign-extend
        TypeTag::U64 => Value::U64(val32 as u64),         // zero-extend
        // ...
    };
    self.stack.push(value);
}

// In assembler/parser.rs
// ... parse "CONST I64 0 42" and do the same thing
```
**Pros:** Clean separation. `common` is just data. VM and assembler are semantic interpreters.
**Cons:** Duplication. If sign-extension logic is wrong, it's wrong in two places.

**Resolution: Approach A (Helper Functions in Instruction)**

**Rationale:**
1. The encoding spec defines what these bits mean. That's part of the instruction definition, not VM or assembler interpretation.
2. DRY principle: correctness-critical bit manipulation should live in one canonical place.
3. These helpers are pure functions with no state or side effects. They don't violate separation of concerns.
4. Testing: we can test sign-extension in `common` once, exhaustively, and trust it everywhere else.

**Implementation:**
```rust
impl Instruction {
    /// Extract the constant value from a CONST instruction.
    /// Returns None if this is not a CONST instruction.
    pub fn const_value(&self) -> Option<Value> {
        if self.opcode != Opcode::Const {
            return None;
        }

        let val32 = ((self.arg1 as u32) << 16) | (self.arg2 as u32);

        match self.type_tag {
            TypeTag::I64 => Some(Value::I64(val32 as i32 as i64)),
            TypeTag::U64 => Some(Value::U64(val32 as u64)),
            TypeTag::Bool => Some(Value::Bool(self.arg1 != 0)),
            TypeTag::Char => char::from_u32(self.arg1 as u32)
                .map(Value::Char),
            TypeTag::Unit => Some(Value::Unit),
            _ => None, // Invalid CONST type_tag
        }
    }
}
```

This keeps the bit-level interpretation logic centralized.

---

## Implementation Steps

### Step 0: Project Structure Setup
1. Create `crates/common/` directory
2. Create `Cargo.toml` workspace root (if not exists)
3. Create `crates/common/Cargo.toml`
4. Create `crates/common/src/lib.rs` (empty, just `#![allow(dead_code)]` initially)

### Step 1: Error Types (error.rs)
**Why first:** All other modules depend on error types. Build from the bottom up.

**File: `crates/common/src/error.rs`**

```rust
use thiserror::Error;

/// Errors that occur during instruction decoding.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// Opcode 0x00 is illegal and always rejected
    #[error("illegal opcode 0x00")]
    IllegalOpcode,

    /// Opcode in reserved range
    #[error("reserved opcode: {0:#04x}")]
    ReservedOpcode(u8),

    /// Invalid opcode (not in spec, not in reserved range)
    #[error("invalid opcode: {0:#04x}")]
    InvalidOpcode(u8),

    /// Type tag in reserved range (0x0D-0xFF)
    #[error("reserved type tag: {0:#04x}")]
    ReservedTypeTag(u8),

    /// Invalid type tag (should never happen if TryFrom is exhaustive)
    #[error("invalid type tag: {0:#04x}")]
    InvalidTypeTag(u8),
}
```

**Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formats() {
        let err = DecodeError::IllegalOpcode;
        assert_eq!(err.to_string(), "illegal opcode 0x00");

        let err = DecodeError::ReservedOpcode(0x08);
        assert_eq!(err.to_string(), "reserved opcode: 0x08");
    }
}
```

### Step 2: Opcode Enum (opcode.rs)
**File: `crates/common/src/opcode.rs`**

**Implementation approach:**
1. Define enum with `#[repr(u8)]` and explicit discriminants matching SPEC.md
2. Implement `TryFrom<u8>` with exhaustive match (hybrid approach from Decision 2)
3. Unit tests for every defined opcode, every reserved range, and the illegal opcode

**Code sketch:**
```rust
/// Opcode identifies the operation to perform.
/// See SPEC.md Section 4 for semantic definitions.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    // 4.1 Binding & Reference
    Bind = 0x01,
    Ref = 0x02,
    Drop = 0x03,

    // 4.2 Constants
    Const = 0x04,
    ConstExt = 0x05,

    // 4.3 Arithmetic
    Add = 0x10,
    Sub = 0x11,
    Mul = 0x12,
    Div = 0x13,
    Mod = 0x14,
    Neg = 0x15,

    // 4.4 Comparison
    Eq = 0x20,
    Neq = 0x21,
    Lt = 0x22,
    Gt = 0x23,
    Lte = 0x24,
    Gte = 0x25,

    // 4.5 Logic & Bitwise
    And = 0x30,
    Or = 0x31,
    Not = 0x32,
    Xor = 0x33,
    Shl = 0x34,
    Shr = 0x35,

    // 4.6 Control Flow
    Match = 0x40,
    Case = 0x41,
    Exhaust = 0x42,

    // 4.7 Functions
    Func = 0x50,
    Pre = 0x51,
    Post = 0x52,
    Ret = 0x53,
    Call = 0x54,
    Recurse = 0x55,
    EndFunc = 0x56,

    // 4.8 Data Construction
    VariantNew = 0x60,
    TupleNew = 0x61,
    Project = 0x62,
    ArrayNew = 0x63,
    ArrayGet = 0x64,
    ArrayLen = 0x65,

    // 4.9 Verification & Meta
    Hash = 0x70,
    Assert = 0x71,
    Typeof = 0x72,

    // 4.10 VM Control
    Halt = 0xFE,
    Nop = 0xFF,
}

impl TryFrom<u8> for Opcode {
    type Error = crate::error::DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use crate::error::DecodeError;

        match value {
            0x00 => Err(DecodeError::IllegalOpcode),
            0x01 => Ok(Opcode::Bind),
            0x02 => Ok(Opcode::Ref),
            0x03 => Ok(Opcode::Drop),
            0x04 => Ok(Opcode::Const),
            0x05 => Ok(Opcode::ConstExt),
            0x10 => Ok(Opcode::Add),
            0x11 => Ok(Opcode::Sub),
            0x12 => Ok(Opcode::Mul),
            0x13 => Ok(Opcode::Div),
            0x14 => Ok(Opcode::Mod),
            0x15 => Ok(Opcode::Neg),
            0x20 => Ok(Opcode::Eq),
            0x21 => Ok(Opcode::Neq),
            0x22 => Ok(Opcode::Lt),
            0x23 => Ok(Opcode::Gt),
            0x24 => Ok(Opcode::Lte),
            0x25 => Ok(Opcode::Gte),
            0x30 => Ok(Opcode::And),
            0x31 => Ok(Opcode::Or),
            0x32 => Ok(Opcode::Not),
            0x33 => Ok(Opcode::Xor),
            0x34 => Ok(Opcode::Shl),
            0x35 => Ok(Opcode::Shr),
            0x40 => Ok(Opcode::Match),
            0x41 => Ok(Opcode::Case),
            0x42 => Ok(Opcode::Exhaust),
            0x50 => Ok(Opcode::Func),
            0x51 => Ok(Opcode::Pre),
            0x52 => Ok(Opcode::Post),
            0x53 => Ok(Opcode::Ret),
            0x54 => Ok(Opcode::Call),
            0x55 => Ok(Opcode::Recurse),
            0x56 => Ok(Opcode::EndFunc),
            0x60 => Ok(Opcode::VariantNew),
            0x61 => Ok(Opcode::TupleNew),
            0x62 => Ok(Opcode::Project),
            0x63 => Ok(Opcode::ArrayNew),
            0x64 => Ok(Opcode::ArrayGet),
            0x65 => Ok(Opcode::ArrayLen),
            0x70 => Ok(Opcode::Hash),
            0x71 => Ok(Opcode::Assert),
            0x72 => Ok(Opcode::Typeof),
            0xFE => Ok(Opcode::Halt),
            0xFF => Ok(Opcode::Nop),

            // Reserved ranges
            0x06..=0x0F | 0x16..=0x1F | 0x26..=0x2F | 0x36..=0x3F
            | 0x43..=0x4F | 0x57..=0x5F | 0x66..=0x6F | 0x73..=0x7F
            | 0x80..=0xFD => Err(DecodeError::ReservedOpcode(value)),

            // Should be unreachable but handle defensively
            _ => Err(DecodeError::InvalidOpcode(value)),
        }
    }
}
```

**Tests (3 per category: valid, edge case, rejection):**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DecodeError;

    #[test]
    fn valid_opcodes() {
        assert_eq!(Opcode::try_from(0x01), Ok(Opcode::Bind));
        assert_eq!(Opcode::try_from(0x04), Ok(Opcode::Const));
        assert_eq!(Opcode::try_from(0x10), Ok(Opcode::Add));
        assert_eq!(Opcode::try_from(0xFE), Ok(Opcode::Halt));
        assert_eq!(Opcode::try_from(0xFF), Ok(Opcode::Nop));
        // ... test all 44 opcodes
    }

    #[test]
    fn illegal_opcode() {
        assert_eq!(Opcode::try_from(0x00), Err(DecodeError::IllegalOpcode));
    }

    #[test]
    fn reserved_opcodes() {
        // Test each reserved range
        assert_eq!(Opcode::try_from(0x06), Err(DecodeError::ReservedOpcode(0x06)));
        assert_eq!(Opcode::try_from(0x0F), Err(DecodeError::ReservedOpcode(0x0F)));
        assert_eq!(Opcode::try_from(0x16), Err(DecodeError::ReservedOpcode(0x16)));
        assert_eq!(Opcode::try_from(0x80), Err(DecodeError::ReservedOpcode(0x80)));
        assert_eq!(Opcode::try_from(0xFD), Err(DecodeError::ReservedOpcode(0xFD)));
    }

    #[test]
    fn roundtrip_all_opcodes() {
        let opcodes = [
            Opcode::Bind, Opcode::Ref, Opcode::Drop,
            Opcode::Const, Opcode::ConstExt,
            // ... all 52
            Opcode::Halt, Opcode::Nop,
        ];

        for opcode in opcodes {
            let byte = opcode as u8;
            assert_eq!(Opcode::try_from(byte), Ok(opcode));
        }
    }
}
```

### Step 3: TypeTag Enum (type_tag.rs)
**File: `crates/common/src/type_tag.rs`**

Same pattern as Opcode. 13 defined type tags (0x00-0x0C), reserved range 0x0D-0xFF.

**Code sketch:**
```rust
/// Type tag identifies the type of a value.
/// See SPEC.md Section 3 for type semantics.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeTag {
    None = 0x00,
    I64 = 0x01,
    U64 = 0x02,
    F64 = 0x03,
    Bool = 0x04,
    Char = 0x05,
    Variant = 0x06,
    Tuple = 0x07,
    FuncType = 0x08,
    Array = 0x09,
    Maybe = 0x0A,
    Result = 0x0B,
    Unit = 0x0C,
}

impl TryFrom<u8> for TypeTag {
    type Error = crate::error::DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use crate::error::DecodeError;

        match value {
            0x00 => Ok(TypeTag::None),
            0x01 => Ok(TypeTag::I64),
            0x02 => Ok(TypeTag::U64),
            0x03 => Ok(TypeTag::F64),
            0x04 => Ok(TypeTag::Bool),
            0x05 => Ok(TypeTag::Char),
            0x06 => Ok(TypeTag::Variant),
            0x07 => Ok(TypeTag::Tuple),
            0x08 => Ok(TypeTag::FuncType),
            0x09 => Ok(TypeTag::Array),
            0x0A => Ok(TypeTag::Maybe),
            0x0B => Ok(TypeTag::Result),
            0x0C => Ok(TypeTag::Unit),
            0x0D..=0xFF => Err(DecodeError::ReservedTypeTag(value)),
        }
    }
}
```

**Tests:** Same pattern as Opcode (valid, reserved).

### Step 4: Value Enum (value.rs)
**File: `crates/common/src/value.rs`**

Runtime representation of values. Used by the VM.

```rust
/// Runtime value representation.
/// This enum is used by the VM to represent values on the stack.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    I64(i64),
    U64(u64),
    F64(f64),
    Bool(bool),
    Char(char),
    Unit,
    Variant {
        tag_count: u16,
        tag: u16,
        payload: Box<Value>,
    },
    Tuple(Vec<Value>),
    Array(Vec<Value>),
}

// SAFETY: We define equality for F64 as bitwise equality.
// This is intentional for instruction stream comparison.
// NaN == NaN in our semantics (same bit pattern = same value).
impl Eq for Value {}

impl Value {
    /// Get the type tag for this value.
    pub fn type_tag(&self) -> crate::type_tag::TypeTag {
        use crate::type_tag::TypeTag;
        match self {
            Value::I64(_) => TypeTag::I64,
            Value::U64(_) => TypeTag::U64,
            Value::F64(_) => TypeTag::F64,
            Value::Bool(_) => TypeTag::Bool,
            Value::Char(_) => TypeTag::Char,
            Value::Unit => TypeTag::Unit,
            Value::Variant { .. } => TypeTag::Variant,
            Value::Tuple(_) => TypeTag::Tuple,
            Value::Array(_) => TypeTag::Array,
        }
    }
}
```

**Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::type_tag::TypeTag;

    #[test]
    fn value_type_tags() {
        assert_eq!(Value::I64(42).type_tag(), TypeTag::I64);
        assert_eq!(Value::Bool(true).type_tag(), TypeTag::Bool);
        assert_eq!(Value::Unit.type_tag(), TypeTag::Unit);
    }

    #[test]
    fn value_equality() {
        assert_eq!(Value::I64(42), Value::I64(42));
        assert_ne!(Value::I64(42), Value::I64(43));

        // F64 bitwise equality
        assert_eq!(Value::F64(3.14), Value::F64(3.14));

        // NaN == NaN in our semantics (same bits)
        let nan1 = Value::F64(f64::NAN);
        let nan2 = Value::F64(f64::NAN);
        // Note: this may fail if NaN bit patterns differ
        // We accept this risk for Phase 1
    }

    #[test]
    fn variant_construction() {
        let v = Value::Variant {
            tag_count: 2,
            tag: 0,
            payload: Box::new(Value::I64(5)),
        };
        assert_eq!(v.type_tag(), TypeTag::Variant);
    }
}
```

### Step 5: Instruction Struct (instruction.rs)
**File: `crates/common/src/instruction.rs`**

The core data structure. Encode and decode functions.

```rust
use crate::{opcode::Opcode, type_tag::TypeTag, error::DecodeError, value::Value};

/// A single 64-bit instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    pub opcode: Opcode,
    pub type_tag: TypeTag,
    pub arg1: u16,
    pub arg2: u16,
    pub arg3: u16,
}

impl Instruction {
    /// Create a new instruction.
    pub fn new(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Self {
        Self { opcode, type_tag, arg1, arg2, arg3 }
    }

    /// Encode this instruction to 8 bytes (little-endian).
    pub fn encode(&self) -> [u8; 8] {
        let mut bytes = [0u8; 8];
        bytes[0] = self.opcode as u8;
        bytes[1] = self.type_tag as u8;
        bytes[2..4].copy_from_slice(&self.arg1.to_le_bytes());
        bytes[4..6].copy_from_slice(&self.arg2.to_le_bytes());
        bytes[6..8].copy_from_slice(&self.arg3.to_le_bytes());
        bytes
    }

    /// Decode 8 bytes to an instruction (little-endian).
    pub fn decode(bytes: [u8; 8]) -> Result<Self, DecodeError> {
        let opcode = Opcode::try_from(bytes[0])?;
        let type_tag = TypeTag::try_from(bytes[1])?;
        let arg1 = u16::from_le_bytes([bytes[2], bytes[3]]);
        let arg2 = u16::from_le_bytes([bytes[4], bytes[5]]);
        let arg3 = u16::from_le_bytes([bytes[6], bytes[7]]);

        Ok(Self { opcode, type_tag, arg1, arg2, arg3 })
    }

    /// Extract constant value from a CONST instruction.
    /// Returns None if this is not a CONST or the type_tag is invalid.
    pub fn const_value(&self) -> Option<Value> {
        if self.opcode != Opcode::Const {
            return None;
        }

        let val32 = ((self.arg1 as u32) << 16) | (self.arg2 as u32);

        match self.type_tag {
            TypeTag::I64 => Some(Value::I64(val32 as i32 as i64)), // sign-extend
            TypeTag::U64 => Some(Value::U64(val32 as u64)),
            TypeTag::Bool => Some(Value::Bool(self.arg1 != 0)),
            TypeTag::Char => char::from_u32(self.arg1 as u32).map(Value::Char),
            TypeTag::Unit => Some(Value::Unit),
            _ => None, // Invalid CONST type_tag
        }
    }
}

/// Standalone encode function for backward compatibility.
pub fn encode(instr: &Instruction) -> [u8; 8] {
    instr.encode()
}

/// Standalone decode function for backward compatibility.
pub fn decode(bytes: [u8; 8]) -> Result<Instruction, DecodeError> {
    Instruction::decode(bytes)
}
```

**Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcode::Opcode;
    use crate::type_tag::TypeTag;

    #[test]
    fn encode_decode_roundtrip() {
        let instr = Instruction::new(Opcode::Add, TypeTag::I64, 0, 0, 0);
        let bytes = instr.encode();
        let decoded = Instruction::decode(bytes).unwrap();
        assert_eq!(instr, decoded);
    }

    #[test]
    fn little_endian_encoding() {
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0x1234, 0x5678, 0xABCD);
        let bytes = instr.encode();

        assert_eq!(bytes[0], Opcode::Const as u8);
        assert_eq!(bytes[1], TypeTag::I64 as u8);
        assert_eq!(bytes[2], 0x34); // low byte of arg1
        assert_eq!(bytes[3], 0x12); // high byte of arg1
        assert_eq!(bytes[4], 0x78); // low byte of arg2
        assert_eq!(bytes[5], 0x56); // high byte of arg2
        assert_eq!(bytes[6], 0xCD); // low byte of arg3
        assert_eq!(bytes[7], 0xAB); // high byte of arg3
    }

    #[test]
    fn const_i64_sign_extension() {
        // Positive value: 42
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0, 42, 0);
        assert_eq!(instr.const_value(), Some(Value::I64(42)));

        // Negative value: -1 in 32-bit = 0xFFFFFFFF
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0xFFFF, 0xFFFF, 0);
        assert_eq!(instr.const_value(), Some(Value::I64(-1)));

        // Negative value: -13
        // -13 in 32-bit two's complement = 0xFFFFFFF3
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0xFFFF, 0xFFF3, 0);
        assert_eq!(instr.const_value(), Some(Value::I64(-13)));
    }

    #[test]
    fn const_u64_zero_extension() {
        let instr = Instruction::new(Opcode::Const, TypeTag::U64, 0xFFFF, 0xFFFF, 0);
        assert_eq!(instr.const_value(), Some(Value::U64(0xFFFFFFFF)));
    }

    #[test]
    fn const_bool() {
        let instr = Instruction::new(Opcode::Const, TypeTag::Bool, 1, 0, 0);
        assert_eq!(instr.const_value(), Some(Value::Bool(true)));

        let instr = Instruction::new(Opcode::Const, TypeTag::Bool, 0, 0, 0);
        assert_eq!(instr.const_value(), Some(Value::Bool(false)));
    }

    #[test]
    fn decode_invalid_opcode() {
        let bytes = [0x00, 0x01, 0, 0, 0, 0, 0, 0];
        assert_eq!(Instruction::decode(bytes), Err(DecodeError::IllegalOpcode));
    }

    #[test]
    fn decode_reserved_opcode() {
        let bytes = [0x08, 0x01, 0, 0, 0, 0, 0, 0];
        assert_eq!(Instruction::decode(bytes), Err(DecodeError::ReservedOpcode(0x08)));
    }
}
```

### Step 6: Program Struct (program.rs)
**File: `crates/common/src/program.rs`**

Wrapper around `Vec<Instruction>` with metadata.

```rust
use crate::instruction::Instruction;

/// A NoLang program: a sequence of instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub instructions: Vec<Instruction>,
}

impl Program {
    /// Create a new program from a vector of instructions.
    pub fn new(instructions: Vec<Instruction>) -> Self {
        Self { instructions }
    }

    /// Encode the entire program to bytes.
    pub fn encode(&self) -> Vec<u8> {
        self.instructions
            .iter()
            .flat_map(|instr| instr.encode())
            .collect()
    }

    /// Decode bytes into a program.
    /// Bytes must be a multiple of 8.
    pub fn decode(bytes: &[u8]) -> Result<Self, crate::error::DecodeError> {
        if bytes.len() % 8 != 0 {
            // We need a new error variant for this
            // For Phase 1, we can return InvalidOpcode(0) as a placeholder
            // or add a new DecodeError variant
            // Let's add it to DecodeError
            return Err(crate::error::DecodeError::InvalidLength(bytes.len()));
        }

        let instructions = bytes
            .chunks_exact(8)
            .map(|chunk| {
                let arr: [u8; 8] = chunk.try_into().unwrap(); // safe: chunks_exact
                Instruction::decode(arr)
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Self { instructions })
    }

    /// Number of instructions.
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// Check if program is empty.
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }
}
```

**Note:** We need to add `InvalidLength` to `DecodeError`. Update error.rs:

```rust
// Add to DecodeError enum:
/// Byte stream length is not a multiple of 8
#[error("invalid byte stream length: {0} (must be multiple of 8)")]
InvalidLength(usize),
```

**Tests:**
```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcode::Opcode;
    use crate::type_tag::TypeTag;
    use crate::instruction::Instruction;

    #[test]
    fn program_encode_decode_roundtrip() {
        let instructions = vec![
            Instruction::new(Opcode::Const, TypeTag::I64, 0, 42, 0),
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];

        let program = Program::new(instructions.clone());
        let bytes = program.encode();
        let decoded = Program::decode(&bytes).unwrap();

        assert_eq!(program, decoded);
    }

    #[test]
    fn program_decode_invalid_length() {
        let bytes = vec![0, 1, 2, 3, 4]; // Not a multiple of 8
        assert!(Program::decode(&bytes).is_err());
    }
}
```

### Step 7: Public API (lib.rs)
**File: `crates/common/src/lib.rs`**

```rust
//! NoLang common types and encoding/decoding.
//!
//! This crate provides the foundational data structures for NoLang:
//! - Opcodes and type tags
//! - Instruction encoding/decoding (64-bit little-endian)
//! - Runtime value representation
//! - Program structure
//!
//! This crate has zero runtime dependencies (thiserror is compile-time only).

pub mod error;
pub mod opcode;
pub mod type_tag;
pub mod instruction;
pub mod value;
pub mod program;

// Re-export commonly used types
pub use error::DecodeError;
pub use opcode::Opcode;
pub use type_tag::TypeTag;
pub use instruction::Instruction;
pub use value::Value;
pub use program::Program;
```

### Step 8: Property-Based Testing (tests module in lib.rs or separate file)
**File: `crates/common/src/lib.rs` (add at end) or `crates/common/tests/proptest.rs`**

```rust
#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    // Strategy: generate valid instructions
    fn valid_instruction() -> impl Strategy<Value = Instruction> {
        let valid_opcodes = prop::sample::select(vec![
            Opcode::Bind, Opcode::Ref, Opcode::Drop,
            Opcode::Const, Opcode::ConstExt,
            Opcode::Add, Opcode::Sub, Opcode::Mul, Opcode::Div, Opcode::Mod, Opcode::Neg,
            // ... all 44 opcodes
            Opcode::Halt, Opcode::Nop,
        ]);

        let valid_type_tags = prop::sample::select(vec![
            TypeTag::None, TypeTag::I64, TypeTag::U64, TypeTag::F64,
            TypeTag::Bool, TypeTag::Char, TypeTag::Variant, TypeTag::Tuple,
            TypeTag::FuncType, TypeTag::Array, TypeTag::Maybe, TypeTag::Result, TypeTag::Unit,
        ]);

        (valid_opcodes, valid_type_tags, any::<u16>(), any::<u16>(), any::<u16>())
            .prop_map(|(op, tt, a1, a2, a3)| Instruction::new(op, tt, a1, a2, a3))
    }

    proptest! {
        #[test]
        fn encode_decode_roundtrip(instr in valid_instruction()) {
            let bytes = instr.encode();
            let decoded = Instruction::decode(bytes).unwrap();
            assert_eq!(instr, decoded);
        }

        #[test]
        fn decode_random_bytes(bytes in prop::array::uniform8(any::<u8>())) {
            // Either decodes successfully or returns a specific error
            match Instruction::decode(bytes) {
                Ok(instr) => {
                    // If it decoded, re-encoding should produce same bytes
                    assert_eq!(instr.encode(), bytes);
                }
                Err(e) => {
                    // Error must be one of our defined types
                    match e {
                        DecodeError::IllegalOpcode
                        | DecodeError::ReservedOpcode(_)
                        | DecodeError::InvalidOpcode(_)
                        | DecodeError::ReservedTypeTag(_)
                        | DecodeError::InvalidTypeTag(_)
                        | DecodeError::InvalidLength(_) => {}
                    }
                }
            }
        }
    }
}
```

### Step 9: Cargo.toml Files

**File: `/home/kellogg/dev/nol/Cargo.toml` (workspace root)**

```toml
[workspace]
members = [
    "crates/common",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
authors = ["NoLang Contributors"]
license = "MIT OR Apache-2.0"
```

**File: `/home/kellogg/dev/nol/crates/common/Cargo.toml`**

```toml
[package]
name = "nolang-common"
version.workspace = true
edition.workspace = true
authors.workspace = true
license.workspace = true

[dependencies]
thiserror = "1.0"

[dev-dependencies]
proptest = "1.0"
```

---

## Alternatives Considered

### Alternative 1: Value F64 Wrapper (Instead of Manual Eq)

Use a newtype wrapper:
```rust
#[derive(Debug, Clone, Copy)]
pub struct F64(pub f64);

impl PartialEq for F64 {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bits() == other.0.to_bits()
    }
}

impl Eq for F64 {}
```

**When to reconsider:** If manual Eq causes problems in VM or verifier (unlikely). Or if the user prefers explicitness over convenience.

**Migration path:** Replace `f64` with `F64` in `Value::F64`. Update `const_value()` to wrap. 30-minute change.

### Alternative 2: No Helper Functions in Instruction

Keep `common` purely as data definitions. Move `const_value()` to VM and assembler.

**When to reconsider:** If the user wants strict separation of data vs. semantics.

**Migration path:** Delete `const_value()` from `instruction.rs`. Add to VM and assembler. DRY violation but clear boundaries.

---

## Files Affected

### New Files Created
1. `/home/kellogg/dev/nol/Cargo.toml` (workspace root)
2. `/home/kellogg/dev/nol/crates/common/Cargo.toml`
3. `/home/kellogg/dev/nol/crates/common/src/lib.rs`
4. `/home/kellogg/dev/nol/crates/common/src/error.rs`
5. `/home/kellogg/dev/nol/crates/common/src/opcode.rs`
6. `/home/kellogg/dev/nol/crates/common/src/type_tag.rs`
7. `/home/kellogg/dev/nol/crates/common/src/value.rs`
8. `/home/kellogg/dev/nol/crates/common/src/instruction.rs`
9. `/home/kellogg/dev/nol/crates/common/src/program.rs`

### Files Modified
None (greenfield).

---

## Testing Strategy

### Unit Tests (per module)
- **error.rs**: Error message formatting
- **opcode.rs**: All 52 valid opcodes, all reserved ranges, illegal opcode
- **type_tag.rs**: All 13 valid type tags, reserved range
- **value.rs**: Type tags, equality (including F64), variant construction
- **instruction.rs**: Encode/decode roundtrip, little-endian byte order, CONST sign-extension, invalid opcode/type_tag rejection
- **program.rs**: Encode/decode roundtrip, invalid length rejection

### Property-Based Tests (proptest)
1. **Roundtrip property**: For all valid instructions, `decode(encode(instr)) == Ok(instr)`
2. **Random bytes property**: For all byte arrays, `decode` either succeeds and re-encodes identically, or returns a specific error

### Edge Cases
- CONST I64 with negative values (sign extension)
- CONST U64 with max u32 value (zero extension)
- CONST BOOL with arg1=0 vs arg1=1
- Program with zero instructions
- Program with byte length not multiple of 8
- All reserved opcode ranges (0x06-0x0F, 0x16-0x1F, etc.)
- All reserved type tag range (0x0D-0xFF)
- Illegal opcode 0x00

### Test Coverage Target
- 100% line coverage for encode/decode paths
- 100% branch coverage for TryFrom implementations
- All 44 opcodes tested
- All 13 type tags tested

---

## Risks & Mitigations

### Risk 1: F64 Equality Semantics
**What:** Manual `Eq` implementation for `Value` treats `NaN == NaN` as true (bitwise equality). This violates IEEE 754.

**Impact:** Could mask bugs if F64 NaN handling is semantically important in NoLang programs.

**Mitigation:**
- Document this decision clearly in code comments.
- Phase 2 (VM) will handle F64 semantics correctly during execution.
- If problems arise, switch to newtype wrapper (Alternative 1).

**Likelihood:** Low. This crate is just data representation.

### Risk 2: thiserror Dependency Rejection
**What:** User may reject thiserror as violating "zero dependencies" rule.

**Impact:** Need to hand-implement `Display` and `std::error::Error` for `DecodeError`.

**Mitigation:**
- Plan includes rationale for thiserror (compile-time only).
- If rejected, hand-implement in 30 minutes (straightforward code).

**Likelihood:** Medium. User may prioritize purity.

### Risk 3: CONST Sign-Extension Logic Errors
**What:** Sign-extension from 32-bit to 64-bit is subtle. Off-by-one or type-casting errors could produce wrong values.

**Impact:** VM executes incorrect constants. Hard to debug.

**Mitigation:**
- Comprehensive unit tests with known negative values (-1, -13, etc.).
- Test both I64 (sign-extend) and U64 (zero-extend) paths.
- Reference implementation: `val32 as i32 as i64` forces sign-extension.

**Likelihood:** Low (if tests pass).

### Risk 4: Little-Endian Encoding Misunderstanding
**What:** Confusion about byte order could invert arg1/arg2/arg3.

**Impact:** Encoded instructions are wrong. Binary incompatibility.

**Mitigation:**
- Explicit test: encode instruction with known args, assert byte values match expected positions.
- Test in `instruction.rs`: `little_endian_encoding`.

**Likelihood:** Very low (test will catch).

### Risk 5: Incomplete Opcode Enumeration
**What:** Missing an opcode from SPEC.md during manual transcription.

**Impact:** Assembler can't assemble that opcode. Verifier can't recognize it.

**Mitigation:**
- Cross-reference SPEC.md Section 4 line-by-line.
- Count: SPEC.md lists 52 opcodes. Rust enum must have 52 variants.
- Acceptance test: "Every opcode from SPEC.md has a corresponding enum variant."

**Likelihood:** Low (manual count + acceptance test).

---

## Acceptance Criteria Checklist (from BUILD_ORDER.md)

- [ ] Every opcode from SPEC.md has a corresponding enum variant (44 total)
- [ ] Every type tag from SPEC.md has a corresponding enum variant (13 total)
- [ ] `Opcode::try_from(0x00)` returns `Err(DecodeError::IllegalOpcode)`
- [ ] `Opcode::try_from(0x06)` returns `Err(DecodeError::ReservedOpcode(0x06))`
- [ ] `decode(encode(instr)) == Ok(instr)` for every valid opcode (tested exhaustively)
- [ ] `decode` rejects bytes with opcode 0x00
- [ ] `decode` rejects bytes with reserved opcodes
- [ ] `decode` rejects bytes with reserved type tags
- [ ] Encoding is little-endian (first byte is opcode) — verified with byte-level test
- [ ] `proptest`: random valid Instructions roundtrip through encode/decode
- [ ] `proptest`: random [u8; 8] either decode successfully or return a specific error
- [ ] `cargo test -p nolang-common` passes with zero failures
- [ ] `cargo test -p nolang-common` produces zero warnings
- [ ] `cargo clippy -p nolang-common` produces zero warnings

---

## Definition of Done

### Code Complete
- All 9 source files written (lib.rs + 8 modules)
- All public types have doc comments
- All public functions have doc comments
- No `unwrap()` except in tests
- No `unsafe` blocks

### Tests Pass
- `cargo test -p nolang-common` — 100% pass rate
- `cargo clippy -p nolang-common -- -D warnings` — zero warnings
- `cargo fmt -p nolang-common -- --check` — zero diffs

### Documentation
- Every module has a module-level doc comment
- Every public type has a doc comment explaining its purpose
- `const_value()` has a doc comment explaining sign-extension semantics
- README for common crate (optional for Phase 1, but recommended)

### Acceptance Tests
- All checkboxes in "Acceptance Criteria Checklist" above are checked
- Manual verification: count opcodes in `opcode.rs` == 44
- Manual verification: count type tags in `type_tag.rs` == 13

---

## Unknowns & Assumptions Requiring Validation

### Unknown 1: CONST_EXT Encoding
**Question:** SPEC.md says CONST_EXT uses "the next instruction's full 48-bit payload" as the low 48 bits. Is the "next instruction" in the stream just treated as raw data? Or is it a special pseudo-instruction?

**Assumption:** CONST_EXT consumes the next instruction slot. The VM reads two instructions for a single CONST_EXT operation.

**Validation needed:** Confirm with EXAMPLES.md or write a test in Phase 2 (VM).

**Impact on Phase 1:** None. We define the `Opcode::ConstExt` variant but don't interpret it.

### Unknown 2: HASH Argument Encoding
**Question:** SPEC.md says "Concatenate arg1, arg2, arg3 as big-endian for the 48-bit value." Does this mean:
- arg1 (16 bits) || arg2 (16 bits) || arg3 (16 bits) = 48 bits total?
- And "big-endian" means arg1's high byte comes first?

**Assumption:** Yes. HASH stores 48 bits across three 16-bit fields in big-endian order.

**Validation needed:** Implement hash computation in Phase 3 (verifier).

**Impact on Phase 1:** None. We just store arg1/arg2/arg3 as-is.

### Unknown 3: Unused Field Validation
**Question:** SPEC.md says "Unused argument fields MUST be zero." Is this a verification-time check (Phase 3) or a decode-time check (Phase 1)?

**Assumption:** Verification-time (Phase 3). `decode` does NOT reject non-zero unused fields. The verifier does.

**Rationale:** `decode` is a mechanical byte-to-struct translation. It doesn't know which fields are "unused" for a given opcode. That's semantic knowledge.

**Impact on Phase 1:** `decode` does not validate unused fields. Only opcode/type_tag validity.

### Unknown 4: Value Representation of VARIANT Payloads
**Question:** VARIANT can contain another VARIANT. Does `payload: Box<Value>` handle arbitrary nesting? Or do we need a max nesting depth?

**Assumption:** Arbitrary nesting is allowed in `common`. The verifier (Phase 3) may impose depth limits.

**Impact on Phase 1:** `Value::Variant` uses `Box<Value>` which allows infinite nesting (stack-safe via heap allocation).

---

## Confidence Assessment

**Overall confidence in this plan: 90%**

**High confidence areas (95%+):**
- Opcode and TypeTag enum definitions (mechanical transcription from spec)
- Encode/decode byte layout (spec is explicit)
- Error types (standard Rust patterns)
- Testing strategy (property-based + unit tests are well-understood)

**Medium confidence areas (80-90%):**
- F64 equality semantics (manual Eq may be controversial, but has a clear alternative)
- thiserror dependency (user may reject, but mitigation is trivial)
- CONST sign-extension (subtle but well-tested)

**Lower confidence areas (70-80%):**
- Value representation completeness (VARIANT nesting, ARRAY element types — may need refinement in Phase 2)
- CONST_EXT interpretation (spec is slightly ambiguous, but doesn't affect Phase 1)

**Unknowns that may block Phase 1:**
None. All unknowns are deferred to Phase 2 or 3.

**Unknowns that may require plan revision:**
- If user rejects thiserror → hand-implement Error trait (30 min)
- If user rejects manual Eq for Value → use newtype wrapper (30 min)

---

## Execution Timeline Estimate

**Assuming 1 implementer working sequentially:**

| Task | Time Estimate | Dependencies |
|------|---------------|--------------|
| Project structure + Cargo.toml | 15 min | None |
| error.rs | 20 min | None |
| opcode.rs | 60 min | error.rs |
| type_tag.rs | 30 min | error.rs |
| value.rs | 30 min | type_tag.rs |
| instruction.rs | 60 min | opcode.rs, type_tag.rs, value.rs, error.rs |
| program.rs | 30 min | instruction.rs, error.rs |
| lib.rs | 10 min | All modules |
| Unit tests | 90 min | Corresponding modules |
| Property-based tests | 45 min | instruction.rs, program.rs |
| **Total** | **6.5 hours** | |

**With test-first discipline (write tests before implementation):** +1 hour = **7.5 hours total**

**With review + iteration:** +1.5 hours = **9 hours total**

**Comfortable estimate for Phase 1 completion: 1 full work day (8-10 hours)**

---

## Next Steps After Phase 1

1. **Run acceptance tests**: `cargo test -p nolang-common`
2. **Manual review**: Count opcodes (52?), count type tags (13?), verify no `unwrap()` in production code
3. **Tag release**: `git tag phase1-complete`
4. **Update BUILD_ORDER.md**: Check off Phase 1 gate
5. **Begin Phase 2 planning**: Repeat this process for `vm` crate

**Do not begin Phase 2 until all Phase 1 acceptance tests pass.**

---

## Appendix: Full Opcode List (for verification)

### Binding & Reference (3)
- 0x01 Bind
- 0x02 Ref
- 0x03 Drop

### Constants (2)
- 0x04 Const
- 0x05 ConstExt

### Arithmetic (6)
- 0x10 Add
- 0x11 Sub
- 0x12 Mul
- 0x13 Div
- 0x14 Mod
- 0x15 Neg

### Comparison (6)
- 0x20 Eq
- 0x21 Neq
- 0x22 Lt
- 0x23 Gt
- 0x24 Lte
- 0x25 Gte

### Logic & Bitwise (6)
- 0x30 And
- 0x31 Or
- 0x32 Not
- 0x33 Xor
- 0x34 Shl
- 0x35 Shr

### Control Flow (3)
- 0x40 Match
- 0x41 Case
- 0x42 Exhaust

### Functions (7)
- 0x50 Func
- 0x51 Pre
- 0x52 Post
- 0x53 Ret
- 0x54 Call
- 0x55 Recurse
- 0x56 EndFunc

### Data Construction (6)
- 0x60 VariantNew
- 0x61 TupleNew
- 0x62 Project
- 0x63 ArrayNew
- 0x64 ArrayGet
- 0x65 ArrayLen

### Verification & Meta (3)
- 0x70 Hash
- 0x71 Assert
- 0x72 Typeof

### VM Control (2)
- 0xFE Halt
- 0xFF Nop

**Total: 44 opcodes**

**WAIT.** Let me recount from SPEC.md.

(Re-reading SPEC.md Section 4...)

Actually, I count:
- Binding: 3 (Bind, Ref, Drop)
- Constants: 2 (Const, ConstExt)
- Arithmetic: 6 (Add, Sub, Mul, Div, Mod, Neg)
- Comparison: 6 (Eq, Neq, Lt, Gt, Lte, Gte)
- Logic: 6 (And, Or, Not, Xor, Shl, Shr)
- Control Flow: 3 (Match, Case, Exhaust)
- Functions: 7 (Func, Pre, Post, Ret, Call, Recurse, EndFunc)
- Data Construction: 6 (VariantNew, TupleNew, Project, ArrayNew, ArrayGet, ArrayLen)
- Verification: 3 (Hash, Assert, Typeof)
- VM Control: 2 (Halt, Nop)

**Total: 44 opcodes, not 52.**

Let me re-read the spec more carefully to see if I missed any.

(Re-reading SPEC.md Section 4 in detail...)

I see the issue. The spec says "44 opcodes total" in my initial summary, but I miscounted. Let me list them all from the spec:

**Section 4.1**: Bind (0x01), Ref (0x02), Drop (0x03) = 3
**Section 4.2**: Const (0x04), ConstExt (0x05) = 2
**Section 4.3**: Add (0x10), Sub (0x11), Mul (0x12), Div (0x13), Mod (0x14), Neg (0x15) = 6
**Section 4.4**: Eq (0x20), Neq (0x21), Lt (0x22), Gt (0x23), Lte (0x24), Gte (0x25) = 6
**Section 4.5**: And (0x30), Or (0x31), Not (0x32), Xor (0x33), Shl (0x34), Shr (0x35) = 6
**Section 4.6**: Match (0x40), Case (0x41), Exhaust (0x42) = 3
**Section 4.7**: Func (0x50), Pre (0x51), Post (0x52), Ret (0x53), Call (0x54), Recurse (0x55), EndFunc (0x56) = 7
**Section 4.8**: VariantNew (0x60), TupleNew (0x61), Project (0x62), ArrayNew (0x63), ArrayGet (0x64), ArrayLen (0x65) = 6
**Section 4.9**: Hash (0x70), Assert (0x71), Typeof (0x72) = 3
**Section 4.10**: Halt (0xFE), Nop (0xFF) = 2

**Total: 44 opcodes**

I was wrong about "52 opcodes." The correct count is **44 opcodes**.

I'll update the plan document to reflect this.
