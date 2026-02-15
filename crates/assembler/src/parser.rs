//! Parser for NoLang assembly tokens → instructions.
//!
//! Dispatches on the opcode to the correct argument pattern (A through K).

use crate::error::AsmError;
use crate::lexer::Token;
use nolang_common::opcode::ALL_OPCODES;
use nolang_common::type_tag::ALL_TYPE_TAGS;
use nolang_common::{Instruction, Opcode, TypeTag};

/// Result of parsing a single assembly line.
#[derive(Debug)]
pub(crate) enum ParseResult {
    /// A single instruction.
    Single(Instruction),
    /// Two instructions (CONST_EXT + data slot).
    Double(Instruction, Instruction),
}

fn lookup_opcode(mnemonic: &str) -> Option<Opcode> {
    ALL_OPCODES
        .iter()
        .find(|op| op.mnemonic() == mnemonic)
        .copied()
}

fn lookup_type_tag(name: &str) -> Option<TypeTag> {
    ALL_TYPE_TAGS.iter().find(|tt| tt.name() == name).copied()
}

/// Parse a sequence of tokens from a single line into an instruction (or two).
///
/// Returns `Ok(None)` for blank lines (empty token list).
pub(crate) fn parse_line(
    tokens: &[Token],
    line_num: usize,
) -> Result<Option<ParseResult>, AsmError> {
    if tokens.is_empty() {
        return Ok(None);
    }

    let mnemonic = match &tokens[0] {
        Token::Ident(s) => s.as_str(),
        Token::Number(n) => {
            return Err(AsmError::UnexpectedToken {
                line: line_num,
                token: n.to_string(),
            })
        }
    };

    let opcode = lookup_opcode(mnemonic).ok_or_else(|| AsmError::UnknownOpcode {
        line: line_num,
        token: mnemonic.to_string(),
    })?;

    let args = &tokens[1..];

    match opcode {
        // Pattern A: No arguments (28 opcodes)
        Opcode::Bind
        | Opcode::Drop
        | Opcode::Neg
        | Opcode::Add
        | Opcode::Sub
        | Opcode::Mul
        | Opcode::Div
        | Opcode::Mod
        | Opcode::Eq
        | Opcode::Neq
        | Opcode::Lt
        | Opcode::Gt
        | Opcode::Lte
        | Opcode::Gte
        | Opcode::And
        | Opcode::Or
        | Opcode::Not
        | Opcode::Xor
        | Opcode::Shl
        | Opcode::Shr
        | Opcode::ArrayGet
        | Opcode::ArrayLen
        | Opcode::Assert
        | Opcode::Ret
        | Opcode::EndFunc
        | Opcode::Exhaust
        | Opcode::Nop
        | Opcode::Halt => {
            expect_end(args, line_num)?;
            Ok(Some(ParseResult::Single(Instruction::new(
                opcode,
                TypeTag::None,
                0,
                0,
                0,
            ))))
        }

        // Pattern B/D: Single decimal arg → arg1 (7 opcodes)
        Opcode::Ref
        | Opcode::Match
        | Opcode::Call
        | Opcode::Recurse
        | Opcode::Project
        | Opcode::Pre
        | Opcode::Post => {
            let arg1 = expect_u16(args, 0, line_num, opcode.mnemonic(), 1)?;
            expect_end(&args[1..], line_num)?;
            Ok(Some(ParseResult::Single(Instruction::new(
                opcode,
                TypeTag::None,
                arg1,
                0,
                0,
            ))))
        }

        // Pattern C: Two decimal args → arg1, arg2 (2 opcodes)
        Opcode::Func | Opcode::Case => {
            let arg1 = expect_u16(args, 0, line_num, opcode.mnemonic(), 2)?;
            let arg2 = expect_u16(args, 1, line_num, opcode.mnemonic(), 2)?;
            expect_end(&args[2..], line_num)?;
            Ok(Some(ParseResult::Single(Instruction::new(
                opcode,
                TypeTag::None,
                arg1,
                arg2,
                0,
            ))))
        }

        // Pattern E: Type name → type_tag (1 opcode)
        Opcode::Param => {
            let tt = expect_type_tag(args, 0, line_num, opcode.mnemonic(), 1)?;
            expect_end(&args[1..], line_num)?;
            Ok(Some(ParseResult::Single(Instruction::new(
                opcode, tt, 0, 0, 0,
            ))))
        }

        // Pattern F: Type name → arg1 as numeric (1 opcode)
        Opcode::Typeof => {
            let tt = expect_type_tag(args, 0, line_num, opcode.mnemonic(), 1)?;
            expect_end(&args[1..], line_num)?;
            Ok(Some(ParseResult::Single(Instruction::new(
                opcode,
                TypeTag::None,
                tt as u16,
                0,
                0,
            ))))
        }

        // Pattern G: Type + two hex/decimal args (1 opcode)
        Opcode::Const => {
            let tt = expect_type_tag(args, 0, line_num, opcode.mnemonic(), 3)?;
            let arg1 = expect_u16(args, 1, line_num, opcode.mnemonic(), 3)?;
            let arg2 = expect_u16(args, 2, line_num, opcode.mnemonic(), 3)?;
            expect_end(&args[3..], line_num)?;
            Ok(Some(ParseResult::Single(Instruction::new(
                opcode, tt, arg1, arg2, 0,
            ))))
        }

        // Pattern H: Type + 64-bit hex value → two instruction slots (1 opcode)
        Opcode::ConstExt => {
            let tt = expect_type_tag(args, 0, line_num, opcode.mnemonic(), 2)?;
            let full_value = expect_number(args, 1, line_num, opcode.mnemonic(), 2)?;
            expect_end(&args[2..], line_num)?;

            let high16 = ((full_value >> 48) & 0xFFFF) as u16;
            let mid_high = ((full_value >> 32) & 0xFFFF) as u16;
            let mid_low = ((full_value >> 16) & 0xFFFF) as u16;
            let low16 = (full_value & 0xFFFF) as u16;

            let ext_instr = Instruction::new(Opcode::ConstExt, tt, high16, 0, 0);
            let data_instr = Instruction::new(Opcode::Nop, TypeTag::None, mid_high, mid_low, low16);

            Ok(Some(ParseResult::Double(ext_instr, data_instr)))
        }

        // Pattern I: Three hex/decimal args (1 opcode)
        Opcode::Hash => {
            let arg1 = expect_u16(args, 0, line_num, opcode.mnemonic(), 3)?;
            let arg2 = expect_u16(args, 1, line_num, opcode.mnemonic(), 3)?;
            let arg3 = expect_u16(args, 2, line_num, opcode.mnemonic(), 3)?;
            expect_end(&args[3..], line_num)?;
            Ok(Some(ParseResult::Single(Instruction::new(
                opcode,
                TypeTag::None,
                arg1,
                arg2,
                arg3,
            ))))
        }

        // Pattern J: Type + two decimal args (1 opcode)
        Opcode::VariantNew => {
            let tt = expect_type_tag(args, 0, line_num, opcode.mnemonic(), 3)?;
            let arg1 = expect_u16(args, 1, line_num, opcode.mnemonic(), 3)?;
            let arg2 = expect_u16(args, 2, line_num, opcode.mnemonic(), 3)?;
            expect_end(&args[3..], line_num)?;
            Ok(Some(ParseResult::Single(Instruction::new(
                opcode, tt, arg1, arg2, 0,
            ))))
        }

        // Pattern K: Type + one decimal arg (2 opcodes)
        Opcode::TupleNew | Opcode::ArrayNew => {
            let tt = expect_type_tag(args, 0, line_num, opcode.mnemonic(), 2)?;
            let arg1 = expect_u16(args, 1, line_num, opcode.mnemonic(), 2)?;
            expect_end(&args[2..], line_num)?;
            Ok(Some(ParseResult::Single(Instruction::new(
                opcode, tt, arg1, 0, 0,
            ))))
        }
    }
}

