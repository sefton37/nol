//! Witness testing — run functions with concrete inputs and compare outputs.

use crate::json::{JsonError, JsonValue};
use nolang_common::{Instruction, Opcode, Program, TypeTag, Value};

/// A single witness test case: inputs to a function, expected output.
#[derive(Debug, Clone, PartialEq)]
pub struct Witness {
    pub inputs: Vec<Value>,
    pub expected: Value,
}

/// Result of running one witness.
#[derive(Debug, Clone)]
pub struct WitnessResult {
    pub index: usize,
    pub passed: bool,
    pub actual: Option<Value>,
    pub expected: Value,
    pub error: Option<String>,
}

/// Errors from witness operations.
#[derive(Debug)]
pub enum WitnessError {
    NoFunctions,
    FunctionNotFound { index: usize, count: usize },
    InputCountMismatch { expected: usize, got: usize },
    InputTypeMismatch { index: usize, expected: TypeTag },
    UnencodableValue(String),
    JsonError(JsonError),
    InvalidFormat(String),
}

impl std::fmt::Display for WitnessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WitnessError::NoFunctions => write!(f, "no functions in program"),
            WitnessError::FunctionNotFound { index, count } => {
                write!(
                    f,
                    "function {index} not found (program has {count} functions)"
                )
            }
            WitnessError::InputCountMismatch { expected, got } => {
                write!(f, "wrong number of inputs: expected {expected}, got {got}")
            }
            WitnessError::InputTypeMismatch { index, expected } => {
                write!(
                    f,
                    "input {index}: expected {expected:?}, got incompatible JSON value"
                )
            }
            WitnessError::UnencodableValue(msg) => {
                write!(f, "cannot encode {msg} as CONST instructions")
            }
            WitnessError::JsonError(e) => write!(f, "JSON parse error: {e}"),
            WitnessError::InvalidFormat(msg) => write!(f, "invalid witness format: {msg}"),
        }
    }
}

impl std::error::Error for WitnessError {}

impl From<JsonError> for WitnessError {
    fn from(e: JsonError) -> Self {
        WitnessError::JsonError(e)
    }
}

/// Parse a witness JSON file.
///
/// The file format is a JSON array of objects:
/// ```json
/// [
///   {"input": [5], "expected": 5},
///   {"input": [-13], "expected": 13}
/// ]
/// ```
///
/// Input values are typed based on `param_types` from the function declaration.
/// Expected values use simple inference (integer→I64, float→F64, bool→Bool, null→Unit).
/// Type-ambiguous values can use: `{"U64": 42}`, `{"Char": 65}`, `{"Bool": true}`.
pub fn parse_witness_file(
    json_str: &str,
    param_types: &[TypeTag],
) -> Result<Vec<Witness>, WitnessError> {
    let root = crate::json::parse(json_str)?;
    let array = root
        .as_array()
        .ok_or_else(|| WitnessError::InvalidFormat("root must be an array".to_string()))?;

    let mut witnesses = Vec::new();

    for elem in array {
        elem.as_object().ok_or_else(|| {
            WitnessError::InvalidFormat("each element must be an object".to_string())
        })?;

        let input_value = elem
            .get("input")
            .ok_or_else(|| WitnessError::InvalidFormat("missing 'input' field".to_string()))?;
        let input_array = input_value
            .as_array()
            .ok_or_else(|| WitnessError::InvalidFormat("'input' must be an array".to_string()))?;

        let expected_value = elem
            .get("expected")
            .ok_or_else(|| WitnessError::InvalidFormat("missing 'expected' field".to_string()))?;

        // Check input count
        if input_array.len() != param_types.len() {
            return Err(WitnessError::InputCountMismatch {
                expected: param_types.len(),
                got: input_array.len(),
            });
        }

        // Convert inputs using param_types
        let mut inputs = Vec::new();
        for (i, json_val) in input_array.iter().enumerate() {
            let value = convert_json_typed(json_val, param_types[i], i)?;
            inputs.push(value);
        }

        // Convert expected using inference
        let expected = convert_json_inferred(expected_value)?;

        witnesses.push(Witness { inputs, expected });
    }

    Ok(witnesses)
}

