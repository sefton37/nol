//! CLI command implementations.

use std::fs;

use nolang_cli::witness;

/// Assemble a .nol text file to .nolb binary.
pub fn assemble(args: &[String]) -> Result<(), i32> {
    if args.is_empty() {
        eprintln!("error: assemble requires an input file");
        eprintln!("Usage: nolang assemble <input.nol> [-o output.nolb]");
        return Err(1);
    }

    let input = &args[0];

    // Parse -o flag
    let output = if args.len() >= 3 && args[1] == "-o" {
        args[2].clone()
    } else if input.ends_with(".nol") {
        format!("{}b", input)
    } else {
        format!("{input}.nolb")
    };

    let text = fs::read_to_string(input).map_err(|e| {
        eprintln!("error: cannot read '{input}': {e}");
        1
    })?;

    let program = nolang_assembler::assemble(&text).map_err(|e| {
        eprintln!("error: {e}");
        1
    })?;

    let bytes = program.encode();
    let instr_count = program.len();

    fs::write(&output, &bytes).map_err(|e| {
        eprintln!("error: cannot write '{output}': {e}");
        1
    })?;

    eprintln!(
        "assembled {instr_count} instructions ({} bytes) -> {output}",
        bytes.len()
    );
    Ok(())
}

/// Verify a .nolb binary program.
pub fn verify(args: &[String]) -> Result<(), i32> {
    if args.is_empty() {
        eprintln!("error: verify requires an input file");
        eprintln!("Usage: nolang verify <input.nolb>");
        return Err(1);
    }

    let input = &args[0];
    let program = read_binary(input)?;

    match nolang_verifier::verify(&program) {
        Ok(()) => {
            println!("OK: {input} ({} instructions)", program.len());
            Ok(())
        }
        Err(errors) => {
            for e in &errors {
                eprintln!("error: {e}");
            }
            Err(2)
        }
    }
}

/// Verify and execute a .nolb binary program.
///
/// Flags:
/// - `--sandbox <PATH>`: restrict file I/O to paths inside the given prefix
/// - `--json`: emit structured JSON output instead of human-readable text
pub fn run(args: &[String]) -> Result<(), i32> {
    if args.is_empty() {
        eprintln!("error: run requires an input file");
        eprintln!("Usage: nolang run <input.nolb> [--sandbox PATH] [--json]");
        return Err(1);
    }

    // Parse flags: --sandbox and --json may appear anywhere after the file arg.
    let input = &args[0];
    let mut sandbox: Option<std::path::PathBuf> = None;
    let mut json_output = false;

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--sandbox" => {
                if i + 1 < args.len() {
                    sandbox = Some(std::path::PathBuf::from(&args[i + 1]));
                    i += 2;
                } else {
                    eprintln!("error: --sandbox requires a path argument");
                    return Err(1);
                }
            }
            "--json" => {
                json_output = true;
                i += 1;
            }
            other => {
                eprintln!("error: unknown flag '{other}'");
                eprintln!("Usage: nolang run <input.nolb> [--sandbox PATH] [--json]");
                return Err(1);
            }
        }
    }

    let program = read_binary(input)?;

    // Verify first
    if let Err(errors) = nolang_verifier::verify(&program) {
        if json_output {
            let msg = errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");
            println!("{}", nolang_cli::json::format_error_json("verification", &msg));
        } else {
            for e in &errors {
                eprintln!("error: {e}");
            }
        }
        return Err(2);
    }

    // Build VM with optional sandbox and default exec allowlist
    let default_allowlist = vec![
        "pytest".to_string(),
        "ruff".to_string(),
        "mypy".to_string(),
        "cargo".to_string(),
        "test".to_string(),
    ];

    let vm_base = nolang_vm::VM::new(&program)
        .with_exec_allowlist(default_allowlist);

    let mut vm = if let Some(prefix) = sandbox {
        vm_base.with_sandbox(prefix)
    } else {
        vm_base
    };

    // Execute
    match vm.execute() {
        Ok(value) => {
            if json_output {
                println!("{}", nolang_cli::json::format_ok_json(&value));
            } else {
                println!("{value}");
            }
            Ok(())
        }
        Err(e) => {
            if json_output {
                println!(
                    "{}",
                    nolang_cli::json::format_error_json("runtime", &e.to_string())
                );
            } else {
                eprintln!("runtime error: {e}");
            }
            Err(3)
        }
    }
}

