//! Opcode definitions for the NoLang instruction set.
//!
//! See SPEC.md Section 4 for semantic definitions of each opcode.

use crate::error::DecodeError;

/// Identifies the operation to perform.
///
/// Each variant corresponds to an opcode defined in SPEC.md Section 4.
/// The `#[repr(u8)]` attribute ensures each variant has a stable byte value.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Opcode {
    // 4.1 Binding & Reference
    /// Pop top of stack, create a new binding at de Bruijn index 0.
    Bind = 0x01,
    /// Push the value at de Bruijn index `arg1` onto the stack.
    Ref = 0x02,
    /// Remove the most recent binding (index 0).
    Drop = 0x03,

    // 4.2 Constants
    /// Push a constant value. Type and value determined by type_tag and args.
    Const = 0x04,
    /// Extended constant (64-bit). Consumes the next instruction slot.
    ConstExt = 0x05,

    // 4.3 Arithmetic
    /// Pop two values, push their sum.
    Add = 0x10,
    /// Pop two values, push (second_popped - first_popped).
    Sub = 0x11,
    /// Pop two values, push their product.
    Mul = 0x12,
    /// Pop two values, push quotient. Division by zero is a runtime error.
    Div = 0x13,
    /// Pop two values, push remainder. I64/U64 only.
    Mod = 0x14,
    /// Pop one value, push its negation. I64/F64 only.
    Neg = 0x15,

    // 4.4 Comparison
    /// Pop two values, push BOOL (1 if equal).
    Eq = 0x20,
    /// Pop two values, push BOOL (1 if not equal).
    Neq = 0x21,
    /// Pop two, push BOOL (1 if second_popped < first_popped).
    Lt = 0x22,
    /// Pop two, push BOOL (1 if second_popped > first_popped).
    Gt = 0x23,
    /// Pop two, push BOOL (1 if second_popped <= first_popped).
    Lte = 0x24,
    /// Pop two, push BOOL (1 if second_popped >= first_popped).
    Gte = 0x25,

    // 4.5 Logic & Bitwise
    /// Bitwise AND for integers, logical AND for BOOL.
    And = 0x30,
    /// Bitwise OR for integers, logical OR for BOOL.
    Or = 0x31,
    /// Bitwise NOT for integers, logical NOT for BOOL.
    Not = 0x32,
    /// Bitwise XOR for integers, logical XOR for BOOL.
    Xor = 0x33,
    /// Shift left.
    Shl = 0x34,
    /// Shift right (arithmetic for I64, logical for U64).
    Shr = 0x35,

    // 4.6 Control Flow — Pattern Matching
    /// Begin pattern match block. arg1 = variant_count.
    Match = 0x40,
    /// Match case for variant tag. arg1 = tag, arg2 = body_len.
    Case = 0x41,
    /// End of MATCH block.
    Exhaust = 0x42,

    // 4.7 Functions
    /// Begin function definition. arg1 = param_count, arg2 = body_len.
    Func = 0x50,
    /// Precondition block. arg1 = condition_len.
    Pre = 0x51,
    /// Postcondition block. arg1 = condition_len.
    Post = 0x52,
    /// Return top of stack from current function.
    Ret = 0x53,
    /// Call function at de Bruijn index arg1.
    Call = 0x54,
    /// Recursive call. arg1 = depth_limit.
    Recurse = 0x55,
    /// End of function block.
    EndFunc = 0x56,
    /// Declare parameter type. One per parameter, at start of FUNC body.
    Param = 0x57,

    // 4.8 Data Construction
    /// Construct variant value. arg1 = total_tags, arg2 = this_tag.
    VariantNew = 0x60,
    /// Construct tuple. arg1 = field_count.
    TupleNew = 0x61,
    /// Extract tuple field. arg1 = field_index.
    Project = 0x62,
    /// Construct array. arg1 = length.
    ArrayNew = 0x63,
    /// Pop index, pop array, push element.
    ArrayGet = 0x64,
    /// Pop array, push its length as U64.
    ArrayLen = 0x65,

    // 4.9 Verification & Meta
    /// 48-bit truncated blake3 hash of enclosing FUNC block.
    Hash = 0x70,
    /// Pop BOOL, runtime error if false.
    Assert = 0x71,
    /// Pop value, push BOOL (type check). Non-destructive: value is pushed back.
    Typeof = 0x72,

    // 4.10 VM Control
    /// Stop execution. Top of stack is the program result.
    Halt = 0xFE,
    /// No operation.
    Nop = 0xFF,
}

