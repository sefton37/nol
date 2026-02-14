//! Type tag definitions for the NoLang type system.
//!
//! See SPEC.md Section 3 for type semantics.

use crate::error::DecodeError;

/// Identifies the type of a value or instruction context.
///
/// Every value on the stack has exactly one type tag. Type tags are
/// assigned at BIND time and checked at REF time.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeTag {
    /// No type / not applicable.
    None = 0x00,
    /// Signed 64-bit integer.
    I64 = 0x01,
    /// Unsigned 64-bit integer.
    U64 = 0x02,
    /// IEEE 754 64-bit float.
    F64 = 0x03,
    /// Boolean (0 = false, 1 = true).
    Bool = 0x04,
    /// Unicode codepoint (u32, zero-extended).
    Char = 0x05,
    /// Tagged union. arg1 = variant tag count.
    Variant = 0x06,
    /// Product type. arg1 = field count.
    Tuple = 0x07,
    /// Function type (metadata only).
    FuncType = 0x08,
    /// Fixed-size array. arg1 = element type, arg2 = length.
    Array = 0x09,
    /// Optional. Sugar for VARIANT(2): SOME(0), NONE(1).
    Maybe = 0x0A,
    /// Ok/Err. Sugar for VARIANT(2): OK(0), ERR(1).
    Result = 0x0B,
    /// Zero-size type.
    Unit = 0x0C,
}

/// All valid type tags, in definition order.
pub const ALL_TYPE_TAGS: [TypeTag; 13] = [
    TypeTag::None,
    TypeTag::I64,
    TypeTag::U64,
    TypeTag::F64,
    TypeTag::Bool,
    TypeTag::Char,
    TypeTag::Variant,
    TypeTag::Tuple,
    TypeTag::FuncType,
    TypeTag::Array,
    TypeTag::Maybe,
    TypeTag::Result,
    TypeTag::Unit,
];

impl TryFrom<u8> for TypeTag {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x00 => Ok(TypeTag::None),
            0x01 => Ok(TypeTag::I64),
            0x02 => Ok(TypeTag::U64),
            0x03 => Ok(TypeTag::F64),
            0x04 => Ok(TypeTag::Bool),
            0x05 => Ok(TypeTag::Char),
            0x06 => Ok(TypeTag::Variant),
            0x07 => Ok(TypeTag::Tuple),
            0x08 => Ok(TypeTag::FuncType),
            0x09 => Ok(TypeTag::Array),
            0x0A => Ok(TypeTag::Maybe),
            0x0B => Ok(TypeTag::Result),
            0x0C => Ok(TypeTag::Unit),
            0x0D..=0xFF => Err(DecodeError::ReservedTypeTag(value)),
        }
    }
}

impl TypeTag {
    /// Returns the assembly name for this type tag.
    pub fn name(&self) -> &'static str {
        match self {
            TypeTag::None => "NONE",
            TypeTag::I64 => "I64",
            TypeTag::U64 => "U64",
            TypeTag::F64 => "F64",
            TypeTag::Bool => "BOOL",
            TypeTag::Char => "CHAR",
            TypeTag::Variant => "VARIANT",
            TypeTag::Tuple => "TUPLE",
            TypeTag::FuncType => "FUNC_TYPE",
            TypeTag::Array => "ARRAY",
            TypeTag::Maybe => "MAYBE",
            TypeTag::Result => "RESULT",
            TypeTag::Unit => "UNIT",
        }
    }

    /// Returns true if this type tag represents a numeric type (I64, U64, F64).
    pub fn is_numeric(&self) -> bool {
        matches!(self, TypeTag::I64 | TypeTag::U64 | TypeTag::F64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::DecodeError;

    #[test]
    fn all_type_tags_count() {
        assert_eq!(ALL_TYPE_TAGS.len(), 13);
    }

    #[test]
    fn roundtrip_all_valid_type_tags() {
        for &tag in &ALL_TYPE_TAGS {
            let byte = tag as u8;
            let decoded = TypeTag::try_from(byte).unwrap();
            assert_eq!(tag, decoded, "roundtrip failed for {tag:?} ({byte:#04x})");
        }
    }

    #[test]
    fn reserved_type_tags() {
        for byte in 0x0D..=0xFFu8 {
            assert_eq!(
                TypeTag::try_from(byte),
                Err(DecodeError::ReservedTypeTag(byte)),
                "byte {byte:#04x} should be reserved"
            );
        }
    }

    #[test]
    fn every_byte_value_resolves() {
        for byte in 0..=255u8 {
            let result = TypeTag::try_from(byte);
            match result {
                Ok(_) | Err(DecodeError::ReservedTypeTag(_)) => {}
                other => panic!("unexpected result for byte {byte:#04x}: {other:?}"),
            }
        }
    }

    #[test]
    fn name_roundtrip() {
        for &tag in &ALL_TYPE_TAGS {
            let n = tag.name();
            assert!(!n.is_empty(), "empty name for {tag:?}");
        }
    }

    #[test]
    fn numeric_types() {
        assert!(TypeTag::I64.is_numeric());
        assert!(TypeTag::U64.is_numeric());
        assert!(TypeTag::F64.is_numeric());
        assert!(!TypeTag::Bool.is_numeric());
        assert!(!TypeTag::Char.is_numeric());
        assert!(!TypeTag::None.is_numeric());
        assert!(!TypeTag::Unit.is_numeric());
    }
}