/// Extract a u64 number from the token at position `idx`.
fn expect_number(
    args: &[Token],
    idx: usize,
    line: usize,
    opcode: &'static str,
    expected: usize,
) -> Result<u64, AsmError> {
    match args.get(idx) {
        Some(Token::Number(n)) => Ok(*n),
        Some(Token::Ident(s)) => Err(AsmError::UnexpectedToken {
            line,
            token: s.clone(),
        }),
        None => Err(AsmError::MissingArgument {
            line,
            opcode,
            expected,
        }),
    }
}

/// Extract a u16 number from the token at position `idx`, validating range.
fn expect_u16(
    args: &[Token],
    idx: usize,
    line: usize,
    opcode: &'static str,
    expected: usize,
) -> Result<u16, AsmError> {
    let n = expect_number(args, idx, line, opcode, expected)?;
    if n > u16::MAX as u64 {
        return Err(AsmError::InvalidNumber {
            line,
            token: format!("{n}"),
        });
    }
    Ok(n as u16)
}

/// Extract a type tag name from the token at position `idx`.
fn expect_type_tag(
    args: &[Token],
    idx: usize,
    line: usize,
    opcode: &'static str,
    expected: usize,
) -> Result<TypeTag, AsmError> {
    match args.get(idx) {
        Some(Token::Ident(s)) => lookup_type_tag(s).ok_or_else(|| AsmError::UnknownTypeTag {
            line,
            token: s.clone(),
        }),
        Some(Token::Number(n)) => Err(AsmError::UnexpectedToken {
            line,
            token: n.to_string(),
        }),
        None => Err(AsmError::MissingArgument {
            line,
            opcode,
            expected,
        }),
    }
}

