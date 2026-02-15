//! Program generator pipeline for Phase 6c corpus building.
//!
//! This module takes program specifications from the catalog and:
//! 1. Assembles templates with placeholder hashes
//! 2. Patches hashes using verifier's `compute_func_hash()`
//! 3. Disassembles to canonical text
//! 4. Verifies with full static analysis
//! 5. Builds and runs witness tests
//! 6. Writes .nol files, .json witness files, and .nolt training corpus

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

use nolang_common::{Program, Value};

use crate::catalog::{self, ProgramSpec, WitnessCase, WitnessValue};
use crate::witness::{self, Witness};

/// Configuration for the generator.
#[derive(Debug, Clone)]
pub struct GenerateConfig {
    /// Where to write .nol files (typically tests/programs).
    pub programs_dir: PathBuf,
    /// Where to write .json witness files (typically tests/witnesses).
    pub witnesses_dir: PathBuf,
    /// Path to the .nolt training corpus file (typically tests/corpus/generated.nolt).
    pub corpus_path: PathBuf,
    /// Optional pattern to filter program IDs (substring match).
    pub filter: Option<String>,
    /// Print detailed progress messages.
    pub verbose: bool,
}

/// Result of generating one program.
#[derive(Debug, Clone)]
pub struct GenerateResult {
    pub id: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Generate all programs from the catalog.
pub fn generate(config: &GenerateConfig) -> Result<Vec<GenerateResult>, String> {
    // Create output directories if needed
    fs::create_dir_all(&config.programs_dir)
        .map_err(|e| format!("failed to create programs directory: {}", e))?;
    fs::create_dir_all(&config.witnesses_dir)
        .map_err(|e| format!("failed to create witnesses directory: {}", e))?;

    // Create parent directory for corpus file if needed
    if let Some(parent) = config.corpus_path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("failed to create corpus directory: {}", e))?;
    }

    // Open corpus file for appending
    let mut corpus_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&config.corpus_path)
        .map_err(|e| format!("failed to open corpus file: {}", e))?;

    // Get all program specs
    let all_specs = catalog::all_programs();

    // Apply filter if provided
    let specs: Vec<&ProgramSpec> = all_specs
        .iter()
        .filter(|spec| {
            if let Some(ref filter) = config.filter {
                spec.id.contains(filter)
            } else {
                true
            }
        })
        .collect();

    if config.verbose {
        eprintln!("Generating {} programs...", specs.len());
    }

    let mut results = Vec::new();

    for spec in specs {
        let result = generate_one(spec, config, &mut corpus_file);

        if config.verbose {
            match &result {
                GenerateResult {
                    success: true, id, ..
                } => {
                    eprintln!("  [OK] {}", id);
                }
                GenerateResult {
                    success: false,
                    id,
                    error: Some(err),
                    ..
                } => {
                    eprintln!("  [FAIL] {}: {}", id, err);
                }
                _ => {}
            }
        }

        results.push(result);
    }

    if config.verbose {
        let success_count = results.iter().filter(|r| r.success).count();
        eprintln!(
            "Generated {}/{} programs successfully",
            success_count,
            results.len()
        );
    }

    Ok(results)
}

