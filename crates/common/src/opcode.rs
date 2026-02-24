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
    /// Logical implication. Pop two BOOLs, push !antecedent || consequent.
    Implies = 0x36,

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
    /// Universal quantifier for arrays. Pop array, execute body for each element.
    Forall = 0x73,

    // 4.11 File & Path I/O
    /// Read file contents. PATH → RESULT(BYTES, STRING).
    FileRead = 0x80,
    /// Write bytes to file. PATH × BYTES → RESULT(UNIT, STRING).
    FileWrite = 0x81,
    /// Append bytes to file. PATH × BYTES → RESULT(UNIT, STRING).
    FileAppend = 0x82,
    /// Check if file exists. PATH → BOOL.
    FileExists = 0x83,
    /// Delete file. PATH → RESULT(UNIT, STRING).
    FileDelete = 0x84,
    /// List directory entries. PATH → RESULT(ARRAY(PATH), STRING).
    DirList = 0x85,
    /// Create directory (and parents). PATH → RESULT(UNIT, STRING).
    DirMake = 0x86,
    /// Join path components. PATH × STRING → PATH (pure).
    PathJoin = 0x87,
    /// Get parent directory. PATH → MAYBE(PATH) (pure).
    PathParent = 0x88,

    // 4.12 String Operations
    /// Get string length in characters. STRING → U64 (pure).
    StrLen = 0x90,
    /// Concatenate two strings. STRING × STRING → STRING (pure).
    StrConcat = 0x91,
    /// Slice string by character indices. STRING × U64 × U64 → STRING (pure).
    StrSlice = 0x92,
    /// Split string by delimiter. STRING × STRING → ARRAY(STRING) (pure).
    StrSplit = 0x93,
    /// Convert string to bytes. STRING → BYTES (pure).
    StrBytes = 0x94,
    /// Convert bytes to string. BYTES → RESULT(STRING, STRING) (pure).
    BytesStr = 0x95,
    /// Push string constant from pool. arg1 = pool_index. → STRING (pure).
    StrConst = 0x96,

    // 4.13 Process Execution
    /// Spawn subprocess. ARRAY(STRING) → RESULT(TUPLE(I64, BYTES, BYTES), STRING).
    ExecSpawn = 0xA0,
    /// Check subprocess result. TUPLE → RESULT(UNIT, STRING).
    ExecCheck = 0xA1,

    // 4.10 VM Control
    /// Stop execution. Top of stack is the program result.
    Halt = 0xFE,
    /// No operation.
    Nop = 0xFF,
}