/// Disassemble a .nolb binary to text.
pub fn disassemble(args: &[String]) -> Result<(), i32> {
    if args.is_empty() {
        eprintln!("error: disassemble requires an input file");
        eprintln!("Usage: nolang disassemble <input.nolb>");
        return Err(1);
    }

    let input = &args[0];
    let program = read_binary(input)?;
    let text = nolang_assembler::disassemble(&program);
    print!("{text}");
    Ok(())
}

/// Compute FUNC block hashes for a .nol text file.
pub fn hash(args: &[String]) -> Result<(), i32> {
    if args.is_empty() {
        eprintln!("error: hash requires an input file");
        eprintln!("Usage: nolang hash <input.nol>");
        return Err(1);
    }

    let input = &args[0];
    let text = fs::read_to_string(input).map_err(|e| {
        eprintln!("error: cannot read '{input}': {e}");
        1
    })?;

    let program = nolang_assembler::assemble(&text).map_err(|e| {
        eprintln!("error: {e}");
        1
    })?;

    let instrs = &program.instructions;
    let (ctx, errors) = nolang_verifier::check_structural(instrs);
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("error: {e}");
        }
        return Err(1);
    }

    for func in &ctx.functions {
        if let Some(hash_pc) = func.hash_pc {
            let hash_instr = nolang_verifier::compute_func_hash(instrs, func.func_pc, hash_pc);
            println!(
                "HASH 0x{:04x} 0x{:04x} 0x{:04x}",
                hash_instr.arg1, hash_instr.arg2, hash_instr.arg3
            );
        }
    }

    Ok(())
}

/// Generate a training pair from a .nol file.
pub fn train(args: &[String]) -> Result<(), i32> {
    if args.is_empty() {
        eprintln!("error: train requires an input file and --intent");
        eprintln!(
            "Usage: nolang train <input.nol> --intent \"description\" [--witnesses <file.json>]"
        );
        return Err(1);
    }

    let input = &args[0];

    // Parse --intent flag
    let intent = parse_intent(&args[1..])?;

    // Parse optional --witnesses flag
    let witnesses_path = parse_witnesses_flag(&args[1..])?;

    let text = fs::read_to_string(input).map_err(|e| {
        eprintln!("error: cannot read '{input}': {e}");
        1
    })?;

    let program = nolang_assembler::assemble(&text).map_err(|e| {
        eprintln!("error: {e}");
        1
    })?;

    // Verify
    if let Err(errors) = nolang_verifier::verify(&program) {
        for e in &errors {
            eprintln!("error: {e}");
        }
        return Err(2);
    }

    let bytes = program.encode();
    let b64 = base64_encode(&bytes);
    let assembly = text.trim();

    // Build witnesses JSON fragment if provided
    let witnesses_json = if let Some(ref wpath) = witnesses_path {
        let wjson_str = fs::read_to_string(wpath).map_err(|e| {
            eprintln!("error: cannot read '{wpath}': {e}");
            1
        })?;
        // Validate the JSON is parseable
        nolang_cli::json::parse(&wjson_str).map_err(|e| {
            eprintln!("error: invalid witness JSON: {e}");
            1
        })?;
        // Include the raw JSON content (already valid JSON)
        Some(wjson_str.trim().to_string())
    } else {
        None
    };

    // Format JSON manually — no serde needed
    let json = if let Some(ref wjson) = witnesses_json {
        format!(
            "{{\"intent\":{},\"assembly\":{},\"binary_b64\":{},\"witnesses\":{}}}",
            json_escape(&intent),
            json_escape(assembly),
            json_escape(&b64),
            wjson
        )
    } else {
        format!(
            "{{\"intent\":{},\"assembly\":{},\"binary_b64\":{}}}",
            json_escape(&intent),
            json_escape(assembly),
            json_escape(&b64)
        )
    };

    println!("{json}");
    Ok(())
}

