//! Integration tests for the NoLang CLI.
//!
//! These tests invoke the `nolang` binary as a subprocess and check
//! exit codes, stdout, and stderr.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

#[allow(deprecated)]
fn nolang() -> Command {
    Command::cargo_bin("nolang").unwrap()
}

/// Return the workspace root (parent of nolang-cli/).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

/// Return the absolute path to a test program file.
fn test_program(name: &str) -> PathBuf {
    workspace_root().join("tests/programs").join(name)
}

/// Helper: assemble a .nol file, returning the path to the .nolb output.
fn assemble_to_temp(dir: &TempDir, nol_content: &str) -> std::path::PathBuf {
    let input = dir.path().join("test.nol");
    let output = dir.path().join("test.nolb");
    fs::write(&input, nol_content).unwrap();
    nolang()
        .args([
            "assemble",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success();
    output
}

// ---- No-args / help ----

#[test]
fn no_args_prints_usage_and_exits_1() {
    nolang()
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Usage: nolang"));
}

#[test]
fn help_flag_exits_0() {
    nolang()
        .arg("--help")
        .assert()
        .success()
        .stderr(predicate::str::contains("Commands:"));
}

#[test]
fn unknown_command_exits_1() {
    nolang()
        .arg("frobnicate")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("unknown command"));
}

// ---- Assemble ----

#[test]
fn assemble_simple_program() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("test.nol");
    let output = dir.path().join("test.nolb");
    fs::write(&input, "CONST I64 0x0000 0x002a\nHALT\n").unwrap();

    nolang()
        .args([
            "assemble",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stderr(predicate::str::contains("assembled 2 instructions"));

    assert!(output.exists());
    let bytes = fs::read(&output).unwrap();
    assert_eq!(bytes.len(), 16); // 2 instructions * 8 bytes
}

#[test]
fn assemble_default_output_name() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("prog.nol");
    fs::write(&input, "HALT\n").unwrap();

    nolang()
        .args(["assemble", input.to_str().unwrap()])
        .assert()
        .success();

    let output = dir.path().join("prog.nolb");
    assert!(output.exists());
}

#[test]
fn assemble_bad_input_exits_1() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("bad.nol");
    fs::write(&input, "FOOBAR\n").unwrap();

    nolang()
        .args(["assemble", input.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("error:"));
}

#[test]
fn assemble_missing_file_exits_1() {
    nolang()
        .args(["assemble", "nonexistent.nol"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("cannot read"));
}

// ---- Verify ----

#[test]
fn verify_valid_program() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_to_temp(&dir, "CONST I64 0x0000 0x002a\nHALT\n");

    nolang()
        .args(["verify", nolb.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("OK:"));
}

#[test]
fn verify_invalid_program_exits_2() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_to_temp(&dir, "ADD\nHALT\n");

    nolang()
        .args(["verify", nolb.to_str().unwrap()])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("error:"));
}

// ---- Run ----

#[test]
fn run_constant_return() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_to_temp(&dir, "CONST I64 0x0000 0x002a\nHALT\n");

    nolang()
        .args(["run", nolb.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("I64(42)"));
}

#[test]
fn run_addition() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_to_temp(
        &dir,
        "CONST I64 0x0000 0x0005\nCONST I64 0x0000 0x0003\nADD\nHALT\n",
    );

    nolang()
        .args(["run", nolb.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains("I64(8)"));
}

#[test]
fn run_invalid_program_exits_2() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_to_temp(&dir, "ADD\nHALT\n");

    nolang()
        .args(["run", nolb.to_str().unwrap()])
        .assert()
        .failure()
        .code(2);
}

// ---- Disassemble ----

