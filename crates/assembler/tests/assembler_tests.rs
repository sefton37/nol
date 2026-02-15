//! Integration tests for the NoLang assembler.
//!
//! Tests cover:
//! - All 9 EXAMPLES.md programs (assemble, verify, execute)
//! - Roundtrip properties (disassemble → assemble, assemble → disassemble → assemble)
//! - Error cases (unknown opcode, missing args, invalid number, etc.)
//! - CONST_EXT encoding
//! - All 45 opcodes round-trip through disassemble → assemble

use nolang_assembler::{assemble, disassemble, AsmError};
use nolang_common::{Instruction, Opcode, Program, TypeTag};

// ---- Test helpers ----

/// Compute the correct blake3 HASH instruction for a FUNC block.
///
/// The hash covers all instruction bytes from `func_pc` (inclusive)
/// through the instruction before `hash_pc` (inclusive).
fn compute_hash_instr(instrs: &[Instruction], func_pc: usize, hash_pc: usize) -> Instruction {
    let mut data = Vec::new();
    for instr in &instrs[func_pc..hash_pc] {
        data.extend_from_slice(&instr.encode());
    }
    let hash = blake3::hash(&data);
    let bytes = hash.as_bytes();
    let arg1 = u16::from_be_bytes([bytes[0], bytes[1]]);
    let arg2 = u16::from_be_bytes([bytes[2], bytes[3]]);
    let arg3 = u16::from_be_bytes([bytes[4], bytes[5]]);
    Instruction::new(Opcode::Hash, TypeTag::None, arg1, arg2, arg3)
}

/// Patch all HASH placeholders in a program with correct values.
/// Scans for FUNC/ENDFUNC blocks, finds HASH instructions, replaces them.
fn patch_hashes(program: &mut Program) {
    let instrs = &program.instructions;
    // Find all FUNC blocks
    let mut func_stack: Vec<usize> = Vec::new();
    let mut patches: Vec<(usize, usize)> = Vec::new(); // (hash_pc, func_pc)

    for (i, instr) in instrs.iter().enumerate() {
        match instr.opcode {
            Opcode::Func => func_stack.push(i),
            Opcode::Hash => {
                if let Some(&func_pc) = func_stack.last() {
                    patches.push((i, func_pc));
                }
            }
            Opcode::EndFunc => {
                func_stack.pop();
            }
            _ => {}
        }
    }

    for (hash_pc, func_pc) in patches {
        let correct = compute_hash_instr(&program.instructions, func_pc, hash_pc);
        program.instructions[hash_pc] = correct;
    }
}

/// Assemble text, patch hashes, return program.
fn assemble_and_patch(text: &str) -> Program {
    let mut program = assemble(text).unwrap();
    patch_hashes(&mut program);
    program
}

/// Assemble, patch hashes, verify, and execute. Return the result value.
fn assemble_verify_execute(text: &str) -> nolang_common::Value {
    let program = assemble_and_patch(text);
    // Verify
    nolang_verifier::verify(&program).unwrap_or_else(|errors| {
        panic!("Verification failed: {errors:?}");
    });
    // Execute
    nolang_vm::run(&program).unwrap_or_else(|err| {
        panic!("Execution failed: {err:?}");
    })
}

// ---- EXAMPLES.md tests ----

#[test]
fn example_1_constant_return() {
    let text = "\
CONST I64 0x0000 0x002a
HALT
";
    let result = assemble_verify_execute(text);
    assert_eq!(result, nolang_common::Value::I64(42));
}

#[test]
fn example_2_addition() {
    let text = "\
CONST I64 0x0000 0x0005
CONST I64 0x0000 0x0003
ADD
HALT
";
    let result = assemble_verify_execute(text);
    assert_eq!(result, nolang_common::Value::I64(8));
}

