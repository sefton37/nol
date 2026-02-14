//! VM state management: stack, bindings, call stack, function table.

use crate::error::RuntimeError;
use nolang_common::{Instruction, Opcode, Program, Value};

/// Maximum stack depth per SPEC.md Section 9.
pub const MAX_STACK_DEPTH: usize = 4096;

/// A call frame for function invocation.
#[derive(Debug, Clone)]
pub struct CallFrame {
    /// Instruction index to return to after RET.
    pub return_pc: usize,
    /// Binding depth before this call (for restoring environment).
    pub saved_binding_depth: usize,
    /// Location of the function body start (first instruction after PRE/POST).
    pub body_start_pc: usize,
    /// Current recursion depth for this function chain.
    pub recursion_depth: u16,
    /// POST condition blocks to check at RET: (start_pc, len).
    pub post_conditions: Vec<(usize, u16)>,
    /// Index into the function table (for RECURSE to look up param_count).
    pub func_index: usize,
}

/// Function metadata discovered during pre-scan.
#[derive(Debug, Clone)]
pub struct FunctionInfo {
    /// Number of parameters (from FUNC arg1).
    pub param_count: u16,
    /// Instruction index where the function body starts (after PRE/POST blocks).
    pub body_start_pc: usize,
    /// PRE condition blocks: (start_pc, len).
    pub pre_conditions: Vec<(usize, u16)>,
    /// POST condition blocks: (start_pc, len).
    pub post_conditions: Vec<(usize, u16)>,
}

/// The NoLang virtual machine.
pub struct VM<'a> {
    /// The program being executed.
    pub(crate) program: &'a Program,
    /// Operand stack.
    pub(crate) stack: Vec<Value>,
    /// Binding environment (de Bruijn indexed, index 0 = last element).
    pub(crate) bindings: Vec<Value>,
    /// Call stack for function invocation.
    pub(crate) call_stack: Vec<CallFrame>,
    /// Program counter (instruction index).
    pub(crate) pc: usize,
    /// Function table indexed by order of appearance.
    pub(crate) functions: Vec<FunctionInfo>,
    /// If Some((body_end_pc, after_exhaust_pc)), we're executing a CASE body.
    /// When pc reaches body_end_pc, jump to after_exhaust_pc.
    pub(crate) case_contexts: Vec<(usize, usize)>,
}

impl<'a> VM<'a> {
    /// Create a new VM for the given program.
    pub fn new(program: &'a Program) -> Self {
        Self {
            program,
            stack: Vec::new(),
            bindings: Vec::new(),
            call_stack: Vec::new(),
            pc: 0,
            functions: Vec::new(),
            case_contexts: Vec::new(),
        }
    }

    /// Pre-scan the program to locate all function definitions.
    ///
    /// Functions are assigned indices in order of appearance:
    /// first FUNC = index 0, second FUNC = index 1, etc.
    pub(crate) fn scan_functions(&mut self) {
        let instrs = &self.program.instructions;
        let mut pc = 0;

        while pc < instrs.len() {
            if instrs[pc].opcode == Opcode::Func {
                let param_count = instrs[pc].arg1;
                let body_len = instrs[pc].arg2 as usize;

                // Scan PRE/POST blocks starting after FUNC
                let mut pre_conditions = Vec::new();
                let mut post_conditions = Vec::new();
                let mut scan_pc = pc + 1;

                while scan_pc < instrs.len() {
                    match instrs[scan_pc].opcode {
                        Opcode::Pre => {
                            let len = instrs[scan_pc].arg1;
                            pre_conditions.push((scan_pc + 1, len));
                            scan_pc += 1 + len as usize;
                        }
                        Opcode::Post => {
                            let len = instrs[scan_pc].arg1;
                            post_conditions.push((scan_pc + 1, len));
                            scan_pc += 1 + len as usize;
                        }
                        _ => break,
                    }
                }

                self.functions.push(FunctionInfo {
                    param_count,
                    body_start_pc: scan_pc,
                    pre_conditions,
                    post_conditions,
                });

                // Skip past FUNC body + ENDFUNC
                // FUNC instruction + body_len instructions + ENDFUNC
                pc += 1 + body_len + 1;
            } else {
                pc += 1;
            }
        }
    }

    /// Find the entry point: first instruction after the last ENDFUNC,
    /// or instruction 0 if there are no functions.
    pub(crate) fn find_entry_point(&self) -> usize {
        let instrs = &self.program.instructions;
        let mut last_endfunc = None;

        for (i, instr) in instrs.iter().enumerate() {
            if instr.opcode == Opcode::EndFunc {
                last_endfunc = Some(i);
            }
        }

        last_endfunc.map(|i| i + 1).unwrap_or(0)
    }

    /// Push a value onto the stack, checking for overflow.
    pub(crate) fn push(&mut self, value: Value) -> Result<(), RuntimeError> {
        if self.stack.len() >= MAX_STACK_DEPTH {
            return Err(RuntimeError::StackOverflow { at: self.pc });
        }
        self.stack.push(value);
        Ok(())
    }

    /// Pop a value from the stack.
    pub(crate) fn pop(&mut self) -> Result<Value, RuntimeError> {
        self.stack
            .pop()
            .ok_or(RuntimeError::StackUnderflow { at: self.pc })
    }

    /// Fetch the instruction at the current pc.
    pub(crate) fn fetch(&self) -> Result<&Instruction, RuntimeError> {
        self.program
            .instructions
            .get(self.pc)
            .ok_or(RuntimeError::UnexpectedEndOfProgram { at: self.pc })
    }

    /// Check that a float value is neither NaN nor infinity.
    pub(crate) fn check_float(&self, val: f64) -> Result<(), RuntimeError> {
        if val.is_nan() {
            Err(RuntimeError::FloatNaN { at: self.pc })
        } else if val.is_infinite() {
            Err(RuntimeError::FloatInfinity { at: self.pc })
        } else {
            Ok(())
        }
    }
}