/// Convert a JSON value to a Value using a known type.
fn convert_json_typed(
    json: &JsonValue,
    type_tag: TypeTag,
    index: usize,
) -> Result<Value, WitnessError> {
    match type_tag {
        TypeTag::I64 => {
            let n = json.as_f64().ok_or(WitnessError::InputTypeMismatch {
                index,
                expected: type_tag,
            })?;
            if !n.is_finite() {
                return Err(WitnessError::InputTypeMismatch {
                    index,
                    expected: type_tag,
                });
            }
            if n != n.trunc() {
                return Err(WitnessError::InputTypeMismatch {
                    index,
                    expected: type_tag,
                });
            }
            Ok(Value::I64(n as i64))
        }
        TypeTag::U64 => {
            let n = json.as_f64().ok_or(WitnessError::InputTypeMismatch {
                index,
                expected: type_tag,
            })?;
            if !n.is_finite() || n < 0.0 {
                return Err(WitnessError::InputTypeMismatch {
                    index,
                    expected: type_tag,
                });
            }
            if n != n.trunc() {
                return Err(WitnessError::InputTypeMismatch {
                    index,
                    expected: type_tag,
                });
            }
            Ok(Value::U64(n as u64))
        }
        TypeTag::F64 => {
            let n = json.as_f64().ok_or(WitnessError::InputTypeMismatch {
                index,
                expected: type_tag,
            })?;
            if !n.is_finite() {
                return Err(WitnessError::InputTypeMismatch {
                    index,
                    expected: type_tag,
                });
            }
            Ok(Value::F64(n))
        }
        TypeTag::Bool => {
            let b = json.as_bool().ok_or(WitnessError::InputTypeMismatch {
                index,
                expected: type_tag,
            })?;
            Ok(Value::Bool(b))
        }
        TypeTag::Char => {
            let n = json.as_f64().ok_or(WitnessError::InputTypeMismatch {
                index,
                expected: type_tag,
            })?;
            if !n.is_finite() || n < 0.0 {
                return Err(WitnessError::InputTypeMismatch {
                    index,
                    expected: type_tag,
                });
            }
            if n != n.trunc() {
                return Err(WitnessError::InputTypeMismatch {
                    index,
                    expected: type_tag,
                });
            }
            let codepoint = n as u32;
            char::from_u32(codepoint)
                .map(Value::Char)
                .ok_or(WitnessError::InputTypeMismatch {
                    index,
                    expected: type_tag,
                })
        }
        TypeTag::Unit => {
            if !json.is_null() {
                return Err(WitnessError::InputTypeMismatch {
                    index,
                    expected: type_tag,
                });
            }
            Ok(Value::Unit)
        }
        _ => Err(WitnessError::InputTypeMismatch {
            index,
            expected: type_tag,
        }),
    }
}