/// Check that there are no extra tokens.
fn expect_end(remaining: &[Token], line: usize) -> Result<(), AsmError> {
    if let Some(tok) = remaining.first() {
        let token = match tok {
            Token::Ident(s) => s.clone(),
            Token::Number(n) => n.to_string(),
        };
        return Err(AsmError::UnexpectedToken { line, token });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident(s: &str) -> Token {
        Token::Ident(s.to_string())
    }

    fn num(n: u64) -> Token {
        Token::Number(n)
    }

    #[test]
    fn parse_empty_tokens() {
        assert!(parse_line(&[], 1).unwrap().is_none());
    }

    #[test]
    fn parse_pattern_a_add() {
        let result = parse_line(&[ident("ADD")], 1).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::Add);
                assert_eq!(i.type_tag, TypeTag::None);
                assert_eq!(i.arg1, 0);
            }
            ParseResult::Double(_, _) => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_pattern_a_rejects_extra_args() {
        let err = parse_line(&[ident("ADD"), num(5)], 1).unwrap_err();
        assert!(matches!(err, AsmError::UnexpectedToken { .. }));
    }

    #[test]
    fn parse_pattern_b_ref() {
        let result = parse_line(&[ident("REF"), num(3)], 1).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::Ref);
                assert_eq!(i.arg1, 3);
            }
            ParseResult::Double(_, _) => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_pattern_b_missing_arg() {
        let err = parse_line(&[ident("REF")], 1).unwrap_err();
        assert!(matches!(
            err,
            AsmError::MissingArgument {
                opcode: "REF",
                expected: 1,
                ..
            }
        ));
    }

    #[test]
    fn parse_pattern_c_func() {
        let result = parse_line(&[ident("FUNC"), num(1), num(8)], 1)
            .unwrap()
            .unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::Func);
                assert_eq!(i.arg1, 1);
                assert_eq!(i.arg2, 8);
            }
            ParseResult::Double(_, _) => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_pattern_e_param() {
        let result = parse_line(&[ident("PARAM"), ident("I64")], 1)
            .unwrap()
            .unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::Param);
                assert_eq!(i.type_tag, TypeTag::I64);
            }
            ParseResult::Double(_, _) => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_pattern_f_typeof() {
        let result = parse_line(&[ident("TYPEOF"), ident("I64")], 1)
            .unwrap()
            .unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::Typeof);
                assert_eq!(i.type_tag, TypeTag::None);
                assert_eq!(i.arg1, TypeTag::I64 as u16);
            }
            ParseResult::Double(_, _) => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_pattern_g_const() {
        let result = parse_line(&[ident("CONST"), ident("I64"), num(0), num(42)], 1)
            .unwrap()
            .unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::Const);
                assert_eq!(i.type_tag, TypeTag::I64);
                assert_eq!(i.arg1, 0);
                assert_eq!(i.arg2, 42);
            }
            ParseResult::Double(_, _) => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_pattern_h_const_ext() {
        let result = parse_line(
            &[ident("CONST_EXT"), ident("I64"), num(0x0000123456789abc)],
            1,
        )
        .unwrap()
        .unwrap();
        match result {
            ParseResult::Double(ext, data) => {
                assert_eq!(ext.opcode, Opcode::ConstExt);
                assert_eq!(ext.type_tag, TypeTag::I64);
                assert_eq!(ext.arg1, 0x0000); // high 16 bits
                assert_eq!(data.opcode, Opcode::Nop);
                assert_eq!(data.arg1, 0x1234); // bits 32-47
                assert_eq!(data.arg2, 0x5678); // bits 16-31
                assert_eq!(data.arg3, 0x9abc); // bits 0-15
            }
            ParseResult::Single(_) => panic!("expected Double"),
        }
    }

    #[test]
    fn parse_pattern_i_hash() {
        let result = parse_line(&[ident("HASH"), num(0xa3f2), num(0x1b4c), num(0x7d9e)], 1)
            .unwrap()
            .unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::Hash);
                assert_eq!(i.arg1, 0xa3f2);
                assert_eq!(i.arg2, 0x1b4c);
                assert_eq!(i.arg3, 0x7d9e);
            }
            ParseResult::Double(_, _) => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_pattern_j_variant_new() {
        let result = parse_line(&[ident("VARIANT_NEW"), ident("VARIANT"), num(2), num(0)], 1)
            .unwrap()
            .unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::VariantNew);
                assert_eq!(i.type_tag, TypeTag::Variant);
                assert_eq!(i.arg1, 2);
                assert_eq!(i.arg2, 0);
            }
            ParseResult::Double(_, _) => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_pattern_k_tuple_new() {
        let result = parse_line(&[ident("TUPLE_NEW"), ident("TUPLE"), num(2)], 1)
            .unwrap()
            .unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::TupleNew);
                assert_eq!(i.type_tag, TypeTag::Tuple);
                assert_eq!(i.arg1, 2);
            }
            ParseResult::Double(_, _) => panic!("expected Single"),
        }
    }

    #[test]
    fn unknown_opcode() {
        let err = parse_line(&[ident("FOOBAR")], 3).unwrap_err();
        assert_eq!(
            err,
            AsmError::UnknownOpcode {
                line: 3,
                token: "FOOBAR".to_string()
            }
        );
    }

    #[test]
    fn unknown_type_tag() {
        let err = parse_line(&[ident("PARAM"), ident("STRING")], 5).unwrap_err();
        assert_eq!(
            err,
            AsmError::UnknownTypeTag {
                line: 5,
                token: "STRING".to_string()
            }
        );
    }

    #[test]
    fn number_too_large_for_u16() {
        let err = parse_line(&[ident("REF"), num(70000)], 2).unwrap_err();
        assert_eq!(
            err,
            AsmError::InvalidNumber {
                line: 2,
                token: "70000".to_string()
            }
        );
    }

    #[test]
    fn number_as_first_token() {
        let err = parse_line(&[num(42)], 1).unwrap_err();
        assert!(matches!(err, AsmError::UnexpectedToken { .. }));
    }
}
