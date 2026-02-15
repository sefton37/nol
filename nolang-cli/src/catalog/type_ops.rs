//! Type operation programs (10): ex200-ex209.
//!
//! Standalone programs demonstrating TYPEOF, type tags, and type-specific constants.

use super::{Category, ProgramSpec};

pub fn programs() -> Vec<ProgramSpec> {
    vec![
        // ex200: TYPEOF I64 (TYPEOF + ASSERT pattern)
        ProgramSpec {
            id: "ex200_typeof_i64",
            intent: "Verify an I64 value has type tag I64, then return the value",
            assembly_template: concat!(
                "CONST I64 0x0000 0x002a\n",
                "TYPEOF I64\n",
                "ASSERT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TypeOps,
        },
        // ex201: TYPEOF Bool
        ProgramSpec {
            id: "ex201_typeof_bool",
            intent: "Verify a Bool value has type tag Bool, then return the value",
            assembly_template: concat!(
                "CONST BOOL 0x0001 0x0000\n",
                "TYPEOF BOOL\n",
                "ASSERT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TypeOps,
        },
        // ex202: TYPEOF U64
        ProgramSpec {
            id: "ex202_typeof_u64",
            intent: "Verify a U64 value has type tag U64, then return the value",
            assembly_template: concat!(
                "CONST U64 0x0000 0x0064\n",
                "TYPEOF U64\n",
                "ASSERT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TypeOps,
        },
        // ex203: TYPEOF mismatch check (I64 is NOT Bool)
        ProgramSpec {
            id: "ex203_typeof_mismatch",
            intent: "Verify an I64 value is not of type Bool, then return the value",
            assembly_template: concat!(
                "CONST I64 0x0000 0x002a\n",
                "TYPEOF BOOL\n",
                "NOT\n",
                "ASSERT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TypeOps,
        },
        // ex204: large I64 constant
        ProgramSpec {
            id: "ex204_const_i64",
            intent: "Create a large I64 constant",
            assembly_template: concat!("CONST I64 0x0001 0x0000\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::TypeOps,
        },
        // ex205: U64 constant
        ProgramSpec {
            id: "ex205_const_u64",
            intent: "Create a U64 constant",
            assembly_template: concat!("CONST U64 0x0000 0x00ff\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::TypeOps,
        },
        // ex206: negative I64 constant
        ProgramSpec {
            id: "ex206_const_neg",
            intent: "Create a negative I64 constant",
            assembly_template: concat!("CONST I64 0xffff 0xff00\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::TypeOps,
        },
        // ex207: U64 arithmetic
        ProgramSpec {
            id: "ex207_u64_add",
            intent: "Add two U64 values",
            assembly_template: concat!(
                "CONST U64 0x0000 0x0064\n",
                "CONST U64 0x0000 0x00c8\n",
                "ADD\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TypeOps,
        },
        // ex208: Bool operations
        ProgramSpec {
            id: "ex208_bool_ops",
            intent: "Perform logical OR on two Bool values",
            assembly_template: concat!(
                "CONST BOOL 0x0001 0x0000\n",
                "CONST BOOL 0x0000 0x0000\n",
                "OR\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TypeOps,
        },
        // ex209: Char constant
        ProgramSpec {
            id: "ex209_char_constant",
            intent: "Create a Char constant representing 'A'",
            assembly_template: concat!("CONST CHAR 0x0041 0x0000\n", "HALT\n",),
            witness_cases: vec![],
            category: Category::TypeOps,
        },
    ]
}
