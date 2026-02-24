//! String operation programs (15): ex240-ex254.
//!
//! Standalone programs demonstrating STR_CONST, STR_LEN, STR_CONCAT,
//! STR_SLICE, STR_SPLIT, STR_BYTES, and BYTES_STR opcodes.
//!
//! All programs are standalone (no witness cases) because the WitnessValue
//! enum does not carry String or Bytes values.

use super::{Category, ProgramSpec};

pub fn programs() -> Vec<ProgramSpec> {
    vec![
        // ex240: push a literal string and return it
        ProgramSpec {
            id: "ex240_str_const",
            intent: "Push the literal string \"hello\" and return it",
            assembly_template: concat!(
                "STR_CONST \"hello\"\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex241: measure the length of a string
        ProgramSpec {
            id: "ex241_str_len",
            intent: "Return the byte length of the string \"hello\"",
            assembly_template: concat!(
                "STR_CONST \"hello\"\n",
                "STR_LEN\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex242: concatenate two strings
        ProgramSpec {
            id: "ex242_str_concat",
            intent: "Concatenate \"foo\" and \"bar\" into \"foobar\"",
            assembly_template: concat!(
                "STR_CONST \"foo\"\n",
                "STR_CONST \"bar\"\n",
                "STR_CONCAT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex243: empty string has length zero
        ProgramSpec {
            id: "ex243_empty_str_len",
            intent: "Return the byte length of the empty string, which is zero",
            assembly_template: concat!(
                "STR_CONST \"\"\n",
                "STR_LEN\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex244: concatenate with empty string
        ProgramSpec {
            id: "ex244_concat_empty",
            intent: "Concatenate \"abc\" with the empty string, yielding \"abc\"",
            assembly_template: concat!(
                "STR_CONST \"abc\"\n",
                "STR_CONST \"\"\n",
                "STR_CONCAT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex245: slice the first three characters
        ProgramSpec {
            id: "ex245_str_slice_prefix",
            intent: "Slice \"hello\" from byte 0 to 3, yielding \"hel\"",
            assembly_template: concat!(
                "STR_CONST \"hello\"\n",
                "CONST U64 0x0000 0x0000\n",   // start = 0
                "CONST U64 0x0000 0x0003\n",   // end   = 3
                "STR_SLICE\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex246: slice the last two characters
        ProgramSpec {
            id: "ex246_str_slice_suffix",
            intent: "Slice \"hello\" from byte 3 to 5, yielding \"lo\"",
            assembly_template: concat!(
                "STR_CONST \"hello\"\n",
                "CONST U64 0x0000 0x0003\n",   // start = 3
                "CONST U64 0x0000 0x0005\n",   // end   = 5
                "STR_SLICE\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex247: full slice (entire string)
        ProgramSpec {
            id: "ex247_str_slice_all",
            intent: "Slice \"hi\" from byte 0 to 2, yielding the whole string",
            assembly_template: concat!(
                "STR_CONST \"hi\"\n",
                "CONST U64 0x0000 0x0000\n",   // start = 0
                "CONST U64 0x0000 0x0002\n",   // end   = 2
                "STR_SLICE\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex248: split a string on a delimiter
        ProgramSpec {
            id: "ex248_str_split",
            intent: "Split \"a,b,c\" on \",\" into an array of parts",
            assembly_template: concat!(
                "STR_CONST \"a,b,c\"\n",
                "STR_CONST \",\"\n",
                "STR_SPLIT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex249: split on a delimiter not present → single-element array
        ProgramSpec {
            id: "ex249_str_split_no_delim",
            intent: "Split \"hello\" on \"/\" where delimiter is absent, yielding one-element array",
            assembly_template: concat!(
                "STR_CONST \"hello\"\n",
                "STR_CONST \"/\"\n",
                "STR_SPLIT\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex250: convert string to bytes
        ProgramSpec {
            id: "ex250_str_bytes",
            intent: "Convert the string \"AB\" to its UTF-8 byte buffer",
            assembly_template: concat!(
                "STR_CONST \"AB\"\n",
                "STR_BYTES\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex251: convert bytes back to string
        ProgramSpec {
            id: "ex251_bytes_str",
            intent: "Convert bytes of \"ok\" back to a string via BYTES_STR, yielding Ok(\"ok\")",
            assembly_template: concat!(
                "STR_CONST \"ok\"\n",
                "STR_BYTES\n",
                "BYTES_STR\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex252: string function taking a STRING param
        ProgramSpec {
            id: "ex252_str_len_func",
            intent: "Function that takes a STRING parameter and returns its byte length",
            assembly_template: concat!(
                "FUNC 1 5\n",
                "PARAM STRING\n",
                "REF 0\n",
                "STR_LEN\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "STR_CONST \"hello\"\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex253: string concat function
        ProgramSpec {
            id: "ex253_str_concat_func",
            intent: "Function that takes two STRING parameters and concatenates them",
            assembly_template: concat!(
                "FUNC 2 7\n",
                "PARAM STRING\n",
                "PARAM STRING\n",
                "REF 1\n",
                "REF 0\n",
                "STR_CONCAT\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "STR_CONST \"hello\"\n",
                "STR_CONST \" world\"\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },

        // ex254: check string length equals a bound
        ProgramSpec {
            id: "ex254_str_len_eq",
            intent: "Check that the length of \"abc\" equals 3",
            assembly_template: concat!(
                "STR_CONST \"abc\"\n",
                "STR_LEN\n",
                "CONST U64 0x0000 0x0003\n",
                "EQ\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::StringOps,
        },
    ]
}
