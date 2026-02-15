//! Minimal recursive descent JSON parser.
//!
//! No external dependencies. Parses standard JSON per RFC 8259.

/// A JSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<JsonValue>),
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// Get as object, returning None if not an object.
    pub fn as_object(&self) -> Option<&[(String, JsonValue)]> {
        match self {
            JsonValue::Object(pairs) => Some(pairs),
            _ => None,
        }
    }

    /// Get a field from an object by key.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.as_object()?
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v)
    }

    /// Get as array.
    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            JsonValue::Array(elems) => Some(elems),
            _ => None,
        }
    }

    /// Get as f64.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            JsonValue::Number(n) => Some(*n),
            _ => None,
        }
    }

    /// Get as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            JsonValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Get as string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            JsonValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Check if null.
    pub fn is_null(&self) -> bool {
        matches!(self, JsonValue::Null)
    }
}

/// A JSON parse error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    pub message: String,
    pub position: usize,
}

impl std::fmt::Display for JsonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "JSON error at position {}: {}",
            self.position, self.message
        )
    }
}

impl std::error::Error for JsonError {}

/// Parse a JSON string into a JsonValue.
pub fn parse(input: &str) -> Result<JsonValue, JsonError> {
    let mut parser = Parser::new(input);
    let value = parser.parse_value()?;
    parser.skip_whitespace();
    if parser.pos < parser.input.len() {
        return Err(JsonError {
            message: "trailing content after root value".to_string(),
            position: parser.pos,
        });
    }
    Ok(value)
}

