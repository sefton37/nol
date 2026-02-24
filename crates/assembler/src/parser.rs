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
///
/// `string_pool` is mutated when a `STR_CONST "..."` literal is encountered:
/// the string is appended (deduped by first-appearance order) and the index
/// is embedded in `arg1`.
pub(crate) fn parse_line(
    tokens: &[Token],
    line_num: usize,
    string_pool: &mut Vec<String>,
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
        Token::StringLit(_) => {
            return Err(AsmError::UnexpectedToken {
                line: line_num,
                token: "\"...\"".to_string(),
            })
        }
    };

    let opcode = lookup_opcode(mnemonic).ok_or_else(|| AsmError::UnknownOpcode {
        line: line_num,
        token: mnemonic.to_string(),
    })?;

    let args = &tokens[1..];

    match opcode {
        // Pattern A: No arguments (original 29 + 17 new I/O opcodes)
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
        | Opcode::Implies
        | Opcode::ArrayGet
        | Opcode::ArrayLen
        | Opcode::Assert
        | Opcode::Ret
        | Opcode::EndFunc
        | Opcode::Exhaust
        | Opcode::Nop
        | Opcode::Halt
        | Opcode::FileRead
        | Opcode::FileWrite
        | Opcode::FileAppend
        | Opcode::FileExists
        | Opcode::FileDelete
        | Opcode::DirList
        | Opcode::DirMake
        | Opcode::PathJoin
        | Opcode::PathParent
        | Opcode::StrLen
        | Opcode::StrConcat
        | Opcode::StrSlice
        | Opcode::StrSplit
        | Opcode::StrBytes
        | Opcode::BytesStr
        | Opcode::ExecSpawn
        | Opcode::ExecCheck => {
            expect_end(args, line_num)?;
            Ok(Some(ParseResult::Single(Instruction::new(
                opcode,
                TypeTag::None,
                0,
                0,
                0,
            ))))
        }

        // Pattern B/D: Single decimal arg → arg1 (8 opcodes)
        Opcode::Ref
        | Opcode::Match
        | Opcode::Call
        | Opcode::Recurse
        | Opcode::Project
        | Opcode::Pre
        | Opcode::Post
        | Opcode::Forall => {
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
            let data_instr =
                Instruction::new(Opcode::Nop, TypeTag::None, mid_high, mid_low, low16);

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

        // Pattern L: STR_CONST — string literal or numeric pool index
        Opcode::StrConst => {
            match args.first() {
                Some(Token::StringLit(s)) => {
                    // Dedup: use existing index if string is already in pool
                    let idx = if let Some(pos) = string_pool.iter().position(|x| x == s) {
                        pos
                    } else {
                        let pos = string_pool.len();
                        string_pool.push(s.clone());
                        pos
                    };
                    if idx > u16::MAX as usize {
                        return Err(AsmError::InvalidNumber {
                            line: line_num,
                            token: format!("{idx}"),
                        });
                    }
                    expect_end(&args[1..], line_num)?;
                    Ok(Some(ParseResult::Single(Instruction::new(
                        opcode,
                        TypeTag::None,
                        idx as u16,
                        0,
                        0,
                    ))))
                }
                Some(Token::Number(_)) => {
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
                Some(Token::Ident(s)) => Err(AsmError::UnexpectedToken {
                    line: line_num,
                    token: s.clone(),
                }),
                None => Err(AsmError::MissingArgument {
                    line: line_num,
                    opcode: opcode.mnemonic(),
                    expected: 1,
                }),
            }
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
        Some(Token::StringLit(_)) => Err(AsmError::UnexpectedToken {
            line,
            token: "\"...\"".to_string(),
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
        Some(Token::StringLit(_)) => Err(AsmError::UnexpectedToken {
            line,
            token: "\"...\"".to_string(),
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
        let token_str = match tok {
            Token::Ident(s) => s.clone(),
            Token::Number(n) => n.to_string(),
            Token::StringLit(_) => "\"...\"".to_string(),
        };
        return Err(AsmError::UnexpectedToken {
            line,
            token: token_str,
        });
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

    fn strlit(s: &str) -> Token {
        Token::StringLit(s.to_string())
    }

    fn parse(tokens: &[Token]) -> Result<Option<ParseResult>, AsmError> {
        let mut pool = Vec::new();
        parse_line(tokens, 1, &mut pool)
    }

    fn parse_with_pool(
        tokens: &[Token],
        pool: &mut Vec<String>,
    ) -> Result<Option<ParseResult>, AsmError> {
        parse_line(tokens, 1, pool)
    }

    #[test]
    fn parse_empty_tokens() {
        assert!(parse(&[]).unwrap().is_none());
    }

    #[test]
    fn parse_pattern_a_add() {
        let result = parse(&[ident("ADD")]).unwrap().unwrap();
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
        let err = parse(&[ident("ADD"), num(5)]).unwrap_err();
        assert!(matches!(err, AsmError::UnexpectedToken { .. }));
    }

    #[test]
    fn parse_pattern_b_ref() {
        let result = parse(&[ident("REF"), num(3)]).unwrap().unwrap();
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
        let err = parse(&[ident("REF")]).unwrap_err();
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
        let result = parse(&[ident("FUNC"), num(1), num(8)]).unwrap().unwrap();
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
        let result = parse(&[ident("PARAM"), ident("I64")]).unwrap().unwrap();
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
        let result = parse(&[ident("TYPEOF"), ident("I64")]).unwrap().unwrap();
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
        let result = parse(&[ident("CONST"), ident("I64"), num(0), num(42)])
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
        let result = parse(&[ident("CONST_EXT"), ident("I64"), num(0x0000123456789abc)])
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
        let result = parse(&[ident("HASH"), num(0xa3f2), num(0x1b4c), num(0x7d9e)])
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
        let result =
            parse(&[ident("VARIANT_NEW"), ident("VARIANT"), num(2), num(0)])
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
        let result = parse(&[ident("TUPLE_NEW"), ident("TUPLE"), num(2)])
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
        let err = parse(&[ident("FOOBAR")]).unwrap_err();
        assert_eq!(
            err,
            AsmError::UnknownOpcode {
                line: 1,
                token: "FOOBAR".to_string()
            }
        );
    }

    #[test]
    fn unknown_type_tag() {
        let err = parse(&[ident("PARAM"), ident("STRING_TAG_UNKNOWN")]).unwrap_err();
        assert!(matches!(err, AsmError::UnknownTypeTag { .. }));
    }

    #[test]
    fn number_too_large_for_u16() {
        let err = parse(&[ident("REF"), num(70000)]).unwrap_err();
        assert_eq!(
            err,
            AsmError::InvalidNumber {
                line: 1,
                token: "70000".to_string()
            }
        );
    }

    #[test]
    fn number_as_first_token() {
        let err = parse(&[num(42)]).unwrap_err();
        assert!(matches!(err, AsmError::UnexpectedToken { .. }));
    }

    // I/O opcode tests

    #[test]
    fn parse_file_read() {
        let result = parse(&[ident("FILE_READ")]).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => assert_eq!(i.opcode, Opcode::FileRead),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_file_write() {
        let result = parse(&[ident("FILE_WRITE")]).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => assert_eq!(i.opcode, Opcode::FileWrite),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_file_append() {
        let result = parse(&[ident("FILE_APPEND")]).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => assert_eq!(i.opcode, Opcode::FileAppend),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_file_exists() {
        let result = parse(&[ident("FILE_EXISTS")]).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => assert_eq!(i.opcode, Opcode::FileExists),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_dir_list() {
        let result = parse(&[ident("DIR_LIST")]).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => assert_eq!(i.opcode, Opcode::DirList),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_str_len() {
        let result = parse(&[ident("STR_LEN")]).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => assert_eq!(i.opcode, Opcode::StrLen),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_str_concat() {
        let result = parse(&[ident("STR_CONCAT")]).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => assert_eq!(i.opcode, Opcode::StrConcat),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_exec_spawn() {
        let result = parse(&[ident("EXEC_SPAWN")]).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => assert_eq!(i.opcode, Opcode::ExecSpawn),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_exec_check() {
        let result = parse(&[ident("EXEC_CHECK")]).unwrap().unwrap();
        match result {
            ParseResult::Single(i) => assert_eq!(i.opcode, Opcode::ExecCheck),
            _ => panic!("expected Single"),
        }
    }

    #[test]
    fn parse_io_opcode_rejects_extra_args() {
        let err = parse(&[ident("FILE_READ"), num(0)]).unwrap_err();
        assert!(matches!(err, AsmError::UnexpectedToken { .. }));
    }

    // STR_CONST tests

    #[test]
    fn parse_str_const_with_string_literal() {
        let mut pool = Vec::new();
        let result = parse_with_pool(&[ident("STR_CONST"), strlit("hello")], &mut pool)
            .unwrap()
            .unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::StrConst);
                assert_eq!(i.type_tag, TypeTag::None);
                assert_eq!(i.arg1, 0);
                assert_eq!(i.arg2, 0);
                assert_eq!(i.arg3, 0);
            }
            _ => panic!("expected Single"),
        }
        assert_eq!(pool, vec!["hello".to_string()]);
    }

    #[test]
    fn parse_str_const_with_numeric_index() {
        let mut pool = Vec::new();
        let result = parse_with_pool(&[ident("STR_CONST"), num(5)], &mut pool)
            .unwrap()
            .unwrap();
        match result {
            ParseResult::Single(i) => {
                assert_eq!(i.opcode, Opcode::StrConst);
                assert_eq!(i.arg1, 5);
            }
            _ => panic!("expected Single"),
        }
        // Pool is not modified when a numeric index is given
        assert!(pool.is_empty());
    }

    #[test]
    fn parse_str_const_deduplication() {
        let mut pool = Vec::new();
        // Add "hello" twice — should get same index both times
        let r1 = parse_with_pool(&[ident("STR_CONST"), strlit("hello")], &mut pool)
            .unwrap()
            .unwrap();
        let r2 = parse_with_pool(&[ident("STR_CONST"), strlit("hello")], &mut pool)
            .unwrap()
            .unwrap();
        match (r1, r2) {
            (ParseResult::Single(i1), ParseResult::Single(i2)) => {
                assert_eq!(i1.arg1, 0);
                assert_eq!(i2.arg1, 0);
            }
            _ => panic!("expected Singles"),
        }
        assert_eq!(pool.len(), 1);
        assert_eq!(pool[0], "hello");
    }

    #[test]
    fn parse_str_const_multiple_strings() {
        let mut pool = Vec::new();
        let r1 = parse_with_pool(&[ident("STR_CONST"), strlit("alpha")], &mut pool)
            .unwrap()
            .unwrap();
        let r2 = parse_with_pool(&[ident("STR_CONST"), strlit("beta")], &mut pool)
            .unwrap()
            .unwrap();
        match (r1, r2) {
            (ParseResult::Single(i1), ParseResult::Single(i2)) => {
                assert_eq!(i1.arg1, 0);
                assert_eq!(i2.arg1, 1);
            }
            _ => panic!("expected Singles"),
        }
        assert_eq!(pool, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn parse_str_const_missing_arg() {
        let mut pool = Vec::new();
        let err = parse_with_pool(&[ident("STR_CONST")], &mut pool).unwrap_err();
        assert!(matches!(
            err,
            AsmError::MissingArgument {
                opcode: "STR_CONST",
                ..
            }
        ));
    }

    #[test]
    fn parse_str_const_ident_arg_rejected() {
        let mut pool = Vec::new();
        let err =
            parse_with_pool(&[ident("STR_CONST"), ident("SOMETHING")], &mut pool).unwrap_err();
        assert!(matches!(err, AsmError::UnexpectedToken { .. }));
    }
}
