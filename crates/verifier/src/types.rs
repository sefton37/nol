//! Type checking pass for NoLang programs.
//!
//! With PARAM declarations, every binding has a known type. This pass
//! tracks types through the stack and binding environment, flagging
//! mismatches.

use crate::error::VerifyError;
use crate::structural::ProgramContext;
use nolang_common::{Instruction, Opcode, TypeTag};

/// Run the type checking pass.
///
/// Walks the entry point and function bodies, tracking types on a
/// simulated stack. Reports mismatches.
pub fn check_types(instrs: &[Instruction], ctx: &ProgramContext) -> Vec<VerifyError> {
    let mut errors = Vec::new();

    // Check entry point
    if ctx.entry_point < instrs.len() {
        let mut checker = TypeChecker::new(instrs, ctx);
        checker.check_range(ctx.entry_point, instrs.len(), &[]);
        errors.extend(checker.errors);
    }

    // Check each function body
    for func in &ctx.functions {
        let mut checker = TypeChecker::new(instrs, ctx);
        // Function parameters are the initial bindings
        let param_types: Vec<TypeTag> = func.param_types.clone();
        checker.check_range(func.body_start_pc, func.endfunc_pc, &param_types);
        errors.extend(checker.errors);
    }

    errors
}

struct TypeChecker<'a> {
    instrs: &'a [Instruction],
    ctx: &'a ProgramContext,
    /// Simulated type stack.
    stack: Vec<TypeTag>,
    /// Binding environment types.
    bindings: Vec<TypeTag>,
    errors: Vec<VerifyError>,
}

