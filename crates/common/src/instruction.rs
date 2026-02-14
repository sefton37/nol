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
}
