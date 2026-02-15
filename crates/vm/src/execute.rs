//! Main execution loop and opcode dispatch for the NoLang VM.

use crate::error::RuntimeError;
use crate::machine::{CallFrame, VM};
use nolang_common::{Instruction, Opcode, TypeTag, Value};

impl<'a> VM<'a> {
    /// Execute the program until HALT or error.
    pub fn execute(&mut self) -> Result<Value, RuntimeError> {
        self.scan_functions();
        self.pc = self.find_entry_point();

        loop {
            // Check if we've finished a CASE body and need to jump to after EXHAUST.
            if let Some(&(body_end_pc, after_exhaust_pc)) = self.case_contexts.last() {
                if self.pc >= body_end_pc {
                    self.case_contexts.pop();
                    self.pc = after_exhaust_pc;
                    continue;
                }
            }

            let instr = *self.fetch()?;
            self.pc += 1;

            match instr.opcode {
                // VM Control
                Opcode::Halt => return self.exec_halt(),
                Opcode::Nop => {}

                // Group A: Foundation
                Opcode::Const => self.exec_const(&instr)?,
                Opcode::ConstExt => self.exec_const_ext(&instr)?,
                Opcode::Bind => self.exec_bind()?,
                Opcode::Ref => self.exec_ref(&instr)?,
                Opcode::Drop => self.exec_drop()?,

                // Group B: Arithmetic
                Opcode::Add => self.exec_binary_arith(
                    |a, b| a.wrapping_add(b),
                    |a, b| a.wrapping_add(b),
                    |a, b| a + b,
                )?,
                Opcode::Sub => self.exec_binary_arith(
                    |a, b| a.wrapping_sub(b),
                    |a, b| a.wrapping_sub(b),
                    |a, b| a - b,
                )?,
                Opcode::Mul => self.exec_binary_arith(
                    |a, b| a.wrapping_mul(b),
                    |a, b| a.wrapping_mul(b),
                    |a, b| a * b,
                )?,
                Opcode::Div => self.exec_div()?,
                Opcode::Mod => self.exec_mod()?,
                Opcode::Neg => self.exec_neg()?,

                // Comparison
                Opcode::Eq => self.exec_comparison(|a, b| a == b, |a, b| a == b, |a, b| a == b)?,
                Opcode::Neq => self.exec_comparison(|a, b| a != b, |a, b| a != b, |a, b| a != b)?,
                Opcode::Lt => self.exec_comparison(|a, b| a < b, |a, b| a < b, |a, b| a < b)?,
                Opcode::Gt => self.exec_comparison(|a, b| a > b, |a, b| a > b, |a, b| a > b)?,
                Opcode::Lte => self.exec_comparison(|a, b| a <= b, |a, b| a <= b, |a, b| a <= b)?,
                Opcode::Gte => self.exec_comparison(|a, b| a >= b, |a, b| a >= b, |a, b| a >= b)?,

                // Logic & Bitwise
                Opcode::And => self.exec_logic_and()?,
                Opcode::Or => self.exec_logic_or()?,
                Opcode::Not => self.exec_logic_not()?,
                Opcode::Xor => self.exec_logic_xor()?,
                Opcode::Shl => self.exec_shift_left()?,
                Opcode::Shr => self.exec_shift_right()?,
                Opcode::Implies => self.exec_implies()?,

                // Group C: Pattern matching
                Opcode::Match => self.exec_match(&instr)?,
                Opcode::Case => {
                    // CASE encountered outside MATCH context — skip body.
                    // This happens for unmatched cases; MATCH already jumped past them.
                    // Should not normally be reached if MATCH works correctly.
                    let body_len = instr.arg2 as usize;
                    self.pc += body_len;
                }
                Opcode::Exhaust => {} // End of MATCH block, continue

                // Group D: Functions
                Opcode::Func => {
                    // Skip function body during linear execution.
                    let body_len = instr.arg2 as usize;
                    self.pc += body_len + 1; // +1 for ENDFUNC
                }
                Opcode::EndFunc => {} // Should not be reached; skip
                Opcode::Pre => {
                    // Skip PRE during linear execution (handled at CALL time).
                    let len = instr.arg1 as usize;
                    self.pc += len;
                }
                Opcode::Post => {
                    // Skip POST during linear execution (handled at RET time).
                    let len = instr.arg1 as usize;
                    self.pc += len;
                }
                Opcode::Call => self.exec_call(&instr)?,
                Opcode::Recurse => self.exec_recurse(&instr)?,
                Opcode::Ret => self.exec_ret()?,

                // Group E: Data structures
                Opcode::VariantNew => self.exec_variant_new(&instr)?,
                Opcode::TupleNew => self.exec_tuple_new(&instr)?,
                Opcode::Project => self.exec_project(&instr)?,
                Opcode::ArrayNew => self.exec_array_new(&instr)?,
                Opcode::ArrayGet => self.exec_array_get()?,
                Opcode::ArrayLen => self.exec_array_len()?,

                // Group F: Meta
                Opcode::Param => {} // NOP during execution (verifier uses it)
                Opcode::Hash => {}  // NOP during execution (verification only)
                Opcode::Assert => self.exec_assert()?,
                Opcode::Typeof => self.exec_typeof(&instr)?,
                Opcode::Forall => self.exec_forall(&instr)?,
            }
        }
    }