/// All valid opcodes, in definition order. Useful for exhaustive testing.
pub const ALL_OPCODES: [Opcode; 45] = [
    Opcode::Bind,
    Opcode::Ref,
    Opcode::Drop,
    Opcode::Const,
    Opcode::ConstExt,
    Opcode::Add,
    Opcode::Sub,
    Opcode::Mul,
    Opcode::Div,
    Opcode::Mod,
    Opcode::Neg,
    Opcode::Eq,
    Opcode::Neq,
    Opcode::Lt,
    Opcode::Gt,
    Opcode::Lte,
    Opcode::Gte,
    Opcode::And,
    Opcode::Or,
    Opcode::Not,
    Opcode::Xor,
    Opcode::Shl,
    Opcode::Shr,
    Opcode::Match,
    Opcode::Case,
    Opcode::Exhaust,
    Opcode::Func,
    Opcode::Pre,
    Opcode::Post,
    Opcode::Ret,
    Opcode::Call,
    Opcode::Recurse,
    Opcode::EndFunc,
    Opcode::Param,
    Opcode::VariantNew,
    Opcode::TupleNew,
    Opcode::Project,
    Opcode::ArrayNew,
    Opcode::ArrayGet,
    Opcode::ArrayLen,
    Opcode::Hash,
    Opcode::Assert,
    Opcode::Typeof,
    Opcode::Halt,
    Opcode::Nop,
];

impl TryFrom<u8> for Opcode {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Err(DecodeError::IllegalOpcode),

            // 4.1 Binding & Reference
            0x01 => Ok(Opcode::Bind),
            0x02 => Ok(Opcode::Ref),
            0x03 => Ok(Opcode::Drop),

            // 4.2 Constants
            0x04 => Ok(Opcode::Const),
            0x05 => Ok(Opcode::ConstExt),

            // 4.3 Arithmetic
            0x10 => Ok(Opcode::Add),
            0x11 => Ok(Opcode::Sub),
            0x12 => Ok(Opcode::Mul),
            0x13 => Ok(Opcode::Div),
            0x14 => Ok(Opcode::Mod),
            0x15 => Ok(Opcode::Neg),

            // 4.4 Comparison
            0x20 => Ok(Opcode::Eq),
            0x21 => Ok(Opcode::Neq),
            0x22 => Ok(Opcode::Lt),
            0x23 => Ok(Opcode::Gt),
            0x24 => Ok(Opcode::Lte),
            0x25 => Ok(Opcode::Gte),

            // 4.5 Logic & Bitwise
            0x30 => Ok(Opcode::And),
            0x31 => Ok(Opcode::Or),
            0x32 => Ok(Opcode::Not),
            0x33 => Ok(Opcode::Xor),
            0x34 => Ok(Opcode::Shl),
            0x35 => Ok(Opcode::Shr),

            // 4.6 Control Flow
            0x40 => Ok(Opcode::Match),
            0x41 => Ok(Opcode::Case),
            0x42 => Ok(Opcode::Exhaust),

            // 4.7 Functions
            0x50 => Ok(Opcode::Func),
            0x51 => Ok(Opcode::Pre),
            0x52 => Ok(Opcode::Post),
            0x53 => Ok(Opcode::Ret),
            0x54 => Ok(Opcode::Call),
            0x55 => Ok(Opcode::Recurse),
            0x56 => Ok(Opcode::EndFunc),
            0x57 => Ok(Opcode::Param),

            // 4.8 Data Construction
            0x60 => Ok(Opcode::VariantNew),
            0x61 => Ok(Opcode::TupleNew),
            0x62 => Ok(Opcode::Project),
            0x63 => Ok(Opcode::ArrayNew),
            0x64 => Ok(Opcode::ArrayGet),
            0x65 => Ok(Opcode::ArrayLen),

            // 4.9 Verification & Meta
            0x70 => Ok(Opcode::Hash),
            0x71 => Ok(Opcode::Assert),
            0x72 => Ok(Opcode::Typeof),

            // 4.10 VM Control
            0xFE => Ok(Opcode::Halt),
            0xFF => Ok(Opcode::Nop),

            // All remaining values are reserved (SPEC.md Section 4.11).
            // This covers 0x06..=0x0F, 0x16..=0x1F, 0x26..=0x2F, 0x36..=0x3F,
            // 0x43..=0x4F, 0x58..=0x5F, 0x66..=0x6F, 0x73..=0x7F, 0x80..=0xFD.
            _ => Err(DecodeError::ReservedOpcode(value)),
        }
    }
}