/// All valid opcodes, in definition order. Useful for exhaustive testing.
pub const ALL_OPCODES: [Opcode; 65] = [
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
    Opcode::Implies,
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
    Opcode::Forall,
    Opcode::FileRead,
    Opcode::FileWrite,
    Opcode::FileAppend,
    Opcode::FileExists,
    Opcode::FileDelete,
    Opcode::DirList,
    Opcode::DirMake,
    Opcode::PathJoin,
    Opcode::PathParent,
    Opcode::StrLen,
    Opcode::StrConcat,
    Opcode::StrSlice,
    Opcode::StrSplit,
    Opcode::StrBytes,
    Opcode::BytesStr,
    Opcode::StrConst,
    Opcode::ExecSpawn,
    Opcode::ExecCheck,
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
            0x36 => Ok(Opcode::Implies),

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
            0x73 => Ok(Opcode::Forall),

            // 4.11 File & Path I/O
            0x80 => Ok(Opcode::FileRead),
            0x81 => Ok(Opcode::FileWrite),
            0x82 => Ok(Opcode::FileAppend),
            0x83 => Ok(Opcode::FileExists),
            0x84 => Ok(Opcode::FileDelete),
            0x85 => Ok(Opcode::DirList),
            0x86 => Ok(Opcode::DirMake),
            0x87 => Ok(Opcode::PathJoin),
            0x88 => Ok(Opcode::PathParent),

            // 4.12 String Operations
            0x90 => Ok(Opcode::StrLen),
            0x91 => Ok(Opcode::StrConcat),
            0x92 => Ok(Opcode::StrSlice),
            0x93 => Ok(Opcode::StrSplit),
            0x94 => Ok(Opcode::StrBytes),
            0x95 => Ok(Opcode::BytesStr),
            0x96 => Ok(Opcode::StrConst),

            // 4.13 Process Execution
            0xA0 => Ok(Opcode::ExecSpawn),
            0xA1 => Ok(Opcode::ExecCheck),

            // 4.10 VM Control
            0xFE => Ok(Opcode::Halt),
            0xFF => Ok(Opcode::Nop),

            // All remaining values are reserved.
            // This covers 0x06..=0x0F, 0x16..=0x1F, 0x26..=0x2F, 0x37..=0x3F,
            // 0x43..=0x4F, 0x58..=0x5F, 0x66..=0x6F, 0x74..=0x7F, 0x89..=0x8F,
            // 0x97..=0x9F, 0xA2..=0xFD.
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
            Opcode::Implies => "IMPLIES",
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
            Opcode::Forall => "FORALL",
            Opcode::FileRead => "FILE_READ",
            Opcode::FileWrite => "FILE_WRITE",
            Opcode::FileAppend => "FILE_APPEND",
            Opcode::FileExists => "FILE_EXISTS",
            Opcode::FileDelete => "FILE_DELETE",
            Opcode::DirList => "DIR_LIST",
            Opcode::DirMake => "DIR_MAKE",
            Opcode::PathJoin => "PATH_JOIN",
            Opcode::PathParent => "PATH_PARENT",
            Opcode::StrLen => "STR_LEN",
            Opcode::StrConcat => "STR_CONCAT",
            Opcode::StrSlice => "STR_SLICE",
            Opcode::StrSplit => "STR_SPLIT",
            Opcode::StrBytes => "STR_BYTES",
            Opcode::BytesStr => "BYTES_STR",
            Opcode::StrConst => "STR_CONST",
            Opcode::ExecSpawn => "EXEC_SPAWN",
            Opcode::ExecCheck => "EXEC_CHECK",
            Opcode::Halt => "HALT",
            Opcode::Nop => "NOP",
        }
    }

    /// Returns true if this opcode performs I/O (effectful or pure I/O-related).
    pub fn is_io(&self) -> bool {
        matches!(
            self,
            Opcode::FileRead
                | Opcode::FileWrite
                | Opcode::FileAppend
                | Opcode::FileExists
                | Opcode::FileDelete
                | Opcode::DirList
                | Opcode::DirMake
                | Opcode::PathJoin
                | Opcode::PathParent
                | Opcode::StrLen
                | Opcode::StrConcat
                | Opcode::StrSlice
                | Opcode::StrSplit
                | Opcode::StrBytes
                | Opcode::BytesStr
                | Opcode::StrConst
                | Opcode::ExecSpawn
                | Opcode::ExecCheck
        )
    }

    /// Returns true if this opcode has side effects (file/process I/O).
    /// Pure string/path operations return false.
    pub fn is_effectful(&self) -> bool {
        matches!(
            self,
            Opcode::FileRead
                | Opcode::FileWrite
                | Opcode::FileAppend
                | Opcode::FileExists
                | Opcode::FileDelete
                | Opcode::DirList
                | Opcode::DirMake
                | Opcode::ExecSpawn
                | Opcode::ExecCheck
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DecodeError;

    #[test]
    fn all_opcodes_count() {
        assert_eq!(ALL_OPCODES.len(), 65);
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
        for byte in 0x37..=0x3Fu8 {
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
        for byte in 0x74..=0x7Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn reserved_io_file_range() {
        for byte in 0x89..=0x8Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn reserved_string_range() {
        for byte in 0x97..=0x9Fu8 {
            assert_eq!(
                Opcode::try_from(byte),
                Err(DecodeError::ReservedOpcode(byte))
            );
        }
    }

    #[test]
    fn reserved_process_range() {
        for byte in 0xA2..=0xFDu8 {
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
