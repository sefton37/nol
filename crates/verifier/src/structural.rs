//! Structural validation pass for NoLang programs.
//!
//! Checks block matching, unused fields, CASE ordering, PARAM validation,
//! and builds the ProgramContext used by later passes.

use crate::error::VerifyError;
use nolang_common::{Instruction, Opcode, TypeTag};

/// Metadata about a function discovered during structural analysis.
#[derive(Debug, Clone)]
pub struct FuncInfo {
    /// Instruction index of the FUNC instruction.
    pub func_pc: usize,
    /// Instruction index of the matching ENDFUNC.
    pub endfunc_pc: usize,
    /// Number of parameters (from FUNC arg1).
    pub param_count: u16,
    /// body_len from FUNC arg2.
    pub body_len: u16,
    /// Parameter types from PARAM instructions.
    pub param_types: Vec<TypeTag>,
    /// PRE condition blocks: (pre_instr_pc, start_pc, len).
    pub pre_conditions: Vec<(usize, usize, u16)>,
    /// POST condition blocks: (post_instr_pc, start_pc, len).
    pub post_conditions: Vec<(usize, usize, u16)>,
    /// Instruction index where the actual body starts (after PARAM/PRE/POST).
    pub body_start_pc: usize,
    /// Instruction index of the HASH instruction, if found.
    pub hash_pc: Option<usize>,
}

/// Metadata about a MATCH block.
#[derive(Debug, Clone)]
pub struct MatchInfo {
    /// Instruction index of the MATCH instruction.
    pub match_pc: usize,
    /// Expected variant count from MATCH arg1.
    pub variant_count: u16,
    /// CASE branches: (case_pc, tag, body_len).
    pub cases: Vec<(usize, u16, u16)>,
    /// Instruction index of the EXHAUST instruction.
    pub exhaust_pc: usize,
}

/// Context built from the structural pass, consumed by later passes.
#[derive(Debug, Clone)]
pub struct ProgramContext {
    /// All function definitions found.
    pub functions: Vec<FuncInfo>,
    /// All match blocks found.
    pub matches: Vec<MatchInfo>,
    /// Entry point (first instruction after last ENDFUNC, or 0).
    pub entry_point: usize,
    /// Whether a fatal structural error occurred (callers should skip later passes).
    pub fatal: bool,
}