/// Generate a single program from a spec.
fn generate_one(
    spec: &ProgramSpec,
    config: &GenerateConfig,
    corpus_file: &mut File,
) -> GenerateResult {
    // 1. Assemble the template
    let program = match nolang_assembler::assemble(spec.assembly_template) {
        Ok(p) => p,
        Err(e) => {
            return GenerateResult {
                id: spec.id.to_string(),
                success: false,
                error: Some(format!("assembly failed: {}", e)),
            };
        }
    };

    // 2. Patch HASH instructions
    let patched_program = match patch_hashes(&program) {
        Ok(p) => p,
        Err(e) => {
            return GenerateResult {
                id: spec.id.to_string(),
                success: false,
                error: Some(format!("hash patching failed: {}", e)),
            };
        }
    };

    // 3. Disassemble to canonical text
    let canonical_text = nolang_assembler::disassemble(&patched_program);

    // 4. Verify the patched program
    if let Err(errors) = nolang_verifier::verify(&patched_program) {
        let error_msg = errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return GenerateResult {
            id: spec.id.to_string(),
            success: false,
            error: Some(format!("verification failed: {}", error_msg)),
        };
    }

    // 5. If witness cases exist, build and run them
    let witnesses_json = if !spec.witness_cases.is_empty() {
        match run_witness_tests(&patched_program, &spec.witness_cases) {
            Ok(json) => Some(json),
            Err(e) => {
                return GenerateResult {
                    id: spec.id.to_string(),
                    success: false,
                    error: Some(format!("witness test failed: {}", e)),
                };
            }
        }
    } else {
        None
    };

    // 6. Write .nol file
    let nol_path = config.programs_dir.join(format!("{}.nol", spec.id));
    if let Err(e) = fs::write(&nol_path, &canonical_text) {
        return GenerateResult {
            id: spec.id.to_string(),
            success: false,
            error: Some(format!("failed to write .nol file: {}", e)),
        };
    }

    // 7. Write .json witness file if witnesses exist
    if let Some(ref json) = witnesses_json {
        let json_path = config.witnesses_dir.join(format!("{}.json", spec.id));
        if let Err(e) = fs::write(&json_path, json) {
            return GenerateResult {
                id: spec.id.to_string(),
                success: false,
                error: Some(format!("failed to write witness file: {}", e)),
            };
        }
    }

    // 8. Append training pair to corpus
    if let Err(e) = write_corpus_line(
        corpus_file,
        spec.intent,
        &canonical_text,
        &patched_program,
        &spec.witness_cases,
    ) {
        return GenerateResult {
            id: spec.id.to_string(),
            success: false,
            error: Some(format!("failed to write corpus entry: {}", e)),
        };
    }

    GenerateResult {
        id: spec.id.to_string(),
        success: true,
        error: None,
    }
}

/// Patch all HASH instructions in a program using the verifier's hash computation.
fn patch_hashes(program: &Program) -> Result<Program, String> {
    let (ctx, errors) = nolang_verifier::check_structural(&program.instructions);

    if !errors.is_empty() {
        let error_msg = errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(format!("structural check failed: {}", error_msg));
    }

    let mut instructions = program.instructions.clone();

    // For each function with a hash_pc, compute and patch the hash
    for func_info in &ctx.functions {
        if let Some(hash_pc) = func_info.hash_pc {
            let hash_instr =
                nolang_verifier::compute_func_hash(&instructions, func_info.func_pc, hash_pc);
            instructions[hash_pc] = hash_instr;
        }
    }

    Ok(Program::new(instructions))
}

/// Convert WitnessValue to Value.
fn witness_value_to_value(wv: &WitnessValue) -> Value {
    match wv {
        WitnessValue::I64(v) => Value::I64(*v),
        WitnessValue::U64(v) => Value::U64(*v),
        WitnessValue::F64(v) => Value::F64(*v),
        WitnessValue::Bool(v) => Value::Bool(*v),
        WitnessValue::Char(c) => Value::Char(*c),
        WitnessValue::Unit => Value::Unit,
    }
}

