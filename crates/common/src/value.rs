//! Runtime value representation for the NoLang VM.
//!
//! Values are what live on the stack during execution.

use crate::type_tag::TypeTag;

/// Runtime value representation.
///
/// This enum is used by the VM to represent values on the stack and
/// in the binding environment.
#[derive(Debug, Clone)]
pub enum Value {
    /// Signed 64-bit integer.
    I64(i64),
    /// Unsigned 64-bit integer.
    U64(u64),
    /// IEEE 754 64-bit float. NaN and infinity never occur in valid programs
    /// (the VM rejects them as runtime errors).
    F64(f64),
    /// Boolean value.
    Bool(bool),
    /// Unicode codepoint.
    Char(char),
    /// Zero-size unit value.
    Unit,
    /// Tagged union value.
    Variant {
        /// Total number of possible tags for this variant type.
        tag_count: u16,
        /// The active tag (0-indexed).
        tag: u16,
        /// The payload value.
        payload: Box<Value>,
    },
    /// Product type (ordered collection of values).
    Tuple(Vec<Value>),
    /// Fixed-size array (all elements same type).
    Array(Vec<Value>),
}

// We use bitwise equality for F64 values via to_bits(). This means
// NaN == NaN when the bit patterns match. In practice, the VM rejects
// NaN and infinity as runtime errors, so this case should never arise
// in valid programs. This approach keeps Value well-behaved in Rust
// (implements Eq, usable in HashMaps) while the VM enforces the
// stronger guarantee that NaN simply doesn't exist.
impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::I64(a), Value::I64(b)) => a == b,
            (Value::U64(a), Value::U64(b)) => a == b,
            (Value::F64(a), Value::F64(b)) => a.to_bits() == b.to_bits(),
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Unit, Value::Unit) => true,
            (
                Value::Variant {
                    tag_count: tc1,
                    tag: t1,
                    payload: p1,
                },
                Value::Variant {
                    tag_count: tc2,
                    tag: t2,
                    payload: p2,
                },
            ) => tc1 == tc2 && t1 == t2 && p1 == p2,
            (Value::Tuple(a), Value::Tuple(b)) => a == b,
            (Value::Array(a), Value::Array(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Value {}

impl Value {
    /// Returns the type tag for this value.
    pub fn type_tag(&self) -> TypeTag {
        match self {
            Value::I64(_) => TypeTag::I64,
            Value::U64(_) => TypeTag::U64,
            Value::F64(_) => TypeTag::F64,
            Value::Bool(_) => TypeTag::Bool,
            Value::Char(_) => TypeTag::Char,
            Value::Unit => TypeTag::Unit,
            Value::Variant { .. } => TypeTag::Variant,
            Value::Tuple(_) => TypeTag::Tuple,
            Value::Array(_) => TypeTag::Array,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn type_tags() {
        assert_eq!(Value::I64(42).type_tag(), TypeTag::I64);
        assert_eq!(Value::U64(42).type_tag(), TypeTag::U64);
        assert_eq!(Value::F64(3.14).type_tag(), TypeTag::F64);
        assert_eq!(Value::Bool(true).type_tag(), TypeTag::Bool);
        assert_eq!(Value::Char('a').type_tag(), TypeTag::Char);
        assert_eq!(Value::Unit.type_tag(), TypeTag::Unit);
        assert_eq!(
            Value::Variant {
                tag_count: 2,
                tag: 0,
                payload: Box::new(Value::Unit)
            }
            .type_tag(),
            TypeTag::Variant
        );
        assert_eq!(Value::Tuple(vec![]).type_tag(), TypeTag::Tuple);
        assert_eq!(Value::Array(vec![]).type_tag(), TypeTag::Array);
    }

    #[test]
    fn equality_i64() {
        assert_eq!(Value::I64(42), Value::I64(42));
        assert_ne!(Value::I64(42), Value::I64(43));
    }

    #[test]
    fn equality_f64_normal() {
        assert_eq!(Value::F64(3.14), Value::F64(3.14));
        assert_ne!(Value::F64(3.14), Value::F64(2.71));
    }

    #[test]
    fn equality_f64_bitwise_nan() {
        // NaN == NaN via bitwise comparison (same bit pattern)
        let nan = f64::NAN;
        assert_eq!(Value::F64(nan), Value::F64(nan));
    }

    #[test]
    fn equality_f64_positive_negative_zero() {
        // +0.0 and -0.0 have different bit patterns
        assert_ne!(Value::F64(0.0), Value::F64(-0.0));
    }

    #[test]
    fn equality_different_types() {
        assert_ne!(Value::I64(42), Value::U64(42));
        assert_ne!(Value::Bool(true), Value::I64(1));
    }

    #[test]
    fn equality_variant() {
        let v1 = Value::Variant {
            tag_count: 2,
            tag: 0,
            payload: Box::new(Value::I64(5)),
        };
        let v2 = Value::Variant {
            tag_count: 2,
            tag: 0,
            payload: Box::new(Value::I64(5)),
        };
        let v3 = Value::Variant {
            tag_count: 2,
            tag: 1,
            payload: Box::new(Value::I64(5)),
        };
        assert_eq!(v1, v2);
        assert_ne!(v1, v3);
    }

    #[test]
    fn equality_tuple() {
        let t1 = Value::Tuple(vec![Value::I64(1), Value::Bool(true)]);
        let t2 = Value::Tuple(vec![Value::I64(1), Value::Bool(true)]);
        let t3 = Value::Tuple(vec![Value::I64(1), Value::Bool(false)]);
        assert_eq!(t1, t2);
        assert_ne!(t1, t3);
    }

    #[test]
    fn equality_array() {
        let a1 = Value::Array(vec![Value::I64(10), Value::I64(20)]);
        let a2 = Value::Array(vec![Value::I64(10), Value::I64(20)]);
        let a3 = Value::Array(vec![Value::I64(10)]);
        assert_eq!(a1, a2);
        assert_ne!(a1, a3);
    }

    #[test]
    fn clone_deep() {
        let original = Value::Variant {
            tag_count: 2,
            tag: 0,
            payload: Box::new(Value::Tuple(vec![Value::I64(1), Value::F64(2.0)])),
        };
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }
}
