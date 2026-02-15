//! Array and FORALL programs (15): ex130-ex144.

use super::{Category, ProgramSpec};

pub fn programs() -> Vec<ProgramSpec> {
    vec![
        // ex130: array_length — Create [1,2,3], ARRAY_LEN → 3 (as U64).
        ProgramSpec {
            id: "ex130_array_length",
            intent: "Create an array of three integers and return its length",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "CONST I64 0x0000 0x0002\n",
                "CONST I64 0x0000 0x0003\n",
                "ARRAY_NEW I64 3\n",
                "ARRAY_LEN\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex131: array_first — Create [10,20,30], CONST U64 0, ARRAY_GET → 10.
        ProgramSpec {
            id: "ex131_array_first",
            intent: "Create an array and retrieve the first element",
            assembly_template: concat!(
                "CONST I64 0x0000 0x000a\n",
                "CONST I64 0x0000 0x0014\n",
                "CONST I64 0x0000 0x001e\n",
                "ARRAY_NEW I64 3\n",
                "CONST U64 0x0000 0x0000\n",
                "ARRAY_GET\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex132: array_last — Create [10,20,30], CONST U64 2, ARRAY_GET → 30.
        ProgramSpec {
            id: "ex132_array_last",
            intent: "Create an array and retrieve the last element",
            assembly_template: concat!(
                "CONST I64 0x0000 0x000a\n",
                "CONST I64 0x0000 0x0014\n",
                "CONST I64 0x0000 0x001e\n",
                "ARRAY_NEW I64 3\n",
                "CONST U64 0x0000 0x0002\n",
                "ARRAY_GET\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex133: forall_positive — Create [1,2,3], FORALL 3, REF 0, CONST 0, GT → true.
        ProgramSpec {
            id: "ex133_forall_positive",
            intent: "Verify all elements in [1, 2, 3] are positive",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "CONST I64 0x0000 0x0002\n",
                "CONST I64 0x0000 0x0003\n",
                "ARRAY_NEW I64 3\n",
                "FORALL 3\n",
                "REF 0\n",
                "CONST I64 0x0000 0x0000\n",
                "GT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex134: forall_negative — Create [-1,-2,-3], FORALL 3, REF 0, CONST 0, LT → true.
        ProgramSpec {
            id: "ex134_forall_negative",
            intent: "Verify all elements in [-1, -2, -3] are negative",
            assembly_template: concat!(
                "CONST I64 0xffff 0xffff\n",
                "CONST I64 0xffff 0xfffe\n",
                "CONST I64 0xffff 0xfffd\n",
                "ARRAY_NEW I64 3\n",
                "FORALL 3\n",
                "REF 0\n",
                "CONST I64 0x0000 0x0000\n",
                "LT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex135: forall_bounded — Create [1,5,9], FORALL 7, REF 0, CONST 0, GT, REF 0, CONST 10, LT, AND → true.
        ProgramSpec {
            id: "ex135_forall_bounded",
            intent: "Verify all elements in [1, 5, 9] are between 0 and 10",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "CONST I64 0x0000 0x0005\n",
                "CONST I64 0x0000 0x0009\n",
                "ARRAY_NEW I64 3\n",
                "FORALL 7\n",
                "REF 0\n",
                "CONST I64 0x0000 0x0000\n",
                "GT\n",
                "REF 0\n",
                "CONST I64 0x0000 0x000a\n",
                "LT\n",
                "AND\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex136: forall_even — Create [2,4,6], FORALL 5, REF 0, CONST 2, MOD, CONST 0, EQ → true.
        ProgramSpec {
            id: "ex136_forall_even",
            intent: "Verify all elements in [2, 4, 6] are even",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0002\n",
                "CONST I64 0x0000 0x0004\n",
                "CONST I64 0x0000 0x0006\n",
                "ARRAY_NEW I64 3\n",
                "FORALL 5\n",
                "REF 0\n",
                "CONST I64 0x0000 0x0002\n",
                "MOD\n",
                "CONST I64 0x0000 0x0000\n",
                "EQ\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex137: forall_nonzero — Create [1,2,3], FORALL 3, REF 0, CONST 0, NEQ → true.
        ProgramSpec {
            id: "ex137_forall_nonzero",
            intent: "Verify all elements in [1, 2, 3] are non-zero",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "CONST I64 0x0000 0x0002\n",
                "CONST I64 0x0000 0x0003\n",
                "ARRAY_NEW I64 3\n",
                "FORALL 3\n",
                "REF 0\n",
                "CONST I64 0x0000 0x0000\n",
                "NEQ\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex138: array_single — Create [42], CONST U64 0, ARRAY_GET → 42.
        ProgramSpec {
            id: "ex138_array_single",
            intent: "Create a single-element array and retrieve its value",
            assembly_template: concat!(
                "CONST I64 0x0000 0x002a\n",
                "ARRAY_NEW I64 1\n",
                "CONST U64 0x0000 0x0000\n",
                "ARRAY_GET\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex139: array_middle — Create [1,2,3,4,5], CONST U64 2, ARRAY_GET → 3.
        ProgramSpec {
            id: "ex139_array_middle",
            intent: "Create a five-element array and retrieve the middle element",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "CONST I64 0x0000 0x0002\n",
                "CONST I64 0x0000 0x0003\n",
                "CONST I64 0x0000 0x0004\n",
                "CONST I64 0x0000 0x0005\n",
                "ARRAY_NEW I64 5\n",
                "CONST U64 0x0000 0x0002\n",
                "ARRAY_GET\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex140: forall_mixed — Create [1,-2,3], FORALL 3, REF 0, CONST 0, GT → false (because -2 < 0).
        ProgramSpec {
            id: "ex140_forall_mixed",
            intent: "Test FORALL with mixed signs, expecting false due to negative element",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "CONST I64 0xffff 0xfffe\n",
                "CONST I64 0x0000 0x0003\n",
                "ARRAY_NEW I64 3\n",
                "FORALL 3\n",
                "REF 0\n",
                "CONST I64 0x0000 0x0000\n",
                "GT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex141: forall_all_same — Create [5,5,5], FORALL 3, REF 0, CONST 5, EQ → true.
        ProgramSpec {
            id: "ex141_forall_all_same",
            intent: "Verify all elements in [5, 5, 5] equal 5",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0005\n",
                "CONST I64 0x0000 0x0005\n",
                "CONST I64 0x0000 0x0005\n",
                "ARRAY_NEW I64 3\n",
                "FORALL 3\n",
                "REF 0\n",
                "CONST I64 0x0000 0x0005\n",
                "EQ\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex142: forall_empty — Create empty I64 array []. FORALL 3, REF 0, CONST 0, GT → true (vacuous).
        ProgramSpec {
            id: "ex142_forall_empty",
            intent: "Test FORALL on an empty array (vacuously true)",
            assembly_template: concat!(
                "ARRAY_NEW I64 0\n",
                "FORALL 3\n",
                "REF 0\n",
                "CONST I64 0x0000 0x0000\n",
                "GT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex143: array_two — Create [100,200], CONST U64 1, ARRAY_GET → 200.
        ProgramSpec {
            id: "ex143_array_two",
            intent: "Create a two-element array and retrieve the second element",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0064\n",
                "CONST I64 0x0000 0x00c8\n",
                "ARRAY_NEW I64 2\n",
                "CONST U64 0x0000 0x0001\n",
                "ARRAY_GET\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
        // ex144: forall_le_ten — Create [3,7,10], FORALL 3, REF 0, CONST 10, LTE → true.
        ProgramSpec {
            id: "ex144_forall_le_ten",
            intent: "Verify all elements in [3, 7, 10] are less than or equal to 10",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0003\n",
                "CONST I64 0x0000 0x0007\n",
                "CONST I64 0x0000 0x000a\n",
                "ARRAY_NEW I64 3\n",
                "FORALL 3\n",
                "REF 0\n",
                "CONST I64 0x0000 0x000a\n",
                "LTE\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ArrayForall,
        },
    ]
}