    /// Execute a single instruction (used by execute_range for PRE/POST).
    fn execute_one(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        match instr.opcode {
            Opcode::Halt => {
                // Shouldn't happen in PRE/POST, but don't panic
                Ok(())
            }
            Opcode::Nop | Opcode::Hash | Opcode::Param | Opcode::EndFunc | Opcode::Exhaust => {
                Ok(())
            }
            Opcode::Const => self.exec_const(instr),
            Opcode::ConstExt => self.exec_const_ext(instr),
            Opcode::Bind => self.exec_bind(),
            Opcode::Ref => self.exec_ref(instr),
            Opcode::Drop => self.exec_drop(),
            Opcode::Add => self.exec_binary_arith(
                |a, b| a.wrapping_add(b),
                |a, b| a.wrapping_add(b),
                |a, b| a + b,
            ),
            Opcode::Sub => self.exec_binary_arith(
                |a, b| a.wrapping_sub(b),
                |a, b| a.wrapping_sub(b),
                |a, b| a - b,
            ),
            Opcode::Mul => self.exec_binary_arith(
                |a, b| a.wrapping_mul(b),
                |a, b| a.wrapping_mul(b),
                |a, b| a * b,
            ),
            Opcode::Div => self.exec_div(),
            Opcode::Mod => self.exec_mod(),
            Opcode::Neg => self.exec_neg(),
            Opcode::Eq => self.exec_comparison(|a, b| a == b, |a, b| a == b, |a, b| a == b),
            Opcode::Neq => self.exec_comparison(|a, b| a != b, |a, b| a != b, |a, b| a != b),
            Opcode::Lt => self.exec_comparison(|a, b| a < b, |a, b| a < b, |a, b| a < b),
            Opcode::Gt => self.exec_comparison(|a, b| a > b, |a, b| a > b, |a, b| a > b),
            Opcode::Lte => self.exec_comparison(|a, b| a <= b, |a, b| a <= b, |a, b| a <= b),
            Opcode::Gte => self.exec_comparison(|a, b| a >= b, |a, b| a >= b, |a, b| a >= b),
            Opcode::And => self.exec_logic_and(),
            Opcode::Or => self.exec_logic_or(),
            Opcode::Not => self.exec_logic_not(),
            Opcode::Xor => self.exec_logic_xor(),
            Opcode::Shl => self.exec_shift_left(),
            Opcode::Shr => self.exec_shift_right(),
            Opcode::Implies => self.exec_implies(),
            Opcode::Match => self.exec_match(instr),
            Opcode::Case => {
                let body_len = instr.arg2 as usize;
                self.pc += body_len;
                Ok(())
            }
            Opcode::Func => {
                let body_len = instr.arg2 as usize;
                self.pc += body_len + 1;
                Ok(())
            }
            Opcode::Pre => {
                self.pc += instr.arg1 as usize;
                Ok(())
            }
            Opcode::Post => {
                self.pc += instr.arg1 as usize;
                Ok(())
            }
            Opcode::Call => self.exec_call(instr),
            Opcode::Recurse => self.exec_recurse(instr),
            Opcode::Ret => self.exec_ret(),
            Opcode::VariantNew => self.exec_variant_new(instr),
            Opcode::TupleNew => self.exec_tuple_new(instr),
            Opcode::Project => self.exec_project(instr),
            Opcode::ArrayNew => self.exec_array_new(instr),
            Opcode::ArrayGet => self.exec_array_get(),
            Opcode::ArrayLen => self.exec_array_len(),
            Opcode::Assert => self.exec_assert(),
            Opcode::Typeof => self.exec_typeof(instr),
            Opcode::Forall => self.exec_forall(instr),
        }
    }

