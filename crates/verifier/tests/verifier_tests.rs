//! Integration tests for the NoLang verifier.
//!
//! Tests map directly to BUILD_ORDER.md acceptance criteria.

use nolang_common::{Instruction, Opcode, Program, TypeTag};
use nolang_verifier::{verify, VerifyError};

fn instr(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Instruction {
    Instruction::new(opcode, type_tag, arg1, arg2, arg3)
}

/// Compute the correct blake3 HASH instruction for instructions [func_pc..hash_pc].
fn compute_hash(instrs: &[Instruction], func_pc: usize, hash_pc: usize) -> Instruction {
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

// ========================================================
// Acceptance Criteria: Valid programs pass verification
// ========================================================

#[test]
fn accept_minimal_program() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    assert!(verify(&p).is_ok());
}

#[test]
fn accept_arithmetic_program() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::I64, 0, 5, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 3, 0),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    assert!(verify(&p).is_ok());
}

#[test]
fn accept_boolean_match() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
        instr(Opcode::Match, TypeTag::None, 2, 0, 0),
        instr(Opcode::Case, TypeTag::None, 0, 1, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 0, 0),
        instr(Opcode::Case, TypeTag::None, 1, 1, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    assert!(verify(&p).is_ok());
}

#[test]
fn accept_bind_ref_drop() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
        instr(Opcode::Bind, TypeTag::None, 0, 0, 0),
        instr(Opcode::Ref, TypeTag::None, 0, 0, 0),
        instr(Opcode::Drop, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    assert!(verify(&p).is_ok());
}

#[test]
fn accept_function_with_correct_hash() {
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
    instrs[4] = compute_hash(&instrs, 0, 4);
    let p = Program::new(instrs);
    assert!(verify(&p).is_ok());
}

// ========================================================
// Acceptance Criteria: Missing HALT → MissingHalt
// ========================================================

#[test]
fn reject_missing_halt() {
    let p = Program::new(vec![instr(Opcode::Const, TypeTag::I64, 0, 42, 0)]);
    let errors = verify(&p).unwrap_err();
    assert!(errors.iter().any(|e| matches!(e, VerifyError::MissingHalt)));
}

// ========================================================
// Acceptance Criteria: Unmatched FUNC → UnmatchedFunc
// ========================================================

#[test]
fn reject_unmatched_func() {
    let p = Program::new(vec![
        instr(Opcode::Func, TypeTag::None, 0, 100, 0), // body_len exceeds program
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::UnmatchedFunc { .. })));
}

// ========================================================
// Acceptance Criteria: CASE out of order → CaseOrderViolation
// ========================================================

#[test]
fn reject_case_order_violation() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
        instr(Opcode::Match, TypeTag::None, 2, 0, 0),
        instr(Opcode::Case, TypeTag::None, 1, 1, 0), // wrong order
        instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
        instr(Opcode::Case, TypeTag::None, 0, 1, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 0, 0),
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::CaseOrderViolation { .. })));
}

// ========================================================
// Acceptance Criteria: REF beyond binding depth → UnresolvableRef
// ========================================================

#[test]
fn reject_unresolvable_ref() {
    let p = Program::new(vec![
        instr(Opcode::Ref, TypeTag::None, 5, 0, 0), // no bindings
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::UnresolvableRef { .. })));
}

// ========================================================
// Acceptance Criteria: I64 + F64 arithmetic → TypeMismatch
// ========================================================

#[test]
fn reject_mixed_type_arithmetic() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::I64, 0, 5, 0),
        instr(Opcode::Const, TypeTag::U64, 0, 3, 0),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::TypeMismatch { .. })));
}

// ========================================================
// Acceptance Criteria: Non-exhaustive MATCH → NonExhaustiveMatch
// ========================================================

#[test]
fn reject_non_exhaustive_match() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
        instr(Opcode::Match, TypeTag::None, 3, 0, 0), // expects 3 but only 2
        instr(Opcode::Case, TypeTag::None, 0, 1, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 0, 0),
        instr(Opcode::Case, TypeTag::None, 1, 1, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::NonExhaustiveMatch { .. })));
}

// ========================================================
// Acceptance Criteria: Wrong hash → HashMismatch
// ========================================================

#[test]
fn reject_wrong_hash() {
    let p = Program::new(vec![
        instr(Opcode::Func, TypeTag::None, 1, 4, 0),
        instr(Opcode::Param, TypeTag::I64, 0, 0, 0),
        instr(Opcode::Ref, TypeTag::None, 0, 0, 0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::Hash, TypeTag::None, 0xDEAD, 0xBEEF, 0xCAFE),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
        instr(Opcode::Call, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::HashMismatch { .. })));
}

// ========================================================
// Acceptance Criteria: Missing hash → MissingHash
// ========================================================

#[test]
fn reject_missing_hash() {
    let p = Program::new(vec![
        instr(Opcode::Func, TypeTag::None, 0, 3, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::Nop, TypeTag::None, 0, 0, 0), // Not Hash
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::MissingHash { .. })));
}

// ========================================================
// Acceptance Criteria: Unreachable code → UnreachableInstruction
// ========================================================

#[test]
fn reject_unreachable_code() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        instr(Opcode::Nop, TypeTag::None, 0, 0, 0), // unreachable
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::UnreachableInstruction { .. })));
}

// ========================================================
// Acceptance Criteria: Stack underflow → StackUnderflow
// ========================================================

#[test]
fn reject_stack_underflow() {
    let p = Program::new(vec![
        instr(Opcode::Add, TypeTag::None, 0, 0, 0), // empty stack
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::StackUnderflow { .. })));
}

// ========================================================
// Acceptance Criteria: Multiple values at HALT → UnbalancedStack
// ========================================================

