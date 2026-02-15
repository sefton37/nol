//! Hash verification for FUNC blocks.
//!
//! Every FUNC block must contain a HASH instruction with the correct
//! 48-bit truncated blake3 hash of the block body.

use crate::error::VerifyError;
use crate::structural::ProgramContext;
use nolang_common::Instruction;

/// Compute the correct blake3 HASH instruction for a FUNC block.
///
/// The hash covers all instruction bytes from `func_pc` (inclusive)
/// through the instruction before `hash_pc` (inclusive).
/// Returns a HASH instruction with the 48-bit truncated hash.
pub fn compute_func_hash(instrs: &[Instruction], func_pc: usize, hash_pc: usize) -> Instruction {
    let mut data = Vec::new();
    for instr in &instrs[func_pc..hash_pc] {
        data.extend_from_slice(&instr.encode());
    }
    let hash = blake3::hash(&data);
    let bytes = hash.as_bytes();
    let arg1 = u16::from_be_bytes([bytes[0], bytes[1]]);
    let arg2 = u16::from_be_bytes([bytes[2], bytes[3]]);
    let arg3 = u16::from_be_bytes([bytes[4], bytes[5]]);
    Instruction::new(
        nolang_common::Opcode::Hash,
        nolang_common::TypeTag::None,
        arg1,
        arg2,
        arg3,
    )
}

/// Run the hash verification pass.
pub fn check_hashing(instrs: &[Instruction], ctx: &ProgramContext) -> Vec<VerifyError> {
    let mut errors = Vec::new();

    for func in &ctx.functions {
        match func.hash_pc {
            None => {
                errors.push(VerifyError::MissingHash {
                    func_at: func.func_pc,
                });
            }
            Some(hash_pc) => {
                // Compute hash over FUNC (inclusive) through instruction before HASH (inclusive)
                // Per SPEC.md Section 6:
                // 1. Collect all instruction bytes from FUNC through the instruction before HASH
                // 2. Compute blake3
                // 3. Truncate to 48 bits (first 6 bytes)
                let mut data = Vec::new();
                for instr in &instrs[func.func_pc..hash_pc] {
                    data.extend_from_slice(&instr.encode());
                }

                let hash = blake3::hash(&data);
                let hash_bytes = hash.as_bytes();
                let computed: [u8; 6] = [
                    hash_bytes[0],
                    hash_bytes[1],
                    hash_bytes[2],
                    hash_bytes[3],
                    hash_bytes[4],
                    hash_bytes[5],
                ];

                // Extract stored hash from HASH instruction args
                // Per SPEC.md: arg1, arg2, arg3 as big-endian for the 48-bit value
                let hash_instr = &instrs[hash_pc];
                let a1 = hash_instr.arg1.to_be_bytes();
                let a2 = hash_instr.arg2.to_be_bytes();
                let a3 = hash_instr.arg3.to_be_bytes();
                let expected: [u8; 6] = [a1[0], a1[1], a2[0], a2[1], a3[0], a3[1]];

                if expected != computed {
                    errors.push(VerifyError::HashMismatch {
                        at: hash_pc,
                        expected,
                        computed,
                    });
                }
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural::check_structural;
    use nolang_common::{Instruction, Opcode, TypeTag};

    fn instr(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Instruction {
        Instruction::new(opcode, type_tag, arg1, arg2, arg3)
    }

    /// Compute the correct hash for a function and return the HASH instruction.
    fn compute_hash_instr(instrs: &[Instruction], func_pc: usize, hash_pc: usize) -> Instruction {
        let mut data = Vec::new();
        for instr in &instrs[func_pc..hash_pc] {
            data.extend_from_slice(&instr.encode());
        }
        let hash = blake3::hash(&data);
        let bytes = hash.as_bytes();
        let arg1 = u16::from_be_bytes([bytes[0], bytes[1]]);
        let arg2 = u16::from_be_bytes([bytes[2], bytes[3]]);
        let arg3 = u16::from_be_bytes([bytes[4], bytes[5]]);
        Instruction::new(Opcode::Hash, TypeTag::None, arg1, arg2, arg3)
    }

    #[test]
    fn correct_hash_passes() {
        let mut instrs = vec![
            instr(Opcode::Func, TypeTag::None, 1, 4, 0),
            instr(Opcode::Param, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Ref, TypeTag::None, 0, 0, 0),
            instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
            instr(Opcode::Hash, TypeTag::None, 0, 0, 0), // placeholder
            instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            instr(Opcode::Call, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        // Replace placeholder with correct hash
        instrs[4] = compute_hash_instr(&instrs, 0, 4);

        let (ctx, structural_errors) = check_structural(&instrs);
        assert!(structural_errors.is_empty(), "{structural_errors:?}");
        let errors = check_hashing(&instrs, &ctx);
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn wrong_hash_detected() {
        let instrs = vec![
            instr(Opcode::Func, TypeTag::None, 1, 4, 0),
            instr(Opcode::Param, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Ref, TypeTag::None, 0, 0, 0),
            instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
            instr(Opcode::Hash, TypeTag::None, 0xDEAD, 0xBEEF, 0xCAFE), // wrong hash
            instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            instr(Opcode::Call, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_hashing(&instrs, &ctx);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::HashMismatch { .. })));
    }

    #[test]
    fn missing_hash_detected() {
        let instrs = vec![
            // FUNC with 3 body instrs, but no HASH — RET is second-to-last
            instr(Opcode::Func, TypeTag::None, 0, 3, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
            instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
            instr(Opcode::Nop, TypeTag::None, 0, 0, 0),
            instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_hashing(&instrs, &ctx);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::MissingHash { .. })));
    }
}
