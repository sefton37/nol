//! Tokenizer for NoLang assembly text.

use crate::error::AsmError;

/// A single token from an assembly line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    /// An identifier (opcode mnemonic, type tag name). Always uppercase.
    Ident(String),
    /// A numeric literal (decimal or hex).
    Number(u64),
    /// A string literal (contents without surrounding quotes, escapes resolved).
    StringLit(String),
}

/// Tokenize a single line of assembly text.
///
/// Returns an empty Vec for blank lines and comment-only lines.
/// Comments start with `;` and extend to end of line.
/// String literals are delimited by `"` and support `\"` and `\\` escapes.
pub(crate) fn tokenize_line(line: &str, line_num: usize) -> Result<Vec<Token>, AsmError> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // Skip whitespace
        if ch.is_ascii_whitespace() {
            i += 1;
            continue;
        }

        // Comment — discard rest of line
        if ch == ';' {
            break;
        }

        // String literal
        if ch == '"' {
            i += 1; // skip opening quote
            let mut s = String::new();
            let mut closed = false;
            while i < len {
                let c = chars[i];
                if c == '\\' && i + 1 < len {
                    let escaped = chars[i + 1];
                    match escaped {
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        'n' => s.push('\n'),
                        't' => s.push('\t'),
                        'r' => s.push('\r'),
                        other => {
                            s.push('\\');
                            s.push(other);
                        }
                    }
                    i += 2;
                } else if c == '"' {
                    closed = true;
                    i += 1;
                    break;
                } else {
                    s.push(c);
                    i += 1;
                }
            }
            if !closed {
                return Err(AsmError::InvalidString { line: line_num });
            }
            tokens.push(Token::StringLit(s));
            continue;
        }

        // Collect a word (runs until whitespace, `;`, or `"`)
        let start = i;
        while i < len {
            let c = chars[i];
            if c.is_ascii_whitespace() || c == ';' || c == '"' {
                break;
            }
            i += 1;
        }
        let word: String = chars[start..i].iter().collect();

        let tok = if word.starts_with("0x") || word.starts_with("0X") {
            let hex_str = &word[2..];
            let value = u64::from_str_radix(hex_str, 16).map_err(|_| AsmError::InvalidNumber {
                line: line_num,
                token: word.clone(),
            })?;
            Token::Number(value)
        } else if word.as_bytes().first().is_some_and(|b| b.is_ascii_digit()) {
            let value: u64 = word.parse().map_err(|_| AsmError::InvalidNumber {
                line: line_num,
                token: word.clone(),
            })?;
            Token::Number(value)
        } else {
            Token::Ident(word.to_uppercase())
        };
        tokens.push(tok);
    }

    Ok(tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_line() {
        assert_eq!(tokenize_line("", 1).unwrap(), vec![]);
    }

    #[test]
    fn whitespace_only() {
        assert_eq!(tokenize_line("   \t  ", 1).unwrap(), vec![]);
    }

    #[test]
    fn comment_only() {
        assert_eq!(tokenize_line("; this is a comment", 1).unwrap(), vec![]);
    }

    #[test]
    fn simple_opcode() {
        assert_eq!(
            tokenize_line("ADD", 1).unwrap(),
            vec![Token::Ident("ADD".to_string())]
        );
    }

    #[test]
    fn opcode_with_comment() {
        assert_eq!(
            tokenize_line("ADD ; add two values", 1).unwrap(),
            vec![Token::Ident("ADD".to_string())]
        );
    }

    #[test]
    fn opcode_with_decimal_arg() {
        assert_eq!(
            tokenize_line("REF 0", 1).unwrap(),
            vec![Token::Ident("REF".to_string()), Token::Number(0)]
        );
    }

    #[test]
    fn opcode_with_hex_args() {
        assert_eq!(
            tokenize_line("CONST I64 0x0000 0x002a", 1).unwrap(),
            vec![
                Token::Ident("CONST".to_string()),
                Token::Ident("I64".to_string()),
                Token::Number(0),
                Token::Number(42),
            ]
        );
    }

    #[test]
    fn leading_whitespace() {
        assert_eq!(
            tokenize_line("  BIND", 1).unwrap(),
            vec![Token::Ident("BIND".to_string())]
        );
    }

    #[test]
    fn lowercase_opcode_uppercased() {
        assert_eq!(
            tokenize_line("add", 1).unwrap(),
            vec![Token::Ident("ADD".to_string())]
        );
    }

    #[test]
    fn hex_number_uppercase_prefix() {
        assert_eq!(
            tokenize_line("HASH 0Xabcd 0X1234 0X5678", 1).unwrap(),
            vec![
                Token::Ident("HASH".to_string()),
                Token::Number(0xabcd),
                Token::Number(0x1234),
                Token::Number(0x5678),
            ]
        );
    }

    #[test]
    fn invalid_hex_number() {
        let err = tokenize_line("CONST I64 0xZZZZ 0x0000", 3).unwrap_err();
        assert_eq!(
            err,
            AsmError::InvalidNumber {
                line: 3,
                token: "0xZZZZ".to_string()
            }
        );
    }

    #[test]
    fn invalid_decimal_number() {
        let err = tokenize_line("REF 99999999999999999999999", 5).unwrap_err();
        assert_eq!(
            err,
            AsmError::InvalidNumber {
                line: 5,
                token: "99999999999999999999999".to_string()
            }
        );
    }

    #[test]
    fn large_hex_value() {
        assert_eq!(
            tokenize_line("CONST_EXT I64 0x0000123456789abc", 1).unwrap(),
            vec![
                Token::Ident("CONST_EXT".to_string()),
                Token::Ident("I64".to_string()),
                Token::Number(0x0000123456789abc),
            ]
        );
    }

    #[test]
    fn string_literal_simple() {
        assert_eq!(
            tokenize_line(r#"STR_CONST "hello""#, 1).unwrap(),
            vec![
                Token::Ident("STR_CONST".to_string()),
                Token::StringLit("hello".to_string()),
            ]
        );
    }

    #[test]
    fn string_literal_with_escaped_quote() {
        // Input: STR_CONST "say \"hi\""
        let line = "STR_CONST \"say \\\"hi\\\"\"";
        assert_eq!(
            tokenize_line(line, 1).unwrap(),
            vec![
                Token::Ident("STR_CONST".to_string()),
                Token::StringLit("say \"hi\"".to_string()),
            ]
        );
    }

    #[test]
    fn string_literal_with_escaped_backslash() {
        // Input: STR_CONST "a\\b"
        let line = "STR_CONST \"a\\\\b\"";
        assert_eq!(
            tokenize_line(line, 1).unwrap(),
            vec![
                Token::Ident("STR_CONST".to_string()),
                Token::StringLit("a\\b".to_string()),
            ]
        );
    }

    #[test]
    fn string_literal_empty() {
        assert_eq!(
            tokenize_line("STR_CONST \"\"", 1).unwrap(),
            vec![
                Token::Ident("STR_CONST".to_string()),
                Token::StringLit(String::new()),
            ]
        );
    }

    #[test]
    fn unterminated_string_literal() {
        let err = tokenize_line("STR_CONST \"hello", 7).unwrap_err();
        assert_eq!(err, AsmError::InvalidString { line: 7 });
    }

    #[test]
    fn string_literal_with_comment_after() {
        assert_eq!(
            tokenize_line("STR_CONST \"hi\" ; comment", 1).unwrap(),
            vec![
                Token::Ident("STR_CONST".to_string()),
                Token::StringLit("hi".to_string()),
            ]
        );
    }
}