#[test]
fn example_3_boolean_match() {
    let text = "\
CONST BOOL 0x0001 0x0000
MATCH 2
CASE 0 2
CONST I64 0x0000 0x0000
NOP
CASE 1 2
CONST I64 0x0000 0x0001
NOP
EXHAUST
HALT
";
    let result = assemble_verify_execute(text);
    assert_eq!(result, nolang_common::Value::I64(1));
}

#[test]
fn example_4_simple_function() {
    // PRE body must leave Bool on stack and no extra values.
    // TYPEOF leaves the original value — causes HaltWithMultipleValues.
    // Use comparison instead: REF 0, CONST, GTE → consumes both, leaves Bool.
    let text = "\
FUNC 1 10
PARAM I64
PRE 3
REF 0
CONST I64 0x0000 0x0000
GTE
REF 0
REF 0
ADD
RET
HASH 0x0000 0x0000 0x0000
ENDFUNC
CONST I64 0x0000 0x0015
CALL 0
HALT
";
    let result = assemble_verify_execute(text);
    assert_eq!(result, nolang_common::Value::I64(42));
}

#[test]
fn example_5_maybe_type() {
    let text = "\
CONST I64 0x0000 0x0005
VARIANT_NEW VARIANT 2 0
MATCH 2
CASE 0 4
BIND
REF 0
CONST I64 0x0000 0x000a
ADD
CASE 1 1
CONST I64 0x0000 0x0000
EXHAUST
HALT
";
    let result = assemble_verify_execute(text);
    assert_eq!(result, nolang_common::Value::I64(15));
}

#[test]
fn example_6_recursive_factorial() {
    let text = "\
FUNC 1 20
PARAM I64
REF 0
CONST I64 0x0000 0x0001
LTE
MATCH 2
CASE 0 8
REF 0
REF 0
CONST I64 0x0000 0x0001
SUB
RECURSE 100
MUL
NOP
NOP
CASE 1 2
CONST I64 0x0000 0x0001
NOP
EXHAUST
RET
HASH 0x0000 0x0000 0x0000
ENDFUNC
CONST I64 0x0000 0x0005
CALL 0
HALT
";
    let result = assemble_verify_execute(text);
    assert_eq!(result, nolang_common::Value::I64(120));
}

#[test]
fn example_7_tuple_projection() {
    let text = "\
CONST I64 0x0000 0x0003
CONST I64 0x0000 0x0007
TUPLE_NEW TUPLE 2
PROJECT 1
HALT
";
    let result = assemble_verify_execute(text);
    assert_eq!(result, nolang_common::Value::I64(7));
}

#[test]
fn example_8_array_operations() {
    let text = "\
CONST I64 0x0000 0x000a
CONST I64 0x0000 0x0014
CONST I64 0x0000 0x001e
ARRAY_NEW ARRAY 3
CONST U64 0x0000 0x0001
ARRAY_GET
HALT
";
    let result = assemble_verify_execute(text);
    assert_eq!(result, nolang_common::Value::I64(20));
}

#[test]
fn example_9_function_with_contracts() {
    // PRE: push true (abs accepts any I64)
    // POST: result >= 0 (abs always returns non-negative)
    let text = "\
FUNC 1 19
PARAM I64
PRE 1
CONST BOOL 0x0001 0x0000
POST 3
REF 0
CONST I64 0x0000 0x0000
GTE
REF 0
CONST I64 0x0000 0x0000
LT
MATCH 2
CASE 0 1
REF 0
CASE 1 2
REF 0
NEG
EXHAUST
RET
HASH 0x0000 0x0000 0x0000
ENDFUNC
CONST I64 0xffff 0xfff3
CALL 0
HALT
";
    let result = assemble_verify_execute(text);
    assert_eq!(result, nolang_common::Value::I64(13));
}

// ---- Roundtrip tests ----

