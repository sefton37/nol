//! NoLang common types and instruction encoding.
//!
//! This crate provides the foundational data structures for the NoLang
//! instruction set:
//!
//! - [`Opcode`] — all 45 opcodes from SPEC.md Section 4
//! - [`TypeTag`] — all 13 type tags from SPEC.md Section 3
//! - [`Instruction`] — the 64-bit instruction struct with encode/decode
//! - [`Value`] — runtime value representation for the VM stack
//! - [`Program`] — a sequence of instructions
//! - [`DecodeError`] — errors from decoding byte streams
//!
//! # Dependencies
//!
//! This crate uses `thiserror` (compile-time proc-macro, zero runtime cost)
//! and has no other dependencies.

pub mod error;
pub mod instruction;
pub mod opcode;
pub mod program;
pub mod type_tag;
pub mod value;

// Re-export commonly used types at the crate root.
pub use error::DecodeError;
pub use instruction::Instruction;
pub use opcode::Opcode;
pub use program::Program;
pub use type_tag::TypeTag;
pub use value::Value;

#[cfg(test)]
mod proptests {
    use super::*;
    use proptest::prelude::*;

    /// Strategy that generates a random valid Opcode.
    fn arb_opcode() -> impl Strategy<Value = Opcode> {
        prop::sample::select(&opcode::ALL_OPCODES[..])
    }

    /// Strategy that generates a random valid TypeTag.
    fn arb_type_tag() -> impl Strategy<Value = TypeTag> {
        prop::sample::select(&type_tag::ALL_TYPE_TAGS[..])
    }

    /// Strategy that generates a random valid Instruction.
    fn arb_instruction() -> impl Strategy<Value = Instruction> {
        (
            arb_opcode(),
            arb_type_tag(),
            any::<u16>(),
            any::<u16>(),
            any::<u16>(),
        )
            .prop_map(|(op, tt, a1, a2, a3)| Instruction::new(op, tt, a1, a2, a3))
    }

    proptest! {
        /// For all valid instructions, encode then decode produces the original.
        #[test]
        fn encode_decode_roundtrip(instr in arb_instruction()) {
            let bytes = instr.encode();
            let decoded = Instruction::decode(bytes).unwrap();
            prop_assert_eq!(instr, decoded);
        }

        /// For any 8 random bytes, decode either succeeds (and re-encodes
        /// identically) or returns a specific DecodeError.
        #[test]
        fn random_bytes_decode(bytes in prop::array::uniform8(any::<u8>())) {
            match Instruction::decode(bytes) {
                Ok(instr) => {
                    // If decode succeeds, re-encoding must produce the same bytes.
                    prop_assert_eq!(instr.encode(), bytes);
                }
                Err(e) => {
                    // Must be one of our defined error variants.
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

        /// Program encode/decode roundtrip with random valid programs.
        #[test]
        fn program_roundtrip(
            instrs in prop::collection::vec(arb_instruction(), 0..50)
        ) {
            let program = Program::new(instrs);
            let bytes = program.encode();
            let decoded = Program::decode(&bytes).unwrap();
            prop_assert_eq!(program, decoded);
        }
    }
}
