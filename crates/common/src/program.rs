//! Program representation for NoLang instruction streams.
//!
//! A program is a sequence of 64-bit instructions plus an optional string pool.
//! Binary format (.nolb):
//!
//! ```text
//! [u32 LE: instruction_count]
//! [instruction_count * 8 bytes: instructions]
//! [u32 LE: string_pool_count]
//! [for each string: u32 LE byte_len, UTF-8 bytes]
//! ```

use crate::error::DecodeError;
use crate::instruction::Instruction;

/// A NoLang program: a sequence of instructions and a string pool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// The instruction stream.
    pub instructions: Vec<Instruction>,
    /// String pool for STR_CONST opcodes. Index matches arg1.
    pub string_pool: Vec<String>,
}

impl Program {
    /// Create a new program from a vector of instructions (no string pool).
    pub fn new(instructions: Vec<Instruction>) -> Self {
        Self {
            instructions,
            string_pool: Vec::new(),
        }
    }

    /// Create a new program with instructions and a string pool.
    pub fn with_string_pool(instructions: Vec<Instruction>, string_pool: Vec<String>) -> Self {
        Self {
            instructions,
            string_pool,
        }
    }

    /// Encode the entire program to bytes.
    ///
    /// Format:
    /// - `u32 LE`: instruction count
    /// - `instruction_count * 8` bytes: encoded instructions
    /// - `u32 LE`: string pool entry count
    /// - For each string: `u32 LE` byte length, then UTF-8 bytes
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(
            4 + self.instructions.len() * 8 + 4 + self.string_pool.iter().map(|s| 4 + s.len()).sum::<usize>()
        );

        // Instruction count header
        bytes.extend_from_slice(&(self.instructions.len() as u32).to_le_bytes());

        // Instructions
        for instr in &self.instructions {
            bytes.extend_from_slice(&instr.encode());
        }

        // String pool
        bytes.extend_from_slice(&(self.string_pool.len() as u32).to_le_bytes());
        for s in &self.string_pool {
            let sb = s.as_bytes();
            bytes.extend_from_slice(&(sb.len() as u32).to_le_bytes());
            bytes.extend_from_slice(sb);
        }