    /// Execute a range of instructions (for PRE/POST contract evaluation and FORALL).
    ///
    /// Uses PC-based bounds instead of counting iterations, so opcodes like
    /// FORALL that advance PC past their body work correctly.
    fn execute_range(&mut self, start_pc: usize, len: u16) -> Result<(), RuntimeError> {
        let saved_pc = self.pc;
        self.pc = start_pc;
        let end_pc = start_pc + len as usize;

        while self.pc < end_pc {
            let instr = *self.fetch()?;
            self.pc += 1;
            self.execute_one(&instr)?;
        }

        self.pc = saved_pc;
        Ok(())
    }

    // ---- Group A: Foundation ----

    fn exec_halt(&mut self) -> Result<Value, RuntimeError> {
        match self.stack.len() {
            0 => Err(RuntimeError::HaltWithEmptyStack),
            1 => Ok(self.stack.pop().unwrap()),
            n => Err(RuntimeError::HaltWithMultipleValues { count: n }),
        }
    }

    fn exec_const(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let value = instr
            .const_value()
            .ok_or(RuntimeError::UnexpectedEndOfProgram { at: self.pc - 1 })?;
        self.push(value)
    }

    fn exec_const_ext(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        // CONST_EXT: arg1 of this instruction = high 16 bits.
        // Next instruction's full 48-bit payload = low 48 bits.
        // The "48-bit payload" is type_tag(8) + arg1(16) + arg2(16) + arg3(16) = 56 bits?
        // No — re-reading SPEC: "the next instruction's full 48-bit payload
        // (type_tag + arg1 + arg2 + arg3) is treated as the low 48 bits"
        // type_tag is 8 bits, arg1/2/3 are 16 each = 8+16+16+16 = 56 bits.
        // But spec says 48 bits. This means: arg1(16) + arg2(16) + arg3(16) = 48 bits.
        // The type_tag byte of the next instruction is NOT part of the payload.
        // Wait, spec says "type_tag + arg1 + arg2 + arg3" = 48. That's 8+16+16+8=48?
        // No. Let me re-read: the fields are type_tag(8) + arg1(16) + arg2(16) + arg3(16) = 56.
        //
        // The spec says "48-bit payload (type_tag + arg1 + arg2 + arg3)".
        // This is ambiguous. But "arg1 of CONST_EXT is the high 16 bits.
        // Together: 64-bit constant." So: 16 (from CONST_EXT arg1) + 48 (from next) = 64.
        // If next provides 48 bits, that must be bytes[2..8] of the next instruction
        // (arg1 + arg2 + arg3 = 16+16+16 = 48 bits). Type_tag byte is skipped.

        let next = self.fetch()?;
        let next_copy = *next;
        self.pc += 1; // consume next instruction

        let high16 = instr.arg1 as u64;
        // Low 48 bits from next instruction's arg fields
        let low48 = ((next_copy.arg1 as u64) << 32)
            | ((next_copy.arg2 as u64) << 16)
            | (next_copy.arg3 as u64);

        let full_value = (high16 << 48) | low48;

        let value = match instr.type_tag {
            TypeTag::I64 => Value::I64(full_value as i64),
            TypeTag::U64 => Value::U64(full_value),
            TypeTag::F64 => {
                let f = f64::from_bits(full_value);
                self.check_float(f)?;
                Value::F64(f)
            }
            _ => return Err(RuntimeError::UnexpectedEndOfProgram { at: self.pc - 2 }),
        };

        self.push(value)
    }

    fn exec_bind(&mut self) -> Result<(), RuntimeError> {
        let value = self.pop()?;
        self.bindings.push(value);
        Ok(())
    }

    fn exec_ref(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let index = instr.arg1 as usize;
        let depth = self.bindings.len();

        if index >= depth {
            return Err(RuntimeError::BindingOutOfRange {
                at: self.pc - 1,
                index: instr.arg1,
                depth,
            });
        }

        // De Bruijn: index 0 = most recent binding = last element
        let binding_pos = depth - 1 - index;
        let value = self.bindings[binding_pos].clone();
        self.push(value)
    }

