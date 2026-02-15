//! Character operation programs (10): ex230-ex239.
//!
//! Function programs that take Char inputs and return Bool results.

use super::{Category, ProgramSpec, WitnessCase, WitnessValue};

/// Helper to build a witness case with Char input and Bool expected output.
fn char_to_bool_case(input: char, expected: bool) -> WitnessCase {
    WitnessCase {
        inputs: vec![WitnessValue::Char(input)],
        expected: WitnessValue::Bool(expected),
    }
}

/// Helper to build a witness case with two Char inputs and Bool expected output.
fn char2_to_bool_case(input1: char, input2: char, expected: bool) -> WitnessCase {
    WitnessCase {
        inputs: vec![WitnessValue::Char(input1), WitnessValue::Char(input2)],
        expected: WitnessValue::Bool(expected),
    }
}

pub fn programs() -> Vec<ProgramSpec> {
    vec![
        // ex230: is uppercase 'A'
        ProgramSpec {
            id: "ex230_is_uppercase_a",
            intent: "Check if a character equals 'A'",
            assembly_template: concat!(
                "FUNC 1 6\n",
                "PARAM CHAR\n",
                "REF 0\n",
                "CONST CHAR 0x0041 0x0000\n",
                "EQ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST CHAR 0x0041 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![
                char_to_bool_case('A', true),
                char_to_bool_case('B', false),
                char_to_bool_case('a', false),
                char_to_bool_case('0', false),
            ],
            category: Category::CharOps,
        },
        // ex231: is lowercase 'a'
        ProgramSpec {
            id: "ex231_is_lowercase_a",
            intent: "Check if a character equals 'a'",
            assembly_template: concat!(
                "FUNC 1 6\n",
                "PARAM CHAR\n",
                "REF 0\n",
                "CONST CHAR 0x0061 0x0000\n",
                "EQ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST CHAR 0x0061 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![
                char_to_bool_case('a', true),
                char_to_bool_case('A', false),
                char_to_bool_case('b', false),
                char_to_bool_case('1', false),
            ],
            category: Category::CharOps,
        },
        // ex232: is digit zero
        ProgramSpec {
            id: "ex232_is_digit_zero",
            intent: "Check if a character equals '0'",
            assembly_template: concat!(
                "FUNC 1 6\n",
                "PARAM CHAR\n",
                "REF 0\n",
                "CONST CHAR 0x0030 0x0000\n",
                "EQ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST CHAR 0x0030 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![
                char_to_bool_case('0', true),
                char_to_bool_case('1', false),
                char_to_bool_case('O', false),
                char_to_bool_case(' ', false),
            ],
            category: Category::CharOps,
        },
        // ex233: is space
        ProgramSpec {
            id: "ex233_is_space",
            intent: "Check if a character equals space",
            assembly_template: concat!(
                "FUNC 1 6\n",
                "PARAM CHAR\n",
                "REF 0\n",
                "CONST CHAR 0x0020 0x0000\n",
                "EQ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST CHAR 0x0020 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![
                char_to_bool_case(' ', true),
                char_to_bool_case('\t', false),
                char_to_bool_case('_', false),
                char_to_bool_case('A', false),
            ],
            category: Category::CharOps,
        },
        // ex234: is newline
        ProgramSpec {
            id: "ex234_is_newline",
            intent: "Check if a character equals newline",
            assembly_template: concat!(
                "FUNC 1 6\n",
                "PARAM CHAR\n",
                "REF 0\n",
                "CONST CHAR 0x000a 0x0000\n",
                "EQ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST CHAR 0x000a 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![
                char_to_bool_case('\n', true),
                char_to_bool_case('\r', false),
                char_to_bool_case(' ', false),
                char_to_bool_case('n', false),
            ],
            category: Category::CharOps,
        },
        // ex235: char equality (2 params)
        ProgramSpec {
            id: "ex235_char_eq",
            intent: "Check if two characters are equal",
            assembly_template: concat!(
                "FUNC 2 7\n",
                "PARAM CHAR\n",
                "PARAM CHAR\n",
                "REF 1\n",
                "REF 0\n",
                "EQ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST CHAR 0x0041 0x0000\n",
                "CONST CHAR 0x0041 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![
                char2_to_bool_case('A', 'A', true),
                char2_to_bool_case('A', 'B', false),
                char2_to_bool_case('a', 'a', true),
                char2_to_bool_case('0', '1', false),
            ],
            category: Category::CharOps,
        },
        // ex236: is exclamation mark
        ProgramSpec {
            id: "ex236_is_exclaim",
            intent: "Check if a character equals '!'",
            assembly_template: concat!(
                "FUNC 1 6\n",
                "PARAM CHAR\n",
                "REF 0\n",
                "CONST CHAR 0x0021 0x0000\n",
                "EQ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST CHAR 0x0021 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![
                char_to_bool_case('!', true),
                char_to_bool_case('?', false),
                char_to_bool_case('.', false),
                char_to_bool_case('1', false),
            ],
            category: Category::CharOps,
        },
        // ex237: is period
        ProgramSpec {
            id: "ex237_is_period",
            intent: "Check if a character equals '.'",
            assembly_template: concat!(
                "FUNC 1 6\n",
                "PARAM CHAR\n",
                "REF 0\n",
                "CONST CHAR 0x002e 0x0000\n",
                "EQ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST CHAR 0x002e 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![
                char_to_bool_case('.', true),
                char_to_bool_case(',', false),
                char_to_bool_case('!', false),
                char_to_bool_case(' ', false),
            ],
            category: Category::CharOps,
        },
        // ex238: is comma
        ProgramSpec {
            id: "ex238_is_comma",
            intent: "Check if a character equals ','",
            assembly_template: concat!(
                "FUNC 1 6\n",
                "PARAM CHAR\n",
                "REF 0\n",
                "CONST CHAR 0x002c 0x0000\n",
                "EQ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST CHAR 0x002c 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![
                char_to_bool_case(',', true),
                char_to_bool_case('.', false),
                char_to_bool_case(';', false),
                char_to_bool_case(' ', false),
            ],
            category: Category::CharOps,
        },
        // ex239: is null character
        ProgramSpec {
            id: "ex239_is_null",
            intent: "Check if a character equals null character",
            assembly_template: concat!(
                "FUNC 1 6\n",
                "PARAM CHAR\n",
                "REF 0\n",
                "CONST CHAR 0x0000 0x0000\n",
                "EQ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST CHAR 0x0000 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![
                char_to_bool_case('\0', true),
                char_to_bool_case('0', false),
                char_to_bool_case(' ', false),
                char_to_bool_case('a', false),
            ],
            category: Category::CharOps,
        },
    ]
}
