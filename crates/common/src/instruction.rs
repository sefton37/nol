//! Instruction encoding and decoding for the NoLang instruction set.
//!
//! Every instruction is exactly 64 bits (8 bytes), encoded little-endian:
//! ```text
//! Byte 0:   opcode (u8)
//! Byte 1:   type_tag (u8)
//! Bytes 2-3: arg1 (u16, little-endian)
//! Bytes 4-5: arg2 (u16, little-endian)
//! Bytes 6-7: arg3 (u16, little-endian)
//! ```

use crate::error::DecodeError;
use crate::opcode::Opcode;
use crate::type_tag::TypeTag;
use crate::value::Value;

/// A single 64-bit NoLang instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instruction {
    /// The operation to perform.
    pub opcode: Opcode,
    /// Type context for this instruction. `TypeTag::None` if not applicable.
    pub type_tag: TypeTag,
    /// First operand. Meaning depends on opcode.
    pub arg1: u16,
    /// Second operand. Meaning depends on opcode.
    pub arg2: u16,
    /// Third operand. Meaning depends on opcode.
    pub arg3: u16,
}

impl Instruction {
    /// Create a new instruction.
    pub fn new(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Self {
        Self {
            opcode,
            type_tag,
            arg1,
            arg2,
            arg3,
        }
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

    /// Decode 8 bytes into an instruction (little-endian).
    pub fn decode(bytes: [u8; 8]) -> Result<Self, DecodeError> {
        let opcode = Opcode::try_from(bytes[0])?;
        let type_tag = TypeTag::try_from(bytes[1])?;
        let arg1 = u16::from_le_bytes([bytes[2], bytes[3]]);
        let arg2 = u16::from_le_bytes([bytes[4], bytes[5]]);
        let arg3 = u16::from_le_bytes([bytes[6], bytes[7]]);

        Ok(Self {
            opcode,
            type_tag,
            arg1,
            arg2,
            arg3,
        })
    }

    /// Extract the constant value from a CONST instruction.
    ///
    /// For I64, the 32-bit value `(arg1 << 16) | arg2` is **sign-extended** to 64 bits.
    /// For U64, the 32-bit value is **zero-extended**.
    /// For BOOL, `arg1 != 0` is true.
    /// For CHAR, `arg1` is the Unicode codepoint.
    /// For UNIT, no args needed.
    ///
    /// Returns `None` if this is not a CONST instruction or the type_tag is
    /// not valid for CONST.
    pub fn const_value(&self) -> Option<Value> {
        if self.opcode != Opcode::Const {
            return None;
        }

        let val32 = ((self.arg1 as u32) << 16) | (self.arg2 as u32);

        match self.type_tag {
            TypeTag::I64 => Some(Value::I64(val32 as i32 as i64)),
            TypeTag::U64 => Some(Value::U64(val32 as u64)),
            TypeTag::Bool => Some(Value::Bool(self.arg1 != 0)),
            TypeTag::Char => char::from_u32(self.arg1 as u32).map(Value::Char),
            TypeTag::Unit => Some(Value::Unit),
            _ => None,
        }
    }

    /// Encode a Value as one or two CONST instructions.
    ///
    /// Returns Ok(vec) where vec has 1 element (single CONST) or 2 elements
    /// (CONST_EXT + NOP carrier). Returns Err for compound values that
    /// cannot be encoded as constants (Variant, Tuple, Array).
    pub fn from_value(value: &Value) -> Result<Vec<Instruction>, &'static str> {
        match value {
            Value::I64(val) => {
                // If value fits in i32 range, use single CONST
                if *val >= i32::MIN as i64 && *val <= i32::MAX as i64 {
                    let val32 = *val as i32 as u32;
                    let arg1 = (val32 >> 16) as u16;
                    let arg2 = val32 as u16;
                    Ok(vec![Instruction::new(
                        Opcode::Const,
                        TypeTag::I64,
                        arg1,
                        arg2,
                        0,
                    )])
                } else {
                    // Use CONST_EXT + NOP carrier
                    let bits = *val as u64;
                    let const_ext =
                        Instruction::new(Opcode::ConstExt, TypeTag::I64, (bits >> 48) as u16, 0, 0);
                    let nop_carrier = Instruction::new(
                        Opcode::Nop,
                        TypeTag::None,
                        (bits >> 32) as u16,
                        (bits >> 16) as u16,
                        bits as u16,
                    );
                    Ok(vec![const_ext, nop_carrier])
                }
            }
            Value::U64(val) => {
                // If value fits in u32 range, use single CONST
                if *val <= u32::MAX as u64 {
                    let arg1 = (*val >> 16) as u16;
                    let arg2 = *val as u16;
                    Ok(vec![Instruction::new(
                        Opcode::Const,
                        TypeTag::U64,
                        arg1,
                        arg2,
                        0,
                    )])
                } else {
                    // Use CONST_EXT + NOP carrier
                    let bits = *val;
                    let const_ext =
                        Instruction::new(Opcode::ConstExt, TypeTag::U64, (bits >> 48) as u16, 0, 0);
                    let nop_carrier = Instruction::new(
                        Opcode::Nop,
                        TypeTag::None,
                        (bits >> 32) as u16,
                        (bits >> 16) as u16,
                        bits as u16,
                    );
                    Ok(vec![const_ext, nop_carrier])
                }
            }
            Value::F64(val) => {
                // Reject NaN and infinity
                if val.is_nan() {
                    return Err("cannot encode NaN as CONST");
                }
                if val.is_infinite() {
                    return Err("cannot encode infinity as CONST");
                }
                // F64 always uses CONST_EXT + NOP carrier
                let bits = val.to_bits();
                let const_ext =
                    Instruction::new(Opcode::ConstExt, TypeTag::F64, (bits >> 48) as u16, 0, 0);
                let nop_carrier = Instruction::new(
                    Opcode::Nop,
                    TypeTag::None,
                    (bits >> 32) as u16,
                    (bits >> 16) as u16,
                    bits as u16,
                );
                Ok(vec![const_ext, nop_carrier])
            }
            Value::Bool(val) => Ok(vec![Instruction::new(
                Opcode::Const,
                TypeTag::Bool,
                if *val { 1 } else { 0 },
                0,
                0,
            )]),
            Value::Char(val) => Ok(vec![Instruction::new(
                Opcode::Const,
                TypeTag::Char,
                *val as u16,
                0,
                0,
            )]),
            Value::Unit => Ok(vec![Instruction::new(
                Opcode::Const,
                TypeTag::Unit,
                0,
                0,
                0,
            )]),
            Value::Variant { .. } => Err("cannot encode compound value as CONST"),
            Value::Tuple(_) => Err("cannot encode compound value as CONST"),
            Value::Array(_) => Err("cannot encode compound value as CONST"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- Encode/Decode roundtrip ---

    #[test]
    fn encode_decode_roundtrip_simple() {
        let instr = Instruction::new(Opcode::Add, TypeTag::None, 0, 0, 0);
        let bytes = instr.encode();
        let decoded = Instruction::decode(bytes).unwrap();
        assert_eq!(instr, decoded);
    }

    #[test]
    fn encode_decode_roundtrip_with_args() {
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0x1234, 0x5678, 0);
        let bytes = instr.encode();
        let decoded = Instruction::decode(bytes).unwrap();
        assert_eq!(instr, decoded);
    }

    #[test]
    fn encode_decode_roundtrip_all_opcodes() {
        for &opcode in &crate::opcode::ALL_OPCODES {
            let instr = Instruction::new(opcode, TypeTag::None, 0, 0, 0);
            let bytes = instr.encode();
            let decoded = Instruction::decode(bytes).unwrap();
            assert_eq!(instr, decoded, "roundtrip failed for {opcode:?}");
        }
    }

    #[test]
    fn encode_decode_roundtrip_all_type_tags() {
        for &tag in &crate::type_tag::ALL_TYPE_TAGS {
            let instr = Instruction::new(Opcode::Nop, tag, 0, 0, 0);
            let bytes = instr.encode();
            let decoded = Instruction::decode(bytes).unwrap();
            assert_eq!(instr, decoded, "roundtrip failed for {tag:?}");
        }
    }

    #[test]
    fn encode_decode_roundtrip_max_args() {
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0xFFFF, 0xFFFF, 0xFFFF);
        let bytes = instr.encode();
        let decoded = Instruction::decode(bytes).unwrap();
        assert_eq!(instr, decoded);
    }

    // --- Little-endian byte order ---

    #[test]
    fn little_endian_encoding() {
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0x1234, 0x5678, 0xABCD);
        let bytes = instr.encode();

        assert_eq!(bytes[0], 0x04); // Const opcode
        assert_eq!(bytes[1], 0x01); // I64 type tag
        assert_eq!(bytes[2], 0x34); // arg1 low byte
        assert_eq!(bytes[3], 0x12); // arg1 high byte
        assert_eq!(bytes[4], 0x78); // arg2 low byte
        assert_eq!(bytes[5], 0x56); // arg2 high byte
        assert_eq!(bytes[6], 0xCD); // arg3 low byte
        assert_eq!(bytes[7], 0xAB); // arg3 high byte
    }

    #[test]
    fn opcode_is_first_byte() {
        let instr = Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0);
        let bytes = instr.encode();
        assert_eq!(bytes[0], 0xFE);
    }