/// Build Witness objects from catalog WitnessCases and run them.
fn run_witness_tests(program: &Program, cases: &[WitnessCase]) -> Result<String, String> {
    // Build Witness objects
    let witnesses: Vec<Witness> = cases
        .iter()
        .map(|case| Witness {
            inputs: case.inputs.iter().map(witness_value_to_value).collect(),
            expected: witness_value_to_value(&case.expected),
        })
        .collect();

    // Run witnesses against function 0
    let results = witness::run_witnesses(program, 0, &witnesses);

    // Check all passed
    for result in &results {
        if !result.passed {
            let error_msg = if let Some(ref err) = result.error {
                err.clone()
            } else if let Some(ref actual) = result.actual {
                format!("expected {:?}, got {:?}", result.expected, actual)
            } else {
                "witness failed".to_string()
            };
            return Err(format!("witness {} failed: {}", result.index, error_msg));
        }
    }

    // Build witness JSON (array of objects with input/expected)
    let mut json = String::from("[\n");
    for (i, case) in cases.iter().enumerate() {
        if i > 0 {
            json.push_str(",\n");
        }
        json.push_str("  {\"input\": [");
        for (j, input) in case.inputs.iter().enumerate() {
            if j > 0 {
                json.push_str(", ");
            }
            json.push_str(&input.to_input_json());
        }
        json.push_str("], \"expected\": ");
        json.push_str(&case.expected.to_json());
        json.push('}');
    }
    json.push_str("\n]\n");

    Ok(json)
}

/// Write a training pair line to the corpus file.
fn write_corpus_line(
    file: &mut File,
    intent: &str,
    assembly: &str,
    program: &Program,
    witness_cases: &[WitnessCase],
) -> Result<(), String> {
    let bytes = program.encode();
    let binary_b64 = base64_encode(&bytes);

    // Build JSON line
    let mut line = String::new();
    line.push_str("{\"intent\":");
    line.push_str(&json_escape(intent));
    line.push_str(",\"assembly\":");
    line.push_str(&json_escape(assembly));
    line.push_str(",\"binary_b64\":");
    line.push_str(&json_escape(&binary_b64));

    // Add witnesses if present
    if !witness_cases.is_empty() {
        line.push_str(",\"witnesses\":[");
        for (i, case) in witness_cases.iter().enumerate() {
            if i > 0 {
                line.push(',');
            }
            line.push_str("{\"input\":[");
            for (j, input) in case.inputs.iter().enumerate() {
                if j > 0 {
                    line.push(',');
                }
                line.push_str(&input.to_input_json());
            }
            line.push_str("],\"expected\":");
            line.push_str(&case.expected.to_json());
            line.push('}');
        }
        line.push(']');
    }

    line.push_str("}\n");

    file.write_all(line.as_bytes())
        .map_err(|e| format!("failed to write to corpus file: {}", e))?;

    Ok(())
}