        bytes
    }

    /// Decode a byte slice into a program.
    ///
    /// Reads the instruction count header, then instruction bytes, then string pool.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if bytes.is_empty() {
            // Empty input → empty program (backward compat)
            return Ok(Self {
                instructions: Vec::new(),
                string_pool: Vec::new(),
            });
        }

        if bytes.len() < 4 {
            return Err(DecodeError::InvalidLength(bytes.len()));
        }

        let mut offset = 0;

        // Read instruction count
        let instr_count = u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        offset += 4;

        // Read instructions
        let instr_bytes = instr_count * 8;
        if offset + instr_bytes > bytes.len() {
            return Err(DecodeError::InvalidLength(bytes.len()));
        }

        let mut instructions = Vec::with_capacity(instr_count);
        for i in 0..instr_count {
            let start = offset + i * 8;
            let arr: [u8; 8] = bytes[start..start + 8]
                .try_into()
                .expect("bounds already checked");
            instructions.push(Instruction::decode(arr)?);
        }
        offset += instr_bytes;

        // Read string pool
        let mut string_pool = Vec::new();
        if offset + 4 <= bytes.len() {
            let pool_count = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]) as usize;
            offset += 4;

            string_pool.reserve(pool_count);
            for _ in 0..pool_count {
                if offset + 4 > bytes.len() {
                    return Err(DecodeError::InvalidLength(bytes.len()));
                }
                let str_len = u32::from_le_bytes([
                    bytes[offset],
                    bytes[offset + 1],
                    bytes[offset + 2],
                    bytes[offset + 3],
                ]) as usize;
                offset += 4;

                if offset + str_len > bytes.len() {
                    return Err(DecodeError::InvalidLength(bytes.len()));
                }
                let s = std::str::from_utf8(&bytes[offset..offset + str_len])
                    .map_err(|_| DecodeError::InvalidLength(bytes.len()))?;
                string_pool.push(s.to_string());
                offset += str_len;
            }
        }

        Ok(Self {
            instructions,
            string_pool,
        })
    }

    /// Number of instructions in the program.
    pub fn len(&self) -> usize {
        self.instructions.len()
    }

    /// Returns true if the program has no instructions.
    pub fn is_empty(&self) -> bool {
        self.instructions.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::opcode::Opcode;
    use crate::type_tag::TypeTag;

    #[test]
    fn empty_program() {
        let program = Program::new(vec![]);
        assert!(program.is_empty());
        assert_eq!(program.len(), 0);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let instructions = vec![
            Instruction::new(Opcode::Const, TypeTag::I64, 0, 42, 0),
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];

        let program = Program::new(instructions);
        let bytes = program.encode();
        let decoded = Program::decode(&bytes).unwrap();
        assert_eq!(program, decoded);
    }

    #[test]
    fn encode_decode_with_string_pool() {
        let instructions = vec![
            Instruction::new(Opcode::StrConst, TypeTag::None, 0, 0, 0),
            Instruction::new(Opcode::StrConst, TypeTag::None, 1, 0, 0),
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];

        let pool = vec!["hello".to_string(), "world".to_string()];
        let program = Program::with_string_pool(instructions, pool);
        let bytes = program.encode();
        let decoded = Program::decode(&bytes).unwrap();
        assert_eq!(program, decoded);
        assert_eq!(decoded.string_pool.len(), 2);
        assert_eq!(decoded.string_pool[0], "hello");
        assert_eq!(decoded.string_pool[1], "world");
    }

    #[test]
    fn encode_decode_empty_string_pool() {
        let instructions = vec![
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let program = Program::new(instructions);
        let bytes = program.encode();
        let decoded = Program::decode(&bytes).unwrap();
        assert_eq!(program, decoded);
        assert!(decoded.string_pool.is_empty());
    }

    #[test]
    fn decode_empty_bytes() {
        let program = Program::decode(&[]).unwrap();
        assert!(program.is_empty());
        assert!(program.string_pool.is_empty());
    }

    #[test]
    fn decode_truncated_header() {
        let bytes = vec![0, 0, 0]; // 3 bytes, too short for header
        assert_eq!(Program::decode(&bytes), Err(DecodeError::InvalidLength(3)));
    }

    #[test]
    fn decode_truncated_instructions() {
        // Header says 10 instructions but only has enough for 1
        let mut bytes = (10u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(&[0xFE, 0x00, 0, 0, 0, 0, 0, 0]); // 1 HALT
        assert_eq!(
            Program::decode(&bytes),
            Err(DecodeError::InvalidLength(bytes.len()))
        );
    }

    #[test]
    fn decode_propagates_instruction_errors() {
        // Header: 2 instructions. First: HALT. Second: illegal opcode.
        let mut bytes = (2u32).to_le_bytes().to_vec();
        bytes.extend_from_slice(&[0xFE, 0x00, 0, 0, 0, 0, 0, 0]); // HALT
        bytes.extend_from_slice(&[0x00, 0x00, 0, 0, 0, 0, 0, 0]); // illegal
        assert_eq!(Program::decode(&bytes), Err(DecodeError::IllegalOpcode));
    }

    #[test]
    fn len_and_is_empty() {
        let program = Program::new(vec![
            Instruction::new(Opcode::Nop, TypeTag::None, 0, 0, 0),
            Instruction::new(Opcode::Nop, TypeTag::None, 0, 0, 0),
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        assert_eq!(program.len(), 3);
        assert!(!program.is_empty());
    }

    #[test]
    fn string_pool_unicode() {
        let pool = vec!["hello 🌍".to_string(), "日本語".to_string()];
        let program = Program::with_string_pool(
            vec![Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0)],
            pool.clone(),
        );
        let bytes = program.encode();
        let decoded = Program::decode(&bytes).unwrap();
        assert_eq!(decoded.string_pool, pool);
    }

    #[test]
    fn string_pool_empty_strings() {
        let pool = vec!["".to_string(), "".to_string()];
        let program = Program::with_string_pool(
            vec![Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0)],
            pool.clone(),
        );
        let bytes = program.encode();
        let decoded = Program::decode(&bytes).unwrap();
        assert_eq!(decoded.string_pool, pool);
    }
}