    fn exec_drop(&mut self) -> Result<(), RuntimeError> {
        if self.bindings.is_empty() {
            return Err(RuntimeError::BindingOutOfRange {
                at: self.pc - 1,
                index: 0,
                depth: 0,
            });
        }
        self.bindings.pop();
        Ok(())
    }

    // ---- Group B: Arithmetic ----

    /// Binary arithmetic: pop two same-type numeric values, apply op, push result.
    fn exec_binary_arith(
        &mut self,
        i64_op: fn(i64, i64) -> i64,
        u64_op: fn(u64, u64) -> u64,
        f64_op: fn(f64, f64) -> f64,
    ) -> Result<(), RuntimeError> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = match (a, b) {
            (Value::I64(x), Value::I64(y)) => Value::I64(i64_op(x, y)),
            (Value::U64(x), Value::U64(y)) => Value::U64(u64_op(x, y)),
            (Value::F64(x), Value::F64(y)) => {
                let r = f64_op(x, y);
                self.check_float(r)?;
                Value::F64(r)
            }
            // Type mismatch — VM trusts verifier, return error for safety
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(result)
    }

    fn exec_div(&mut self) -> Result<(), RuntimeError> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = match (a, b) {
            (Value::I64(_), Value::I64(0)) | (Value::U64(_), Value::U64(0)) => {
                return Err(RuntimeError::DivisionByZero { at: self.pc - 1 });
            }
            (Value::I64(x), Value::I64(y)) => Value::I64(x.wrapping_div(y)),
            (Value::U64(x), Value::U64(y)) => Value::U64(x / y),
            (Value::F64(_), Value::F64(0.0)) => {
                return Err(RuntimeError::DivisionByZero { at: self.pc - 1 });
            }
            (Value::F64(x), Value::F64(y)) => {
                let r = x / y;
                self.check_float(r)?;
                Value::F64(r)
            }
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(result)
    }

    fn exec_mod(&mut self) -> Result<(), RuntimeError> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = match (a, b) {
            (Value::I64(_), Value::I64(0)) | (Value::U64(_), Value::U64(0)) => {
                return Err(RuntimeError::DivisionByZero { at: self.pc - 1 });
            }
            (Value::I64(x), Value::I64(y)) => Value::I64(x.wrapping_rem(y)),
            (Value::U64(x), Value::U64(y)) => Value::U64(x % y),
            // MOD is I64/U64 only per spec
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(result)
    }

