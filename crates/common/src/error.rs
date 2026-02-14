//! Decode errors for NoLang instruction streams.

use thiserror::Error;

/// Errors that occur during instruction decoding.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DecodeError {
    /// Opcode 0x00 is illegal and always rejected.
    #[error("illegal opcode 0x00")]
    IllegalOpcode,

    /// Opcode falls in a reserved range.
    #[error("reserved opcode: {0:#04x}")]
    ReservedOpcode(u8),

    /// Opcode not recognized (should not occur if reserved ranges are exhaustive).
    #[error("invalid opcode: {0:#04x}")]
    InvalidOpcode(u8),

    /// Type tag in reserved range (0x0D-0xFF).
    #[error("reserved type tag: {0:#04x}")]
    ReservedTypeTag(u8),

    /// Type tag not recognized.
    #[error("invalid type tag: {0:#04x}")]
    InvalidTypeTag(u8),

    /// Byte stream length is not a multiple of 8.
    #[error("invalid byte stream length: {0} (must be multiple of 8)")]
    InvalidLength(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_illegal_opcode() {
        assert_eq!(
            DecodeError::IllegalOpcode.to_string(),
            "illegal opcode 0x00"
        );
    }

    #[test]
    fn display_reserved_opcode() {
        assert_eq!(
            DecodeError::ReservedOpcode(0x08).to_string(),
            "reserved opcode: 0x08"
        );
    }

    #[test]
    fn display_reserved_type_tag() {
        assert_eq!(
            DecodeError::ReservedTypeTag(0x0D).to_string(),
            "reserved type tag: 0x0d"
        );
    }

    #[test]
    fn display_invalid_length() {
        assert_eq!(
            DecodeError::InvalidLength(7).to_string(),
            "invalid byte stream length: 7 (must be multiple of 8)"
        );
    }
}
