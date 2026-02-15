//! Constant programs (10): ex210-ex219.
//!
//! Standalone programs demonstrating various constant values.

use super::{Category, ProgramSpec};

pub fn programs() -> Vec<ProgramSpec> {
    vec![
        // ex210: zero
        ProgramSpec {
            id: "ex210_zero",
            intent: "Return the constant zero",
            assembly_template: concat!("CONST I64 0x0000 0x0000\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::Constants,
        },
        // ex211: one
        ProgramSpec {
            id: "ex211_one",
            intent: "Return the constant one",
            assembly_template: concat!("CONST I64 0x0000 0x0001\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::Constants,
        },
        // ex212: negative one
        ProgramSpec {
            id: "ex212_neg_one",
            intent: "Return the constant negative one",
            assembly_template: concat!("CONST I64 0xffff 0xffff\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::Constants,
        },
        // ex213: max I16 in I64
        ProgramSpec {
            id: "ex213_max_i16",
            intent: "Return the maximum 16-bit signed integer",
            assembly_template: concat!("CONST I64 0x0000 0x7fff\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::Constants,
        },
        // ex214: min I16 in I64
        ProgramSpec {
            id: "ex214_min_i16",
            intent: "Return the minimum 16-bit signed integer",
            assembly_template: concat!("CONST I64 0xffff 0x8000\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::Constants,
        },
        // ex215: true
        ProgramSpec {
            id: "ex215_true",
            intent: "Return the constant true",
            assembly_template: concat!("CONST BOOL 0x0001 0x0000\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::Constants,
        },
        // ex216: false
        ProgramSpec {
            id: "ex216_false",
            intent: "Return the constant false",
            assembly_template: concat!("CONST BOOL 0x0000 0x0000\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::Constants,
        },
        // ex217: unit
        ProgramSpec {
            id: "ex217_unit",
            intent: "Return the unit value",
            assembly_template: concat!("CONST UNIT 0x0000 0x0000\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::Constants,
        },
        // ex218: char 'A'
        ProgramSpec {
            id: "ex218_char_a",
            intent: "Return the character 'A'",
            assembly_template: concat!("CONST CHAR 0x0041 0x0000\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::Constants,
        },
        // ex219: large I64
        ProgramSpec {
            id: "ex219_large_i64",
            intent: "Return a large I64 constant (65536)",
            assembly_template: concat!("CONST I64 0x0001 0x0000\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::Constants,
        },
    ]
}