#[test]
fn disassemble_roundtrip() {
    let dir = TempDir::new().unwrap();
    let original = "CONST I64 0x0000 0x002a\nHALT\n";
    let nolb = assemble_to_temp(&dir, original);

    nolang()
        .args(["disassemble", nolb.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::eq(original));
}

// ---- Hash ----

#[test]
fn hash_computes_for_func_block() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("func.nol");
    fs::write(
        &input,
        "\
FUNC 1 4
PARAM I64
REF 0
RET
HASH 0x0000 0x0000 0x0000
ENDFUNC
CONST I64 0x0000 0x002a
CALL 0
HALT
",
    )
    .unwrap();

    nolang()
        .args(["hash", input.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("HASH 0x"));
}

// ---- Train ----

#[test]
fn train_generates_json_line() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("test.nol");
    fs::write(&input, "CONST I64 0x0000 0x002a\nHALT\n").unwrap();

    nolang()
        .args([
            "train",
            input.to_str().unwrap(),
            "--intent",
            "Return the constant 42",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"intent\":"))
        .stdout(predicate::str::contains("\"assembly\":"))
        .stdout(predicate::str::contains("\"binary_b64\":"));
}

#[test]
fn train_missing_intent_exits_1() {
    let dir = TempDir::new().unwrap();
    let input = dir.path().join("test.nol");
    fs::write(&input, "CONST I64 0x0000 0x002a\nHALT\n").unwrap();

    nolang()
        .args(["train", input.to_str().unwrap()])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("--intent"));
}

// ---- Full pipeline tests with example files ----

/// Run a test program through: source .nol → assemble → verify → run → check output
fn pipeline_test(nol_file: &str, expected_output: &str) {
    let nol_path = test_program(nol_file);
    let dir = TempDir::new().unwrap();
    let nolb = dir.path().join("out.nolb");

    nolang()
        .args([
            "assemble",
            nol_path.to_str().unwrap(),
            "-o",
            nolb.to_str().unwrap(),
        ])
        .assert()
        .success();

    nolang()
        .args(["verify", nolb.to_str().unwrap()])
        .assert()
        .success();

    nolang()
        .args(["run", nolb.to_str().unwrap()])
        .assert()
        .success()
        .stdout(predicate::str::contains(expected_output));
}

#[test]
fn pipeline_ex01_constant_return() {
    pipeline_test("ex01_constant_return.nol", "I64(42)");
}

#[test]
fn pipeline_ex02_addition() {
    pipeline_test("ex02_addition.nol", "I64(8)");
}

#[test]
fn pipeline_ex03_boolean_match() {
    pipeline_test("ex03_boolean_match.nol", "I64(1)");
}

#[test]
fn pipeline_ex04_simple_function() {
    pipeline_test("ex04_simple_function.nol", "I64(42)");
}

#[test]
fn pipeline_ex05_maybe_type() {
    pipeline_test("ex05_maybe_type.nol", "I64(15)");
}

#[test]
fn pipeline_ex06_recursive_factorial() {
    pipeline_test("ex06_recursive_factorial.nol", "I64(120)");
}

#[test]
fn pipeline_ex07_tuple_projection() {
    pipeline_test("ex07_tuple_projection.nol", "I64(7)");
}

#[test]
fn pipeline_ex08_array_operations() {
    pipeline_test("ex08_array_operations.nol", "I64(20)");
}

#[test]
fn pipeline_ex09_abs() {
    pipeline_test("ex09_abs.nol", "I64(13)");
}

// ---- Disassemble roundtrip for all examples ----

#[test]
fn disassemble_roundtrip_all_examples() {
    let dir = TempDir::new().unwrap();
    let examples = [
        "ex01_constant_return.nol",
        "ex02_addition.nol",
        "ex03_boolean_match.nol",
        "ex07_tuple_projection.nol",
        "ex08_array_operations.nol",
    ];

    for name in &examples {
        let nol_path = test_program(name);
        let original = fs::read_to_string(&nol_path).unwrap();
        let nolb = dir.path().join("round.nolb");

        nolang()
            .args([
                "assemble",
                nol_path.to_str().unwrap(),
                "-o",
                nolb.to_str().unwrap(),
            ])
            .assert()
            .success();

        let output = nolang()
            .args(["disassemble", nolb.to_str().unwrap()])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone();
        let disasm = String::from_utf8(output).unwrap();
        assert_eq!(original, disasm, "roundtrip failed for {name}");
    }
}