/// Run the structural validation pass.
///
/// Returns the ProgramContext and any errors found.
pub fn check_structural(instrs: &[Instruction]) -> (ProgramContext, Vec<VerifyError>) {
    let mut errors = Vec::new();
    let mut functions = Vec::new();
    let mut matches = Vec::new();
    let mut fatal = false;

    // Check: program ends with HALT
    if instrs.is_empty() || instrs.last().map(|i| i.opcode) != Some(Opcode::Halt) {
        errors.push(VerifyError::MissingHalt);
    }

    // Single linear scan for block structure
    let mut pc = 0;
    let mut in_func: Option<usize> = None; // Some(func_pc) if inside FUNC

    while pc < instrs.len() {
        let instr = &instrs[pc];

        match instr.opcode {
            Opcode::Func => {
                if in_func.is_some() {
                    errors.push(VerifyError::NestedFunc { at: pc });
                    fatal = true;
                    pc += 1;
                    continue;
                }

                let param_count = instr.arg1;
                let body_len = instr.arg2 as usize;
                let expected_endfunc = pc + 1 + body_len;

                // Check ENDFUNC is in bounds and correct
                if expected_endfunc >= instrs.len()
                    || instrs[expected_endfunc].opcode != Opcode::EndFunc
                {
                    errors.push(VerifyError::UnmatchedFunc { at: pc });
                    fatal = true;
                    pc += 1;
                    continue;
                }

                // Scan PARAM/PRE/POST inside the function
                let mut scan_pc = pc + 1;
                let mut param_types = Vec::new();

                // Collect PARAMs
                while scan_pc < expected_endfunc && instrs[scan_pc].opcode == Opcode::Param {
                    param_types.push(instrs[scan_pc].type_tag);
                    scan_pc += 1;
                }

                // Validate PARAM count
                if param_types.len() as u16 != param_count {
                    errors.push(VerifyError::ParamCountMismatch {
                        at: pc,
                        expected: param_count,
                        found: param_types.len() as u16,
                    });
                }

                // Collect PRE blocks
                let mut pre_conditions = Vec::new();
                while scan_pc < expected_endfunc && instrs[scan_pc].opcode == Opcode::Pre {
                    let len = instrs[scan_pc].arg1;
                    pre_conditions.push((scan_pc, scan_pc + 1, len));
                    scan_pc += 1 + len as usize;
                }

                // Collect POST blocks
                let mut post_conditions = Vec::new();
                while scan_pc < expected_endfunc && instrs[scan_pc].opcode == Opcode::Post {
                    let len = instrs[scan_pc].arg1;
                    post_conditions.push((scan_pc, scan_pc + 1, len));
                    scan_pc += 1 + len as usize;
                }

                let body_start_pc = scan_pc;

                // Look for HASH instruction (should be second-to-last before ENDFUNC)
                let hash_pc = if expected_endfunc >= 2
                    && instrs[expected_endfunc - 1].opcode == Opcode::Hash
                {
                    Some(expected_endfunc - 1)
                } else {
                    None
                };

                functions.push(FuncInfo {
                    func_pc: pc,
                    endfunc_pc: expected_endfunc,
                    param_count: instr.arg1,
                    body_len: instr.arg2,
                    param_types,
                    pre_conditions,
                    post_conditions,
                    body_start_pc,
                    hash_pc,
                });

                // Now scan inside the function for MATCH blocks
                in_func = Some(pc);
                pc += 1; // Move past FUNC, continue scanning body
                continue;
            }

            Opcode::EndFunc => {
                if in_func.is_none() {
                    errors.push(VerifyError::UnmatchedFunc { at: pc });
                    fatal = true;
                }
                in_func = None;
                pc += 1;
                continue;
            }

            Opcode::Match => {
                let variant_count = instr.arg1;
                let match_pc = pc;
                let mut case_entries = Vec::new();
                let mut scan_pc = pc + 1;

                // Scan CASEs
                for _ in 0..variant_count {
                    if scan_pc >= instrs.len() || instrs[scan_pc].opcode != Opcode::Case {
                        break;
                    }
                    let case_tag = instrs[scan_pc].arg1;
                    let body_len = instrs[scan_pc].arg2;
                    case_entries.push((scan_pc, case_tag, body_len));
                    scan_pc += 1 + body_len as usize;
                }

                // Check EXHAUST
                let exhaust_pc =
                    if scan_pc < instrs.len() && instrs[scan_pc].opcode == Opcode::Exhaust {
                        scan_pc
                    } else {
                        errors.push(VerifyError::UnmatchedMatch { at: match_pc });
                        // Try to continue anyway
                        pc += 1;
                        continue;
                    };

                // Check CASE ascending order
                for i in 1..case_entries.len() {
                    let (prev_pc, prev_tag, _) = case_entries[i - 1];
                    let (cur_pc, cur_tag, _) = case_entries[i];
                    let _ = prev_pc;
                    if cur_tag <= prev_tag {
                        errors.push(VerifyError::CaseOrderViolation {
                            at: cur_pc,
                            expected_tag: prev_tag + 1,
                            found_tag: cur_tag,
                        });
                    }
                }

                matches.push(MatchInfo {
                    match_pc,
                    variant_count,
                    cases: case_entries,
                    exhaust_pc,
                });

                // Skip past the MATCH block — we've already scanned its innards
                pc = exhaust_pc + 1;
                continue;
            }

            _ => {}
        }

        // Check unused fields
        check_unused_fields(instr, pc, &mut errors);

        pc += 1;

        // For CONST_EXT, skip the next instruction (it's a data payload)
        if instr.opcode == Opcode::ConstExt && pc < instrs.len() {
            pc += 1;
        }
    }

    // If we're still inside a FUNC at EOF, that's unmatched
    if let Some(func_pc) = in_func {
        errors.push(VerifyError::UnmatchedFunc { at: func_pc });
        fatal = true;
    }

    // Find entry point
    let entry_point = functions.last().map(|f| f.endfunc_pc + 1).unwrap_or(0);

    let ctx = ProgramContext {
        functions,
        matches,
        entry_point,
        fatal,
    };

    (ctx, errors)
}

