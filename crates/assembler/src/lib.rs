//! NoLang assembler — bidirectional text ↔ binary translation.
//!
//! The assembler is a mechanical 1:1 translation. No optimization, no sugar.
//!
//! # Usage
//!
//! ```
//! use nolang_assembler::{assemble, disassemble};
//!
//! let text = "CONST I64 0x0000 0x002a\nHALT\n";
//! let program = assemble(text).unwrap();
//! let roundtripped = disassemble(&program);
//! assert_eq!(roundtripped, text);
//! ```
//!
//! # Roundtrip Guarantee
//!
//! `assemble(disassemble(program)) == program` holds for all valid programs.
//! The disassembler outputs canonical text; the assembler accepts both
//! canonical and non-canonical input (e.g., decimal where hex is canonical).

pub mod error;

mod disassembler;
mod lexer;
mod parser;

pub use error::AsmError;

use lexer::tokenize_line;
use nolang_common::Program;
use parser::{parse_line, ParseResult};

/// Assemble text into a binary program.
///
/// Returns the first error encountered. Fix one error at a time.
///
/// String literals in `STR_CONST` instructions are collected into the program's
/// string pool in order of first appearance (deduped).
pub fn assemble(text: &str) -> Result<Program, AsmError> {
    let mut instructions = Vec::new();
    let mut string_pool: Vec<String> = Vec::new();

    for (idx, line) in text.lines().enumerate() {
        let line_num = idx + 1;
        let tokens = tokenize_line(line, line_num)?;
        if let Some(result) = parse_line(&tokens, line_num, &mut string_pool)? {
            match result {
                ParseResult::Single(instr) => instructions.push(instr),
                ParseResult::Double(a, b) => {
                    instructions.push(a);
                    instructions.push(b);
                }
            }
        }
    }

    if string_pool.is_empty() {
        Ok(Program::new(instructions))
    } else {
        Ok(Program::with_string_pool(instructions, string_pool))
    }
}

