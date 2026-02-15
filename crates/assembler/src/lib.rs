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
pub fn assemble(text: &str) -> Result<Program, AsmError> {
    let mut instructions = Vec::new();

    for (idx, line) in text.lines().enumerate() {
        let line_num = idx + 1;
        let tokens = tokenize_line(line, line_num)?;
        if let Some(result) = parse_line(&tokens, line_num)? {
            match result {
                ParseResult::Single(instr) => instructions.push(instr),
                ParseResult::Double(a, b) => {
                    instructions.push(a);
                    instructions.push(b);
                }
            }
        }
    }

    Ok(Program::new(instructions))
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
        ];
        for tag in &tags {
            let text = format!("PARAM {tag}\n");
            let program = assemble(&text).unwrap();
            let disasm = disassemble(&program);
            assert_eq!(disasm, text, "roundtrip failed for PARAM {tag}");
        }
    }
}
