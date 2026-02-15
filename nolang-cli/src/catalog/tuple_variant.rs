//! Tuple and variant programs (15): ex100-ex114.

use super::{Category, ProgramSpec};

pub fn programs() -> Vec<ProgramSpec> {
    vec![
        // ex100: pair_first — Extract the first element from a pair
        ProgramSpec {
            id: "ex100_pair_first",
            intent: "Extract the first element from a pair of integers (3, 7)",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0003\n",
                "CONST I64 0x0000 0x0007\n",
                "TUPLE_NEW TUPLE 2\n",
                "PROJECT 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex101: pair_second — Extract the second element from a pair
        ProgramSpec {
            id: "ex101_pair_second",
            intent: "Extract the second element from a pair of integers (3, 7)",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0003\n",
                "CONST I64 0x0000 0x0007\n",
                "TUPLE_NEW TUPLE 2\n",
                "PROJECT 1\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex102: triple_first — Extract the first element from a triple
        ProgramSpec {
            id: "ex102_triple_first",
            intent: "Extract the first element from a triple of integers (1, 2, 3)",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "CONST I64 0x0000 0x0002\n",
                "CONST I64 0x0000 0x0003\n",
                "TUPLE_NEW TUPLE 3\n",
                "PROJECT 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex103: triple_second — Extract the second element from a triple
        ProgramSpec {
            id: "ex103_triple_second",
            intent: "Extract the second element from a triple of integers (1, 2, 3)",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "CONST I64 0x0000 0x0002\n",
                "CONST I64 0x0000 0x0003\n",
                "TUPLE_NEW TUPLE 3\n",
                "PROJECT 1\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex104: triple_third — Extract the third element from a triple
        ProgramSpec {
            id: "ex104_triple_third",
            intent: "Extract the third element from a triple of integers (1, 2, 3)",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "CONST I64 0x0000 0x0002\n",
                "CONST I64 0x0000 0x0003\n",
                "TUPLE_NEW TUPLE 3\n",
                "PROJECT 2\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex105: unwrap_some — Unwrap a Some(42) variant, returning the inner value
        ProgramSpec {
            id: "ex105_unwrap_some",
            intent: "Unwrap a Some(42) variant, returning the inner value",
            assembly_template: concat!(
                "CONST I64 0x0000 0x002a\n",
                "VARIANT_NEW VARIANT 2 0\n",
                "MATCH 2\n",
                "CASE 0 2\n",
                "BIND\n",
                "REF 0\n",
                "CASE 1 1\n",
                "CONST I64 0x0000 0x0000\n",
                "EXHAUST\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex106: unwrap_none — Handle a None variant with a default value of zero
        ProgramSpec {
            id: "ex106_unwrap_none",
            intent: "Handle a None variant with a default value of zero",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0000\n",
                "VARIANT_NEW VARIANT 2 1\n",
                "MATCH 2\n",
                "CASE 0 2\n",
                "BIND\n",
                "REF 0\n",
                "CASE 1 1\n",
                "CONST I64 0x0000 0x0000\n",
                "EXHAUST\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex107: result_ok — Create an Ok(10) result and extract its value
        ProgramSpec {
            id: "ex107_result_ok",
            intent: "Create an Ok(10) result and extract its value",
            assembly_template: concat!(
                "CONST I64 0x0000 0x000a\n",
                "VARIANT_NEW VARIANT 2 0\n",
                "MATCH 2\n",
                "CASE 0 2\n",
                "BIND\n",
                "REF 0\n",
                "CASE 1 1\n",
                "CONST I64 0xffff 0xffff\n",
                "EXHAUST\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex108: result_err — Create an Err(-1) result and return a default on error
        ProgramSpec {
            id: "ex108_result_err",
            intent: "Create an Err(-1) result and return a default on error",
            assembly_template: concat!(
                "CONST I64 0xffff 0xffff\n",
                "VARIANT_NEW VARIANT 2 1\n",
                "MATCH 2\n",
                "CASE 0 2\n",
                "BIND\n",
                "REF 0\n",
                "CASE 1 1\n",
                "CONST I64 0x0000 0x0000\n",
                "EXHAUST\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex109: pair_sum — Create a pair and sum both elements
        ProgramSpec {
            id: "ex109_pair_sum",
            intent: "Create a pair and sum both elements (5 + 8 = 13)",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0005\n",
                "CONST I64 0x0000 0x0008\n",
                "ADD\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex110: nested_tuple — Create a nested structure and extract an inner value
        ProgramSpec {
            id: "ex110_nested_tuple",
            intent: "Create a nested tuple ((1, 2), 3) and extract the inner first element",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "CONST I64 0x0000 0x0002\n",
                "TUPLE_NEW TUPLE 2\n",
                "CONST I64 0x0000 0x0003\n",
                "TUPLE_NEW TUPLE 2\n",
                "PROJECT 0\n",
                "PROJECT 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex111: triple_sum — Create a triple and compute the sum of all elements
        ProgramSpec {
            id: "ex111_triple_sum",
            intent: "Create a triple and compute the sum of all elements (4 + 5 + 6 = 15)",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0004\n",
                "CONST I64 0x0000 0x0005\n",
                "ADD\n",
                "CONST I64 0x0000 0x0006\n",
                "ADD\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex112: variant_tag_check — Create a 3-variant value and match all tags
        ProgramSpec {
            id: "ex112_variant_tag_check",
            intent: "Create a 3-variant value with tag 1 and return a tag-specific value",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0064\n",
                "VARIANT_NEW VARIANT 3 1\n",
                "MATCH 3\n",
                "CASE 0 1\n",
                "CONST I64 0x0000 0x0001\n",
                "CASE 1 2\n",
                "BIND\n",
                "CONST I64 0x0000 0x0002\n",
                "CASE 2 1\n",
                "CONST I64 0x0000 0x0003\n",
                "EXHAUST\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex113: maybe_map — Unwrap Some(5) and add 10 to get 15
        ProgramSpec {
            id: "ex113_maybe_map",
            intent: "Unwrap Some(5) and add 10 to get 15",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0005\n",
                "VARIANT_NEW VARIANT 2 0\n",
                "MATCH 2\n",
                "CASE 0 4\n",
                "BIND\n",
                "REF 0\n",
                "CONST I64 0x0000 0x000a\n",
                "ADD\n",
                "CASE 1 1\n",
                "CONST I64 0x0000 0x0000\n",
                "EXHAUST\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
        // ex114: pair_max — Create a pair and return the larger element
        ProgramSpec {
            id: "ex114_pair_max",
            intent: "Return the maximum of two values (7, 5)",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0007\n",
                "CONST I64 0x0000 0x0005\n",
                "GTE\n",
                "MATCH 2\n",
                "CASE 0 1\n",
                "CONST I64 0x0000 0x0005\n",
                "CASE 1 1\n",
                "CONST I64 0x0000 0x0007\n",
                "EXHAUST\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::TupleVariant,
        },
    ]
}
