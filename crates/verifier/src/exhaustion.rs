//! Exhaustion checking for pattern matches.
//!
//! Verifies that every MATCH block has exactly the right number of CASE
//! branches, no duplicates, and no missing tags.

use crate::error::VerifyError;
use crate::structural::ProgramContext;

/// Run the exhaustion check.
pub fn check_exhaustion(ctx: &ProgramContext) -> Vec<VerifyError> {
    let mut errors = Vec::new();

    for match_info in &ctx.matches {
        let expected = match_info.variant_count;
        let found = match_info.cases.len() as u16;

        // Check count
        if found != expected {
            errors.push(VerifyError::NonExhaustiveMatch {
                at: match_info.match_pc,
                expected,
                found,
            });
        }

        // Check for duplicates
        let mut seen_tags = Vec::new();
        for &(case_pc, tag, _) in &match_info.cases {
            if seen_tags.contains(&tag) {
                errors.push(VerifyError::DuplicateCase { at: case_pc, tag });
            } else {
                seen_tags.push(tag);
            }
        }
    }

    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural::{check_structural, MatchInfo};
    use nolang_common::{Instruction, Opcode, TypeTag};

    fn instr(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Instruction {
        Instruction::new(opcode, type_tag, arg1, arg2, arg3)
    }

    #[test]
    fn correct_exhaustion_no_errors() {
        let instrs = [
            instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
            instr(Opcode::Match, TypeTag::None, 2, 0, 0),
            instr(Opcode::Case, TypeTag::None, 0, 1, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Case, TypeTag::None, 1, 1, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
            instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_exhaustion(&ctx);
        assert!(errors.is_empty());
    }

    #[test]
    fn missing_case_reports_non_exhaustive() {
        // Create a context with mismatched case count
        let ctx = ProgramContext {
            functions: vec![],
            matches: vec![MatchInfo {
                match_pc: 1,
                variant_count: 3,
                cases: vec![(2, 0, 1), (4, 1, 1)], // only 2 of 3
                exhaust_pc: 6,
            }],
            entry_point: 0,
            fatal: false,
        };
        let errors = check_exhaustion(&ctx);
        assert!(errors.iter().any(|e| matches!(
            e,
            VerifyError::NonExhaustiveMatch {
                expected: 3,
                found: 2,
                ..
            }
        )));
    }

    #[test]
    fn duplicate_case_detected() {
        let ctx = ProgramContext {
            functions: vec![],
            matches: vec![MatchInfo {
                match_pc: 1,
                variant_count: 2,
                cases: vec![(2, 0, 1), (4, 0, 1)], // duplicate tag 0
                exhaust_pc: 6,
            }],
            entry_point: 0,
            fatal: false,
        };
        let errors = check_exhaustion(&ctx);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::DuplicateCase { tag: 0, .. })));
    }
}
