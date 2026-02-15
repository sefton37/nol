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

#[test]
fn pipeline_ex10_abs_rich() {
    pipeline_test("ex10_abs_rich.nol", "I64(13)");
}

#[test]
fn pipeline_ex11_max() {
    pipeline_test("ex11_max.nol", "I64(5)");
}

#[test]
fn pipeline_ex12_clamp() {
    pipeline_test("ex12_clamp.nol", "I64(5)");
}

#[test]
fn pipeline_ex13_sign() {
    pipeline_test("ex13_sign.nol", "I64(1)");
}

#[test]
fn pipeline_ex14_min() {
    pipeline_test("ex14_min.nol", "I64(3)");
}

#[test]
fn pipeline_ex15_is_positive() {
    pipeline_test("ex15_is_positive.nol", "Bool(true)");
}

#[test]
fn pipeline_ex16_all_positive() {
    pipeline_test("ex16_all_positive.nol", "Bool(true)");
}

#[test]
fn pipeline_ex17_forall_bounded() {
    pipeline_test("ex17_forall_bounded.nol", "Bool(true)");
}

#[test]
fn pipeline_ex18_forall_empty() {
    pipeline_test("ex18_forall_empty.nol", "Bool(true)");
}

#[test]
fn pipeline_ex19_forall_fails() {
    pipeline_test("ex19_forall_fails.nol", "Bool(false)");
}

// ---- Witness ----

/// Helper: assemble a .nol file, returning path to .nolb
fn assemble_program(dir: &TempDir, nol_file: &str) -> PathBuf {
    let nol_path = test_program(nol_file);
    let nolb = dir.path().join("prog.nolb");
    nolang()
        .args([
            "assemble",
            nol_path.to_str().unwrap(),
            "-o",
            nolb.to_str().unwrap(),
        ])
        .assert()
        .success();
    nolb
}

/// Return the absolute path to a witness file.
fn test_witness(name: &str) -> PathBuf {
    workspace_root().join("tests/witnesses").join(name)
}

#[test]
fn witness_ex04_simple_function() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex04_simple_function.nol");
    let witness_path = test_witness("ex04_simple_function.json");

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("4/4 witnesses passed"));
}

#[test]
fn witness_ex06_recursive_factorial() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex06_recursive_factorial.nol");
    let witness_path = test_witness("ex06_recursive_factorial.json");

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("4/4 witnesses passed"));
}

#[test]
fn witness_ex09_abs() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex09_abs.nol");
    let witness_path = test_witness("ex09_abs.json");

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("4/4 witnesses passed"));
}

#[test]
fn witness_ex10_abs_rich() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex10_abs_rich.nol");
    let witness_path = test_witness("ex10_abs_rich.json");

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("4/4 witnesses passed"));
}

#[test]
fn witness_ex11_max() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex11_max.nol");
    let witness_path = test_witness("ex11_max.json");

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("4/4 witnesses passed"));
}

#[test]
fn witness_ex12_clamp() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex12_clamp.nol");
    let witness_path = test_witness("ex12_clamp.json");

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("5/5 witnesses passed"));
}

#[test]
fn witness_ex13_sign() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex13_sign.nol");
    let witness_path = test_witness("ex13_sign.json");

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("5/5 witnesses passed"));
}

#[test]
fn witness_ex14_min() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex14_min.nol");
    let witness_path = test_witness("ex14_min.json");

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("4/4 witnesses passed"));
}

#[test]
fn witness_ex15_is_positive() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex15_is_positive.nol");
    let witness_path = test_witness("ex15_is_positive.json");

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("4/4 witnesses passed"));
}

#[test]
fn witness_failure_exits_3() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex09_abs.nol");
    let bad_witness = dir.path().join("bad.json");
    fs::write(&bad_witness, r#"[{"input": [5], "expected": 999}]"#).unwrap();

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            bad_witness.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(3)
        .stdout(predicate::str::contains("FAIL witness 0"))
        .stdout(predicate::str::contains("0/1 witnesses passed"));
}

#[test]
fn witness_invalid_json_exits_1() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex09_abs.nol");
    let bad_json = dir.path().join("bad.json");
    fs::write(&bad_json, "not json").unwrap();

    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            bad_json.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn witness_missing_file_exits_1() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex09_abs.nol");

    nolang()
        .args(["witness", nolb.to_str().unwrap(), "nonexistent.json"])
        .assert()
        .failure()
        .code(1);
}

#[test]
fn witness_no_args_exits_1() {
    nolang()
        .arg("witness")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("Usage:"));
}

#[test]
fn witness_with_func_flag() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex09_abs.nol");
    let witness_path = test_witness("ex09_abs.json");

    // --func 0 is the default, should work
    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
            "--func",
            "0",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("4/4 witnesses passed"));
}

#[test]
fn witness_invalid_func_index_exits_1() {
    let dir = TempDir::new().unwrap();
    let nolb = assemble_program(&dir, "ex09_abs.nol");
    let witness_path = test_witness("ex09_abs.json");

    // Function 99 doesn't exist
    nolang()
        .args([
            "witness",
            nolb.to_str().unwrap(),
            witness_path.to_str().unwrap(),
            "--func",
            "99",
        ])
        .assert()
        .failure()
        .code(1);
}

// ---- Train with witnesses ----

#[test]
fn train_with_witnesses() {
    let witness_path = test_witness("ex09_abs.json");
    let nol_path = test_program("ex09_abs.nol");

    nolang()
        .args([
            "train",
            nol_path.to_str().unwrap(),
            "--intent",
            "Compute absolute value",
            "--witnesses",
            witness_path.to_str().unwrap(),
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("\"witnesses\":"));
}

#[test]
fn train_without_witnesses_still_works() {
    let nol_path = test_program("ex09_abs.nol");

    let output = nolang()
        .args([
            "train",
            nol_path.to_str().unwrap(),
            "--intent",
            "Compute absolute value",
        ])
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();

    let stdout = String::from_utf8(output).unwrap();
    assert!(
        !stdout.contains("\"witnesses\":"),
        "should not contain witnesses field"
    );
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
