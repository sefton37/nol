//! NoLang CLI — assemble, verify, execute, and train.
//!
//! Exit codes:
//! - 0: Success
//! - 1: Input/decode/assembly/structural error
//! - 2: Verification failure
//! - 3: Runtime error

mod commands;

use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let result = match args[1].as_str() {
        "assemble" => commands::assemble(&args[2..]),
        "verify" => commands::verify(&args[2..]),
        "run" => commands::run(&args[2..]),
        "disassemble" => commands::disassemble(&args[2..]),
        "hash" => commands::hash(&args[2..]),
        "train" => commands::train(&args[2..]),
        "witness" => commands::witness_cmd(&args[2..]),
        "generate" => commands::generate(&args[2..]),
        "--help" | "-h" | "help" => {
            print_usage();
            process::exit(0);
        }
        other => {
            eprintln!("error: unknown command '{other}'");
            eprintln!();
            print_usage();
            process::exit(1);
        }
    };

    if let Err(code) = result {
        process::exit(code);
    }
}

fn print_usage() {
    eprintln!("Usage: nolang <command> [args]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  assemble <input.nol> [-o output.nolb]   Assemble text to binary");
    eprintln!("  verify <input.nolb>                     Verify a binary program");
    eprintln!("  run <input.nolb>                        Verify and execute a binary program");
    eprintln!("  disassemble <input.nolb>                Disassemble binary to text");
    eprintln!("  hash <input.nol>                        Compute FUNC block hashes");
    eprintln!("  train <input.nol> --intent \"desc\"        Generate training pair");
    eprintln!("  witness <prog.nolb> <wit.json> [--func N]  Run witness tests");
    eprintln!("  generate [--output-dir DIR] [--filter PAT]  Generate corpus programs");
}