/// Check that unused argument fields are zero for the given instruction.
fn check_unused_fields(instr: &Instruction, at: usize, errors: &mut Vec<VerifyError>) {
    // Per SPEC.md: "Unused argument fields MUST be zero."
    // We define which fields are "used" per opcode.
    let (used_tt, used_a1, used_a2, used_a3) = match instr.opcode {
        // Binding & Reference
        Opcode::Bind => (false, false, false, false),
        Opcode::Ref => (false, true, false, false),
        Opcode::Drop => (false, false, false, false),

        // Constants
        Opcode::Const => (true, true, true, false),
        Opcode::ConstExt => (true, true, false, false),

        // Arithmetic (no args)
        Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div | Opcode::Mod | Opcode::Neg => {
            (false, false, false, false)
        }

        // Comparison (no args)
        Opcode::Eq | Opcode::Neq | Opcode::Lt | Opcode::Gt | Opcode::Lte | Opcode::Gte => {
            (false, false, false, false)
        }

        // Logic & Bitwise (no args)
        Opcode::And | Opcode::Or | Opcode::Not | Opcode::Xor | Opcode::Shl | Opcode::Shr => {
            (false, false, false, false)
        }

        // Pattern Matching
        Opcode::Match => (false, true, false, false),
        Opcode::Case => (false, true, true, false),
        Opcode::Exhaust => (false, false, false, false),

        // Functions
        Opcode::Func => (false, true, true, false),
        Opcode::Pre => (false, true, false, false),
        Opcode::Post => (false, true, false, false),
        Opcode::Ret => (false, false, false, false),
        Opcode::Call => (false, true, false, false),
        Opcode::Recurse => (false, true, false, false),
        Opcode::EndFunc => (false, false, false, false),
        Opcode::Param => (true, false, false, false),

        // Data Construction
        Opcode::VariantNew => (true, true, true, false),
        Opcode::TupleNew => (true, true, false, false),
        Opcode::Project => (false, true, false, false),
        Opcode::ArrayNew => (true, true, false, false),
        Opcode::ArrayGet => (false, false, false, false),
        Opcode::ArrayLen => (false, false, false, false),

        // Verification & Meta
        Opcode::Hash => (false, true, true, true), // All 3 args are the hash
        Opcode::Assert => (false, false, false, false),
        Opcode::Typeof => (false, true, false, false),

        // VM Control
        Opcode::Halt => (false, false, false, false),
        Opcode::Nop => (false, false, false, false),
    };

    let has_nonzero = (!used_tt && instr.type_tag != TypeTag::None)
        || (!used_a1 && instr.arg1 != 0)
        || (!used_a2 && instr.arg2 != 0)
        || (!used_a3 && instr.arg3 != 0);

    if has_nonzero {
        errors.push(VerifyError::NonZeroUnusedField { at });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nolang_common::Instruction;

    fn instr(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Instruction {
        Instruction::new(opcode, type_tag, arg1, arg2, arg3)
    }

    #[test]
    fn empty_program_reports_missing_halt() {
        let (_, errors) = check_structural(&[]);
        assert!(errors.iter().any(|e| matches!(e, VerifyError::MissingHalt)));
    }

    #[test]
    fn minimal_valid_program() {
        let instrs = [
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, errors) = check_structural(&instrs);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(ctx.entry_point, 0);
        assert!(ctx.functions.is_empty());
    }

    #[test]
    fn function_with_param() {
        let instrs = [
            // FUNC 1 param, 4 body instructions
            instr(Opcode::Func, TypeTag::None, 1, 4, 0),
            instr(Opcode::Param, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Ref, TypeTag::None, 0, 0, 0),
            instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
            instr(Opcode::Hash, TypeTag::None, 0, 0, 0),
            instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            instr(Opcode::Call, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, errors) = check_structural(&instrs);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(ctx.functions.len(), 1);
        assert_eq!(ctx.functions[0].param_types, vec![TypeTag::I64]);
        assert_eq!(ctx.entry_point, 6);
    }

    #[test]
    fn param_count_mismatch() {
        let instrs = [
            // FUNC says 2 params, but only 1 PARAM instruction
            instr(Opcode::Func, TypeTag::None, 2, 3, 0),
            instr(Opcode::Param, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
            instr(Opcode::Hash, TypeTag::None, 0, 0, 0),
            instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (_, errors) = check_structural(&instrs);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::ParamCountMismatch { .. })));
    }

    #[test]
    fn unmatched_func_reports_error() {
        let instrs = [
            instr(Opcode::Func, TypeTag::None, 0, 100, 0), // body_len too large
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, errors) = check_structural(&instrs);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::UnmatchedFunc { .. })));
        assert!(ctx.fatal);
    }

    #[test]
    fn nested_func_reports_error() {
        // Outer FUNC has body_len=6, so ENDFUNC at index 7.
        // Inner FUNC at index 1 is nested inside the outer.
        let instrs = [
            instr(Opcode::Func, TypeTag::None, 0, 6, 0), // 0: outer FUNC
            instr(Opcode::Func, TypeTag::None, 0, 1, 0), // 1: nested FUNC!
            instr(Opcode::Ret, TypeTag::None, 0, 0, 0),  // 2
            instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0), // 3: inner ENDFUNC
            instr(Opcode::Nop, TypeTag::None, 0, 0, 0),  // 4
            instr(Opcode::Ret, TypeTag::None, 0, 0, 0),  // 5
            instr(Opcode::Hash, TypeTag::None, 0, 0, 0), // 6
            instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0), // 7: outer ENDFUNC
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0), // 8
        ];
        let (_, errors) = check_structural(&instrs);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::NestedFunc { .. })));
    }

    #[test]
    fn match_with_correct_cases() {
        let instrs = [
            instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
            instr(Opcode::Match, TypeTag::None, 2, 0, 0),
            instr(Opcode::Case, TypeTag::None, 0, 1, 0), // tag 0, 1 instr
            instr(Opcode::Const, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Case, TypeTag::None, 1, 1, 0), // tag 1, 1 instr
            instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
            instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, errors) = check_structural(&instrs);
        assert!(errors.is_empty(), "unexpected errors: {errors:?}");
        assert_eq!(ctx.matches.len(), 1);
        assert_eq!(ctx.matches[0].cases.len(), 2);
    }

    #[test]
    fn case_order_violation() {
        let instrs = [
            instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
            instr(Opcode::Match, TypeTag::None, 2, 0, 0),
            instr(Opcode::Case, TypeTag::None, 1, 1, 0), // tag 1 first!
            instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
            instr(Opcode::Case, TypeTag::None, 0, 1, 0), // tag 0 second!
            instr(Opcode::Const, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (_, errors) = check_structural(&instrs);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::CaseOrderViolation { .. })));
    }

    #[test]
    fn nonzero_unused_field_detected() {
        let instrs = [
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            // ADD has no args, but arg1 is nonzero
            Instruction::new(Opcode::Add, TypeTag::None, 1, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (_, errors) = check_structural(&instrs);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::NonZeroUnusedField { .. })));
    }
}
