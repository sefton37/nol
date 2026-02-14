//! NoLang virtual machine — executes verified instruction streams.
//!
//! The VM is a stack-based machine with:
//! - An operand stack for intermediate values
//! - A binding environment using de Bruijn indices
//! - A call stack for function invocation and recursion
//!
//! # Usage
//!
//! ```
//! use nolang_common::{Instruction, Opcode, TypeTag, Program, Value};
//! use nolang_vm::run;
//!
//! let program = Program::new(vec![
//!     Instruction::new(Opcode::Const, TypeTag::I64, 0, 42, 0),
//!     Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
//! ]);
//!
//! let result = run(&program).unwrap();
//! assert_eq!(result, Value::I64(42));
//! ```

pub mod error;
pub mod execute;
pub mod machine;

pub use error::RuntimeError;
pub use machine::VM;

use nolang_common::{Program, Value};

/// Execute a program and return the result.
///
/// This is the primary entry point for the VM. It:
/// 1. Pre-scans the program for function definitions
/// 2. Locates the entry point (after all function definitions)
/// 3. Executes until HALT
/// 4. Returns the top-of-stack value
///
/// # Errors
///
/// Returns [`RuntimeError`] if execution fails (division by zero,
/// recursion limit, assertion failure, etc.).
pub fn run(program: &Program) -> Result<Value, RuntimeError> {
    let mut vm = VM::new(program);
    vm.execute()
}
