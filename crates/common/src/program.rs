//! Program representation for NoLang instruction streams.
//!
//! A program is a sequence of 64-bit instructions. Binary files (.nolb)
//! are raw concatenations of 8-byte instructions with no header.

use crate::error::DecodeError;
use crate::instruction::Instruction;

/// A NoLang program: a sequence of instructions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    /// The instruction stream.
    pub instructions: Vec<Instruction>,
}

impl Program {
    /// Create a new program from a vector of instructions.
    pub fn new(instructions: Vec<Instruction>) -> Self {
        Self { instructions }
    }

    /// Encode the entire program to bytes.
    ///
    /// Each instruction becomes 8 bytes. The result length is always
    /// `instructions.len() * 8`.
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(self.instructions.len() * 8);
        for instr in &self.instructions {
            bytes.extend_from_slice(&instr.encode());
        }
        bytes
    }

    /// Decode a byte slice into a program.
    ///
    /// The byte slice length must be a multiple of 8. Each 8-byte chunk
    /// is decoded as one instruction.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        if !bytes.len().is_multiple_of(8) {
            return Err(DecodeError::InvalidLength(bytes.len()));
        }

        let mut instructions = Vec::with_capacity(bytes.len() / 8);
        for chunk in bytes.chunks_exact(8) {
            let arr: [u8; 8] = chunk.try_into().expect("chunks_exact guarantees 8 bytes");
            instructions.push(Instruction::decode(arr)?);
        }

        Ok(Self { instructions })
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
        assert_eq!(program.encode(), vec![]);
    }

    #[test]
    fn encode_decode_roundtrip() {
        let instructions = vec![
            Instruction::new(Opcode::Const, TypeTag::I64, 0, 42, 0),
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];

        let program = Program::new(instructions);
        let bytes = program.encode();

        assert_eq!(bytes.len(), 16); // 2 instructions * 8 bytes
        let decoded = Program::decode(&bytes).unwrap();
        assert_eq!(program, decoded);
    }

    #[test]
    fn decode_invalid_length_not_multiple_of_8() {
        let bytes = vec![0; 7];
        assert_eq!(Program::decode(&bytes), Err(DecodeError::InvalidLength(7)));
    }

    #[test]
    fn decode_invalid_length_odd() {
        let bytes = vec![0; 13];
        assert_eq!(Program::decode(&bytes), Err(DecodeError::InvalidLength(13)));
    }

    #[test]
    fn decode_empty_bytes() {
        let program = Program::decode(&[]).unwrap();
        assert!(program.is_empty());
    }

    #[test]
    fn decode_propagates_instruction_errors() {
        // First 8 bytes: valid instruction. Second 8 bytes: illegal opcode.
        let mut bytes = vec![0xFE, 0x00, 0, 0, 0, 0, 0, 0]; // HALT
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
}