    // --- Decode rejection ---

    #[test]
    fn decode_rejects_illegal_opcode() {
        let bytes = [0x00, 0x00, 0, 0, 0, 0, 0, 0];
        assert_eq!(Instruction::decode(bytes), Err(DecodeError::IllegalOpcode));
    }

    #[test]
    fn decode_rejects_reserved_opcode() {
        let bytes = [0x08, 0x00, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            Instruction::decode(bytes),
            Err(DecodeError::ReservedOpcode(0x08))
        );
    }

    #[test]
    fn decode_rejects_reserved_type_tag() {
        let bytes = [0x01, 0x0D, 0, 0, 0, 0, 0, 0]; // Bind + reserved tag
        assert_eq!(
            Instruction::decode(bytes),
            Err(DecodeError::ReservedTypeTag(0x0D))
        );
    }

    // --- CONST value extraction ---

    #[test]
    fn const_i64_positive() {
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0, 42, 0);
        assert_eq!(instr.const_value(), Some(Value::I64(42)));
    }

    #[test]
    fn const_i64_large_positive() {
        // 0x0001_0000 = 65536
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 1, 0, 0);
        assert_eq!(instr.const_value(), Some(Value::I64(65536)));
    }

    #[test]
    fn const_i64_negative_one() {
        // -1 in 32-bit two's complement = 0xFFFF_FFFF
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0xFFFF, 0xFFFF, 0);
        assert_eq!(instr.const_value(), Some(Value::I64(-1)));
    }

    #[test]
    fn const_i64_negative_thirteen() {
        // -13 in 32-bit two's complement = 0xFFFF_FFF3
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0xFFFF, 0xFFF3, 0);
        assert_eq!(instr.const_value(), Some(Value::I64(-13)));
    }

    #[test]
    fn const_i64_min_32bit() {
        // i32::MIN = -2,147,483,648 = 0x8000_0000
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0x8000, 0x0000, 0);
        assert_eq!(instr.const_value(), Some(Value::I64(i32::MIN as i64)));
    }

    #[test]
    fn const_i64_max_32bit() {
        // i32::MAX = 2,147,483,647 = 0x7FFF_FFFF
        let instr = Instruction::new(Opcode::Const, TypeTag::I64, 0x7FFF, 0xFFFF, 0);
        assert_eq!(instr.const_value(), Some(Value::I64(i32::MAX as i64)));
    }

    #[test]
    fn const_u64_zero_extension() {
        // 0xFFFF_FFFF should stay as 4,294,967,295, not become negative
        let instr = Instruction::new(Opcode::Const, TypeTag::U64, 0xFFFF, 0xFFFF, 0);
        assert_eq!(instr.const_value(), Some(Value::U64(0xFFFF_FFFF)));
    }

    #[test]
    fn const_bool_true() {
        let instr = Instruction::new(Opcode::Const, TypeTag::Bool, 1, 0, 0);
        assert_eq!(instr.const_value(), Some(Value::Bool(true)));
    }

    #[test]
    fn const_bool_false() {
        let instr = Instruction::new(Opcode::Const, TypeTag::Bool, 0, 0, 0);
        assert_eq!(instr.const_value(), Some(Value::Bool(false)));
    }

    #[test]
    fn const_char() {
        let instr = Instruction::new(Opcode::Const, TypeTag::Char, 65, 0, 0); // 'A'
        assert_eq!(instr.const_value(), Some(Value::Char('A')));
    }

    #[test]
    fn const_char_invalid_codepoint() {
        // 0xD800 is a surrogate, not a valid char
        let instr = Instruction::new(Opcode::Const, TypeTag::Char, 0xD800, 0, 0);
        assert_eq!(instr.const_value(), None);
    }

    #[test]
    fn const_unit() {
        let instr = Instruction::new(Opcode::Const, TypeTag::Unit, 0, 0, 0);
        assert_eq!(instr.const_value(), Some(Value::Unit));
    }

    #[test]
    fn const_invalid_type_tag() {
        // F64 requires CONST_EXT, not CONST
        let instr = Instruction::new(Opcode::Const, TypeTag::F64, 0, 0, 0);
        assert_eq!(instr.const_value(), None);
    }

    #[test]
    fn const_value_on_non_const_opcode() {
        let instr = Instruction::new(Opcode::Add, TypeTag::None, 0, 0, 0);
        assert_eq!(instr.const_value(), None);
    }

    // --- from_value() tests ---

    #[test]
    fn from_value_i64_small() {
        let instructions = Instruction::from_value(&Value::I64(42)).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::I64);
        // Roundtrip via const_value()
        assert_eq!(instructions[0].const_value(), Some(Value::I64(42)));
    }

    #[test]
    fn from_value_i64_negative() {
        let instructions = Instruction::from_value(&Value::I64(-13)).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::I64);
        assert_eq!(instructions[0].const_value(), Some(Value::I64(-13)));
    }

    #[test]
    fn from_value_i64_zero() {
        let instructions = Instruction::from_value(&Value::I64(0)).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::I64);
        assert_eq!(instructions[0].const_value(), Some(Value::I64(0)));
    }

    #[test]
    fn from_value_i64_min_32() {
        let val = i32::MIN as i64;
        let instructions = Instruction::from_value(&Value::I64(val)).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::I64);
        assert_eq!(instructions[0].const_value(), Some(Value::I64(val)));
    }

    #[test]
    fn from_value_i64_max_32() {
        let val = i32::MAX as i64;
        let instructions = Instruction::from_value(&Value::I64(val)).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::I64);
        assert_eq!(instructions[0].const_value(), Some(Value::I64(val)));
    }

    #[test]
    fn from_value_i64_needs_ext() {
        let val = i64::MAX;
        let instructions = Instruction::from_value(&Value::I64(val)).unwrap();
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0].opcode, Opcode::ConstExt);
        assert_eq!(instructions[0].type_tag, TypeTag::I64);
        assert_eq!(instructions[1].opcode, Opcode::Nop);
        assert_eq!(instructions[1].type_tag, TypeTag::None);

        // Verify encoding details
        let bits = val as u64;
        assert_eq!(instructions[0].arg1, (bits >> 48) as u16);
        assert_eq!(instructions[1].arg1, (bits >> 32) as u16);
        assert_eq!(instructions[1].arg2, (bits >> 16) as u16);
        assert_eq!(instructions[1].arg3, bits as u16);
    }

    #[test]
    fn from_value_i64_negative_needs_ext() {
        let val = i32::MIN as i64 - 1;
        let instructions = Instruction::from_value(&Value::I64(val)).unwrap();
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0].opcode, Opcode::ConstExt);
        assert_eq!(instructions[0].type_tag, TypeTag::I64);
        assert_eq!(instructions[1].opcode, Opcode::Nop);
    }

    #[test]
    fn from_value_u64_small() {
        let instructions = Instruction::from_value(&Value::U64(100)).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::U64);
        assert_eq!(instructions[0].const_value(), Some(Value::U64(100)));
    }

    #[test]
    fn from_value_u64_max_32() {
        let val = u32::MAX as u64;
        let instructions = Instruction::from_value(&Value::U64(val)).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::U64);
        assert_eq!(instructions[0].const_value(), Some(Value::U64(val)));
    }

    #[test]
    fn from_value_u64_needs_ext() {
        let val = u64::MAX;
        let instructions = Instruction::from_value(&Value::U64(val)).unwrap();
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0].opcode, Opcode::ConstExt);
        assert_eq!(instructions[0].type_tag, TypeTag::U64);
        assert_eq!(instructions[1].opcode, Opcode::Nop);
        assert_eq!(instructions[1].type_tag, TypeTag::None);

        // Verify encoding details
        assert_eq!(instructions[0].arg1, (val >> 48) as u16);
        assert_eq!(instructions[1].arg1, (val >> 32) as u16);
        assert_eq!(instructions[1].arg2, (val >> 16) as u16);
        assert_eq!(instructions[1].arg3, val as u16);
    }

    #[test]
    fn from_value_f64() {
        let val = 1.234;
        let instructions = Instruction::from_value(&Value::F64(val)).unwrap();
        assert_eq!(instructions.len(), 2);
        assert_eq!(instructions[0].opcode, Opcode::ConstExt);
        assert_eq!(instructions[0].type_tag, TypeTag::F64);
        assert_eq!(instructions[1].opcode, Opcode::Nop);

        // Verify encoding details
        let bits = val.to_bits();
        assert_eq!(instructions[0].arg1, (bits >> 48) as u16);
        assert_eq!(instructions[1].arg1, (bits >> 32) as u16);
        assert_eq!(instructions[1].arg2, (bits >> 16) as u16);
        assert_eq!(instructions[1].arg3, bits as u16);
    }

    #[test]
    fn from_value_f64_nan_rejected() {
        let result = Instruction::from_value(&Value::F64(f64::NAN));
        assert_eq!(result, Err("cannot encode NaN as CONST"));
    }

    #[test]
    fn from_value_f64_infinity_rejected() {
        let result = Instruction::from_value(&Value::F64(f64::INFINITY));
        assert_eq!(result, Err("cannot encode infinity as CONST"));
    }

    #[test]
    fn from_value_bool_true() {
        let instructions = Instruction::from_value(&Value::Bool(true)).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::Bool);
        assert_eq!(instructions[0].arg1, 1);
        assert_eq!(instructions[0].const_value(), Some(Value::Bool(true)));
    }

    #[test]
    fn from_value_bool_false() {
        let instructions = Instruction::from_value(&Value::Bool(false)).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::Bool);
        assert_eq!(instructions[0].arg1, 0);
        assert_eq!(instructions[0].const_value(), Some(Value::Bool(false)));
    }

    #[test]
    fn from_value_char() {
        let instructions = Instruction::from_value(&Value::Char('A')).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::Char);
        assert_eq!(instructions[0].arg1, 'A' as u16);
        assert_eq!(instructions[0].const_value(), Some(Value::Char('A')));
    }

    #[test]
    fn from_value_unit() {
        let instructions = Instruction::from_value(&Value::Unit).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_eq!(instructions[0].type_tag, TypeTag::Unit);
        assert_eq!(instructions[0].const_value(), Some(Value::Unit));
    }

    #[test]
    fn from_value_variant_rejected() {
        let variant = Value::Variant {
            tag_count: 2,
            tag: 0,
            payload: Box::new(Value::Unit),
        };
        let result = Instruction::from_value(&variant);
        assert_eq!(result, Err("cannot encode compound value as CONST"));
    }

    #[test]
    fn from_value_tuple_rejected() {
        let tuple = Value::Tuple(vec![Value::I64(1), Value::I64(2)]);
        let result = Instruction::from_value(&tuple);
        assert_eq!(result, Err("cannot encode compound value as CONST"));
    }

    #[test]
    fn from_value_array_rejected() {
        let array = Value::Array(vec![Value::I64(1), Value::I64(2), Value::I64(3)]);
        let result = Instruction::from_value(&array);
        assert_eq!(result, Err("cannot encode compound value as CONST"));
    }

    #[test]
    fn from_value_canonical() {
        // Canonical rule: use single CONST when value fits in 32 bits
        // Value 42 fits in i32, so must use single CONST, not CONST_EXT
        let instructions = Instruction::from_value(&Value::I64(42)).unwrap();
        assert_eq!(instructions.len(), 1);
        assert_eq!(instructions[0].opcode, Opcode::Const);
        assert_ne!(instructions[0].opcode, Opcode::ConstExt);
    }
}