/// Disassemble a binary program into canonical assembly text.
///
/// The output is flat text: one instruction per line, no indentation,
/// no comments. CONST_EXT produces one text line from two instruction slots.
pub fn disassemble(program: &Program) -> String {
    disassembler::disassemble(program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nolang_common::{Instruction, Opcode, TypeTag};

    #[test]
    fn assemble_minimal() {
        let program = assemble("CONST I64 0x0000 0x002a\nHALT\n").unwrap();
        assert_eq!(program.instructions.len(), 2);
        assert_eq!(program.instructions[0].opcode, Opcode::Const);
        assert_eq!(program.instructions[0].type_tag, TypeTag::I64);
        assert_eq!(program.instructions[0].arg1, 0);
        assert_eq!(program.instructions[0].arg2, 42);
        assert_eq!(program.instructions[1].opcode, Opcode::Halt);
    }

    #[test]
    fn disassemble_minimal() {
        let program = Program::new(vec![
            Instruction::new(Opcode::Const, TypeTag::I64, 0, 42, 0),
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        assert_eq!(disassemble(&program), "CONST I64 0x0000 0x002a\nHALT\n");
    }

    #[test]
    fn roundtrip_disassemble_then_assemble() {
        let original = Program::new(vec![
            Instruction::new(Opcode::Const, TypeTag::I64, 0, 5, 0),
            Instruction::new(Opcode::Const, TypeTag::I64, 0, 3, 0),
            Instruction::new(Opcode::Add, TypeTag::None, 0, 0, 0),
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        let text = disassemble(&original);
        let reassembled = assemble(&text).unwrap();
        assert_eq!(original, reassembled);
    }

    #[test]
    fn roundtrip_assemble_then_disassemble_then_assemble() {
        let text = "CONST I64 0 42\nHALT\n";
        let first = assemble(text).unwrap();
        let canonical = disassemble(&first);
        let second = assemble(&canonical).unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn assemble_with_comments_and_blanks() {
        let text = "\
; This is a comment
CONST I64 0x0000 0x002a  ; push 42

HALT
";
        let program = assemble(text).unwrap();
        assert_eq!(program.instructions.len(), 2);
    }

    #[test]
    fn assemble_with_indentation() {
        let text = "\
FUNC 1 4
  PARAM I64
  REF 0
  RET
  HASH 0x0000 0x0000 0x0000
ENDFUNC
CONST I64 0x0000 0x002a
CALL 0
HALT
";
        let program = assemble(text).unwrap();
        assert_eq!(program.instructions.len(), 9);
    }

    #[test]
    fn assemble_decimal_and_hex_produce_same_result() {
        let hex = assemble("CONST I64 0x0000 0x002a\nHALT\n").unwrap();
        let dec = assemble("CONST I64 0 42\nHALT\n").unwrap();
        assert_eq!(hex, dec);
    }

    #[test]
    fn assemble_const_ext_roundtrip() {
        let text = "CONST_EXT I64 0x0000123456789abc\nHALT\n";
        let program = assemble(text).unwrap();
        assert_eq!(program.instructions.len(), 3); // CONST_EXT + data + HALT
        let roundtripped = disassemble(&program);
        assert_eq!(roundtripped, text);
    }

    #[test]
    fn error_unknown_opcode() {
        let err = assemble("FOOBAR\n").unwrap_err();
        assert!(matches!(err, AsmError::UnknownOpcode { line: 1, .. }));
    }

    #[test]
    fn error_missing_argument() {
        let err = assemble("REF\n").unwrap_err();
        assert!(matches!(err, AsmError::MissingArgument { line: 1, .. }));
    }

    #[test]
    fn error_invalid_number() {
        let err = assemble("REF 0xZZZZ\n").unwrap_err();
        assert!(matches!(err, AsmError::InvalidNumber { line: 1, .. }));
    }

    #[test]
    fn error_reports_correct_line() {
        let text = "HALT\nFOOBAR\n";
        let err = assemble(text).unwrap_err();
        assert!(matches!(err, AsmError::UnknownOpcode { line: 2, .. }));
    }

    #[test]
    fn all_pattern_a_opcodes_roundtrip() {
        let opcodes = [
            "BIND",
            "DROP",
            "NEG",
            "ADD",
            "SUB",
            "MUL",
            "DIV",
            "MOD",
            "EQ",
            "NEQ",
            "LT",
            "GT",
            "LTE",
            "GTE",
            "AND",
            "OR",
            "NOT",
            "XOR",
            "SHL",
            "SHR",
            "ARRAY_GET",
            "ARRAY_LEN",
            "ASSERT",
            "RET",
            "ENDFUNC",
            "EXHAUST",
            "NOP",
            "HALT",
            // New I/O opcodes
            "FILE_READ",
            "FILE_WRITE",
            "FILE_APPEND",
            "FILE_EXISTS",
            "FILE_DELETE",
            "DIR_LIST",
            "DIR_MAKE",
            "PATH_JOIN",
            "PATH_PARENT",
            "STR_LEN",
            "STR_CONCAT",
            "STR_SLICE",
            "STR_SPLIT",
            "STR_BYTES",
            "BYTES_STR",
            "EXEC_SPAWN",
            "EXEC_CHECK",
        ];
        for mnemonic in &opcodes {
            let text = format!("{mnemonic}\n");
            let program = assemble(&text).unwrap();
            let disasm = disassemble(&program);
            assert_eq!(disasm, text, "roundtrip failed for {mnemonic}");
        }
    }

    #[test]
    fn all_type_tags_roundtrip_via_param() {
        let tags = [
            "NONE",
            "I64",
            "U64",
            "F64",
            "BOOL",
            "CHAR",
            "VARIANT",
            "TUPLE",
            "FUNC_TYPE",
            "ARRAY",
            "MAYBE",
            "RESULT",
            "UNIT",
            // New type tags
            "STRING",
            "BYTES",
            "PATH",
            "HANDLE",
        ];
        for tag in &tags {
            let text = format!("PARAM {tag}\n");
            let program = assemble(&text).unwrap();
            let disasm = disassemble(&program);
            assert_eq!(disasm, text, "roundtrip failed for PARAM {tag}");
        }
    }

    // STR_CONST assembler-level tests

    #[test]
    fn assemble_str_const_literal_builds_pool() {
        let text = "STR_CONST \"hello\"\nHALT\n";
        let program = assemble(text).unwrap();
        assert_eq!(program.instructions.len(), 2);
        assert_eq!(program.instructions[0].opcode, Opcode::StrConst);
        assert_eq!(program.instructions[0].arg1, 0);
        assert_eq!(program.string_pool, vec!["hello".to_string()]);
    }

    #[test]
    fn assemble_str_const_dedup() {
        let text = "STR_CONST \"hi\"\nSTR_CONST \"hi\"\nHALT\n";
        let program = assemble(text).unwrap();
        assert_eq!(program.string_pool.len(), 1);
        assert_eq!(program.instructions[0].arg1, 0);
        assert_eq!(program.instructions[1].arg1, 0);
    }

    #[test]
    fn assemble_str_const_multiple_unique() {
        let text = "STR_CONST \"alpha\"\nSTR_CONST \"beta\"\nHALT\n";
        let program = assemble(text).unwrap();
        assert_eq!(program.string_pool.len(), 2);
        assert_eq!(program.instructions[0].arg1, 0);
        assert_eq!(program.instructions[1].arg1, 1);
    }

    #[test]
    fn assemble_str_const_numeric_index_no_pool() {
        // When using a numeric index directly, pool stays empty
        let text = "STR_CONST 0\nHALT\n";
        let program = assemble(text).unwrap();
        assert!(program.string_pool.is_empty());
        assert_eq!(program.instructions[0].arg1, 0);
    }

    #[test]
    fn disassemble_str_const_with_pool() {
        let program = Program::with_string_pool(
            vec![
                Instruction::new(Opcode::StrConst, TypeTag::None, 0, 0, 0),
                Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
            ],
            vec!["hello".to_string()],
        );
        let text = disassemble(&program);
        assert_eq!(text, "STR_CONST \"hello\"\nHALT\n");
    }

    #[test]
    fn disassemble_str_const_without_pool_uses_index() {
        // If no pool, fall back to numeric index
        let program = Program::new(vec![
            Instruction::new(Opcode::StrConst, TypeTag::None, 3, 0, 0),
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        let text = disassemble(&program);
        assert_eq!(text, "STR_CONST 3\nHALT\n");
    }

    #[test]
    fn str_const_roundtrip_with_literal() {
        let original_text = "STR_CONST \"hello world\"\nHALT\n";
        let program = assemble(original_text).unwrap();
        let disassembled = disassemble(&program);
        assert_eq!(disassembled, original_text);
    }

    #[test]
    fn str_const_roundtrip_with_special_chars() {
        // String containing a backslash and quote — canonical form uses escapes
        let original_text = "STR_CONST \"say \\\"hi\\\"\"\nHALT\n";
        let program = assemble(original_text).unwrap();
        assert_eq!(program.string_pool[0], "say \"hi\"");
        let disassembled = disassemble(&program);
        assert_eq!(disassembled, original_text);
    }

    /// Phase 1 gate: assemble → encode binary → decode → disassemble → reassemble → encode = identical bytes.
    #[test]
    fn string_pool_binary_roundtrip() {
        let text = "STR_CONST \"hello\"\nSTR_CONST \"world\"\nSTR_CONCAT\nHALT\n";
        let program1 = assemble(text).unwrap();
        let bytes1 = program1.encode();

        // Decode the binary back
        let decoded = Program::decode(&bytes1).unwrap();
        assert_eq!(decoded.string_pool, vec!["hello", "world"]);

        // Disassemble and reassemble
        let disassembled = disassemble(&decoded);
        let program2 = assemble(&disassembled).unwrap();
        let bytes2 = program2.encode();

        assert_eq!(bytes1, bytes2, "binary roundtrip must be identical");
    }
}
