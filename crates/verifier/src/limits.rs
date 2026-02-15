//! Limits checking for NoLang programs.
//!
//! Enforces hard limits from SPEC.md Section 9.

use crate::error::VerifyError;
use nolang_common::{Instruction, Opcode};

/// Maximum program size in instructions.
pub const MAX_PROGRAM_SIZE: usize = 65_536;

/// Maximum REF index.
pub const MAX_REF_INDEX: u16 = 4_096;

/// Maximum RECURSE depth limit.
pub const MAX_RECURSION_LIMIT: u16 = 1_024;

/// Run the limits check.
pub fn check_limits(instrs: &[Instruction]) -> Vec<VerifyError> {
    let mut errors = Vec::new();

    if instrs.len() > MAX_PROGRAM_SIZE {
        errors.push(VerifyError::ProgramTooLarge { size: instrs.len() });
    }

    for (i, instr) in instrs.iter().enumerate() {
        match instr.opcode {
            Opcode::Ref if instr.arg1 > MAX_REF_INDEX => {
                errors.push(VerifyError::RefTooDeep {
                    at: i,
                    index: instr.arg1,
                });
            }
            Opcode::Recurse if instr.arg1 > MAX_RECURSION_LIMIT => {
                errors.push(VerifyError::RecursionLimitTooHigh {
                    at: i,
                    limit: instr.arg1,
                });
            }
            _ => {}
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use nolang_common::{Instruction, Opcode, TypeTag};

    fn instr(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Instruction {
        Instruction::new(opcode, type_tag, arg1, arg2, arg3)
    }

    #[test]
    fn small_program_passes() {
        let instrs = [
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let errors = check_limits(&instrs);
        assert!(errors.is_empty());
    }

    #[test]
    fn ref_too_deep() {
        let instrs = [
            instr(Opcode::Ref, TypeTag::None, 5000, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let errors = check_limits(&instrs);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::RefTooDeep { index: 5000, .. })));
    }

    #[test]
    fn recursion_limit_too_high() {
        let instrs = [
            instr(Opcode::Recurse, TypeTag::None, 2000, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let errors = check_limits(&instrs);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::RecursionLimitTooHigh { limit: 2000, .. })));
    }
}