/// Convert a JSON value to a Value using type inference.
fn convert_json_inferred(json: &JsonValue) -> Result<Value, WitnessError> {
    match json {
        JsonValue::Null => Ok(Value::Unit),
        JsonValue::Bool(b) => Ok(Value::Bool(*b)),
        JsonValue::Number(n) => {
            if !n.is_finite() {
                return Err(WitnessError::InvalidFormat(
                    "expected value cannot be NaN or infinity".to_string(),
                ));
            }
            // Integer if no fractional part and fits in i64
            if *n == n.trunc() {
                Ok(Value::I64(*n as i64))
            } else {
                Ok(Value::F64(*n))
            }
        }
        JsonValue::Object(pairs) => {
            // Check for type escape objects like {"U64": 42}
            if pairs.len() == 1 {
                let (key, val) = &pairs[0];
                match key.as_str() {
                    "U64" => {
                        let n = val.as_f64().ok_or_else(|| {
                            WitnessError::InvalidFormat("U64 value must be a number".to_string())
                        })?;
                        if !n.is_finite() || n < 0.0 || n != n.trunc() {
                            return Err(WitnessError::InvalidFormat(
                                "U64 value must be a non-negative integer".to_string(),
                            ));
                        }
                        Ok(Value::U64(n as u64))
                    }
                    "Char" => {
                        let n = val.as_f64().ok_or_else(|| {
                            WitnessError::InvalidFormat("Char value must be a number".to_string())
                        })?;
                        if !n.is_finite() || n < 0.0 || n != n.trunc() {
                            return Err(WitnessError::InvalidFormat(
                                "Char value must be a non-negative integer".to_string(),
                            ));
                        }
                        let codepoint = n as u32;
                        char::from_u32(codepoint).map(Value::Char).ok_or_else(|| {
                            WitnessError::InvalidFormat("invalid Unicode codepoint".to_string())
                        })
                    }
                    "F64" => {
                        let n = val.as_f64().ok_or_else(|| {
                            WitnessError::InvalidFormat("F64 value must be a number".to_string())
                        })?;
                        if !n.is_finite() {
                            return Err(WitnessError::InvalidFormat(
                                "F64 value cannot be NaN or infinity".to_string(),
                            ));
                        }
                        Ok(Value::F64(n))
                    }
                    "Bool" => {
                        let b = val.as_bool().ok_or_else(|| {
                            WitnessError::InvalidFormat("Bool value must be a boolean".to_string())
                        })?;
                        Ok(Value::Bool(b))
                    }
                    _ => Err(WitnessError::InvalidFormat(format!(
                        "unknown type escape: {}",
                        key
                    ))),
                }
            } else {
                Err(WitnessError::InvalidFormat(
                    "expected value cannot be a complex object".to_string(),
                ))
            }
        }
        _ => Err(WitnessError::InvalidFormat(
            "expected value must be null, bool, number, or type escape object".to_string(),
        )),
    }
}

/// Build a wrapper program that calls a function with given inputs.
///
/// The wrapper copies all FUNC/ENDFUNC blocks from the original,
/// then appends CONST instructions for each input, a CALL, and HALT.
pub fn build_witness_program(
    original: &Program,
    func_index: usize,
    inputs: &[Value],
) -> Result<Program, WitnessError> {
    // Find the last ENDFUNC
    let last_endfunc_idx = original
        .instructions
        .iter()
        .enumerate()
        .rev()
        .find(|(_, instr)| instr.opcode == Opcode::EndFunc)
        .map(|(i, _)| i);

    let last_endfunc_idx = last_endfunc_idx.ok_or(WitnessError::NoFunctions)?;

    // Copy instructions up to and including the last ENDFUNC
    let mut instructions = original.instructions[0..=last_endfunc_idx].to_vec();

    // Append CONST instructions for each input
    for input in inputs {
        match Instruction::from_value(input) {
            Ok(instrs) => instructions.extend(instrs),
            Err(msg) => return Err(WitnessError::UnencodableValue(msg.to_string())),
        }
    }

    // Append CALL instruction
    instructions.push(Instruction::new(
        Opcode::Call,
        TypeTag::None,
        func_index as u16,
        0,
        0,
    ));

    // Append HALT instruction
    instructions.push(Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0));

    Ok(Program::new(instructions))
}

/// Run all witnesses against a program function.
pub fn run_witnesses(
    program: &Program,
    func_index: usize,
    witnesses: &[Witness],
) -> Vec<WitnessResult> {
    witnesses
        .iter()
        .enumerate()
        .map(|(index, witness)| {
            // Build wrapper program
            let wrapper = match build_witness_program(program, func_index, &witness.inputs) {
                Ok(w) => w,
                Err(e) => {
                    return WitnessResult {
                        index,
                        passed: false,
                        actual: None,
                        expected: witness.expected.clone(),
                        error: Some(format!("failed to build witness program: {}", e)),
                    };
                }
            };

            // Run the program
            match nolang_vm::run(&wrapper) {
                Ok(actual) => {
                    let passed = actual == witness.expected;
                    WitnessResult {
                        index,
                        passed,
                        actual: Some(actual),
                        expected: witness.expected.clone(),
                        error: None,
                    }
                }
                Err(e) => WitnessResult {
                    index,
                    passed: false,
                    actual: None,
                    expected: witness.expected.clone(),
                    error: Some(format!("runtime error: {}", e)),
                },
            }
        })
        .collect()
}