struct Parser<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Parser {
            input: input.as_bytes(),
            pos: 0,
        }
    }

    fn error(&self, message: impl Into<String>) -> JsonError {
        JsonError {
            message: message.into(),
            position: self.pos,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.input.len() {
            match self.input[self.pos] {
                b' ' | b'\t' | b'\r' | b'\n' => self.pos += 1,
                _ => break,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos])
        } else {
            None
        }
    }

    fn advance(&mut self) -> Option<u8> {
        if self.pos < self.input.len() {
            let ch = self.input[self.pos];
            self.pos += 1;
            Some(ch)
        } else {
            None
        }
    }

    fn expect(&mut self, expected: u8) -> Result<(), JsonError> {
        match self.advance() {
            Some(ch) if ch == expected => Ok(()),
            Some(ch) => Err(self.error(format!(
                "expected '{}', found '{}'",
                expected as char, ch as char
            ))),
            None => Err(self.error(format!("expected '{}', found EOF", expected as char))),
        }
    }

    fn parse_value(&mut self) -> Result<JsonValue, JsonError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'n') => self.parse_literal(b"null", JsonValue::Null),
            Some(b't') => self.parse_literal(b"true", JsonValue::Bool(true)),
            Some(b'f') => self.parse_literal(b"false", JsonValue::Bool(false)),
            Some(b'"') => Ok(JsonValue::String(self.parse_string()?)),
            Some(b'[') => self.parse_array(),
            Some(b'{') => self.parse_object(),
            Some(b'-') | Some(b'0'..=b'9') => self.parse_number(),
            Some(ch) => Err(self.error(format!("unexpected character '{}'", ch as char))),
            None => Err(self.error("unexpected EOF")),
        }
    }

    fn parse_literal(&mut self, expected: &[u8], value: JsonValue) -> Result<JsonValue, JsonError> {
        for &byte in expected {
            self.expect(byte)?;
        }
        Ok(value)
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut result = String::new();

        loop {
            match self.advance() {
                Some(b'"') => return Ok(result),
                Some(b'\\') => {
                    match self.advance() {
                        Some(b'"') => result.push('"'),
                        Some(b'\\') => result.push('\\'),
                        Some(b'/') => result.push('/'),
                        Some(b'b') => result.push('\x08'),
                        Some(b'f') => result.push('\x0C'),
                        Some(b'n') => result.push('\n'),
                        Some(b'r') => result.push('\r'),
                        Some(b't') => result.push('\t'),
                        Some(b'u') => {
                            // Parse \uXXXX unicode escape
                            let hex = self.parse_hex_digits(4)?;
                            if let Some(ch) = char::from_u32(hex) {
                                result.push(ch);
                            } else {
                                return Err(self.error("invalid unicode codepoint"));
                            }
                        }
                        Some(ch) => {
                            return Err(
                                self.error(format!("invalid escape sequence '\\{}'", ch as char))
                            )
                        }
                        None => return Err(self.error("unterminated string")),
                    }
                }
                Some(ch) if ch < 0x20 => {
                    return Err(self.error("unescaped control character in string"))
                }
                Some(ch) => {
                    // Handle UTF-8 sequences
                    if ch < 0x80 {
                        result.push(ch as char);
                    } else {
                        // Decode UTF-8
                        let start_pos = self.pos - 1;
                        let bytes_needed = if ch & 0xE0 == 0xC0 {
                            2
                        } else if ch & 0xF0 == 0xE0 {
                            3
                        } else if ch & 0xF8 == 0xF0 {
                            4
                        } else {
                            return Err(self.error("invalid UTF-8 sequence"));
                        };

                        let end_pos = start_pos + bytes_needed;
                        if end_pos > self.input.len() {
                            return Err(self.error("truncated UTF-8 sequence"));
                        }

                        match std::str::from_utf8(&self.input[start_pos..end_pos]) {
                            Ok(s) => {
                                result.push_str(s);
                                self.pos = end_pos;
                            }
                            Err(_) => return Err(self.error("invalid UTF-8 sequence")),
                        }
                    }
                }
                None => return Err(self.error("unterminated string")),
            }
        }
    }

    fn parse_hex_digits(&mut self, count: usize) -> Result<u32, JsonError> {
        let mut result = 0u32;
        for _ in 0..count {
            let digit = match self.advance() {
                Some(ch @ b'0'..=b'9') => (ch - b'0') as u32,
                Some(ch @ b'a'..=b'f') => (ch - b'a' + 10) as u32,
                Some(ch @ b'A'..=b'F') => (ch - b'A' + 10) as u32,
                Some(ch) => {
                    return Err(self.error(format!("expected hex digit, found '{}'", ch as char)))
                }
                None => return Err(self.error("expected hex digit, found EOF")),
            };
            result = result * 16 + digit;
        }
        Ok(result)
    }

    fn parse_number(&mut self) -> Result<JsonValue, JsonError> {
        let start = self.pos;

        // Optional minus
        if self.peek() == Some(b'-') {
            self.advance();
        }

        // Integer part
        match self.peek() {
            Some(b'0') => {
                self.advance();
                // Leading zero must not be followed by another digit
                if let Some(b'0'..=b'9') = self.peek() {
                    return Err(self.error("invalid number: leading zero"));
                }
            }
            Some(b'1'..=b'9') => {
                self.advance();
                while let Some(b'0'..=b'9') = self.peek() {
                    self.advance();
                }
            }
            _ => return Err(self.error("invalid number")),
        }

        // Optional fractional part
        if self.peek() == Some(b'.') {
            self.advance();
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("invalid number: expected digit after decimal point"));
            }
            while let Some(b'0'..=b'9') = self.peek() {
                self.advance();
            }
        }

        // Optional exponent
        if matches!(self.peek(), Some(b'e') | Some(b'E')) {
            self.advance();
            if matches!(self.peek(), Some(b'+') | Some(b'-')) {
                self.advance();
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("invalid number: expected digit in exponent"));
            }
            while let Some(b'0'..=b'9') = self.peek() {
                self.advance();
            }
        }

        let num_str = std::str::from_utf8(&self.input[start..self.pos])
            .map_err(|_| self.error("invalid UTF-8 in number"))?;
        let num = num_str
            .parse::<f64>()
            .map_err(|_| self.error("invalid number"))?;

        Ok(JsonValue::Number(num))
    }

    fn parse_array(&mut self) -> Result<JsonValue, JsonError> {
        self.expect(b'[')?;
        self.skip_whitespace();

        let mut elements = Vec::new();

        // Empty array?
        if self.peek() == Some(b']') {
            self.advance();
            return Ok(JsonValue::Array(elements));
        }

        loop {
            elements.push(self.parse_value()?);
            self.skip_whitespace();

            match self.peek() {
                Some(b',') => {
                    self.advance();
                    self.skip_whitespace();
                }
                Some(b']') => {
                    self.advance();
                    return Ok(JsonValue::Array(elements));
                }
                Some(ch) => {
                    return Err(self.error(format!("expected ',' or ']', found '{}'", ch as char)))
                }
                None => return Err(self.error("unterminated array")),
            }
        }
    }

    fn parse_object(&mut self) -> Result<JsonValue, JsonError> {
        self.expect(b'{')?;
        self.skip_whitespace();

        let mut pairs = Vec::new();

        // Empty object?
        if self.peek() == Some(b'}') {
            self.advance();
            return Ok(JsonValue::Object(pairs));
        }

        loop {
            // Parse key (must be string)
            if self.peek() != Some(b'"') {
                return Err(self.error("expected string key"));
            }
            let key = self.parse_string()?;
            self.skip_whitespace();

            // Expect colon
            self.expect(b':')?;
            self.skip_whitespace();

            // Parse value
            let value = self.parse_value()?;
            pairs.push((key, value));
            self.skip_whitespace();

            match self.peek() {
                Some(b',') => {
                    self.advance();
                    self.skip_whitespace();
                }
                Some(b'}') => {
                    self.advance();
                    return Ok(JsonValue::Object(pairs));
                }
                Some(ch) => {
                    return Err(self.error(format!("expected ',' or '}}', found '{}'", ch as char)))
                }
                None => return Err(self.error("unterminated object")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_null() {
        let result = parse("null").unwrap();
        assert_eq!(result, JsonValue::Null);
    }

    #[test]
    fn parse_true() {
        let result = parse("true").unwrap();
        assert_eq!(result, JsonValue::Bool(true));
    }

    #[test]
    fn parse_false() {
        let result = parse("false").unwrap();
        assert_eq!(result, JsonValue::Bool(false));
    }

    #[test]
    fn parse_integer() {
        let result = parse("42").unwrap();
        assert_eq!(result, JsonValue::Number(42.0));
    }

    #[test]
    fn parse_negative_integer() {
        let result = parse("-13").unwrap();
        assert_eq!(result, JsonValue::Number(-13.0));
    }

    #[test]
    fn parse_float() {
        let result = parse("3.125").unwrap();
        assert_eq!(result, JsonValue::Number(3.125));
    }

    #[test]
    fn parse_exponent() {
        let result = parse("1e10").unwrap();
        assert_eq!(result, JsonValue::Number(1e10));
    }

    #[test]
    fn parse_negative_exponent() {
        let result = parse("1e-3").unwrap();
        assert_eq!(result, JsonValue::Number(1e-3));
    }

    #[test]
    fn parse_string_simple() {
        let result = parse(r#""hello""#).unwrap();
        assert_eq!(result, JsonValue::String("hello".to_string()));
    }

    #[test]
    fn parse_string_escapes() {
        let result = parse(r#""a\"b\nc""#).unwrap();
        assert_eq!(result, JsonValue::String("a\"b\nc".to_string()));
    }

    #[test]
    fn parse_string_unicode_escape() {
        let result = parse(r#""\u0041""#).unwrap();
        assert_eq!(result, JsonValue::String("A".to_string()));
    }

    #[test]
    fn parse_empty_array() {
        let result = parse("[]").unwrap();
        assert_eq!(result, JsonValue::Array(vec![]));
    }

    #[test]
    fn parse_array_of_numbers() {
        let result = parse("[1, 2, 3]").unwrap();
        assert_eq!(
            result,
            JsonValue::Array(vec![
                JsonValue::Number(1.0),
                JsonValue::Number(2.0),
                JsonValue::Number(3.0),
            ])
        );
    }

    #[test]
    fn parse_empty_object() {
        let result = parse("{}").unwrap();
        assert_eq!(result, JsonValue::Object(vec![]));
    }

    #[test]
    fn parse_object_simple() {
        let result = parse(r#"{"a": 1, "b": 2}"#).unwrap();
        assert_eq!(
            result,
            JsonValue::Object(vec![
                ("a".to_string(), JsonValue::Number(1.0)),
                ("b".to_string(), JsonValue::Number(2.0)),
            ])
        );
    }

    #[test]
    fn parse_nested() {
        let result = parse(r#"{"input": [5], "expected": 5}"#).unwrap();
        match result {
            JsonValue::Object(pairs) => {
                assert_eq!(pairs.len(), 2);
                assert_eq!(pairs[0].0, "input");
                assert_eq!(pairs[0].1, JsonValue::Array(vec![JsonValue::Number(5.0)]));
                assert_eq!(pairs[1].0, "expected");
                assert_eq!(pairs[1].1, JsonValue::Number(5.0));
            }
            _ => panic!("expected object"),
        }
    }

    #[test]
    fn parse_witness_array() {
        let input = r#"[{"input": [5], "expected": 5}, {"input": [-13], "expected": 13}]"#;
        let result = parse(input).unwrap();
        match result {
            JsonValue::Array(items) => {
                assert_eq!(items.len(), 2);
                // First witness
                assert_eq!(
                    items[0].get("input").unwrap().as_array().unwrap()[0]
                        .as_f64()
                        .unwrap(),
                    5.0
                );
                assert_eq!(items[0].get("expected").unwrap().as_f64().unwrap(), 5.0);
                // Second witness
                assert_eq!(
                    items[1].get("input").unwrap().as_array().unwrap()[0]
                        .as_f64()
                        .unwrap(),
                    -13.0
                );
                assert_eq!(items[1].get("expected").unwrap().as_f64().unwrap(), 13.0);
            }
            _ => panic!("expected array"),
        }
    }

    #[test]
    fn parse_whitespace_handling() {
        let result = parse(r#"  { "a" : 1 }  "#).unwrap();
        assert_eq!(
            result,
            JsonValue::Object(vec![("a".to_string(), JsonValue::Number(1.0))])
        );
    }

    #[test]
    fn parse_error_unterminated_string() {
        let result = parse(r#""hello"#);
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("unterminated string"));
    }

    #[test]
    fn parse_error_trailing_content() {
        let result = parse("42 abc");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("trailing content after root value"));
    }

    #[test]
    fn parse_error_empty_input() {
        let result = parse("");
        assert!(result.is_err());
        assert!(result.unwrap_err().message.contains("unexpected EOF"));
    }

    #[test]
    fn parse_zero() {
        let result = parse("0").unwrap();
        assert_eq!(result, JsonValue::Number(0.0));
    }
}