/// Escape a string for JSON (with surrounding quotes).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Base64-encode bytes (standard alphabet, with padding).
fn base64_encode(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    let chunks = data.chunks(3);

    for chunk in chunks {
        match chunk.len() {
            3 => {
                let b0 = chunk[0] as u32;
                let b1 = chunk[1] as u32;
                let b2 = chunk[2] as u32;
                let n = (b0 << 16) | (b1 << 8) | b2;
                out.push(ALPHABET[(n >> 18) as usize & 0x3f] as char);
                out.push(ALPHABET[(n >> 12) as usize & 0x3f] as char);
                out.push(ALPHABET[(n >> 6) as usize & 0x3f] as char);
                out.push(ALPHABET[n as usize & 0x3f] as char);
            }
            2 => {
                let b0 = chunk[0] as u32;
                let b1 = chunk[1] as u32;
                let n = (b0 << 16) | (b1 << 8);
                out.push(ALPHABET[(n >> 18) as usize & 0x3f] as char);
                out.push(ALPHABET[(n >> 12) as usize & 0x3f] as char);
                out.push(ALPHABET[(n >> 6) as usize & 0x3f] as char);
                out.push('=');
            }
            1 => {
                let b0 = chunk[0] as u32;
                let n = b0 << 16;
                out.push(ALPHABET[(n >> 18) as usize & 0x3f] as char);
                out.push(ALPHABET[(n >> 12) as usize & 0x3f] as char);
                out.push('=');
                out.push('=');
            }
            _ => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use nolang_common::{Instruction, Opcode, TypeTag};

    #[test]
    fn test_patch_hashes_simple() {
        // Build a simple function with placeholder hash
        let instructions = vec![
            Instruction::new(Opcode::Func, TypeTag::None, 1, 4, 0), // FUNC 1 4
            Instruction::new(Opcode::Param, TypeTag::I64, 0, 0, 0), // PARAM I64
            Instruction::new(Opcode::Ref, TypeTag::None, 0, 0, 0),  // REF 0
            Instruction::new(Opcode::Ret, TypeTag::None, 0, 0, 0),  // RET
            Instruction::new(Opcode::Hash, TypeTag::None, 0, 0, 0), // HASH (placeholder)
            Instruction::new(Opcode::EndFunc, TypeTag::None, 0, 0, 0), // ENDFUNC
            Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0), // HALT
        ];
        let program = Program::new(instructions);

        let patched = patch_hashes(&program).unwrap();

        // Check that HASH instruction was patched (not all zeros)
        let hash_instr = &patched.instructions[4];
        assert_eq!(hash_instr.opcode, Opcode::Hash);
        // At least one of arg1/arg2/arg3 should be non-zero after patching
        let has_nonzero = hash_instr.arg1 != 0 || hash_instr.arg2 != 0 || hash_instr.arg3 != 0;
        assert!(has_nonzero, "hash should have been computed");
    }

    #[test]
    fn test_witness_value_to_value_i64() {
        assert_eq!(
            witness_value_to_value(&WitnessValue::I64(42)),
            Value::I64(42)
        );
    }

    #[test]
    fn test_witness_value_to_value_u64() {
        assert_eq!(
            witness_value_to_value(&WitnessValue::U64(123)),
            Value::U64(123)
        );
    }

    #[test]
    fn test_witness_value_to_value_f64() {
        assert_eq!(
            witness_value_to_value(&WitnessValue::F64(3.125)),
            Value::F64(3.125)
        );
    }

    #[test]
    fn test_witness_value_to_value_bool() {
        assert_eq!(
            witness_value_to_value(&WitnessValue::Bool(true)),
            Value::Bool(true)
        );
    }

    #[test]
    fn test_witness_value_to_value_char() {
        assert_eq!(
            witness_value_to_value(&WitnessValue::Char('A')),
            Value::Char('A')
        );
    }

    #[test]
    fn test_witness_value_to_value_unit() {
        assert_eq!(witness_value_to_value(&WitnessValue::Unit), Value::Unit);
    }

    #[test]
    fn test_json_escape_simple() {
        assert_eq!(json_escape("hello"), "\"hello\"");
    }

    #[test]
    fn test_json_escape_special_chars() {
        assert_eq!(json_escape("say \"hi\""), "\"say \\\"hi\\\"\"");
        assert_eq!(json_escape("path\\to\\file"), "\"path\\\\to\\\\file\"");
        assert_eq!(json_escape("line\nbreak"), "\"line\\nbreak\"");
        assert_eq!(json_escape("tab\there"), "\"tab\\there\"");
    }

    #[test]
    fn test_json_escape_control_chars() {
        assert_eq!(json_escape("\x01"), "\"\\u0001\"");
        assert_eq!(json_escape("hello\x1fworld"), "\"hello\\u001fworld\"");
    }

    #[test]
    fn test_base64_empty() {
        assert_eq!(base64_encode(&[]), "");
    }

    #[test]
    fn test_base64_one_byte() {
        assert_eq!(base64_encode(&[0x4d]), "TQ==");
    }

    #[test]
    fn test_base64_two_bytes() {
        assert_eq!(base64_encode(&[0x4d, 0x61]), "TWE=");
    }

    #[test]
    fn test_base64_three_bytes() {
        assert_eq!(base64_encode(&[0x4d, 0x61, 0x6e]), "TWFu");
    }

    #[test]
    fn test_base64_longer_string() {
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
    }

    #[test]
    fn test_base64_all_zeros() {
        assert_eq!(base64_encode(&[0, 0, 0]), "AAAA");
    }

    #[test]
    fn test_base64_all_ones() {
        assert_eq!(base64_encode(&[0xff, 0xff, 0xff]), "////");
    }
}