#[test]
fn reject_unbalanced_stack() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 2, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::UnbalancedStack { .. })));
}

// ========================================================
// Acceptance Criteria: All errors collected (3+ in one program)
// ========================================================

#[test]
fn collect_multiple_errors() {
    let p = Program::new(vec![
        // REF too deep (limit violation) + unresolvable ref (type check) + no HALT
        instr(Opcode::Ref, TypeTag::None, 5000, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    // Should have at least: MissingHalt, RefTooDeep, UnresolvableRef
    assert!(
        errors.len() >= 3,
        "expected 3+ errors, got {}: {errors:?}",
        errors.len()
    );

    let has_missing_halt = errors.iter().any(|e| matches!(e, VerifyError::MissingHalt));
    let has_ref_too_deep = errors
        .iter()
        .any(|e| matches!(e, VerifyError::RefTooDeep { .. }));
    let has_unresolvable = errors
        .iter()
        .any(|e| matches!(e, VerifyError::UnresolvableRef { .. }));

    assert!(has_missing_halt, "missing MissingHalt");
    assert!(has_ref_too_deep, "missing RefTooDeep");
    assert!(has_unresolvable, "missing UnresolvableRef");
}

// ========================================================
// Acceptance Criteria: Verifier never panics (random programs)
// ========================================================

#[test]
fn fuzz_verifier_never_panics() {
    use std::time::Instant;

    let start = Instant::now();
    let mut count = 0;

    // Use a simple PRNG (xorshift) for speed
    let mut state: u64 = 0xDEAD_BEEF_CAFE_BABE;
    let mut next_u64 = || -> u64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    // Run for 10,000 random programs or 60 seconds, whichever comes first
    while count < 10_000 && start.elapsed().as_secs() < 60 {
        let len = (next_u64() % 50 + 1) as usize;
        let mut instrs = Vec::with_capacity(len);

        for _ in 0..len {
            let raw = next_u64();
            let bytes = raw.to_le_bytes();
            // Try to decode; if it fails, make a valid NOP
            let instr = match Instruction::decode(bytes) {
                Ok(i) => i,
                Err(_) => Instruction::new(Opcode::Nop, TypeTag::None, 0, 0, 0),
            };
            instrs.push(instr);
        }

        let program = Program::new(instrs);
        // Must not panic — Ok or Err are both fine
        let _ = verify(&program);
        count += 1;
    }

    assert!(count >= 10_000, "only ran {count} programs in 60 seconds");
}

// ========================================================
// Additional: PARAM validation
// ========================================================

#[test]
fn reject_param_count_mismatch() {
    let p = Program::new(vec![
        instr(Opcode::Func, TypeTag::None, 2, 3, 0), // says 2 params
        instr(Opcode::Param, TypeTag::I64, 0, 0, 0), // only 1 PARAM
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::Hash, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::ParamCountMismatch { .. })));
}

#[test]
fn reject_nonzero_unused_field() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
        // ADD has no used args, but arg3 is nonzero
        Instruction::new(Opcode::Add, TypeTag::None, 0, 0, 1),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::NonZeroUnusedField { .. })));
}

// ========================================================
// Additional: Limits
// ========================================================

#[test]
fn reject_program_too_large() {
    let mut instrs: Vec<Instruction> = (0..65_537)
        .map(|_| instr(Opcode::Nop, TypeTag::None, 0, 0, 0))
        .collect();
    if let Some(last) = instrs.last_mut() {
        *last = instr(Opcode::Halt, TypeTag::None, 0, 0, 0);
    }
    let p = Program::new(instrs);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::ProgramTooLarge { .. })));
}

#[test]
fn reject_recursion_limit_too_high() {
    let p = Program::new(vec![
        instr(Opcode::Recurse, TypeTag::None, 2000, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::RecursionLimitTooHigh { .. })));
}

// ========================================================
// Additional: Nested FUNC
// ========================================================

#[test]
fn reject_nested_func() {
    let p = Program::new(vec![
        instr(Opcode::Func, TypeTag::None, 0, 6, 0),
        instr(Opcode::Func, TypeTag::None, 0, 1, 0), // nested!
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        instr(Opcode::Nop, TypeTag::None, 0, 0, 0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::Hash, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::NestedFunc { .. })));
}

// ========================================================
// Additional: Duplicate CASE
// ========================================================

#[test]
fn reject_duplicate_case() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
        instr(Opcode::Match, TypeTag::None, 2, 0, 0),
        instr(Opcode::Case, TypeTag::None, 0, 1, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 0, 0),
        instr(Opcode::Case, TypeTag::None, 0, 1, 0), // duplicate tag 0
        instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    let errors = verify(&p).unwrap_err();
    assert!(errors
        .iter()
        .any(|e| matches!(e, VerifyError::DuplicateCase { .. })));
}

// ========================================================
// Additional: Tuple and Array
// ========================================================

#[test]
fn accept_tuple_program() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::I64, 0, 3, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 7, 0),
        instr(Opcode::TupleNew, TypeTag::Tuple, 2, 0, 0),
        instr(Opcode::Project, TypeTag::None, 1, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    assert!(verify(&p).is_ok());
}

#[test]
fn accept_array_program() {
    let p = Program::new(vec![
        instr(Opcode::Const, TypeTag::I64, 0, 10, 0),
        instr(Opcode::Const, TypeTag::I64, 0, 20, 0),
        instr(Opcode::ArrayNew, TypeTag::Array, 2, 0, 0),
        instr(Opcode::Const, TypeTag::U64, 0, 0, 0),
        instr(Opcode::ArrayGet, TypeTag::None, 0, 0, 0),
        instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ]);
    assert!(verify(&p).is_ok());
}
