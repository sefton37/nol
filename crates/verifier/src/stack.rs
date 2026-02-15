//! Stack balance analysis for NoLang programs.
//!
//! Tracks stack depth at every instruction, checking for underflow
//! and ensuring exactly 1 value at HALT.

use crate::error::VerifyError;
use crate::structural::ProgramContext;
use nolang_common::{Instruction, Opcode};

/// Run the stack balance check.
pub fn check_stack(instrs: &[Instruction], ctx: &ProgramContext) -> Vec<VerifyError> {
    let mut errors = Vec::new();

    // Check entry point
    if ctx.entry_point < instrs.len() {
        check_range(instrs, ctx, ctx.entry_point, instrs.len(), 0, &mut errors);
    }

    // Check each function body
    for func in &ctx.functions {
        // Functions start with param_count bindings but 0 stack values
        check_range(
            instrs,
            ctx,
            func.body_start_pc,
            func.endfunc_pc,
            0,
            &mut errors,
        );
    }

    errors
}

fn check_range(
    instrs: &[Instruction],
    ctx: &ProgramContext,
    start: usize,
    end: usize,
    initial_depth: usize,
    errors: &mut Vec<VerifyError>,
) {
    let mut depth: i64 = initial_depth as i64;
    let mut pc = start;

    while pc < end {
        let instr = &instrs[pc];

        // Check for underflow before the instruction executes
        let required = stack_pops(instr);
        if depth < required as i64 {
            errors.push(VerifyError::StackUnderflow { at: pc });
            return; // Can't meaningfully continue
        }

        let delta = stack_delta(instr);
        depth += delta;

        if depth < 0 {
            errors.push(VerifyError::StackUnderflow { at: pc });
            return;
        }

        // Handle special control flow
        match instr.opcode {
            Opcode::Halt => {
                if depth != 1 {
                    errors.push(VerifyError::UnbalancedStack {
                        at_halt: pc,
                        depth: depth as usize,
                    });
                }
                return;
            }
            Opcode::ConstExt => {
                pc += 2; // Skip data instruction
                continue;
            }
            Opcode::Match => {
                // Skip the MATCH block — CASEs are checked via structural info
                if let Some(mi) = ctx.matches.iter().find(|m| m.match_pc == pc) {
                    // MATCH pops 1, each CASE body should produce 1
                    // Net effect: 0 (consumed matched value, got result)
                    pc = mi.exhaust_pc + 1;
                    // depth already adjusted by stack_delta (-1 for MATCH, +1 conceptual for result)
                    depth += 1; // Result from the match
                    continue;
                }
                pc += 1;
                continue;
            }
            Opcode::Func => {
                // Skip function body
                let body_len = instr.arg2 as usize;
                pc += 1 + body_len + 1;
                // FUNC doesn't affect the outer stack
                depth -= delta; // undo the delta we added
                continue;
            }
            Opcode::Pre | Opcode::Post => {
                let len = instr.arg1 as usize;
                pc += 1 + len;
                depth -= delta; // undo delta
                continue;
            }
            Opcode::Ret => {
                // End of function — return value should be on stack
                return;
            }
            _ => {}
        }

        pc += 1;
    }
}

