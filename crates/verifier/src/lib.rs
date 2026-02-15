//! NoLang verifier — static analysis for instruction streams.
//!
//! The verifier checks a `Program` for correctness BEFORE execution.
//! It collects ALL errors (not just the first) and returns them.
//!
//! # Usage
//!
//! ```
//! use nolang_common::{Instruction, Opcode, TypeTag, Program};
//! use nolang_verifier::verify;
//!
//! let program = Program::new(vec![
//!     Instruction::new(Opcode::Const, TypeTag::I64, 0, 42, 0),
//!     Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
//! ]);
//!
//! let result = verify(&program);
//! assert!(result.is_ok());
//! ```
//!
//! # Passes
//!
//! 1. **Limits** — program size, REF depth, RECURSE limit
//! 2. **Structural** — block matching, unused fields, PARAM validation
//! 3. **Exhaustion** — pattern match completeness
//! 4. **Hashing** — blake3 verification
//! 5. **Types** — type checking with PARAM info
//! 6. **Contracts** — PRE/POST produce Bool
//! 7. **Stack** — stack balance analysis
//! 8. **Reachability** — dead code detection

pub mod contracts;
pub mod error;
pub mod exhaustion;
pub mod hashing;
pub mod limits;
pub mod reachability;
pub mod stack;
pub mod structural;
pub mod types;

pub use error::VerifyError;

use nolang_common::Program;

/// Verify a program for correctness.
///
/// Returns `Ok(())` if the program passes all checks, or
/// `Err(Vec<VerifyError>)` with all errors found.
///
/// If the structural pass finds fatal errors (e.g., unmatched FUNC/ENDFUNC),
/// later passes that depend on structural context are skipped.
pub fn verify(program: &Program) -> Result<(), Vec<VerifyError>> {
    let instrs = &program.instructions;
    let mut all_errors = Vec::new();

    // Pass 1: Limits (independent)
    all_errors.extend(limits::check_limits(instrs));

    // Pass 2: Structural (builds ProgramContext)
    let (ctx, structural_errors) = structural::check_structural(instrs);
    all_errors.extend(structural_errors);

    // If structural pass found fatal errors, skip dependent passes
    if !ctx.fatal {
        // Pass 3: Exhaustion
        all_errors.extend(exhaustion::check_exhaustion(&ctx));

        // Pass 4: Hashing
        all_errors.extend(hashing::check_hashing(instrs, &ctx));

        // Pass 5: Types
        all_errors.extend(types::check_types(instrs, &ctx));

        // Pass 6: Contracts
        all_errors.extend(contracts::check_contracts(instrs, &ctx));

        // Pass 7: Stack
        all_errors.extend(stack::check_stack(instrs, &ctx));

        // Pass 8: Reachability
        all_errors.extend(reachability::check_reachability(instrs, &ctx));
    }

    if all_errors.is_empty() {
        Ok(())
    } else {
        Err(all_errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nolang_common::{Instruction, Opcode, TypeTag};

    fn instr(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Instruction {
        Instruction::new(opcode, type_tag, arg1, arg2, arg3)
    }

    #[test]
    fn minimal_valid_program() {
        let program = Program::new(vec![
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        assert!(verify(&program).is_ok());
    }

    #[test]
    fn missing_halt() {
        let program = Program::new(vec![instr(Opcode::Const, TypeTag::I64, 0, 42, 0)]);
        let errors = verify(&program).unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, VerifyError::MissingHalt)));
    }

    #[test]
    fn empty_program() {
        let program = Program::new(vec![]);
        let errors = verify(&program).unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, VerifyError::MissingHalt)));
    }

    #[test]
    fn multiple_errors_collected() {
        let program = Program::new(vec![
            // REF too deep + no HALT + stack underflow
            instr(Opcode::Ref, TypeTag::None, 5000, 0, 0),
        ]);
        let errors = verify(&program).unwrap_err();
        // Should have at least MissingHalt and RefTooDeep
        assert!(
            errors.len() >= 2,
            "expected multiple errors, got: {errors:?}"
        );
    }

    #[test]
    fn arithmetic_valid() {
        let program = Program::new(vec![
            instr(Opcode::Const, TypeTag::I64, 0, 5, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 3, 0),
            instr(Opcode::Add, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        assert!(verify(&program).is_ok());
    }

    #[test]
    fn boolean_match_valid() {
        let program = Program::new(vec![
            instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
            instr(Opcode::Match, TypeTag::None, 2, 0, 0),
            instr(Opcode::Case, TypeTag::None, 0, 1, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Case, TypeTag::None, 1, 1, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
            instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        assert!(verify(&program).is_ok());
    }

    #[test]
    fn case_order_violation() {
        let program = Program::new(vec![
            instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
            instr(Opcode::Match, TypeTag::None, 2, 0, 0),
            instr(Opcode::Case, TypeTag::None, 1, 1, 0), // wrong order
            instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
            instr(Opcode::Case, TypeTag::None, 0, 1, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        let errors = verify(&program).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::CaseOrderViolation { .. })));
    }

    #[test]
    fn unresolvable_ref() {
        let program = Program::new(vec![
            instr(Opcode::Ref, TypeTag::None, 5, 0, 0), // no bindings
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        let errors = verify(&program).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::UnresolvableRef { .. })));
    }

    #[test]
    fn type_mismatch_i64_plus_f64() {
        let program = Program::new(vec![
            instr(Opcode::Const, TypeTag::I64, 0, 5, 0),
            instr(Opcode::Const, TypeTag::U64, 0, 3, 0),
            instr(Opcode::Add, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        let errors = verify(&program).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::TypeMismatch { .. })));
    }

    #[test]
    fn non_exhaustive_match() {
        let program = Program::new(vec![
            instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
            instr(Opcode::Match, TypeTag::None, 3, 0, 0), // expects 3 cases
            instr(Opcode::Case, TypeTag::None, 0, 1, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Case, TypeTag::None, 1, 1, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
            instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        let errors = verify(&program).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::NonExhaustiveMatch { .. })));
    }

    #[test]
    fn stack_underflow() {
        let program = Program::new(vec![
            instr(Opcode::Add, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        let errors = verify(&program).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::StackUnderflow { .. })));
    }

    #[test]
    fn unbalanced_stack_at_halt() {
        let program = Program::new(vec![
            instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 2, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        let errors = verify(&program).unwrap_err();
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::UnbalancedStack { .. })));
    }
}