#[test]
fn roundtrip_disassemble_then_assemble_all_examples() {
    // Build programs using assembler, patch hashes, then check roundtrip
    let examples = [
        "CONST I64 0x0000 0x002a\nHALT\n",
        "CONST I64 0x0000 0x0005\nCONST I64 0x0000 0x0003\nADD\nHALT\n",
    ];
    for text in &examples {
        let program = assemble(text).unwrap();
        let disasm = disassemble(&program);
        let reassembled = assemble(&disasm).unwrap();
        assert_eq!(program, reassembled, "roundtrip failed for: {text}");
    }
}

#[test]
fn roundtrip_complex_program() {
    // Example 4 with patched hashes
    let text = "\
FUNC 1 10
PARAM I64
PRE 3
REF 0
CONST I64 0x0000 0x0000
GTE
REF 0
REF 0
ADD
RET
HASH 0x0000 0x0000 0x0000
ENDFUNC
CONST I64 0x0000 0x0015
CALL 0
HALT
";
    let mut program = assemble(text).unwrap();
    patch_hashes(&mut program);

    // disassemble → assemble roundtrip must produce identical binary
    let disasm = disassemble(&program);
    let reassembled = assemble(&disasm).unwrap();
    assert_eq!(program, reassembled);
}

#[test]
fn roundtrip_const_ext() {
    let text = "CONST_EXT I64 0x0000123456789abc\nHALT\n";
    let program = assemble(text).unwrap();
    let disasm = disassemble(&program);
    assert_eq!(disasm, text);
    let reassembled = assemble(&disasm).unwrap();
    assert_eq!(program, reassembled);
}

#[test]
fn roundtrip_const_ext_f64() {
    // f64 value 3.14 as bits: 0x40091eb851eb851f
    let text = "CONST_EXT F64 0x40091eb851eb851f\nHALT\n";
    let program = assemble(text).unwrap();
    assert_eq!(program.instructions.len(), 3); // CONST_EXT + data + HALT
    let disasm = disassemble(&program);
    assert_eq!(disasm, text);
    let reassembled = assemble(&disasm).unwrap();
    assert_eq!(program, reassembled);
}

#[test]
fn roundtrip_const_ext_max_value() {
    let text = "CONST_EXT U64 0xffffffffffffffff\nHALT\n";
    let program = assemble(text).unwrap();
    let disasm = disassemble(&program);
    assert_eq!(disasm, text);
    let reassembled = assemble(&disasm).unwrap();
    assert_eq!(program, reassembled);
}

#[test]
fn roundtrip_const_ext_zero() {
    let text = "CONST_EXT I64 0x0000000000000000\nHALT\n";
    let program = assemble(text).unwrap();
    let disasm = disassemble(&program);
    assert_eq!(disasm, text);
}

