//! Error types for the NoLang assembler.

use thiserror::Error;

/// Errors produced during assembly of text to binary.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum AsmError {
    /// An unrecognized opcode mnemonic was encountered.
    #[error("line {line}: unknown opcode '{token}'")]
    UnknownOpcode { line: usize, token: String },

    /// An unrecognized type tag name was encountered.
    #[error("line {line}: unknown type tag '{token}'")]
    UnknownTypeTag { line: usize, token: String },

    /// An opcode did not have enough arguments.
    #[error("line {line}: {opcode} expects {expected} argument(s)")]
    MissingArgument {
        line: usize,
        opcode: &'static str,
        expected: usize,
    },

    /// A numeric literal could not be parsed or is out of range.
    #[error("line {line}: invalid number '{token}'")]
    InvalidNumber { line: usize, token: String },

    /// A token appeared where it was not expected.
    #[error("line {line}: unexpected token '{token}'")]
    UnexpectedToken { line: usize, token: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display_unknown_opcode() {
        let e = AsmError::UnknownOpcode {
            line: 3,
            token: "FOO".to_string(),
        };
        assert_eq!(e.to_string(), "line 3: unknown opcode 'FOO'");
    }

    #[test]
    fn error_display_unknown_type_tag() {
        let e = AsmError::UnknownTypeTag {
            line: 5,
            token: "BAR".to_string(),
        };
        assert_eq!(e.to_string(), "line 5: unknown type tag 'BAR'");
    }

    #[test]
    fn error_display_missing_argument() {
        let e = AsmError::MissingArgument {
            line: 7,
            opcode: "REF",
            expected: 1,
        };
        assert_eq!(e.to_string(), "line 7: REF expects 1 argument(s)");
    }

    #[test]
    fn error_display_invalid_number() {
        let e = AsmError::InvalidNumber {
            line: 2,
            token: "0xZZZZ".to_string(),
        };
        assert_eq!(e.to_string(), "line 2: invalid number '0xZZZZ'");
    }

    #[test]
    fn error_display_unexpected_token() {
        let e = AsmError::UnexpectedToken {
            line: 4,
            token: "EXTRA".to_string(),
        };
        assert_eq!(e.to_string(), "line 4: unexpected token 'EXTRA'");
    }

    #[test]
    fn error_clone_and_eq() {
        let e1 = AsmError::UnknownOpcode {
            line: 1,
            token: "X".to_string(),
        };
        let e2 = e1.clone();
        assert_eq!(e1, e2);
    }
}