/// Run witness tests against a program's function.
pub fn witness_cmd(args: &[String]) -> Result<(), i32> {
    if args.is_empty() {
        eprintln!("error: witness requires a program file and a witness file");
        eprintln!("Usage: nolang witness <program.nolb> <witnesses.json> [--func N]");
        return Err(1);
    }
    if args.len() < 2 {
        eprintln!("error: witness requires a witness JSON file");
        eprintln!("Usage: nolang witness <program.nolb> <witnesses.json> [--func N]");
        return Err(1);
    }

    let program_path = &args[0];
    let witness_path = &args[1];

    // Parse --func flag (default 0)
    let func_index = parse_func_flag(&args[2..])?;

    // Read program
    let program = read_binary(program_path)?;

    // Get function param types
    let param_types = witness::get_function_param_types(&program, func_index).map_err(|e| {
        eprintln!("error: {e}");
        1
    })?;

    // Read and parse witness file
    let json_str = fs::read_to_string(witness_path).map_err(|e| {
        eprintln!("error: cannot read '{witness_path}': {e}");
        1
    })?;

    let witnesses = witness::parse_witness_file(&json_str, &param_types).map_err(|e| {
        eprintln!("error: {e}");
        1
    })?;

    if witnesses.is_empty() {
        eprintln!("warning: no witnesses found in '{witness_path}'");
        return Ok(());
    }

    // Run witnesses
    let results = witness::run_witnesses(&program, func_index, &witnesses);

    // Print results
    let mut pass_count = 0;
    let total = results.len();

    for result in &results {
        if result.passed {
            pass_count += 1;
            println!("PASS witness {}", result.index);
        } else if let Some(ref error) = result.error {
            println!("FAIL witness {}: {}", result.index, error);
        } else {
            println!(
                "FAIL witness {}: expected {}, got {}",
                result.index,
                result.expected,
                result
                    .actual
                    .as_ref()
                    .map_or("(none)".to_string(), |v| v.to_string())
            );
        }
    }

    println!("{pass_count}/{total} witnesses passed");

    if pass_count == total {
        Ok(())
    } else {
        Err(3)
    }
}

/// Generate corpus programs from the catalog.
pub fn generate(args: &[String]) -> Result<(), i32> {
    use nolang_cli::generate::{self, GenerateConfig};
    use std::path::PathBuf;

    // Parse flags
    let mut output_dir: Option<String> = None;
    let mut filter: Option<String> = None;
    let mut verbose = true; // default to verbose

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--output-dir" => {
                if i + 1 < args.len() {
                    output_dir = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("error: --output-dir requires a value");
                    return Err(1);
                }
            }
            "--filter" => {
                if i + 1 < args.len() {
                    filter = Some(args[i + 1].clone());
                    i += 2;
                } else {
                    eprintln!("error: --filter requires a value");
                    return Err(1);
                }
            }
            "--quiet" | "-q" => {
                verbose = false;
                i += 1;
            }
            other => {
                eprintln!("error: unknown flag '{other}'");
                eprintln!("Usage: nolang generate [--output-dir DIR] [--filter PAT] [--quiet]");
                return Err(1);
            }
        }
    }

    let base = output_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("tests"));

    let config = GenerateConfig {
        programs_dir: base.join("programs"),
        witnesses_dir: base.join("witnesses"),
        corpus_path: base.join("corpus/generated.nolt"),
        filter,
        verbose,
    };

    let results = generate::generate(&config).map_err(|e| {
        eprintln!("error: {e}");
        1
    })?;

    let success_count = results.iter().filter(|r| r.success).count();
    let fail_count = results.len() - success_count;

    if fail_count > 0 {
        eprintln!("{fail_count} programs failed");
        return Err(1);
    }

    if success_count == 0 && config.filter.is_none() {
        eprintln!("warning: no programs generated (catalog may be empty)");
    }

    Ok(())
}