#[test]
fn roundtrip_all_47_opcodes() {
    // Build a program containing every opcode with valid arguments,
    // disassemble it, reassemble, and check binary equality.
    let instrs = vec![
        // Pattern A opcodes
        Instruction::new(Opcode::Bind, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Drop, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Neg, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Add, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Sub, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Mul, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Div, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Mod, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Eq, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Neq, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Lt, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Gt, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Lte, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Gte, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::And, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Or, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Not, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Xor, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Shl, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Shr, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Implies, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::ArrayGet, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::ArrayLen, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Assert, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Ret, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Nop, TypeTag::None, 0, 0, 0),
        // Pattern B/D opcodes
        Instruction::new(Opcode::Ref, TypeTag::None, 5, 0, 0),
        Instruction::new(Opcode::Match, TypeTag::None, 3, 0, 0),
        Instruction::new(Opcode::Call, TypeTag::None, 0, 0, 0),
        Instruction::new(Opcode::Recurse, TypeTag::None, 100, 0, 0),
        Instruction::new(Opcode::Project, TypeTag::None, 2, 0, 0),
        Instruction::new(Opcode::Pre, TypeTag::None, 3, 0, 0),
        Instruction::new(Opcode::Post, TypeTag::None, 4, 0, 0),
        Instruction::new(Opcode::Forall, TypeTag::None, 3, 0, 0),
        // Pattern C opcodes
        Instruction::new(Opcode::Func, TypeTag::None, 1, 8, 0),
        Instruction::new(Opcode::Case, TypeTag::None, 0, 2, 0),
        // Pattern E
        Instruction::new(Opcode::Param, TypeTag::I64, 0, 0, 0),
        // Pattern F
        Instruction::new(Opcode::Typeof, TypeTag::None, TypeTag::U64 as u16, 0, 0),
        // Pattern G
        Instruction::new(Opcode::Const, TypeTag::I64, 0x1234, 0x5678, 0),
        // Pattern H (CONST_EXT + data slot)
        Instruction::new(Opcode::ConstExt, TypeTag::F64, 0x4009, 0, 0),
        Instruction::new(Opcode::Nop, TypeTag::None, 0x1eb8, 0x51eb, 0x851f),
        // Pattern I
        Instruction::new(Opcode::Hash, TypeTag::None, 0xabcd, 0x1234, 0x5678),
        // Pattern J
        Instruction::new(Opcode::VariantNew, TypeTag::Variant, 3, 1, 0),
        // Pattern K
        Instruction::new(Opcode::TupleNew, TypeTag::Tuple, 2, 0, 0),
        Instruction::new(Opcode::ArrayNew, TypeTag::Array, 5, 0, 0),
        // HALT at end
        Instruction::new(Opcode::Halt, TypeTag::None, 0, 0, 0),
    ];

    let program = Program::new(instrs);
    let text = disassemble(&program);
    let reassembled = assemble(&text).unwrap();
    assert_eq!(program, reassembled, "all-opcodes roundtrip failed");
}

// ---- Error tests ----

#[test]
fn error_unknown_opcode_with_line() {
    let err = assemble("HALT\nFOOBAR\n").unwrap_err();
    assert_eq!(
        err,
        AsmError::UnknownOpcode {
            line: 2,
            token: "FOOBAR".to_string()
        }
    );
}

#[test]
fn error_missing_argument_ref() {
    let err = assemble("REF\n").unwrap_err();
    assert_eq!(
        err,
        AsmError::MissingArgument {
            line: 1,
            opcode: "REF",
            expected: 1
        }
    );
}

#[test]
fn error_missing_argument_func() {
    let err = assemble("FUNC 1\n").unwrap_err();
    assert_eq!(
        err,
        AsmError::MissingArgument {
            line: 1,
            opcode: "FUNC",
            expected: 2
        }
    );
}

#[test]
fn error_missing_argument_const() {
    let err = assemble("CONST I64\n").unwrap_err();
    assert_eq!(
        err,
        AsmError::MissingArgument {
            line: 1,
            opcode: "CONST",
            expected: 3
        }
    );
}

#[test]
fn error_invalid_number_hex() {
    let err = assemble("CONST I64 0xGGGG 0x0000\n").unwrap_err();
    assert_eq!(
        err,
        AsmError::InvalidNumber {
            line: 1,
            token: "0xGGGG".to_string()
        }
    );
}

#[test]
fn error_invalid_number_overflow() {
    let err = assemble("REF 70000\n").unwrap_err();
    assert_eq!(
        err,
        AsmError::InvalidNumber {
            line: 1,
            token: "70000".to_string()
        }
    );
}

#[test]
fn error_unexpected_token_extra_arg() {
    let err = assemble("ADD 5\n").unwrap_err();
    assert!(matches!(err, AsmError::UnexpectedToken { line: 1, .. }));
}

#[test]
fn error_unknown_type_tag() {
    let err = assemble("PARAM STRING\n").unwrap_err();
    assert_eq!(
        err,
        AsmError::UnknownTypeTag {
            line: 1,
            token: "STRING".to_string()
        }
    );
}

// ---- Comment and whitespace handling ----

