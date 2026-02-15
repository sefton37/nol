//! Reachability analysis for NoLang programs.
//!
//! Every instruction must be reachable from either the entry point
//! or a function entry point. Unreachable instructions are errors.

use crate::error::VerifyError;
use crate::structural::ProgramContext;
use nolang_common::{Instruction, Opcode};

/// Run the reachability check.
pub fn check_reachability(instrs: &[Instruction], ctx: &ProgramContext) -> Vec<VerifyError> {
    let mut errors = Vec::new();
    let len = instrs.len();
    if len == 0 {
        return errors;
    }

    let mut reachable = vec![false; len];

    // Mark entry point region as reachable
    mark_reachable(instrs, ctx, ctx.entry_point, len, &mut reachable);

    // Mark each function body as reachable
    for func in &ctx.functions {
        // FUNC instruction itself is reachable
        reachable[func.func_pc] = true;
        // ENDFUNC is reachable
        reachable[func.endfunc_pc] = true;

        // PARAM instructions
        let mut pc = func.func_pc + 1;
        while pc < func.endfunc_pc && instrs[pc].opcode == Opcode::Param {
            reachable[pc] = true;
            pc += 1;
        }

        // PRE blocks
        for &(pre_pc, start, len) in &func.pre_conditions {
            reachable[pre_pc] = true;
            let end = (start + len as usize).min(instrs.len());
            for flag in &mut reachable[start..end] {
                *flag = true;
            }
        }

        // POST blocks
        for &(post_pc, start, len) in &func.post_conditions {
            reachable[post_pc] = true;
            let end = (start + len as usize).min(instrs.len());
            for flag in &mut reachable[start..end] {
                *flag = true;
            }
        }

        // Function body
        mark_reachable(
            instrs,
            ctx,
            func.body_start_pc,
            func.endfunc_pc,
            &mut reachable,
        );

        // HASH instruction
        if let Some(hash_pc) = func.hash_pc {
            reachable[hash_pc] = true;
        }
    }

    // Report unreachable instructions
    for (i, &is_reachable) in reachable.iter().enumerate() {
        if !is_reachable {
            errors.push(VerifyError::UnreachableInstruction { at: i });
        }
    }

    errors
}

/// Mark instructions as reachable starting from `start` up to `end`.
fn mark_reachable(
    instrs: &[Instruction],
    ctx: &ProgramContext,
    start: usize,
    end: usize,
    reachable: &mut [bool],
) {
    let mut pc = start;
    while pc < end && pc < instrs.len() {
        if reachable[pc] {
            // Already visited (loop prevention)
            pc += 1;
            continue;
        }
        reachable[pc] = true;

        let instr = &instrs[pc];
        match instr.opcode {
            Opcode::Halt | Opcode::Ret => {
                return; // Execution stops here
            }
            Opcode::ConstExt => {
                // Mark both this and next instruction
                if pc + 1 < instrs.len() {
                    reachable[pc + 1] = true;
                }
                pc += 2;
            }
            Opcode::Match => {
                // Mark all CASE bodies and EXHAUST as reachable
                if let Some(mi) = ctx.matches.iter().find(|m| m.match_pc == pc) {
                    for &(case_pc, _, body_len) in &mi.cases {
                        reachable[case_pc] = true;
                        // Mark case body
                        let body_start = case_pc + 1;
                        let body_end = (body_start + body_len as usize).min(instrs.len());
                        for flag in &mut reachable[body_start..body_end] {
                            *flag = true;
                        }
                    }
                    reachable[mi.exhaust_pc] = true;
                    pc = mi.exhaust_pc + 1;
                } else {
                    pc += 1;
                }
            }
            Opcode::Func => {
                // Skip function body (handled separately)
                let body_len = instr.arg2 as usize;
                pc += 1 + body_len + 1;
            }
            Opcode::Case => {
                // Should be handled by Match, but just in case
                let body_len = instr.arg2 as usize;
                pc += 1 + body_len;
            }
            Opcode::Pre | Opcode::Post => {
                let len = instr.arg1 as usize;
                pc += 1 + len;
            }
            _ => {
                pc += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural::check_structural;
    use nolang_common::{Instruction, TypeTag};

    fn instr(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Instruction {
        Instruction::new(opcode, type_tag, arg1, arg2, arg3)
    }

    #[test]
    fn all_reachable_no_errors() {
        let instrs = [
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_reachability(&instrs, &ctx);
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn unreachable_after_halt() {
        let instrs = [
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
            instr(Opcode::Nop, TypeTag::None, 0, 0, 0), // unreachable
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_reachability(&instrs, &ctx);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::UnreachableInstruction { at: 2 })));
    }

    #[test]
    fn function_body_reachable() {
        let instrs = [
            instr(Opcode::Func, TypeTag::None, 0, 3, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
            instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
            instr(Opcode::Hash, TypeTag::None, 0, 0, 0),
            instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_reachability(&instrs, &ctx);
        assert!(errors.is_empty(), "{errors:?}");
    }
}