// --- Helpers ---

/// Read and decode a .nolb binary file.
fn read_binary(path: &str) -> Result<nolang_common::Program, i32> {
    let bytes = fs::read(path).map_err(|e| {
        eprintln!("error: cannot read '{path}': {e}");
        1
    })?;

    nolang_common::Program::decode(&bytes).map_err(|e| {
        eprintln!("error: invalid binary: {e}");
        1
    })
}

/// Parse the --intent flag from arguments.
fn parse_intent(args: &[String]) -> Result<String, i32> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--intent" {
            if i + 1 < args.len() {
                return Ok(args[i + 1].clone());
            }
            eprintln!("error: --intent requires a value");
            return Err(1);
        }
        i += 1;
    }
    eprintln!("error: --intent is required");
    eprintln!("Usage: nolang train <input.nol> --intent \"description\"");
    Err(1)
}

/// Parse the --func flag from arguments (default 0).
fn parse_func_flag(args: &[String]) -> Result<usize, i32> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--func" {
            if i + 1 < args.len() {
                return args[i + 1].parse::<usize>().map_err(|_| {
                    eprintln!("error: --func value must be a non-negative integer");
                    1
                });
            }
            eprintln!("error: --func requires a value");
            return Err(1);
        }
        i += 1;
    }
    Ok(0)
}

/// Parse the --witnesses flag from arguments (optional).
fn parse_witnesses_flag(args: &[String]) -> Result<Option<String>, i32> {
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--witnesses" {
            if i + 1 < args.len() {
                return Ok(Some(args[i + 1].clone()));
            }
            eprintln!("error: --witnesses requires a value");
            return Err(1);
        }
        i += 1;
    }
    Ok(None)
}

/// Escape a string as a JSON string value (with quotes).
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

    // --- json_escape tests ---

    #[test]
    fn json_escape_simple() {
        assert_eq!(json_escape("hello"), "\"hello\"");
    }

    #[test]
    fn json_escape_special_chars() {
        assert_eq!(json_escape("a\"b"), "\"a\\\"b\"");
        assert_eq!(json_escape("a\\b"), "\"a\\\\b\"");
        assert_eq!(json_escape("a\nb"), "\"a\\nb\"");
        assert_eq!(json_escape("a\rb"), "\"a\\rb\"");
        assert_eq!(json_escape("a\tb"), "\"a\\tb\"");
    }

    #[test]
    fn json_escape_control_chars() {
        let s = String::from_utf8(vec![0x01]).unwrap();
        assert_eq!(json_escape(&s), "\"\\u0001\"");
    }

    // --- base64_encode tests ---

    #[test]
    fn base64_empty() {
        assert_eq!(base64_encode(&[]), "");
    }

    #[test]
    fn base64_one_byte() {
        assert_eq!(base64_encode(&[0x4d]), "TQ==");
    }

    #[test]
    fn base64_two_bytes() {
        assert_eq!(base64_encode(&[0x4d, 0x61]), "TWE=");
    }

    #[test]
    fn base64_three_bytes() {
        assert_eq!(base64_encode(&[0x4d, 0x61, 0x6e]), "TWFu");
    }

    #[test]
    fn base64_longer_string() {
        // "Hello" -> "SGVsbG8="
        assert_eq!(base64_encode(b"Hello"), "SGVsbG8=");
    }

    #[test]
    fn base64_all_zeros() {
        assert_eq!(base64_encode(&[0, 0, 0]), "AAAA");
    }

    #[test]
    fn base64_all_ones() {
        assert_eq!(base64_encode(&[0xff, 0xff, 0xff]), "////");
    }
}