    fn exec_neg(&mut self) -> Result<(), RuntimeError> {
        let a = self.pop()?;

        let result = match a {
            Value::I64(x) => Value::I64(x.wrapping_neg()),
            Value::F64(x) => {
                let r = -x;
                self.check_float(r)?;
                Value::F64(r)
            }
            // NEG is I64/F64 only per spec
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(result)
    }

    /// Binary comparison: pop two same-type values, push BOOL.
    fn exec_comparison(
        &mut self,
        i64_op: fn(i64, i64) -> bool,
        u64_op: fn(u64, u64) -> bool,
        f64_op: fn(f64, f64) -> bool,
    ) -> Result<(), RuntimeError> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = match (a, b) {
            (Value::I64(x), Value::I64(y)) => i64_op(x, y),
            (Value::U64(x), Value::U64(y)) => u64_op(x, y),
            (Value::F64(x), Value::F64(y)) => f64_op(x, y),
            (Value::Bool(x), Value::Bool(y)) => i64_op(x as i64, y as i64),
            (Value::Char(x), Value::Char(y)) => i64_op(x as i64, y as i64),
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(Value::Bool(result))
    }

    // ---- Logic & Bitwise ----

    fn exec_logic_and(&mut self) -> Result<(), RuntimeError> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = match (a, b) {
            (Value::Bool(x), Value::Bool(y)) => Value::Bool(x && y),
            (Value::I64(x), Value::I64(y)) => Value::I64(x & y),
            (Value::U64(x), Value::U64(y)) => Value::U64(x & y),
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(result)
    }

    fn exec_logic_or(&mut self) -> Result<(), RuntimeError> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = match (a, b) {
            (Value::Bool(x), Value::Bool(y)) => Value::Bool(x || y),
            (Value::I64(x), Value::I64(y)) => Value::I64(x | y),
            (Value::U64(x), Value::U64(y)) => Value::U64(x | y),
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(result)
    }

    fn exec_logic_not(&mut self) -> Result<(), RuntimeError> {
        let a = self.pop()?;

        let result = match a {
            Value::Bool(x) => Value::Bool(!x),
            Value::I64(x) => Value::I64(!x),
            Value::U64(x) => Value::U64(!x),
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(result)
    }

    fn exec_logic_xor(&mut self) -> Result<(), RuntimeError> {
        let b = self.pop()?;
        let a = self.pop()?;

        let result = match (a, b) {
            (Value::Bool(x), Value::Bool(y)) => Value::Bool(x ^ y),
            (Value::I64(x), Value::I64(y)) => Value::I64(x ^ y),
            (Value::U64(x), Value::U64(y)) => Value::U64(x ^ y),
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(result)
    }

    fn exec_shift_left(&mut self) -> Result<(), RuntimeError> {
        let shift = self.pop()?;
        let value = self.pop()?;

        let result = match (value, shift) {
            (Value::I64(x), Value::I64(s)) => Value::I64(x.wrapping_shl(s as u32)),
            (Value::U64(x), Value::U64(s)) => Value::U64(x.wrapping_shl(s as u32)),
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(result)
    }

    fn exec_shift_right(&mut self) -> Result<(), RuntimeError> {
        let shift = self.pop()?;
        let value = self.pop()?;

        // Arithmetic for I64, logical for U64 per SPEC
        let result = match (value, shift) {
            (Value::I64(x), Value::I64(s)) => Value::I64(x.wrapping_shr(s as u32)),
            (Value::U64(x), Value::U64(s)) => Value::U64(x.wrapping_shr(s as u32)),
            _ => return Err(RuntimeError::StackUnderflow { at: self.pc - 1 }),
        };

        self.push(result)
    }

    fn exec_implies(&mut self) -> Result<(), RuntimeError> {
        let consequent = self.pop()?;
        let antecedent = self.pop()?;

        match (antecedent, consequent) {
            (Value::Bool(a), Value::Bool(b)) => self.push(Value::Bool(!a || b)),
            _ => Err(RuntimeError::TypeMismatch { at: self.pc - 1 }),
        }
    }

    fn exec_forall(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let body_len = instr.arg1;
        let body_start = self.pc; // PC is already past FORALL instruction

        let array = self.pop()?;
        let elements = match array {
            Value::Array(elems) => elems,
            _ => return Err(RuntimeError::TypeMismatch { at: self.pc - 1 }),
        };

        let mut result = true;
        for elem in &elements {
            self.bindings.push(elem.clone());
            self.execute_range(body_start, body_len)?;
            let condition = self.pop()?;
            self.bindings.pop();
            match condition {
                Value::Bool(b) => {
                    if !b {
                        result = false;
                        break;
                    }
                }
                _ => return Err(RuntimeError::TypeMismatch { at: self.pc - 1 }),
            }
        }

        // Skip past body instructions
        self.pc = body_start + body_len as usize;
        self.push(Value::Bool(result))
    }

    // ---- Group C: Pattern Matching ----

    fn exec_match(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let variant_count = instr.arg1;
        let matched_value = self.pop()?;

        // Extract tag from matched value
        let tag = match &matched_value {
            Value::Bool(b) => {
                if *b {
                    1
                } else {
                    0
                }
            }
            Value::Variant { tag, .. } => *tag,
            _ => {
                return Err(RuntimeError::NoMatchingCase {
                    at: self.pc - 1,
                    tag: 0,
                })
            }
        };

        // Scan through all CASEs to find the matching one and locate EXHAUST.
        let mut scan_pc = self.pc;
        let mut match_body_start = None;
        let mut match_body_len = 0usize;

        for _ in 0..variant_count {
            if scan_pc >= self.program.instructions.len() {
                return Err(RuntimeError::UnexpectedEndOfProgram { at: scan_pc });
            }

            let case_instr = &self.program.instructions[scan_pc];
            if case_instr.opcode != Opcode::Case {
                return Err(RuntimeError::NoMatchingCase { at: scan_pc, tag });
            }

            let case_tag = case_instr.arg1;
            let body_len = case_instr.arg2 as usize;

            if case_tag == tag && match_body_start.is_none() {
                match_body_start = Some(scan_pc + 1);
                match_body_len = body_len;
            }

            scan_pc += 1 + body_len; // Skip CASE + body
        }

        // scan_pc now points to EXHAUST
        let after_exhaust_pc = scan_pc + 1; // Skip EXHAUST

        let body_start = match_body_start.ok_or(RuntimeError::NoMatchingCase {
            at: self.pc - 1,
            tag,
        })?;

        // If variant has payload, push it onto the stack
        if let Value::Variant { payload, .. } = matched_value {
            self.push(*payload)?;
        }

        // Jump to matched CASE body
        self.pc = body_start;

        // Track: when body ends, jump past EXHAUST
        let body_end_pc = body_start + match_body_len;
        self.case_contexts.push((body_end_pc, after_exhaust_pc));

        Ok(())
    }

    // ---- Group D: Functions ----

    fn exec_call(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let func_index = instr.arg1 as usize;

        let func_info = self
            .functions
            .get(func_index)
            .ok_or(RuntimeError::UnknownFunction {
                at: self.pc - 1,
                index: instr.arg1,
            })?
            .clone();

        // Pop arguments (last pushed = param index 0)
        let mut args = Vec::new();
        for _ in 0..func_info.param_count {
            args.push(self.pop()?);
        }
        // args[0] = last pushed = de Bruijn index 0
        // We push them so that the last popped (first pushed) is deepest binding

        let saved_binding_depth = self.bindings.len();

        // Bind args: first arg pushed to bindings = deepest index
        // args is [index0, index1, ..., indexN-1] where index0 = most recent
        // We need to push in reverse so that args[0] ends up last (= de Bruijn 0)
        for arg in args.iter().rev() {
            self.bindings.push(arg.clone());
        }

        // Execute PRE conditions
        for &(pre_start, pre_len) in &func_info.pre_conditions {
            self.execute_range(pre_start, pre_len)?;
            let condition = self.pop()?;
            match condition {
                Value::Bool(true) => {}
                _ => return Err(RuntimeError::PreconditionFailed { at: self.pc - 1 }),
            }
        }

        // Push call frame
        self.call_stack.push(CallFrame {
            return_pc: self.pc,
            saved_binding_depth,
            body_start_pc: func_info.body_start_pc,
            recursion_depth: 0,
            post_conditions: func_info.post_conditions.clone(),
            func_index,
        });

        // Jump to function body
        self.pc = func_info.body_start_pc;

        Ok(())
    }

    fn exec_recurse(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let depth_limit = instr.arg1;

        let current_frame = self
            .call_stack
            .last()
            .ok_or(RuntimeError::UnexpectedEndOfProgram { at: self.pc - 1 })?;

        let current_depth = current_frame.recursion_depth;
        if current_depth >= depth_limit {
            return Err(RuntimeError::RecursionDepthExceeded {
                at: self.pc - 1,
                limit: depth_limit,
            });
        }

        let func_index = current_frame.func_index;
        let func_info = self.functions[func_index].clone();

        // Pop arguments
        let mut args = Vec::new();
        for _ in 0..func_info.param_count {
            args.push(self.pop()?);
        }

        let saved_binding_depth = self.bindings.len();

        // Bind args (same as CALL)
        for arg in args.iter().rev() {
            self.bindings.push(arg.clone());
        }

        // Execute PRE conditions
        for &(pre_start, pre_len) in &func_info.pre_conditions {
            self.execute_range(pre_start, pre_len)?;
            let condition = self.pop()?;
            match condition {
                Value::Bool(true) => {}
                _ => return Err(RuntimeError::PreconditionFailed { at: self.pc - 1 }),
            }
        }

        // Push new call frame with incremented depth
        self.call_stack.push(CallFrame {
            return_pc: self.pc,
            saved_binding_depth,
            body_start_pc: func_info.body_start_pc,
            recursion_depth: current_depth + 1,
            post_conditions: func_info.post_conditions.clone(),
            func_index,
        });

        // Jump to function body
        self.pc = func_info.body_start_pc;

        Ok(())
    }

    fn exec_ret(&mut self) -> Result<(), RuntimeError> {
        let return_value = self.pop()?;

        let call_frame = self
            .call_stack
            .pop()
            .ok_or(RuntimeError::UnexpectedEndOfProgram { at: self.pc - 1 })?;

        // Execute POST conditions with return value at binding index 0
        if !call_frame.post_conditions.is_empty() {
            self.bindings.push(return_value.clone());
            for &(post_start, post_len) in &call_frame.post_conditions {
                self.execute_range(post_start, post_len)?;
                let condition = self.pop()?;
                match condition {
                    Value::Bool(true) => {}
                    _ => {
                        return Err(RuntimeError::PostconditionFailed { at: self.pc - 1 });
                    }
                }
            }
            self.bindings.pop(); // Remove return value binding
        }

        // Restore binding environment
        self.bindings.truncate(call_frame.saved_binding_depth);

        // Return to caller
        self.pc = call_frame.return_pc;

        // Push return value
        self.push(return_value)
    }

    // ---- Group E: Data Structures ----

    fn exec_variant_new(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let total_tags = instr.arg1;
        let this_tag = instr.arg2;
        let payload = self.pop()?;

        self.push(Value::Variant {
            tag_count: total_tags,
            tag: this_tag,
            payload: Box::new(payload),
        })
    }

    fn exec_tuple_new(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let field_count = instr.arg1 as usize;
        let mut fields = Vec::with_capacity(field_count);

        // Pop field_count values. First popped = last field.
        for _ in 0..field_count {
            fields.push(self.pop()?);
        }
        fields.reverse(); // First popped becomes last field

        self.push(Value::Tuple(fields))
    }

    fn exec_project(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let field_index = instr.arg1 as usize;
        let tuple = self.pop()?;

        match tuple {
            Value::Tuple(fields) => {
                if field_index >= fields.len() {
                    return Err(RuntimeError::ProjectOutOfBounds {
                        at: self.pc - 1,
                        field: instr.arg1,
                        size: fields.len(),
                    });
                }
                self.push(fields.into_iter().nth(field_index).unwrap())
            }
            _ => Err(RuntimeError::ProjectOnNonTuple { at: self.pc - 1 }),
        }
    }

    fn exec_array_new(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let length = instr.arg1 as usize;
        let mut elements = Vec::with_capacity(length);

        for _ in 0..length {
            elements.push(self.pop()?);
        }
        elements.reverse(); // Match tuple ordering convention

        self.push(Value::Array(elements))
    }

    fn exec_array_get(&mut self) -> Result<(), RuntimeError> {
        let index_val = self.pop()?;
        let array_val = self.pop()?;

        let index = match index_val {
            Value::U64(i) => i,
            _ => return Err(RuntimeError::NotAnArray { at: self.pc - 1 }),
        };

        match array_val {
            Value::Array(elements) => {
                if index >= elements.len() as u64 {
                    return Err(RuntimeError::ArrayIndexOutOfBounds {
                        at: self.pc - 1,
                        index,
                        length: elements.len() as u64,
                    });
                }
                self.push(elements.into_iter().nth(index as usize).unwrap())
            }
            _ => Err(RuntimeError::NotAnArray { at: self.pc - 1 }),
        }
    }

    fn exec_array_len(&mut self) -> Result<(), RuntimeError> {
        let array_val = self.pop()?;

        match array_val {
            Value::Array(elements) => self.push(Value::U64(elements.len() as u64)),
            _ => Err(RuntimeError::NotAnArray { at: self.pc - 1 }),
        }
    }

    // ---- Group F: Meta ----

    fn exec_assert(&mut self) -> Result<(), RuntimeError> {
        let val = self.pop()?;
        match val {
            Value::Bool(true) => Ok(()),
            _ => Err(RuntimeError::AssertFailed { at: self.pc - 1 }),
        }
    }

    fn exec_typeof(&mut self, instr: &Instruction) -> Result<(), RuntimeError> {
        let expected = TypeTag::try_from((instr.arg1 & 0xFF) as u8)
            .map_err(|_| RuntimeError::UnexpectedEndOfProgram { at: self.pc - 1 })?;

        let value = self.pop()?;
        let actual = value.type_tag();
        let matches = actual == expected;

        self.push(value)?; // Non-destructive: push value back
        self.push(Value::Bool(matches)) // Then push result
    }
}
