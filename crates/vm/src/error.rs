//! Runtime errors for the NoLang VM.
//!
//! These are errors that can only happen at runtime, not during static
//! verification. Every error includes the instruction index (`at`) for
//! debugging.

use std::path::PathBuf;
use thiserror::Error;

/// Errors that occur during program execution.
///
/// All variants include an instruction index for debugging. These errors
/// represent conditions that the verifier cannot catch statically.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RuntimeError {
    /// Integer or float division by zero.
    #[error("division by zero at instruction {at}")]
    DivisionByZero { at: usize },

    /// Recursion depth exceeded the limit specified in the RECURSE instruction.
    #[error("recursion depth exceeded limit {limit} at instruction {at}")]
    RecursionDepthExceeded { at: usize, limit: u16 },

    /// Array index out of bounds.
    #[error("array index {index} out of bounds (length {length}) at instruction {at}")]
    ArrayIndexOutOfBounds { at: usize, index: u64, length: u64 },

    /// Function precondition evaluated to false.
    #[error("precondition failed at instruction {at}")]
    PreconditionFailed { at: usize },

    /// Function postcondition evaluated to false.
    #[error("postcondition failed at instruction {at}")]
    PostconditionFailed { at: usize },

    /// Stack exceeded the maximum depth of 4096 slots.
    #[error("stack overflow at instruction {at}")]
    StackOverflow { at: usize },

    /// HALT executed with no values on the stack.
    #[error("HALT with empty stack")]
    HaltWithEmptyStack,

    /// HALT executed with more than one value on the stack.
    #[error("HALT with {count} values on stack (expected 1)")]
    HaltWithMultipleValues { count: usize },

    /// Floating-point operation produced NaN.
    #[error("float operation produced NaN at instruction {at}")]
    FloatNaN { at: usize },

    /// Floating-point operation produced infinity.
    #[error("float operation produced infinity at instruction {at}")]
    FloatInfinity { at: usize },

    /// ASSERT instruction received false.
    #[error("assertion failed at instruction {at}")]
    AssertFailed { at: usize },

    /// Program counter went past the end of the program.
    #[error("unexpected end of program at instruction {at}")]
    UnexpectedEndOfProgram { at: usize },

    /// Stack underflow (pop on empty stack).
    #[error("stack underflow at instruction {at}")]
    StackUnderflow { at: usize },

    /// REF to a binding index beyond current depth.
    #[error("binding index {index} out of range (depth {depth}) at instruction {at}")]
    BindingOutOfRange { at: usize, index: u16, depth: usize },

    /// PROJECT on a value that is not a tuple.
    #[error("PROJECT on non-tuple at instruction {at}")]
    ProjectOnNonTuple { at: usize },

    /// PROJECT field index out of bounds.
    #[error("PROJECT field {field} out of bounds (size {size}) at instruction {at}")]
    ProjectOutOfBounds { at: usize, field: u16, size: usize },

    /// ARRAY_GET or ARRAY_LEN on a value that is not an array.
    #[error("array operation on non-array at instruction {at}")]
    NotAnArray { at: usize },

    /// No matching CASE for the given tag.
    #[error("no matching CASE for tag {tag} at instruction {at}")]
    NoMatchingCase { at: usize, tag: u16 },

    /// CALL or RECURSE references a function that doesn't exist.
    #[error("unknown function at binding index {index} at instruction {at}")]
    UnknownFunction { at: usize, index: u16 },

    /// Type mismatch: expected a specific type but got something else.
    #[error("type mismatch at instruction {at}")]
    TypeMismatch { at: usize },

    /// Path is outside the sandbox prefix.
    #[error("sandbox violation: path {path} is outside prefix {prefix}")]
    SandboxViolation { path: PathBuf, prefix: PathBuf },

    /// Command not in allowlist.
    #[error("command not allowed: {0}")]
    CommandNotAllowed(String),

    /// String pool index out of bounds.
    #[error("string pool index {index} out of bounds (pool size {pool_size})")]
    StringPoolIndexOutOfBounds { index: u16, pool_size: usize },

    /// I/O error during file operation.
    #[error("I/O error: {0}")]
    IoError(String),

    /// Type mismatch for I/O operation.
    #[error("I/O type mismatch: expected {expected}, got {got}")]
    IoTypeMismatch { expected: &'static str, got: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_formats() {
        assert_eq!(
            RuntimeError::DivisionByZero { at: 5 }.to_string(),
            "division by zero at instruction 5"
        );
        assert_eq!(
            RuntimeError::HaltWithEmptyStack.to_string(),
            "HALT with empty stack"
        );
        assert_eq!(
            RuntimeError::HaltWithMultipleValues { count: 3 }.to_string(),
            "HALT with 3 values on stack (expected 1)"
        );
    }
}