/// Extract parameter types for a function from the program.
///
/// Uses the verifier's structural check to find PARAM declarations.
pub fn get_function_param_types(
    program: &Program,
    func_index: usize,
) -> Result<Vec<TypeTag>, WitnessError> {
    let (ctx, _errors) = nolang_verifier::check_structural(&program.instructions);

    if func_index >= ctx.functions.len() {
        return Err(WitnessError::FunctionNotFound {
            index: func_index,
            count: ctx.functions.len(),
        });
    }

    Ok(ctx.functions[func_index].param_types.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_witness_simple() {
        let json = r#"[{"input": [5], "expected": 5}]"#;
        let param_types = vec![TypeTag::I64];
        let witnesses = parse_witness_file(json, &param_types).unwrap();

        assert_eq!(witnesses.len(), 1);
        assert_eq!(witnesses[0].inputs, vec![Value::I64(5)]);
        assert_eq!(witnesses[0].expected, Value::I64(5));
    }

    #[test]
    fn test_parse_witness_multiple() {
        let json = r#"[
            {"input": [5], "expected": 5},
            {"input": [-13], "expected": 13},
            {"input": [0], "expected": 0}
        ]"#;
        let param_types = vec![TypeTag::I64];
        let witnesses = parse_witness_file(json, &param_types).unwrap();

        assert_eq!(witnesses.len(), 3);
        assert_eq!(witnesses[0].inputs, vec![Value::I64(5)]);
        assert_eq!(witnesses[0].expected, Value::I64(5));
        assert_eq!(witnesses[1].inputs, vec![Value::I64(-13)]);
        assert_eq!(witnesses[1].expected, Value::I64(13));
        assert_eq!(witnesses[2].inputs, vec![Value::I64(0)]);
        assert_eq!(witnesses[2].expected, Value::I64(0));
    }

    #[test]
    fn test_parse_witness_bool_input() {
        let json = r#"[{"input": [true], "expected": false}]"#;
        let param_types = vec![TypeTag::Bool];
        let witnesses = parse_witness_file(json, &param_types).unwrap();

        assert_eq!(witnesses.len(), 1);
        assert_eq!(witnesses[0].inputs, vec![Value::Bool(true)]);
        assert_eq!(witnesses[0].expected, Value::Bool(false));
    }

    #[test]
    fn test_parse_witness_type_mismatch() {
        let json = r#"[{"input": ["hello"], "expected": 5}]"#;
        let param_types = vec![TypeTag::I64];
        let result = parse_witness_file(json, &param_types);

        assert!(result.is_err());
        match result.unwrap_err() {
            WitnessError::InputTypeMismatch { index, expected } => {
                assert_eq!(index, 0);
                assert_eq!(expected, TypeTag::I64);
            }
            _ => panic!("expected InputTypeMismatch error"),
        }
    }

    #[test]
    fn test_parse_witness_wrong_input_count() {
        let json = r#"[{"input": [5, 10], "expected": 15}]"#;
        let param_types = vec![TypeTag::I64]; // expects 1 param, got 2
        let result = parse_witness_file(json, &param_types);

        assert!(result.is_err());
        match result.unwrap_err() {
            WitnessError::InputCountMismatch { expected, got } => {
                assert_eq!(expected, 1);
                assert_eq!(got, 2);
            }
            _ => panic!("expected InputCountMismatch error"),
        }
    }

    #[test]
    fn test_infer_expected_integer() {
        let json = crate::json::parse("42").unwrap();
        let value = convert_json_inferred(&json).unwrap();
        assert_eq!(value, Value::I64(42));
    }

    #[test]
    fn test_infer_expected_float() {
        let json = crate::json::parse("3.125").unwrap();
        let value = convert_json_inferred(&json).unwrap();
        assert_eq!(value, Value::F64(3.125));
    }

    #[test]
    fn test_infer_expected_bool() {
        let json = crate::json::parse("true").unwrap();
        let value = convert_json_inferred(&json).unwrap();
        assert_eq!(value, Value::Bool(true));
    }

    #[test]
    fn test_infer_expected_null() {
        let json = crate::json::parse("null").unwrap();
        let value = convert_json_inferred(&json).unwrap();
        assert_eq!(value, Value::Unit);
    }

    #[test]
    fn test_infer_expected_u64_escape() {
        let json = crate::json::parse(r#"{"U64": 42}"#).unwrap();
        let value = convert_json_inferred(&json).unwrap();
        assert_eq!(value, Value::U64(42));
    }

    #[test]
    fn test_build_witness_program_simple() {
        // Build a simple program with one FUNC
        let instructions = vec![
            Instruction::new(Opcode::Func, TypeTag::None, 1, 4, 0), // FUNC 1 4
            Instruction::new(Opcode::Param, TypeTag::I64, 0, 0, 0), // PARAM I64
            Instruction::new(Opcode::Ref, TypeTag::None, 0, 0, 0),  // REF 0
            Instruction::new(Opcode::Ret, TypeTag::None, 0, 0, 0),  // RET
            Instruction::new(Opcode::Hash, TypeTag::None, 0, 0, 0), // HASH (dummy)
            Instruction::new(Opcode::EndFunc, TypeTag::None, 0, 0, 0), // ENDFUNC
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0), // HALT
        ];
        let program = Program::new(instructions);

        let inputs = vec![Value::I64(42)];
        let wrapper = build_witness_program(&program, 0, &inputs).unwrap();

        // Check that CONST + CALL + HALT were appended after ENDFUNC
        let instrs = &wrapper.instructions;
        assert_eq!(instrs[5].opcode, Opcode::EndFunc);
        assert_eq!(instrs[6].opcode, Opcode::Const); // CONST for 42
        assert_eq!(instrs[6].type_tag, TypeTag::I64);
        assert_eq!(instrs[7].opcode, Opcode::Call);
        assert_eq!(instrs[7].arg1, 0); // func_index = 0
        assert_eq!(instrs[8].opcode, Opcode::Halt);
    }

    #[test]
    fn test_build_witness_no_functions() {
        let instructions = vec![Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0)];
        let program = Program::new(instructions);

        let inputs = vec![Value::I64(42)];
        let result = build_witness_program(&program, 0, &inputs);

        assert!(result.is_err());
        match result.unwrap_err() {
            WitnessError::NoFunctions => {}
            _ => panic!("expected NoFunctions error"),
        }
    }

    #[test]
    fn test_run_witnesses_identity() {
        // Build a simple identity function: FUNC 1 4 / PARAM I64 / REF 0 / RET / HASH / ENDFUNC / HALT
        let instructions = vec![
            Instruction::new(Opcode::Func, TypeTag::None, 1, 4, 0),
            Instruction::new(Opcode::Param, TypeTag::I64, 0, 0, 0),
            Instruction::new(Opcode::Ref, TypeTag::None, 0, 0, 0),
            Instruction::new(Opcode::Ret, TypeTag::None, 0, 0, 0),
            Instruction::new(Opcode::Hash, TypeTag::None, 0, 0, 0),
            Instruction::new(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ];
        let program = Program::new(instructions);

        let witnesses = vec![Witness {
            inputs: vec![Value::I64(42)],
            expected: Value::I64(42),
        }];

        let results = run_witnesses(&program, 0, &witnesses);

        assert_eq!(results.len(), 1);
        assert!(results[0].passed);
        assert_eq!(results[0].actual, Some(Value::I64(42)));
        assert_eq!(results[0].expected, Value::I64(42));
        assert!(results[0].error.is_none());
    }
}