impl<'a> TypeChecker<'a> {
    fn new(instrs: &'a [Instruction], ctx: &'a ProgramContext) -> Self {
        Self {
            instrs,
            ctx,
            stack: Vec::new(),
            bindings: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn check_range(&mut self, start: usize, end: usize, initial_bindings: &[TypeTag]) {
        self.bindings = initial_bindings.to_vec();
        self.stack.clear();
        let mut pc = start;

        while pc < end {
            let instr = &self.instrs[pc];
            match instr.opcode {
                Opcode::Const => {
                    self.stack.push(instr.type_tag);
                    pc += 1;
                }
                Opcode::ConstExt => {
                    self.stack.push(instr.type_tag);
                    pc += 2; // Skip next instruction (data payload)
                }
                Opcode::Bind => {
                    if let Some(tt) = self.stack.pop() {
                        self.bindings.push(tt);
                    }
                    pc += 1;
                }
                Opcode::Ref => {
                    let index = instr.arg1 as usize;
                    if index >= self.bindings.len() {
                        self.errors.push(VerifyError::UnresolvableRef {
                            at: pc,
                            index: instr.arg1,
                            binding_depth: self.bindings.len() as u16,
                        });
                        // Push unknown type to continue analysis
                        self.stack.push(TypeTag::None);
                    } else {
                        let binding_pos = self.bindings.len() - 1 - index;
                        self.stack.push(self.bindings[binding_pos]);
                    }
                    pc += 1;
                }
                Opcode::Drop => {
                    self.bindings.pop();
                    pc += 1;
                }
                Opcode::Add | Opcode::Sub | Opcode::Mul | Opcode::Div => {
                    if self.stack.len() >= 2 {
                        let b = self.stack.pop().unwrap();
                        let a = self.stack.pop().unwrap();
                        if a != b {
                            self.errors.push(VerifyError::TypeMismatch {
                                at: pc,
                                expected: a,
                                found: b,
                            });
                        }
                        if !a.is_numeric() {
                            self.errors.push(VerifyError::TypeMismatch {
                                at: pc,
                                expected: TypeTag::I64,
                                found: a,
                            });
                        }
                        self.stack.push(a); // Result same type as operands
                    }
                    pc += 1;
                }
                Opcode::Mod => {
                    if self.stack.len() >= 2 {
                        let b = self.stack.pop().unwrap();
                        let a = self.stack.pop().unwrap();
                        if a != b {
                            self.errors.push(VerifyError::TypeMismatch {
                                at: pc,
                                expected: a,
                                found: b,
                            });
                        }
                        if a != TypeTag::I64 && a != TypeTag::U64 {
                            self.errors.push(VerifyError::TypeMismatch {
                                at: pc,
                                expected: TypeTag::I64,
                                found: a,
                            });
                        }
                        self.stack.push(a);
                    }
                    pc += 1;
                }
                Opcode::Neg => {
                    if let Some(a) = self.stack.pop() {
                        if a != TypeTag::I64 && a != TypeTag::F64 {
                            self.errors.push(VerifyError::TypeMismatch {
                                at: pc,
                                expected: TypeTag::I64,
                                found: a,
                            });
                        }
                        self.stack.push(a);
                    }
                    pc += 1;
                }
                Opcode::Eq | Opcode::Neq | Opcode::Lt | Opcode::Gt | Opcode::Lte | Opcode::Gte => {
                    if self.stack.len() >= 2 {
                        let b = self.stack.pop().unwrap();
                        let a = self.stack.pop().unwrap();
                        if a != b {
                            self.errors.push(VerifyError::TypeMismatch {
                                at: pc,
                                expected: a,
                                found: b,
                            });
                        }
                    }
                    self.stack.push(TypeTag::Bool); // Comparisons always produce Bool
                    pc += 1;
                }
                Opcode::And | Opcode::Or | Opcode::Xor => {
                    if self.stack.len() >= 2 {
                        let b = self.stack.pop().unwrap();
                        let a = self.stack.pop().unwrap();
                        if a != b {
                            self.errors.push(VerifyError::TypeMismatch {
                                at: pc,
                                expected: a,
                                found: b,
                            });
                        }
                        self.stack.push(a);
                    }
                    pc += 1;
                }
                Opcode::Not => {
                    // Pop one, push same type
                    if let Some(a) = self.stack.pop() {
                        self.stack.push(a);
                    }
                    pc += 1;
                }
                Opcode::Shl | Opcode::Shr => {
                    if self.stack.len() >= 2 {
                        let shift = self.stack.pop().unwrap();
                        let value = self.stack.pop().unwrap();
                        if shift != value {
                            self.errors.push(VerifyError::TypeMismatch {
                                at: pc,
                                expected: value,
                                found: shift,
                            });
                        }
                        self.stack.push(value);
                    }
                    pc += 1;
                }
                Opcode::Match => {
                    // Pop the matched value, proceed into CASE bodies
                    // We skip the match block for type checking — case bodies
                    // are checked separately through the structural info
                    self.stack.pop();
                    // Skip past the match block (CASEs + EXHAUST)
                    if let Some(mi) = self.ctx.matches.iter().find(|m| m.match_pc == pc) {
                        pc = mi.exhaust_pc + 1;
                        // After match, one result value should be on stack
                        // We push a placeholder (the type depends on case body analysis)
                        // For now, use None as a conservative placeholder
                        self.stack.push(TypeTag::None);
                    } else {
                        pc += 1;
                    }
                }
                Opcode::Case => {
                    // Skip case body during linear walk
                    let body_len = instr.arg2 as usize;
                    pc += 1 + body_len;
                }
                Opcode::Exhaust => {
                    pc += 1;
                }
                Opcode::Func => {
                    // Skip function body
                    let body_len = instr.arg2 as usize;
                    pc += 1 + body_len + 1; // FUNC + body + ENDFUNC
                }
                Opcode::EndFunc | Opcode::Param => {
                    pc += 1;
                }
                Opcode::Pre | Opcode::Post => {
                    let len = instr.arg1 as usize;
                    pc += 1 + len;
                }
                Opcode::Call => {
                    let func_index = instr.arg1 as usize;
                    if let Some(func) = self.ctx.functions.get(func_index) {
                        // Pop param_count args from stack
                        for (param_idx, &expected_type) in func.param_types.iter().enumerate().rev()
                        {
                            if let Some(actual_type) = self.stack.pop() {
                                if actual_type != expected_type && expected_type != TypeTag::None {
                                    self.errors.push(VerifyError::TypeMismatch {
                                        at: pc,
                                        expected: expected_type,
                                        found: actual_type,
                                    });
                                }
                            }
                            let _ = param_idx;
                        }
                        // Push return type (unknown without deeper analysis)
                        self.stack.push(TypeTag::None);
                    }
                    pc += 1;
                }
                Opcode::Recurse => {
                    // Similar to Call — pop args, push return type
                    // For recursive calls, we'd need the enclosing function's info
                    // For now, push None as return type placeholder
                    self.stack.push(TypeTag::None);
                    pc += 1;
                }
                Opcode::Ret => {
                    // Pop return value (end of function analysis)
                    pc += 1;
                }
                Opcode::VariantNew => {
                    // Pop payload, push Variant
                    self.stack.pop();
                    self.stack.push(TypeTag::Variant);
                    pc += 1;
                }
                Opcode::TupleNew => {
                    let field_count = instr.arg1 as usize;
                    for _ in 0..field_count {
                        self.stack.pop();
                    }
                    self.stack.push(TypeTag::Tuple);
                    pc += 1;
                }
                Opcode::Project => {
                    // Pop tuple, push field type (unknown statically without deeper tracking)
                    self.stack.pop();
                    self.stack.push(TypeTag::None);
                    pc += 1;
                }
                Opcode::ArrayNew => {
                    let length = instr.arg1 as usize;
                    for _ in 0..length {
                        self.stack.pop();
                    }
                    self.stack.push(TypeTag::Array);
                    pc += 1;
                }
                Opcode::ArrayGet => {
                    // Pop index (U64), pop array, push element type
                    self.stack.pop(); // index
                    self.stack.pop(); // array
                    self.stack.push(TypeTag::None); // element type unknown
                    pc += 1;
                }
                Opcode::ArrayLen => {
                    self.stack.pop(); // array
                    self.stack.push(TypeTag::U64);
                    pc += 1;
                }
                Opcode::Hash | Opcode::Nop => {
                    pc += 1;
                }
                Opcode::Assert => {
                    // Pop BOOL
                    if let Some(tt) = self.stack.pop() {
                        if tt != TypeTag::Bool && tt != TypeTag::None {
                            self.errors.push(VerifyError::TypeMismatch {
                                at: pc,
                                expected: TypeTag::Bool,
                                found: tt,
                            });
                        }
                    }
                    pc += 1;
                }
                Opcode::Typeof => {
                    // Pop value, push it back, then push BOOL
                    // Net: +1 (BOOL on top)
                    self.stack.push(TypeTag::Bool);
                    pc += 1;
                }
                Opcode::Halt => {
                    pc += 1;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::structural::check_structural;
    use nolang_common::Instruction;

    fn instr(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Instruction {
        Instruction::new(opcode, type_tag, arg1, arg2, arg3)
    }

    #[test]
    fn valid_arithmetic_no_errors() {
        let instrs = [
            instr(Opcode::Const, TypeTag::I64, 0, 5, 0),
            instr(Opcode::Const, TypeTag::I64, 0, 3, 0),
            instr(Opcode::Add, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_types(&instrs, &ctx);
        assert!(errors.is_empty(), "{errors:?}");
    }

    #[test]
    fn mixed_type_arithmetic_detected() {
        let instrs = [
            instr(Opcode::Const, TypeTag::I64, 0, 5, 0),
            instr(Opcode::Const, TypeTag::F64, 0, 0, 0), // F64 via CONST is invalid but tests type checker
            instr(Opcode::Add, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_types(&instrs, &ctx);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::TypeMismatch { .. })));
    }

    #[test]
    fn ref_beyond_depth_detected() {
        let instrs = [
            instr(Opcode::Ref, TypeTag::None, 5, 0, 0), // no bindings
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_types(&instrs, &ctx);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::UnresolvableRef { .. })));
    }

    #[test]
    fn function_param_type_checked() {
        let instrs = [
            // FUNC with I64 param
            instr(Opcode::Func, TypeTag::None, 1, 4, 0),
            instr(Opcode::Param, TypeTag::I64, 0, 0, 0),
            instr(Opcode::Ref, TypeTag::None, 0, 0, 0),
            instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
            instr(Opcode::Hash, TypeTag::None, 0, 0, 0),
            instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
            // Call with Bool (wrong type)
            instr(Opcode::Const, TypeTag::Bool, 1, 0, 0),
            instr(Opcode::Call, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let (ctx, _) = check_structural(&instrs);
        let errors = check_types(&instrs, &ctx);
        assert!(errors
            .iter()
            .any(|e| matches!(e, VerifyError::TypeMismatch { .. })));
    }
}