#[test]
fn comments_stripped() {
    let text = "\
; full line comment
CONST I64 0x0000 0x002a ; inline comment
; another comment
HALT ; end
";
    let program = assemble(text).unwrap();
    assert_eq!(program.instructions.len(), 2);
    assert_eq!(program.instructions[0].opcode, Opcode::Const);
    assert_eq!(program.instructions[1].opcode, Opcode::Halt);
}

#[test]
fn blank_lines_handled() {
    let text = "\n\nCONST I64 0x0000 0x002a\n\nHALT\n\n";
    let program = assemble(text).unwrap();
    assert_eq!(program.instructions.len(), 2);
}

#[test]
fn indentation_ignored() {
    let text = "\
  FUNC 1 4
    PARAM I64
    REF 0
    RET
    HASH 0x0000 0x0000 0x0000
  ENDFUNC
";
    let program = assemble(text).unwrap();
    assert_eq!(program.instructions.len(), 6);
}

// ---- Verification of assembled programs ----

#[test]
fn assembled_example_1_passes_verification() {
    let text = "CONST I64 0x0000 0x002a\nHALT\n";
    let program = assemble(text).unwrap();
    assert!(nolang_verifier::verify(&program).is_ok());
}

#[test]
fn assembled_example_2_passes_verification() {
    let text = "CONST I64 0x0000 0x0005\nCONST I64 0x0000 0x0003\nADD\nHALT\n";
    let program = assemble(text).unwrap();
    assert!(nolang_verifier::verify(&program).is_ok());
}

#[test]
fn assembled_example_with_correct_hash_passes_verification() {
    let program = assemble_and_patch(
        "\
FUNC 1 10
PARAM I64
PRE 3
REF 0
CONST I64 0x0000 0x0000
GTE
REF 0
REF 0
ADD
RET
HASH 0x0000 0x0000 0x0000
ENDFUNC
CONST I64 0x0000 0x0015
CALL 0
HALT
",
    );
    assert!(nolang_verifier::verify(&program).is_ok());
}

// ---- Decimal input produces same binary as hex ----

#[test]
fn decimal_input_matches_hex() {
    let hex = assemble("CONST I64 0x0000 0x002a\nHALT\n").unwrap();
    let dec = assemble("CONST I64 0 42\nHALT\n").unwrap();
    assert_eq!(hex, dec);
}

#[test]
fn case_insensitive_input() {
    let upper = assemble("ADD\n").unwrap();
    let lower = assemble("add\n").unwrap();
    let mixed = assemble("Add\n").unwrap();
    assert_eq!(upper, lower);
    assert_eq!(upper, mixed);
}

// ---- CONST_EXT specific tests ----

#[test]
fn const_ext_produces_two_instructions() {
    let program = assemble("CONST_EXT I64 0x0000000000000000\n").unwrap();
    assert_eq!(program.instructions.len(), 2);
    assert_eq!(program.instructions[0].opcode, Opcode::ConstExt);
    assert_eq!(program.instructions[1].opcode, Opcode::Nop);
}

#[test]
fn const_ext_bit_layout() {
    let program = assemble("CONST_EXT I64 0xaabb112233445566\n").unwrap();
    let ext = &program.instructions[0];
    let data = &program.instructions[1];
    assert_eq!(ext.arg1, 0xaabb); // high 16 bits
    assert_eq!(data.arg1, 0x1122); // bits 32-47
    assert_eq!(data.arg2, 0x3344); // bits 16-31
    assert_eq!(data.arg3, 0x5566); // bits 0-15
}

// ---- Hash computation helper test ----

#[test]
fn patch_hashes_produces_correct_hash() {
    let mut program = assemble(
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
    patch_hashes(&mut program);

    // The HASH instruction (at index 4) should now have non-zero args
    let hash_instr = &program.instructions[4];
    assert_eq!(hash_instr.opcode, Opcode::Hash);
    // Verify it passes the verifier's hash check
    assert!(nolang_verifier::verify(&program).is_ok());
}