impl Opcode {
    /// Returns the assembly mnemonic for this opcode.
    pub fn mnemonic(&self) -> &'static str {
        match self {
            Opcode::Bind => "BIND",
            Opcode::Ref => "REF",
            Opcode::Drop => "DROP",
            Opcode::Const => "CONST",
            Opcode::ConstExt => "CONST_EXT",
            Opcode::Add => "ADD",
            Opcode::Sub => "SUB",
            Opcode::Mul => "MUL",
            Opcode::Div => "DIV",
            Opcode::Mod => "MOD",
            Opcode::Neg => "NEG",
            Opcode::Eq => "EQ",
            Opcode::Neq => "NEQ",
            Opcode::Lt => "LT",
            Opcode::Gt => "GT",
            Opcode::Lte => "LTE",
            Opcode::Gte => "GTE",
            Opcode::And => "AND",
            Opcode::Or => "OR",
            Opcode::Not => "NOT",
            Opcode::Xor => "XOR",
            Opcode::Shl => "SHL",
            Opcode::Shr => "SHR",
            Opcode::Match => "MATCH",
            Opcode::Case => "CASE",
            Opcode::Exhaust => "EXHAUST",
            Opcode::Func => "FUNC",
            Opcode::Pre => "PRE",
            Opcode::Post => "POST",
            Opcode::Ret => "RET",
            Opcode::Call => "CALL",
            Opcode::Recurse => "RECURSE",
            Opcode::EndFunc => "ENDFUNC",
            Opcode::Param => "PARAM",
            Opcode::VariantNew => "VARIANT_NEW",
            Opcode::TupleNew => "TUPLE_NEW",
            Opcode::Project => "PROJECT",
            Opcode::ArrayNew => "ARRAY_NEW",
            Opcode::ArrayGet => "ARRAY_GET",
            Opcode::ArrayLen => "ARRAY_LEN",
            Opcode::Hash => "HASH",
            Opcode::Assert => "ASSERT",
            Opcode::Typeof => "TYPEOF",
            Opcode::Halt => "HALT",
            Opcode::Nop => "NOP",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DecodeError;

    #[test]
    fn all_opcodes_count() {
        assert_eq!(ALL_OPCODES.len(), 45);
    }

    #[test]
    fn roundtrip_all_valid_opcodes() {
        for &opcode in &ALL_OPCODES {
            let byte = opcode as u8;
            let decoded = Opcode::try_from(byte).unwrap();
            assert_eq!(
                opcode, decoded,
                "roundtrip failed for {opcode:?} ({byte:#04x})"
            );
        }
    }

    #[test]
    fn illegal_opcode_zero() {
        assert_eq!(Opcode::try_from(0x00), Err(DecodeError::IllegalOpcode));
    }

    #[test]
    fn reserved_binding_range() {
        for byte in 0x06..=0x0Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte)),
                "byte {byte:#04x} should be reserved"
            );
        }
    }

    #[test]
    fn reserved_arithmetic_range() {
        for byte in 0x16..=0x1Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn reserved_comparison_range() {
        for byte in 0x26..=0x2Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn reserved_logic_range() {
        for byte in 0x36..=0x3Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn reserved_control_range() {
        for byte in 0x43..=0x4Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn reserved_function_range() {
        for byte in 0x58..=0x5Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn reserved_data_range() {
        for byte in 0x66..=0x6Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn reserved_meta_range() {
        for byte in 0x73..=0x7Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn reserved_expansion_range() {
        for byte in 0x80..=0xFDu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn every_byte_value_resolves() {
        // Every u8 value must produce either Ok or a specific Err — never panic.
        for byte in 0..=255u8 {
            let result = Opcode::try_from(byte);
            match result {
                Ok(_)
                | Err(DecodeError::IllegalOpcode)
                | Err(DecodeError::ReservedOpcode(_))
                | Err(DecodeError::InvalidOpcode(_)) => {}
                other => panic!("unexpected result for byte {byte:#04x}: {other:?}"),
            }
        }
    }

    #[test]
    fn mnemonic_roundtrip() {
        // Every opcode has a non-empty mnemonic
        for &opcode in &ALL_OPCODES {
            let m = opcode.mnemonic();
            assert!(!m.is_empty(), "empty mnemonic for {opcode:?}");
            assert_eq!(m, m.to_uppercase(), "mnemonic should be uppercase: {m}");
        }
    }
}
