//! CLI command implementations.

use std::fs;

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
pub fn run(args: &[String]) -> Result<(), i32> {
    if args.is_empty() {
        eprintln!("error: run requires an input file");
        eprintln!("Usage: nolang run <input.nolb>");
        return Err(1);
    }

    let input = &args[0];
    let program = read_binary(input)?;

    // Verify first
    if let Err(errors) = nolang_verifier::verify(&program) {
        for e in &errors {
            eprintln!("error: {e}");
        }
        return Err(2);
    }

    // Execute
    match nolang_vm::run(&program) {
        Ok(value) => {
            println!("{value}");
            Ok(())
        }
        Err(e) => {
            eprintln!("runtime error: {e}");
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
        eprintln!("Usage: nolang train <input.nol> --intent \"description\"");
        return Err(1);
    }

    let input = &args[0];

    // Parse --intent flag
    let intent = parse_intent(&args[1..])?;

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

    // Format JSON manually — no serde needed
    let json = format!(
        "{{\"intent\":{},\"assembly\":{},\"binary_b64\":{}}}",
        json_escape(&intent),
        json_escape(assembly),
        json_escape(&b64)
    );

    println!("{json}");
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
