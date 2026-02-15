//! Program catalog — 220 program specifications for corpus generation.
//!
//! Each program is defined as a `ProgramSpec` with assembly template,
//! natural-language intent, and optional witness test cases.

pub mod array_forall;
pub mod binding_drop;
pub mod bitwise_ops;
pub mod boolean_logic;
pub mod char_ops;
pub mod comparison;
pub mod constants;
pub mod integer_math;
pub mod multi_function;
pub mod pattern_match;
pub mod recursive;
pub mod rich_contracts;
pub mod tuple_variant;
pub mod type_ops;

/// A program specification for corpus generation.
#[derive(Debug, Clone)]
pub struct ProgramSpec {
    /// Unique identifier, e.g., "ex020_square".
    pub id: &'static str,
    /// Natural-language description of what the program does.
    pub intent: &'static str,
    /// Assembly text with placeholder hashes (`HASH 0x0000 0x0000 0x0000`).
    pub assembly_template: &'static str,
    /// Witness test cases (empty for standalone programs).
    pub witness_cases: Vec<WitnessCase>,
    /// Program category.
    pub category: Category,
}

/// A single witness test case: concrete inputs and expected output.
#[derive(Debug, Clone)]
pub struct WitnessCase {
    pub inputs: Vec<WitnessValue>,
    pub expected: WitnessValue,
}

/// A typed value for witness test cases.
///
/// These are converted to JSON for witness files and to `Value` for VM execution.
#[derive(Debug, Clone)]
pub enum WitnessValue {
    I64(i64),
    U64(u64),
    F64(f64),
    Bool(bool),
    Char(char),
    Unit,
}

/// Program category for organization and opcode coverage tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    IntegerMath,
    Comparison,
    BooleanLogic,
    BitwiseOps,
    TupleVariant,
    PatternMatch,
    ArrayForall,
    Recursive,
    RichContracts,
    MultiFunction,
    TypeOps,
    Constants,
    BindingDrop,
    CharOps,
}

/// Collect all 220 program specifications from every category module.
pub fn all_programs() -> Vec<ProgramSpec> {
    let mut programs = Vec::with_capacity(220);
    programs.extend(integer_math::programs());
    programs.extend(comparison::programs());
    programs.extend(boolean_logic::programs());
    programs.extend(bitwise_ops::programs());
    programs.extend(tuple_variant::programs());
    programs.extend(pattern_match::programs());
    programs.extend(array_forall::programs());
    programs.extend(recursive::programs());
    programs.extend(rich_contracts::programs());
    programs.extend(multi_function::programs());
    programs.extend(type_ops::programs());
    programs.extend(constants::programs());
    programs.extend(binding_drop::programs());
    programs.extend(char_ops::programs());
    programs
}

impl std::fmt::Display for Category {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Category::IntegerMath => write!(f, "integer_math"),
            Category::Comparison => write!(f, "comparison"),
            Category::BooleanLogic => write!(f, "boolean_logic"),
            Category::BitwiseOps => write!(f, "bitwise_ops"),
            Category::TupleVariant => write!(f, "tuple_variant"),
            Category::PatternMatch => write!(f, "pattern_match"),
            Category::ArrayForall => write!(f, "array_forall"),
            Category::Recursive => write!(f, "recursive"),
            Category::RichContracts => write!(f, "rich_contracts"),
            Category::MultiFunction => write!(f, "multi_function"),
            Category::TypeOps => write!(f, "type_ops"),
            Category::Constants => write!(f, "constants"),
            Category::BindingDrop => write!(f, "binding_drop"),
            Category::CharOps => write!(f, "char_ops"),
        }
    }
}

impl WitnessValue {
    /// Convert to JSON string for witness files.
    pub fn to_json(&self) -> String {
        match self {
            WitnessValue::I64(v) => format!("{v}"),
            WitnessValue::U64(v) => format!("{{\"U64\":{v}}}"),
            WitnessValue::F64(v) => {
                // Ensure .0 for integer-valued floats
                if v.fract() == 0.0 && v.is_finite() {
                    format!("{{\"F64\":{v}.0}}")
                } else {
                    format!("{{\"F64\":{v}}}")
                }
            }
            WitnessValue::Bool(v) => format!("{v}"),
            WitnessValue::Char(c) => format!("{{\"Char\":{}}}", *c as u32),
            WitnessValue::Unit => "null".to_string(),
        }
    }

    /// Convert to JSON string for use as an input element.
    /// For I64 inputs, just the number. For others, use type escapes.
    pub fn to_input_json(&self) -> String {
        self.to_json()
    }
}

/// Helper to build a witness case with I64 inputs and I64 expected output.
pub fn i64_case(inputs: &[i64], expected: i64) -> WitnessCase {
    WitnessCase {
        inputs: inputs.iter().map(|&v| WitnessValue::I64(v)).collect(),
        expected: WitnessValue::I64(expected),
    }
}

/// Helper to build a witness case with I64 inputs and Bool expected output.
pub fn i64_to_bool_case(inputs: &[i64], expected: bool) -> WitnessCase {
    WitnessCase {
        inputs: inputs.iter().map(|&v| WitnessValue::I64(v)).collect(),
        expected: WitnessValue::Bool(expected),
    }
}

/// Helper to build a witness case with Bool inputs and Bool expected output.
pub fn bool_case(inputs: &[bool], expected: bool) -> WitnessCase {
    WitnessCase {
        inputs: inputs.iter().map(|&v| WitnessValue::Bool(v)).collect(),
        expected: WitnessValue::Bool(expected),
    }
}

/// Helper to build a witness case with Bool inputs and I64 expected output.
pub fn bool_to_i64_case(inputs: &[bool], expected: i64) -> WitnessCase {
    WitnessCase {
        inputs: inputs.iter().map(|&v| WitnessValue::Bool(v)).collect(),
        expected: WitnessValue::I64(expected),
    }
}