/// Number of values popped by an instruction.
fn stack_pops(instr: &Instruction) -> usize {
    match instr.opcode {
        // Pop 0
        Opcode::Const
        | Opcode::ConstExt
        | Opcode::Ref
        | Opcode::Nop
        | Opcode::Hash
        | Opcode::Halt
        | Opcode::EndFunc
        | Opcode::Func
        | Opcode::Pre
        | Opcode::Post
        | Opcode::Param
        | Opcode::Exhaust => 0,

        // Pop 1
        Opcode::Bind
        | Opcode::Neg
        | Opcode::Not
        | Opcode::Match
        | Opcode::Assert
        | Opcode::Drop
        | Opcode::ArrayLen
        | Opcode::Ret
        | Opcode::VariantNew => 1,

        // Pop 2
        Opcode::Add
        | Opcode::Sub
        | Opcode::Mul
        | Opcode::Div
        | Opcode::Mod
        | Opcode::Eq
        | Opcode::Neq
        | Opcode::Lt
        | Opcode::Gt
        | Opcode::Lte
        | Opcode::Gte
        | Opcode::And
        | Opcode::Or
        | Opcode::Xor
        | Opcode::Shl
        | Opcode::Shr
        | Opcode::ArrayGet => 2,

        // Special
        Opcode::Typeof => 1,   // pops 1, pushes 2 (value back + bool)
        Opcode::Case => 0,     // Handled by MATCH
        Opcode::Call => 0,     // Dynamic (param_count), handled specially
        Opcode::Recurse => 0,  // Dynamic (param_count)
        Opcode::Project => 1,  // Pop tuple, push field
        Opcode::TupleNew => 0, // Dynamic (field_count)
        Opcode::ArrayNew => 0, // Dynamic (length)
    }
}

/// Net stack depth change for an instruction.
fn stack_delta(instr: &Instruction) -> i64 {
    match instr.opcode {
        // Push 1, pop 0 → +1
        Opcode::Const | Opcode::ConstExt | Opcode::Ref => 1,

        // Pop 1, push 0 → -1
        Opcode::Bind | Opcode::Assert => -1,

        // Pop 0, push 0 → 0
        Opcode::Drop
        | Opcode::Nop
        | Opcode::Hash
        | Opcode::EndFunc
        | Opcode::Func
        | Opcode::Pre
        | Opcode::Post
        | Opcode::Param
        | Opcode::Exhaust
        | Opcode::Halt
        | Opcode::Case => 0,

        // Pop 2, push 1 → -1
        Opcode::Add
        | Opcode::Sub
        | Opcode::Mul
        | Opcode::Div
        | Opcode::Mod
        | Opcode::Eq
        | Opcode::Neq
        | Opcode::Lt
        | Opcode::Gt
        | Opcode::Lte
        | Opcode::Gte
        | Opcode::And
        | Opcode::Or
        | Opcode::Xor
        | Opcode::Shl
        | Opcode::Shr => -1,

        // Pop 1, push 1 → 0
        Opcode::Neg | Opcode::Not => 0,

        // Pop 2, push 1 → -1
        Opcode::ArrayGet => -1,

        // Pop 1, push 1 → 0
        Opcode::ArrayLen | Opcode::Project => 0,

        // Pop 1, push 2 → +1
        Opcode::Typeof => 1,

        // Pop N, push 1 → -(N-1)
        Opcode::TupleNew => -(instr.arg1 as i64 - 1),
        Opcode::ArrayNew => -(instr.arg1 as i64 - 1),

        // Pop 1 (payload), push 1 (variant) → 0
        Opcode::VariantNew => 0,

        // MATCH: pop 1 → -1 (result added by match resolution)
        Opcode::Match => -1,

        // RET: pop 1 (return value) → -1
        Opcode::Ret => -1,

        // CALL: pop N args, push 1 return → -(N-1)
        // We approximate as 0 since we handle it specially
        Opcode::Call => 0,
        Opcode::Recurse => 0,
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
    fn balanced_stack_at_halt() {
        let instrs = [
            instr(Opcode::Const, TypeTag::I64, 0, 42, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_stack(&instrs, &ctx);
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn empty_stack_at_halt() {
        let instrs = [instr(Opcode::Halt, TypeTag::None, 0, 0, 0)];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_stack(&instrs, &ctx);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::UnbalancedStack { depth: 0, .. })));
    }

    #[test]
    fn multiple_values_at_halt() {
        let instrs = [
            instr(Opcode::Const, TypeTag::I64, 0, 1, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 2, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_stack(&instrs, &ctx);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::UnbalancedStack { depth: 2, .. })));
    }

    #[test]
    fn stack_underflow_detected() {
        let instrs = [
            // ADD with nothing on stack
            instr(Opcode::Add, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_stack(&instrs, &ctx);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::StackUnderflow { .. })));
    }
}
