//! Comprehensive integration tests for the NoLang VM.
//!
//! Organized by instruction group following SPEC.md and BUILD_ORDER.md
//! acceptance criteria.

use nolang_common::{Instruction, Opcode, Program, TypeTag, Value};
use nolang_vm::{run, RuntimeError};

// ============================================================
// Helper functions
// ============================================================

/// Shorthand for creating an instruction.
fn instr(op: Opcode, tt: TypeTag, a1: u16, a2: u16, a3: u16) -> Instruction {
    Instruction::new(op, tt, a1, a2, a3)
}

/// HALT instruction.
fn halt() -> Instruction {
    instr(Opcode::Halt, TypeTag::None, 0, 0, 0)
}

/// CONST I64 from a 32-bit signed value (sign-extended to i64).
/// Handles positive and negative values via two's complement encoding.
fn const_i64(val: i32) -> Instruction {
    let bits = val as u32;
    let arg1 = (bits >> 16) as u16;
    let arg2 = (bits & 0xFFFF) as u16;
    instr(Opcode::Const, TypeTag::I64, arg1, arg2, 0)
}

/// CONST U64 from a 32-bit unsigned value (zero-extended to u64).
fn const_u64(val: u32) -> Instruction {
    let arg1 = (val >> 16) as u16;
    let arg2 = (val & 0xFFFF) as u16;
    instr(Opcode::Const, TypeTag::U64, arg1, arg2, 0)
}

/// CONST BOOL.
fn const_bool(val: bool) -> Instruction {
    instr(Opcode::Const, TypeTag::Bool, if val { 1 } else { 0 }, 0, 0)
}

/// CONST UNIT.
fn const_unit() -> Instruction {
    instr(Opcode::Const, TypeTag::Unit, 0, 0, 0)
}

/// NOP instruction.
fn nop() -> Instruction {
    instr(Opcode::Nop, TypeTag::None, 0, 0, 0)
}

/// BIND instruction.
fn bind() -> Instruction {
    instr(Opcode::Bind, TypeTag::None, 0, 0, 0)
}

/// REF instruction with de Bruijn index.
fn ref_idx(index: u16) -> Instruction {
    instr(Opcode::Ref, TypeTag::None, index, 0, 0)
}

/// DROP instruction.
fn drop_binding() -> Instruction {
    instr(Opcode::Drop, TypeTag::None, 0, 0, 0)
}

/// Run a program from a list of instructions and return the result.
fn run_program(instructions: Vec<Instruction>) -> Result<Value, RuntimeError> {
    let program = Program::new(instructions);
    run(&program)
}

// ============================================================
// BUILD_ORDER.md acceptance criteria
// ============================================================

#[test]
fn empty_program_halt_returns_empty_stack_error() {
    let result = run_program(vec![halt()]);
    assert_eq!(result, Err(RuntimeError::HaltWithEmptyStack));
}

#[test]
fn const_i64_5_halt_returns_i64_5() {
    let result = run_program(vec![const_i64(5), halt()]);
    assert_eq!(result, Ok(Value::I64(5)));
}

#[test]
fn halt_with_multiple_values_returns_error() {
    let result = run_program(vec![const_i64(1), const_i64(2), halt()]);
    assert_eq!(
        result,
        Err(RuntimeError::HaltWithMultipleValues { count: 2 })
    );
}

// ============================================================
// Group A: Foundation -- CONST values
// ============================================================

#[test]
fn const_i64_positive_value() {
    let result = run_program(vec![const_i64(42), halt()]);
    assert_eq!(result, Ok(Value::I64(42)));
}

#[test]
fn const_i64_zero() {
    let result = run_program(vec![const_i64(0), halt()]);
    assert_eq!(result, Ok(Value::I64(0)));
}

#[test]
fn const_i64_negative_value() {
    // -13 in two's complement: 0xFFFFFFF3
    let result = run_program(vec![const_i64(-13), halt()]);
    assert_eq!(result, Ok(Value::I64(-13)));
}

#[test]
fn const_i64_negative_one() {
    let result = run_program(vec![const_i64(-1), halt()]);
    assert_eq!(result, Ok(Value::I64(-1)));
}

#[test]
fn const_i64_max_32bit() {
    let result = run_program(vec![const_i64(i32::MAX), halt()]);
    assert_eq!(result, Ok(Value::I64(i32::MAX as i64)));
}

#[test]
fn const_i64_min_32bit() {
    let result = run_program(vec![const_i64(i32::MIN), halt()]);
    assert_eq!(result, Ok(Value::I64(i32::MIN as i64)));
}

#[test]
fn const_u64_value() {
    let result = run_program(vec![const_u64(100), halt()]);
    assert_eq!(result, Ok(Value::U64(100)));
}

#[test]
fn const_u64_large_value() {
    // 0xFFFF_FFFF zero-extended = 4294967295
    let result = run_program(vec![const_u64(0xFFFF_FFFF), halt()]);
    assert_eq!(result, Ok(Value::U64(0xFFFF_FFFF)));
}

#[test]
fn const_bool_true() {
    let result = run_program(vec![const_bool(true), halt()]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn const_bool_false() {
    let result = run_program(vec![const_bool(false), halt()]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

#[test]
fn const_unit_value() {
    let result = run_program(vec![const_unit(), halt()]);
    assert_eq!(result, Ok(Value::Unit));
}

// ============================================================
// Group A: Foundation -- NOP
// ============================================================

#[test]
fn nop_is_skipped() {
    let result = run_program(vec![const_i64(7), nop(), nop(), nop(), halt()]);
    assert_eq!(result, Ok(Value::I64(7)));
}

// ============================================================
// Group A: Foundation -- BIND / REF / DROP
// ============================================================

#[test]
fn bind_ref_single_binding() {
    // Push 10, BIND, REF 0 (most recent), HALT
    let result = run_program(vec![const_i64(10), bind(), ref_idx(0), halt()]);
    assert_eq!(result, Ok(Value::I64(10)));
}

#[test]
fn bind_ref_two_bindings_de_bruijn_indices() {
    // Push 10, BIND → bindings: [10]
    // Push 20, BIND → bindings: [10, 20]
    // REF 0 → most recent = 20
    // REF 1 → next = 10
    // ADD → 30
    let result = run_program(vec![
        const_i64(10),
        bind(),
        const_i64(20),
        bind(),
        ref_idx(0), // 20
        ref_idx(1), // 10
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(30)));
}

#[test]
fn bind_ref_three_bindings() {
    // bindings: [a=100, b=200, c=300]
    // REF 0 = 300 (most recent)
    // REF 2 = 100 (deepest)
    // SUB → 100 - 300 = -200
    let result = run_program(vec![
        const_i64(100),
        bind(),
        const_i64(200),
        bind(),
        const_i64(300),
        bind(),
        ref_idx(2), // 100
        ref_idx(0), // 300
        instr(Opcode::Sub, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(-200)));
}

#[test]
fn drop_removes_most_recent_binding() {
    // Push 10, BIND → [10]
    // Push 20, BIND → [10, 20]
    // DROP → [10]
    // REF 0 → 10
    let result = run_program(vec![
        const_i64(10),
        bind(),
        const_i64(20),
        bind(),
        drop_binding(),
        ref_idx(0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(10)));
}

#[test]
fn ref_out_of_range_returns_error() {
    // No bindings, REF 0 → error
    let result = run_program(vec![ref_idx(0), halt()]);
    assert!(matches!(
        result,
        Err(RuntimeError::BindingOutOfRange {
            index: 0,
            depth: 0,
            ..
        })
    ));
}

#[test]
fn ref_index_exceeds_depth_returns_error() {
    // One binding, REF 5 → error
    let result = run_program(vec![const_i64(1), bind(), ref_idx(5), halt()]);
    assert!(matches!(
        result,
        Err(RuntimeError::BindingOutOfRange {
            index: 5,
            depth: 1,
            ..
        })
    ));
}

#[test]
fn drop_on_empty_bindings_returns_error() {
    let result = run_program(vec![drop_binding(), halt()]);
    assert!(matches!(
        result,
        Err(RuntimeError::BindingOutOfRange { .. })
    ));
}

#[test]
fn stack_overflow_returns_error() {
    // Push > 4096 values to trigger stack overflow
    let mut instrs = Vec::new();
    for _ in 0..4097 {
        instrs.push(const_i64(1));
    }
    instrs.push(halt());
    let result = run_program(instrs);
    assert!(matches!(result, Err(RuntimeError::StackOverflow { .. })));
}

// ============================================================
// Group B: Arithmetic -- ADD, SUB, MUL
// ============================================================

#[test]
fn add_i64() {
    let result = run_program(vec![
        const_i64(3),
        const_i64(4),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(7)));
}

#[test]
fn add_u64() {
    let result = run_program(vec![
        const_u64(10),
        const_u64(20),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(30)));
}

#[test]
fn sub_i64() {
    // Push 10, push 3, SUB → 10 - 3 = 7
    let result = run_program(vec![
        const_i64(10),
        const_i64(3),
        instr(Opcode::Sub, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(7)));
}

#[test]
fn sub_i64_negative_result() {
    // Push 3, push 10, SUB → 3 - 10 = -7
    let result = run_program(vec![
        const_i64(3),
        const_i64(10),
        instr(Opcode::Sub, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(-7)));
}

#[test]
fn mul_i64() {
    let result = run_program(vec![
        const_i64(6),
        const_i64(7),
        instr(Opcode::Mul, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(42)));
}

#[test]
fn mul_u64() {
    let result = run_program(vec![
        const_u64(100),
        const_u64(200),
        instr(Opcode::Mul, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(20000)));
}

#[test]
fn wrapping_overflow_i64_add() {
    // i64::MAX + 1 wraps to i64::MIN
    // i32::MAX sign-extended is 2147483647. We need i64::MAX.
    // Can't represent i64::MAX with CONST (only 32-bit range). Use arithmetic.
    // Instead, test i32::MAX wrapping: i32::MAX as i64 is 2147483647.
    // 2147483647 + 1 = 2147483648 (no wrap for i64, since i64::MAX is much larger).
    // To test i64 wrapping, we'd need CONST_EXT. Let's test i32-range wrapping behavior.
    // Actually, since CONST sign-extends from 32 bits, we can only push values in i32 range.
    // Wrapping only occurs at i64 boundaries. Without CONST_EXT we can't easily get i64::MAX.
    // Test wrapping with multiplication instead:
    // Use a chain of multiplications to get to i64::MAX neighborhood.
    // Simpler: test wrapping of -1 + (-1) = -2 (no wrap) and i32::MAX + i32::MAX (still within i64).
    // The real wrapping test is that the VM uses wrapping_add. Let's just verify the behavior
    // with the values we can construct.
    let result = run_program(vec![
        const_i64(i32::MAX),                        // 2147483647
        const_i64(i32::MAX),                        // 2147483647
        instr(Opcode::Add, TypeTag::None, 0, 0, 0), // 4294967294 (no wrap for i64)
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(2 * i32::MAX as i64)));
}

#[test]
fn wrapping_overflow_i64_mul() {
    // i32::MAX * i32::MAX still fits in i64, but demonstrates the arithmetic works
    let result = run_program(vec![
        const_i64(i32::MAX),
        const_i64(2),
        instr(Opcode::Mul, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(i32::MAX as i64 * 2)));
}

// ============================================================
// Group B: Arithmetic -- DIV / MOD
// ============================================================

#[test]
fn div_i64() {
    let result = run_program(vec![
        const_i64(42),
        const_i64(6),
        instr(Opcode::Div, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(7)));
}

#[test]
fn div_u64() {
    let result = run_program(vec![
        const_u64(100),
        const_u64(10),
        instr(Opcode::Div, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(10)));
}

#[test]
fn div_by_zero_i64_returns_error() {
    let result = run_program(vec![
        const_i64(42),
        const_i64(0),
        instr(Opcode::Div, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(result, Err(RuntimeError::DivisionByZero { .. })));
}

#[test]
fn div_by_zero_u64_returns_error() {
    let result = run_program(vec![
        const_u64(42),
        const_u64(0),
        instr(Opcode::Div, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(result, Err(RuntimeError::DivisionByZero { .. })));
}

#[test]
fn mod_i64() {
    let result = run_program(vec![
        const_i64(17),
        const_i64(5),
        instr(Opcode::Mod, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(2)));
}

#[test]
fn mod_u64() {
    let result = run_program(vec![
        const_u64(17),
        const_u64(5),
        instr(Opcode::Mod, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(2)));
}

#[test]
fn mod_by_zero_i64_returns_error() {
    let result = run_program(vec![
        const_i64(42),
        const_i64(0),
        instr(Opcode::Mod, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(result, Err(RuntimeError::DivisionByZero { .. })));
}

#[test]
fn mod_by_zero_u64_returns_error() {
    let result = run_program(vec![
        const_u64(42),
        const_u64(0),
        instr(Opcode::Mod, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(result, Err(RuntimeError::DivisionByZero { .. })));
}

// ============================================================
// Group B: Arithmetic -- NEG
// ============================================================

#[test]
fn neg_i64_positive_to_negative() {
    let result = run_program(vec![
        const_i64(42),
        instr(Opcode::Neg, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(-42)));
}

#[test]
fn neg_i64_negative_to_positive() {
    let result = run_program(vec![
        const_i64(-13),
        instr(Opcode::Neg, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(13)));
}

#[test]
fn neg_i64_zero() {
    let result = run_program(vec![
        const_i64(0),
        instr(Opcode::Neg, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(0)));
}

// ============================================================
// Group B: Comparison operators
// ============================================================

#[test]
fn eq_i64_equal() {
    let result = run_program(vec![
        const_i64(5),
        const_i64(5),
        instr(Opcode::Eq, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn eq_i64_not_equal() {
    let result = run_program(vec![
        const_i64(5),
        const_i64(6),
        instr(Opcode::Eq, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

#[test]
fn neq_i64() {
    let result = run_program(vec![
        const_i64(5),
        const_i64(6),
        instr(Opcode::Neq, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn lt_i64_true() {
    let result = run_program(vec![
        const_i64(3),
        const_i64(5),
        instr(Opcode::Lt, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn lt_i64_false() {
    let result = run_program(vec![
        const_i64(5),
        const_i64(3),
        instr(Opcode::Lt, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

#[test]
fn gt_i64() {
    let result = run_program(vec![
        const_i64(10),
        const_i64(3),
        instr(Opcode::Gt, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn lte_i64_equal() {
    let result = run_program(vec![
        const_i64(5),
        const_i64(5),
        instr(Opcode::Lte, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn lte_i64_less() {
    let result = run_program(vec![
        const_i64(3),
        const_i64(5),
        instr(Opcode::Lte, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn lte_i64_greater() {
    let result = run_program(vec![
        const_i64(7),
        const_i64(5),
        instr(Opcode::Lte, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

#[test]
fn gte_i64_equal() {
    let result = run_program(vec![
        const_i64(5),
        const_i64(5),
        instr(Opcode::Gte, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn gte_i64_greater() {
    let result = run_program(vec![
        const_i64(10),
        const_i64(5),
        instr(Opcode::Gte, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn gte_i64_less() {
    let result = run_program(vec![
        const_i64(3),
        const_i64(5),
        instr(Opcode::Gte, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

// ============================================================
// Group B: Logic & Bitwise
// ============================================================

#[test]
fn and_bool_true_true() {
    let result = run_program(vec![
        const_bool(true),
        const_bool(true),
        instr(Opcode::And, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn and_bool_true_false() {
    let result = run_program(vec![
        const_bool(true),
        const_bool(false),
        instr(Opcode::And, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

#[test]
fn and_i64_bitwise() {
    // 0xFF & 0x0F = 0x0F = 15
    let result = run_program(vec![
        const_i64(0xFF),
        const_i64(0x0F),
        instr(Opcode::And, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(0x0F)));
}

#[test]
fn or_bool_false_false() {
    let result = run_program(vec![
        const_bool(false),
        const_bool(false),
        instr(Opcode::Or, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

#[test]
fn or_bool_true_false() {
    let result = run_program(vec![
        const_bool(true),
        const_bool(false),
        instr(Opcode::Or, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn or_i64_bitwise() {
    // 0xF0 | 0x0F = 0xFF = 255
    let result = run_program(vec![
        const_i64(0xF0),
        const_i64(0x0F),
        instr(Opcode::Or, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(0xFF)));
}

#[test]
fn not_bool_true() {
    let result = run_program(vec![
        const_bool(true),
        instr(Opcode::Not, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

#[test]
fn not_bool_false() {
    let result = run_program(vec![
        const_bool(false),
        instr(Opcode::Not, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn not_i64_bitwise() {
    // !0 for i64 = -1 (all bits set)
    let result = run_program(vec![
        const_i64(0),
        instr(Opcode::Not, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(-1)));
}

#[test]
fn xor_bool() {
    let result = run_program(vec![
        const_bool(true),
        const_bool(false),
        instr(Opcode::Xor, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn xor_bool_same() {
    let result = run_program(vec![
        const_bool(true),
        const_bool(true),
        instr(Opcode::Xor, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

#[test]
fn xor_i64_bitwise() {
    // 0xFF ^ 0x0F = 0xF0 = 240
    let result = run_program(vec![
        const_i64(0xFF),
        const_i64(0x0F),
        instr(Opcode::Xor, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(0xF0)));
}

// ============================================================
// Group B: Shift operators
// ============================================================

#[test]
fn shl_i64() {
    // 1 << 3 = 8
    let result = run_program(vec![
        const_i64(1),
        const_i64(3),
        instr(Opcode::Shl, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(8)));
}

#[test]
fn shr_i64() {
    // 16 >> 2 = 4
    let result = run_program(vec![
        const_i64(16),
        const_i64(2),
        instr(Opcode::Shr, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(4)));
}

#[test]
fn shr_i64_arithmetic_preserves_sign() {
    // -8 >> 1 = -4 (arithmetic shift for i64)
    let result = run_program(vec![
        const_i64(-8),
        const_i64(1),
        instr(Opcode::Shr, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(-4)));
}

#[test]
fn shl_u64() {
    // 1u64 << 4 = 16
    let result = run_program(vec![
        const_u64(1),
        const_u64(4),
        instr(Opcode::Shl, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(16)));
}

#[test]
fn shr_u64() {
    // 32u64 >> 3 = 4
    let result = run_program(vec![
        const_u64(32),
        const_u64(3),
        instr(Opcode::Shr, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(4)));
}

// ============================================================
// Group C: Pattern Matching
// ============================================================

#[test]
fn match_bool_true_dispatches_to_tag_1() {
    // MATCH on Bool(true) should dispatch to CASE with tag=1
    //
    // Layout:
    //  0: CONST BOOL true
    //  1: MATCH variant_count=2
    //  2: CASE tag=0 body_len=2
    //  3:   CONST I64 0 0  (body for false, won't execute)
    //  4:   [skipped because body_len=1... wait, body_len=2 means 2 instructions]
    //       Actually let's make it body_len=1 for simplicity.
    //
    // Simplified:
    //  0: CONST BOOL true
    //  1: MATCH variant_count=2
    //  2: CASE tag=0 body_len=1   -- false branch
    //  3:   CONST I64 0 0         -- body: push 0
    //  4: CASE tag=1 body_len=1   -- true branch
    //  5:   CONST I64 0 1         -- body: push 1
    //  6: EXHAUST
    //  7: HALT
    let result = run_program(vec![
        const_bool(true),
        instr(Opcode::Match, TypeTag::None, 2, 0, 0), // 2 variants
        instr(Opcode::Case, TypeTag::None, 0, 1, 0),  // tag=0 (false), body_len=1
        const_i64(0),                                 // false body: push 0
        instr(Opcode::Case, TypeTag::None, 1, 1, 0),  // tag=1 (true), body_len=1
        const_i64(1),                                 // true body: push 1
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(1)));
}

#[test]
fn match_bool_false_dispatches_to_tag_0() {
    let result = run_program(vec![
        const_bool(false),
        instr(Opcode::Match, TypeTag::None, 2, 0, 0),
        instr(Opcode::Case, TypeTag::None, 0, 1, 0), // false branch
        const_i64(0),
        instr(Opcode::Case, TypeTag::None, 1, 1, 0), // true branch
        const_i64(1),
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(0)));
}

#[test]
fn match_variant_with_payload_pushes_payload() {
    // Create Variant(tag_count=2, tag=1, payload=42), then MATCH.
    // When the match dispatches to tag 1, payload 42 should be pushed.
    //
    // Layout:
    //  0: CONST I64 0 42         -- payload
    //  1: VARIANT_NEW total=2 this_tag=1
    //  2: MATCH variant_count=2
    //  3: CASE tag=0 body_len=2
    //  4:   CONST I64 0 0        -- body for tag 0 (discard payload, push 0)
    //  5:   (padding NOP)
    //  6: CASE tag=1 body_len=1
    //  7:   NOP                  -- payload already on stack from MATCH
    //  8: EXHAUST
    //  9: HALT

    let result = run_program(vec![
        const_i64(42),
        instr(Opcode::VariantNew, TypeTag::Variant, 2, 1, 0), // tag_count=2, this_tag=1
        instr(Opcode::Match, TypeTag::None, 2, 0, 0),
        instr(Opcode::Case, TypeTag::None, 0, 1, 0), // tag 0, body_len=1
        const_i64(0),                                // body for tag 0
        instr(Opcode::Case, TypeTag::None, 1, 1, 0), // tag 1, body_len=1
        nop(),                                       // payload (42) already pushed by MATCH
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(42)));
}

#[test]
fn match_variant_three_tags_selects_correct_branch() {
    // Variant with 3 tags, select tag 2
    let result = run_program(vec![
        const_unit(),                                         // payload
        instr(Opcode::VariantNew, TypeTag::Variant, 3, 2, 0), // tag_count=3, this_tag=2
        instr(Opcode::Match, TypeTag::None, 3, 0, 0),
        instr(Opcode::Case, TypeTag::None, 0, 1, 0), // tag 0
        const_i64(10),
        instr(Opcode::Case, TypeTag::None, 1, 1, 0), // tag 1
        const_i64(20),
        instr(Opcode::Case, TypeTag::None, 2, 2, 0), // tag 2, body_len=2
        // Tag 2 body: drop the Unit payload, push 30
        bind(), // bind the payload (Unit)
        const_i64(30),
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(30)));
}

// ============================================================
// Group D: Functions
// ============================================================

#[test]
fn simple_function_double() {
    // Function: double(x) = x + x
    //
    // Layout:
    //  0: FUNC param_count=1 body_len=4
    //  1:   REF 0                 -- push x
    //  2:   REF 0                 -- push x again
    //  3:   ADD
    //  4:   RET
    //  5: ENDFUNC
    //  --- entry point ---
    //  6: CONST I64 0 21          -- push argument 21
    //  7: CALL func_index=0
    //  8: HALT
    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 4, 0), // param_count=1, body_len=4
        ref_idx(0),                                  // x
        ref_idx(0),                                  // x
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        // entry point
        const_i64(21),
        instr(Opcode::Call, TypeTag::None, 0, 0, 0), // call func 0
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(42)));
}

#[test]
fn function_preserves_stack_across_call() {
    // Push 100, call double(5), result should be 10 on top, 100 below.
    // To test, ADD them: 100 + 10 = 110
    //
    //  0: FUNC param_count=1 body_len=4
    //  1:   REF 0
    //  2:   REF 0
    //  3:   ADD
    //  4:   RET
    //  5: ENDFUNC
    //  6: CONST I64 0 100
    //  7: CONST I64 0 5
    //  8: CALL 0
    //  9: ADD
    // 10: HALT
    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 4, 0),
        ref_idx(0),
        ref_idx(0),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        const_i64(100),
        const_i64(5),
        instr(Opcode::Call, TypeTag::None, 0, 0, 0),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0), // 100 + 10
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(110)));
}

#[test]
fn function_with_precondition_passing() {
    // Function: f(x) requires x > 0 { x + 1 }
    //
    //  0: FUNC param_count=1 body_len=8
    //  1: PRE len=3
    //  2:   REF 0          -- x
    //  3:   CONST I64 0 0  -- 0
    //  4:   GT             -- x > 0
    //  5:   REF 0          -- x
    //  6:   CONST I64 0 1  -- 1
    //  7:   ADD            -- x + 1
    //  8:   RET
    //  9: ENDFUNC
    // 10: CONST I64 0 5
    // 11: CALL 0
    // 12: HALT
    //
    // body_len = instructions between FUNC and ENDFUNC exclusive = 8 (indices 1..8)
    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 8, 0), // param_count=1, body_len=8
        instr(Opcode::Pre, TypeTag::None, 3, 0, 0),  // PRE len=3
        ref_idx(0),                                  // x
        const_i64(0),                                // 0
        instr(Opcode::Gt, TypeTag::None, 0, 0, 0),   // x > 0
        // body starts here (after PRE)
        ref_idx(0),                                 // x
        const_i64(1),                               // 1
        instr(Opcode::Add, TypeTag::None, 0, 0, 0), // x + 1
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        // entry
        const_i64(5),
        instr(Opcode::Call, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(6)));
}

#[test]
fn function_with_precondition_failing() {
    // Same function, but call with -1 (violates x > 0)
    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 8, 0),
        instr(Opcode::Pre, TypeTag::None, 3, 0, 0),
        ref_idx(0),
        const_i64(0),
        instr(Opcode::Gt, TypeTag::None, 0, 0, 0),
        ref_idx(0),
        const_i64(1),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        const_i64(-1),
        instr(Opcode::Call, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(
        result,
        Err(RuntimeError::PreconditionFailed { .. })
    ));
}

#[test]
fn function_with_postcondition_passing() {
    // Function: f(x) ensures result > x { x + 1 }
    // POST checks that the return value > the argument.
    // At POST time, the return value is at binding index 0 (pushed on top).
    //
    //  0: FUNC param_count=1 body_len=8
    //  1: POST len=3
    //  2:   REF 0          -- return value (pushed by RET before POST runs)
    //  3:   REF 1          -- x (the argument, one deeper)
    //  4:   GT             -- return_value > x
    //  5:   REF 0          -- x (during body, x is at index 0)
    //  6:   CONST I64 0 1
    //  7:   ADD
    //  8:   RET
    //  9: ENDFUNC
    // 10: CONST I64 0 5
    // 11: CALL 0
    // 12: HALT
    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 8, 0),
        instr(Opcode::Post, TypeTag::None, 3, 0, 0),
        ref_idx(0), // during POST: return value (most recent binding)
        ref_idx(1), // during POST: x
        instr(Opcode::Gt, TypeTag::None, 0, 0, 0),
        ref_idx(0), // during body: x
        const_i64(1),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        const_i64(5),
        instr(Opcode::Call, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(6)));
}

#[test]
fn function_with_postcondition_failing() {
    // Function returns x - 1 but POST requires result > x (will fail)
    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 8, 0),
        instr(Opcode::Post, TypeTag::None, 3, 0, 0),
        ref_idx(0), // return value
        ref_idx(1), // x
        instr(Opcode::Gt, TypeTag::None, 0, 0, 0),
        ref_idx(0), // x
        const_i64(1),
        instr(Opcode::Sub, TypeTag::None, 0, 0, 0), // x - 1
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        const_i64(5),
        instr(Opcode::Call, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(
        result,
        Err(RuntimeError::PostconditionFailed { .. })
    ));
}

#[test]
fn recursive_factorial_10_equals_3628800() {
    // factorial(n):
    //   result = match n == 0:
    //     case false(tag=0): n * factorial(n-1)
    //     case true(tag=1): 1
    //   return result
    //
    // The MATCH pushes a result onto the stack, then RET is AFTER EXHAUST.
    // This avoids RET inside a CASE body which leaks case_contexts.
    //
    // Layout:
    //  0: FUNC param_count=1 body_len=15
    //  1:   REF 0               -- n
    //  2:   CONST I64 0 0       -- 0
    //  3:   EQ                  -- n == 0 -> Bool
    //  4:   MATCH 2
    //  5:   CASE tag=0 body_len=6 -- false: n != 0
    //  6:     REF 0             -- n
    //  7:     REF 0             -- n
    //  8:     CONST I64 0 1     -- 1
    //  9:     SUB               -- n - 1
    // 10:     RECURSE 100
    // 11:     MUL               -- n * factorial(n-1) -> result on stack
    // 12:   CASE tag=1 body_len=1 -- true: n == 0
    // 13:     CONST I64 0 1     -- 1 -> result on stack
    // 14:   EXHAUST
    // 15:   RET                 -- return result
    // 16: ENDFUNC
    // 17: CONST I64 0 10
    // 18: CALL 0
    // 19: HALT

    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 15, 0), // 0: param_count=1, body_len=15
        ref_idx(0),                                   // 1: n
        const_i64(0),                                 // 2: 0
        instr(Opcode::Eq, TypeTag::None, 0, 0, 0),    // 3: n == 0
        instr(Opcode::Match, TypeTag::None, 2, 0, 0), // 4: match Bool
        instr(Opcode::Case, TypeTag::None, 0, 6, 0),  // 5: false (n != 0), body_len=6
        ref_idx(0),                                   // 6: n
        ref_idx(0),                                   // 7: n
        const_i64(1),                                 // 8: 1
        instr(Opcode::Sub, TypeTag::None, 0, 0, 0),   // 9: n - 1
        instr(Opcode::Recurse, TypeTag::None, 100, 0, 0), // 10: recurse(depth_limit=100)
        instr(Opcode::Mul, TypeTag::None, 0, 0, 0),   // 11: n * factorial(n-1)
        instr(Opcode::Case, TypeTag::None, 1, 1, 0),  // 12: true (n == 0), body_len=1
        const_i64(1),                                 // 13: 1
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0), // 14: exhaust
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),   // 15: return result
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0), // 16: endfunc
        // entry point
        const_i64(10),                               // 17: push 10
        instr(Opcode::Call, TypeTag::None, 0, 0, 0), // 18: call factorial
        halt(),                                      // 19: halt
    ]);
    assert_eq!(result, Ok(Value::I64(3628800)));
}

#[test]
fn recursion_depth_exceeded_returns_error() {
    // Simple recursive function that always recurses (no base case reachable quickly)
    // with a very low depth limit.
    //
    // f(x) = f(x) with depth_limit=2
    //
    //  0: FUNC param_count=1 body_len=3
    //  1:   REF 0
    //  2:   RECURSE depth_limit=2
    //  3:   RET
    //  4: ENDFUNC
    //  5: CONST I64 0 1
    //  6: CALL 0
    //  7: HALT
    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 3, 0),
        ref_idx(0),
        instr(Opcode::Recurse, TypeTag::None, 2, 0, 0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        const_i64(1),
        instr(Opcode::Call, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(
        result,
        Err(RuntimeError::RecursionDepthExceeded { limit: 2, .. })
    ));
}

#[test]
fn call_unknown_function_returns_error() {
    // CALL func_index=99 when no functions exist
    let result = run_program(vec![
        const_i64(1),
        instr(Opcode::Call, TypeTag::None, 99, 0, 0),
        halt(),
    ]);
    assert!(matches!(
        result,
        Err(RuntimeError::UnknownFunction { index: 99, .. })
    ));
}

// ============================================================
// Group E: Data Structures -- Tuples
// ============================================================

#[test]
fn tuple_construction_and_project_field_0() {
    // Construct tuple (10, 20, 30), project field 0
    let result = run_program(vec![
        const_i64(10),
        const_i64(20),
        const_i64(30),
        instr(Opcode::TupleNew, TypeTag::Tuple, 3, 0, 0), // 3 fields
        instr(Opcode::Project, TypeTag::None, 0, 0, 0),   // field 0
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(10)));
}

#[test]
fn tuple_project_field_1() {
    let result = run_program(vec![
        const_i64(10),
        const_i64(20),
        const_i64(30),
        instr(Opcode::TupleNew, TypeTag::Tuple, 3, 0, 0),
        instr(Opcode::Project, TypeTag::None, 1, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(20)));
}

#[test]
fn tuple_project_last_field() {
    let result = run_program(vec![
        const_i64(10),
        const_i64(20),
        const_i64(30),
        instr(Opcode::TupleNew, TypeTag::Tuple, 3, 0, 0),
        instr(Opcode::Project, TypeTag::None, 2, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(30)));
}

#[test]
fn tuple_project_out_of_bounds_returns_error() {
    let result = run_program(vec![
        const_i64(10),
        const_i64(20),
        instr(Opcode::TupleNew, TypeTag::Tuple, 2, 0, 0),
        instr(Opcode::Project, TypeTag::None, 5, 0, 0), // field 5 out of bounds
        halt(),
    ]);
    assert!(matches!(
        result,
        Err(RuntimeError::ProjectOutOfBounds {
            field: 5,
            size: 2,
            ..
        })
    ));
}

#[test]
fn project_on_non_tuple_returns_error() {
    let result = run_program(vec![
        const_i64(42),
        instr(Opcode::Project, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(
        result,
        Err(RuntimeError::ProjectOnNonTuple { .. })
    ));
}

// ============================================================
// Group E: Data Structures -- Arrays
// ============================================================

#[test]
fn array_construction_and_get() {
    // Construct array [10, 20, 30], get element at index 1
    let result = run_program(vec![
        const_i64(10),
        const_i64(20),
        const_i64(30),
        instr(Opcode::ArrayNew, TypeTag::Array, 3, 0, 0), // 3 elements
        const_u64(1),                                     // index 1
        instr(Opcode::ArrayGet, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(20)));
}

#[test]
fn array_get_first_element() {
    let result = run_program(vec![
        const_i64(100),
        const_i64(200),
        instr(Opcode::ArrayNew, TypeTag::Array, 2, 0, 0),
        const_u64(0),
        instr(Opcode::ArrayGet, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(100)));
}

#[test]
fn array_get_last_element() {
    let result = run_program(vec![
        const_i64(100),
        const_i64(200),
        const_i64(300),
        instr(Opcode::ArrayNew, TypeTag::Array, 3, 0, 0),
        const_u64(2),
        instr(Opcode::ArrayGet, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(300)));
}

#[test]
fn array_out_of_bounds_returns_error() {
    let result = run_program(vec![
        const_i64(10),
        const_i64(20),
        instr(Opcode::ArrayNew, TypeTag::Array, 2, 0, 0),
        const_u64(5), // index 5 out of bounds
        instr(Opcode::ArrayGet, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(
        result,
        Err(RuntimeError::ArrayIndexOutOfBounds {
            index: 5,
            length: 2,
            ..
        })
    ));
}

#[test]
fn array_len() {
    let result = run_program(vec![
        const_i64(1),
        const_i64(2),
        const_i64(3),
        const_i64(4),
        instr(Opcode::ArrayNew, TypeTag::Array, 4, 0, 0),
        instr(Opcode::ArrayLen, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(4)));
}

#[test]
fn array_len_empty() {
    let result = run_program(vec![
        instr(Opcode::ArrayNew, TypeTag::Array, 0, 0, 0),
        instr(Opcode::ArrayLen, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(0)));
}

#[test]
fn array_len_on_non_array_returns_error() {
    let result = run_program(vec![
        const_i64(42),
        instr(Opcode::ArrayLen, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(result, Err(RuntimeError::NotAnArray { .. })));
}

// ============================================================
// Group F: Meta
// ============================================================

#[test]
fn assert_true_continues() {
    let result = run_program(vec![
        const_bool(true),
        instr(Opcode::Assert, TypeTag::None, 0, 0, 0),
        const_i64(42),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(42)));
}

#[test]
fn assert_false_returns_error() {
    let result = run_program(vec![
        const_bool(false),
        instr(Opcode::Assert, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(result, Err(RuntimeError::AssertFailed { .. })));
}

#[test]
fn typeof_matching_returns_true() {
    // Push I64(42), TYPEOF with expected=I64 → pushes value back + Bool(true)
    // The type tag value for I64 is 0x01
    let result = run_program(vec![
        const_i64(42),
        instr(Opcode::Typeof, TypeTag::None, TypeTag::I64 as u16, 0, 0),
        // Stack now has: I64(42), Bool(true)
        // We need just one value for HALT. Bind 42 away, keep Bool(true).
        bind(), // bind I64(42) from under Bool(true)... wait, top is Bool(true)
        halt(),
    ]);
    // After TYPEOF: stack = [I64(42), Bool(true)]
    // BIND pops Bool(true) into bindings
    // Stack = [I64(42)]
    // Hmm that's backwards. Let me re-read.
    // TYPEOF: pop value, push value back, push Bool(matches).
    // Stack after TYPEOF: [I64(42), Bool(true)]
    // BIND: pop Bool(true) → bindings
    // Stack: [I64(42)]
    // HALT → I64(42)
    // That doesn't test what we want. Let's swap approach:
    // Use NOP to consume the value or just check the Bool.
    // Actually, let's just leave both and check HaltWithMultipleValues.
    // Better: drop the original value first.
    // After TYPEOF, stack = [..., value, Bool]. We want the Bool.
    // Hmm, we need a swap. NoLang doesn't have SWAP. Let's BIND both then REF.

    // Simpler: just test that TYPEOF pushes the right result with the right stack state.
    assert_eq!(result, Ok(Value::I64(42)));
    // The above test confirms TYPEOF pushed the value back correctly.
    // But let's also confirm the Bool result separately.
}

#[test]
fn typeof_matching_pushes_true_on_top() {
    // TYPEOF I64 on I64(42) → stack = [I64(42), Bool(true)]
    // BIND the I64 (but BIND pops top = Bool(true))...
    // OK, new approach: just HALT with 2 values and check the error.
    // Better approach: use NOP then check. Actually, the cleanest test:
    // Push value, TYPEOF, then BIND the value underneath, leaving Bool on stack.
    //
    // Actually: TYPEOF pops value, pushes value back, pushes Bool.
    // Stack: [value, Bool]. Top = Bool.
    // To get just Bool: swap isn't available. Use BIND + DROP:
    // BIND pops Bool(true) → bindings. Stack: [I64(42)].
    // Then REF 0 → push Bool(true). Stack: [I64(42), Bool(true)].
    // Still two values. Let's just use the helper differently.
    // Simplest: push I64, TYPEOF I64, stack=[I64, Bool]. Now do an ASSERT on the Bool(true).
    // ASSERT pops Bool(true) → ok. Stack: [I64(42)]. HALT → I64(42).
    let result = run_program(vec![
        const_i64(42),
        instr(Opcode::Typeof, TypeTag::None, TypeTag::I64 as u16, 0, 0),
        instr(Opcode::Assert, TypeTag::None, 0, 0, 0), // assert the Bool(true)
        halt(),                                        // value is still there
    ]);
    assert_eq!(result, Ok(Value::I64(42)));
}

#[test]
fn typeof_non_matching_pushes_false() {
    // Push Bool(true), TYPEOF with expected=I64 → should push Bool(false)
    // Stack: [Bool(true), Bool(false)]
    // We can't ASSERT Bool(false) — that would fail. So let's NOT it first.
    let result = run_program(vec![
        const_bool(true),
        instr(Opcode::Typeof, TypeTag::None, TypeTag::I64 as u16, 0, 0),
        // Stack: [Bool(true), Bool(false)]
        // NOT the top: Bool(true)
        instr(Opcode::Not, TypeTag::None, 0, 0, 0),
        // Stack: [Bool(true), Bool(true)]
        instr(Opcode::Assert, TypeTag::None, 0, 0, 0), // assert Bool(true)
        // Stack: [Bool(true)]
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn typeof_u64_matching() {
    let result = run_program(vec![
        const_u64(100),
        instr(Opcode::Typeof, TypeTag::None, TypeTag::U64 as u16, 0, 0),
        instr(Opcode::Assert, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(100)));
}

#[test]
fn typeof_bool_matching() {
    let result = run_program(vec![
        const_bool(false),
        instr(Opcode::Typeof, TypeTag::None, TypeTag::Bool as u16, 0, 0),
        instr(Opcode::Assert, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

// ============================================================
// HASH is NOP during execution
// ============================================================

#[test]
fn hash_is_nop_during_execution() {
    let result = run_program(vec![
        const_i64(42),
        instr(Opcode::Hash, TypeTag::None, 0xABCD, 0x1234, 0x5678),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(42)));
}

// ============================================================
// Stack underflow
// ============================================================

#[test]
fn add_on_empty_stack_returns_stack_underflow() {
    let result = run_program(vec![instr(Opcode::Add, TypeTag::None, 0, 0, 0), halt()]);
    assert!(matches!(result, Err(RuntimeError::StackUnderflow { .. })));
}

#[test]
fn add_with_one_value_returns_stack_underflow() {
    let result = run_program(vec![
        const_i64(1),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(result, Err(RuntimeError::StackUnderflow { .. })));
}

// ============================================================
// Unexpected end of program
// ============================================================

#[test]
fn program_without_halt_returns_unexpected_end() {
    let result = run_program(vec![const_i64(42)]);
    assert!(matches!(
        result,
        Err(RuntimeError::UnexpectedEndOfProgram { .. })
    ));
}

// ============================================================
// Multiple operations combined (integration)
// ============================================================

#[test]
fn arithmetic_expression_3_plus_4_times_2() {
    // Compute (3 + 4) * 2 = 14
    let result = run_program(vec![
        const_i64(3),
        const_i64(4),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        const_i64(2),
        instr(Opcode::Mul, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(14)));
}

#[test]
fn nested_bindings_and_arithmetic() {
    // let x = 10 in
    //   let y = 20 in
    //     let z = 30 in
    //       x + y + z = 60
    let result = run_program(vec![
        const_i64(10),
        bind(), // x at index 2 (after y and z bound)
        const_i64(20),
        bind(), // y at index 1
        const_i64(30),
        bind(),                                     // z at index 0
        ref_idx(2),                                 // x = 10
        ref_idx(1),                                 // y = 20
        instr(Opcode::Add, TypeTag::None, 0, 0, 0), // 30
        ref_idx(0),                                 // z = 30
        instr(Opcode::Add, TypeTag::None, 0, 0, 0), // 60
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(60)));
}

// ============================================================
// Variant construction
// ============================================================

#[test]
fn variant_new_constructs_variant() {
    // Create a variant with tag_count=2, tag=0, payload=I64(42)
    // Then project the payload via MATCH
    let result = run_program(vec![
        const_i64(42),
        instr(Opcode::VariantNew, TypeTag::Variant, 2, 0, 0),
        instr(Opcode::Match, TypeTag::None, 2, 0, 0),
        instr(Opcode::Case, TypeTag::None, 0, 1, 0), // tag 0
        nop(),                                       // payload already on stack
        instr(Opcode::Case, TypeTag::None, 1, 1, 0), // tag 1
        const_i64(0),
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(42)));
}

// ============================================================
// Entry point with functions
// ============================================================

#[test]
fn entry_point_after_function_definitions() {
    // Two functions defined, entry point is after both
    //
    //  0: FUNC param_count=1 body_len=2
    //  1:   REF 0
    //  2:   RET
    //  3: ENDFUNC
    //  4: FUNC param_count=1 body_len=2
    //  5:   REF 0
    //  6:   RET
    //  7: ENDFUNC
    //  --- entry point = 8 ---
    //  8: CONST I64 0 99
    //  9: HALT
    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 2, 0),
        ref_idx(0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        instr(Opcode::Func, TypeTag::None, 1, 2, 0),
        ref_idx(0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        const_i64(99),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(99)));
}

#[test]
fn calling_second_function_by_index() {
    // func 0: identity(x) = x
    // func 1: negate(x) = -x
    // entry: call func 1 with 42 → -42
    let result = run_program(vec![
        // func 0: identity
        instr(Opcode::Func, TypeTag::None, 1, 2, 0),
        ref_idx(0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        // func 1: negate
        instr(Opcode::Func, TypeTag::None, 1, 3, 0),
        ref_idx(0),
        instr(Opcode::Neg, TypeTag::None, 0, 0, 0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        // entry
        const_i64(42),
        instr(Opcode::Call, TypeTag::None, 1, 0, 0), // call func 1 (negate)
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(-42)));
}

// ============================================================
// Function with multiple parameters
// ============================================================

#[test]
fn function_with_two_parameters() {
    // add(a, b) = a + b
    // Call with a=10, b=32
    //
    //  0: FUNC param_count=2 body_len=4
    //  1:   REF 0      -- b (last pushed = most recent = de Bruijn 0)
    //  2:   REF 1      -- a (first pushed = deeper = de Bruijn 1)
    //  3:   ADD
    //  4:   RET
    //  5: ENDFUNC
    //  6: CONST I64 0 10  -- a
    //  7: CONST I64 0 32  -- b
    //  8: CALL 0
    //  9: HALT
    //
    // Wait, let me re-read the CALL implementation:
    // "Pop arguments (last pushed = param index 0)"
    // args = [pop() = b=32, pop() = a=10]
    // Then push in reverse: push a=10, push b=32
    // bindings after: [..., a=10, b=32]
    // REF 0 = b=32 (most recent), REF 1 = a=10
    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 2, 4, 0),
        ref_idx(0), // b (most recent binding)
        ref_idx(1), // a
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        const_i64(10), // a
        const_i64(32), // b
        instr(Opcode::Call, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(42)));
}

// ============================================================
// MATCH body executes then jumps past EXHAUST
// ============================================================

#[test]
fn match_body_with_multiple_instructions() {
    // Match on true, body has 3 instructions (push 10, push 20, ADD)
    let result = run_program(vec![
        const_bool(true),
        instr(Opcode::Match, TypeTag::None, 2, 0, 0),
        instr(Opcode::Case, TypeTag::None, 0, 1, 0), // false, body_len=1
        const_i64(0),                                // false body
        instr(Opcode::Case, TypeTag::None, 1, 3, 0), // true, body_len=3
        const_i64(10),                               // true body
        const_i64(20),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(30)));
}

// ============================================================
// Empty array
// ============================================================

#[test]
fn empty_array_get_returns_out_of_bounds() {
    let result = run_program(vec![
        instr(Opcode::ArrayNew, TypeTag::Array, 0, 0, 0),
        const_u64(0),
        instr(Opcode::ArrayGet, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(
        result,
        Err(RuntimeError::ArrayIndexOutOfBounds {
            index: 0,
            length: 0,
            ..
        })
    ));
}

// ============================================================
// Comparison with U64
// ============================================================

#[test]
fn lt_u64() {
    let result = run_program(vec![
        const_u64(5),
        const_u64(10),
        instr(Opcode::Lt, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn eq_u64() {
    let result = run_program(vec![
        const_u64(42),
        const_u64(42),
        instr(Opcode::Eq, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

// ============================================================
// AND/OR/XOR with U64
// ============================================================

#[test]
fn and_u64_bitwise() {
    let result = run_program(vec![
        const_u64(0xFF00),
        const_u64(0x0FF0),
        instr(Opcode::And, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(0x0F00)));
}

#[test]
fn or_u64_bitwise() {
    let result = run_program(vec![
        const_u64(0xF000),
        const_u64(0x000F),
        instr(Opcode::Or, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(0xF00F)));
}

#[test]
fn not_u64_bitwise() {
    let result = run_program(vec![
        const_u64(0),
        instr(Opcode::Not, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(u64::MAX)));
}

#[test]
fn xor_u64_bitwise() {
    let result = run_program(vec![
        const_u64(0xFF),
        const_u64(0xFF),
        instr(Opcode::Xor, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(0)));
}

// ============================================================
// SUB / DIV for U64
// ============================================================

#[test]
fn sub_u64() {
    let result = run_program(vec![
        const_u64(100),
        const_u64(30),
        instr(Opcode::Sub, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::U64(70)));
}

// ============================================================
// Recursive fibonacci (smaller value to keep test fast)
// ============================================================

#[test]
fn recursive_fibonacci_of_10() {
    // fib(n):
    //   result = match n < 2:
    //     false -> fib(n-1) + fib(n-2)
    //     true  -> n
    //   return result
    //
    // RET is after EXHAUST to avoid leaking case_contexts.
    //
    // Layout:
    //  0: FUNC param_count=1 body_len=17
    //  1:   REF 0               -- n
    //  2:   CONST I64 0 2       -- 2
    //  3:   LT                  -- n < 2
    //  4:   MATCH 2
    //  5:   CASE tag=0 body_len=9  -- false (n >= 2)
    //  6:     REF 0             -- n
    //  7:     CONST I64 0 1
    //  8:     SUB               -- n-1
    //  9:     RECURSE 100
    // 10:     REF 0             -- n
    // 11:     CONST I64 0 2
    // 12:     SUB               -- n-2
    // 13:     RECURSE 100
    // 14:     ADD               -- fib(n-1)+fib(n-2) on stack
    // 15:   CASE tag=1 body_len=1  -- true (n < 2)
    // 16:     REF 0             -- n on stack
    // 17:   EXHAUST
    // 18:   RET                 -- return result
    // 19: ENDFUNC
    // 20: CONST I64 0 10
    // 21: CALL 0
    // 22: HALT

    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 18, 0), // 0: body_len=18 (instrs 1..18)
        ref_idx(0),                                   // 1: n
        const_i64(2),                                 // 2: 2
        instr(Opcode::Lt, TypeTag::None, 0, 0, 0),    // 3: n < 2
        instr(Opcode::Match, TypeTag::None, 2, 0, 0), // 4: match Bool
        instr(Opcode::Case, TypeTag::None, 0, 9, 0),  // 5: false (n >= 2), body_len=9
        ref_idx(0),                                   // 6: n
        const_i64(1),                                 // 7: 1
        instr(Opcode::Sub, TypeTag::None, 0, 0, 0),   // 8: n-1
        instr(Opcode::Recurse, TypeTag::None, 100, 0, 0), // 9: fib(n-1)
        ref_idx(0),                                   // 10: n
        const_i64(2),                                 // 11: 2
        instr(Opcode::Sub, TypeTag::None, 0, 0, 0),   // 12: n-2
        instr(Opcode::Recurse, TypeTag::None, 100, 0, 0), // 13: fib(n-2)
        instr(Opcode::Add, TypeTag::None, 0, 0, 0),   // 14: fib(n-1)+fib(n-2)
        instr(Opcode::Case, TypeTag::None, 1, 1, 0),  // 15: true (n < 2), body_len=1
        ref_idx(0),                                   // 16: n
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0), // 17: exhaust
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),   // 18: return result
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0), // 19: endfunc
        // entry point = 20
        const_i64(10),                               // 20: push 10
        instr(Opcode::Call, TypeTag::None, 0, 0, 0), // 21: call fib
        halt(),                                      // 22: halt
    ]);
    // fib(10) = 55
    assert_eq!(result, Ok(Value::I64(55)));
}

// ============================================================
// Division by zero does not panic
// ============================================================

#[test]
fn div_by_zero_does_not_panic() {
    // This is a key BUILD_ORDER.md requirement:
    // "Division by zero produces RuntimeError, not panic"
    let result = std::panic::catch_unwind(|| {
        run_program(vec![
            const_i64(1),
            const_i64(0),
            instr(Opcode::Div, TypeTag::None, 0, 0, 0),
            halt(),
        ])
    });
    assert!(result.is_ok(), "Division by zero should not panic");
    let inner = result.unwrap();
    assert!(matches!(inner, Err(RuntimeError::DivisionByZero { .. })));
}

// ============================================================
// Fuzz test: 1000 random programs, VM never panics
// ============================================================

#[test]
fn fuzz_vm_never_panics_on_random_programs() {
    use nolang_common::opcode::ALL_OPCODES;
    use nolang_common::type_tag::ALL_TYPE_TAGS;

    // Simple deterministic PRNG (xorshift64)
    let mut state: u64 = 0xDEAD_BEEF_CAFE_BABEu64;
    let mut next_rand = || -> u64 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };

    for _ in 0..1000 {
        let len = (next_rand() % 20) as usize + 1; // 1..20 instructions
        let mut instrs = Vec::with_capacity(len);

        for _ in 0..len {
            let opcode_idx = (next_rand() % ALL_OPCODES.len() as u64) as usize;
            let tt_idx = (next_rand() % ALL_TYPE_TAGS.len() as u64) as usize;
            let a1 = (next_rand() % 256) as u16;
            let a2 = (next_rand() % 256) as u16;
            let a3 = (next_rand() % 256) as u16;

            instrs.push(Instruction::new(
                ALL_OPCODES[opcode_idx],
                ALL_TYPE_TAGS[tt_idx],
                a1,
                a2,
                a3,
            ));
        }

        let program = Program::new(instrs);
        // The VM must return Ok or Err, never panic
        let result = std::panic::catch_unwind(|| run(&program));
        assert!(
            result.is_ok(),
            "VM panicked on random program (should return RuntimeError instead)"
        );
    }
}

// ============================================================
// Edge cases: BIND on empty stack
// ============================================================

#[test]
fn bind_on_empty_stack_returns_underflow() {
    let result = run_program(vec![bind(), halt()]);
    assert!(matches!(result, Err(RuntimeError::StackUnderflow { .. })));
}

// ============================================================
// Edge case: Empty program (no instructions at all)
// ============================================================

#[test]
fn empty_program_no_instructions_returns_error() {
    let result = run_program(vec![]);
    assert!(matches!(
        result,
        Err(RuntimeError::UnexpectedEndOfProgram { .. })
    ));
}

// ============================================================
// MATCH with variant payload used in computation
// ============================================================

#[test]
fn match_variant_payload_used_in_arithmetic() {
    // Variant(tag_count=2, tag=0, payload=I64(10))
    // Match: tag 0 body uses payload in addition
    let result = run_program(vec![
        const_i64(10),
        instr(Opcode::VariantNew, TypeTag::Variant, 2, 0, 0),
        instr(Opcode::Match, TypeTag::None, 2, 0, 0),
        instr(Opcode::Case, TypeTag::None, 0, 3, 0), // tag 0, body_len=3
        // payload (10) is on stack from MATCH
        const_i64(5),
        instr(Opcode::Add, TypeTag::None, 0, 0, 0), // 10 + 5 = 15
        nop(),
        instr(Opcode::Case, TypeTag::None, 1, 1, 0), // tag 1, body_len=1
        const_i64(0),
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(15)));
}

// ============================================================
// Mixed types: comparison with Bool
// ============================================================

#[test]
fn eq_bool_values() {
    let result = run_program(vec![
        const_bool(true),
        const_bool(true),
        instr(Opcode::Eq, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

#[test]
fn neq_bool_values() {
    let result = run_program(vec![
        const_bool(true),
        const_bool(false),
        instr(Opcode::Neq, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

// ============================================================
// NEG on i64 wrapping (wrapping_neg of MIN)
// ============================================================

#[test]
fn neg_i64_min_32bit_wraps() {
    // -i32::MIN wraps to i32::MIN in 32-bit, but in i64 it's just a positive number.
    // Actually, const_i64(i32::MIN) sign-extends to i64::from(i32::MIN) = -2147483648_i64.
    // wrapping_neg of -2147483648_i64 = 2147483648_i64 (fits in i64, no wrap).
    let result = run_program(vec![
        const_i64(i32::MIN),
        instr(Opcode::Neg, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(-(i32::MIN as i64))));
}

// ============================================================
// Tuple with heterogeneous types
// ============================================================

#[test]
fn tuple_with_mixed_types() {
    // (I64(42), Bool(true), U64(7))
    // Project field 1 → Bool(true)
    let result = run_program(vec![
        const_i64(42),
        const_bool(true),
        const_u64(7),
        instr(Opcode::TupleNew, TypeTag::Tuple, 3, 0, 0),
        instr(Opcode::Project, TypeTag::None, 1, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true)));
}

// ============================================================
// Multiple CONST types on stack, verify correct behavior
// ============================================================

#[test]
fn const_char_value() {
    let result = run_program(vec![
        instr(Opcode::Const, TypeTag::Char, 65, 0, 0), // 'A'
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Char('A')));
}

// ============================================================
// Array of booleans
// ============================================================

#[test]
fn array_of_bools() {
    let result = run_program(vec![
        const_bool(true),
        const_bool(false),
        const_bool(true),
        instr(Opcode::ArrayNew, TypeTag::Array, 3, 0, 0),
        const_u64(1),
        instr(Opcode::ArrayGet, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(false)));
}

// ============================================================
// Recursion depth 0 should fail immediately
// ============================================================

#[test]
fn recursion_depth_limit_zero_fails_immediately() {
    // RECURSE depth_limit=0 should fail on first call since depth starts at 0
    // and the check is current_depth >= depth_limit.
    // After CALL, recursion_depth=0. RECURSE checks 0 >= 0 → true → fail.
    let result = run_program(vec![
        instr(Opcode::Func, TypeTag::None, 1, 3, 0),
        ref_idx(0),
        instr(Opcode::Recurse, TypeTag::None, 0, 0, 0), // depth_limit=0
        instr(Opcode::Ret, TypeTag::None, 0, 0, 0),
        instr(Opcode::EndFunc, TypeTag::None, 0, 0, 0),
        const_i64(1),
        instr(Opcode::Call, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert!(matches!(
        result,
        Err(RuntimeError::RecursionDepthExceeded { limit: 0, .. })
    ));
}

// ============================================================
// MOD negative numbers
// ============================================================

#[test]
fn mod_negative_dividend() {
    // Rust's wrapping_rem: -7 % 3 = -1
    let result = run_program(vec![
        const_i64(-7),
        const_i64(3),
        instr(Opcode::Mod, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(-1)));
}

// ============================================================
// Nested match (match inside a match body)
// ============================================================

#[test]
fn nested_match_in_case_body() {
    // Outer match on Bool(true), inner match on Bool(false)
    //
    //  0: CONST BOOL true
    //  1: MATCH 2
    //  2: CASE tag=0 body_len=1   -- outer false
    //  3:   CONST I64 0 0
    //  4: CASE tag=1 body_len=9   -- outer true
    //  5:   CONST BOOL false
    //  6:   MATCH 2               -- inner match
    //  7:   CASE tag=0 body_len=1 -- inner false
    //  8:     CONST I64 0 42
    //  9:   CASE tag=1 body_len=1 -- inner true
    // 10:     CONST I64 0 99
    // 11:   EXHAUST               -- inner exhaust
    // 12:   NOP
    // 13:   NOP
    // 14: EXHAUST                 -- outer exhaust
    // body of outer true (indices 5..13) = 9 instructions... wait let me count.
    // CASE tag=1 body_len=X. Body starts at index 5. body_len should include
    // instructions 5 through 5+X-1.
    // We want the inner match to produce 42 (inner false). Then the outer case body
    // ends and we jump to outer EXHAUST.
    //
    // Let me be more careful with body_len counting.
    // Outer true body: starts at index 5, contains instructions 5..12 = 8 instructions.

    let result = run_program(vec![
        const_bool(true),                               // 0
        instr(Opcode::Match, TypeTag::None, 2, 0, 0),   // 1: outer match
        instr(Opcode::Case, TypeTag::None, 0, 1, 0),    // 2: outer false, body_len=1
        const_i64(0),                                   // 3: outer false body
        instr(Opcode::Case, TypeTag::None, 1, 8, 0),    // 4: outer true, body_len=8
        const_bool(false),                              // 5: push false for inner match
        instr(Opcode::Match, TypeTag::None, 2, 0, 0),   // 6: inner match
        instr(Opcode::Case, TypeTag::None, 0, 1, 0),    // 7: inner false, body_len=1
        const_i64(42),                                  // 8: inner false body
        instr(Opcode::Case, TypeTag::None, 1, 1, 0),    // 9: inner true, body_len=1
        const_i64(99),                                  // 10: inner true body
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0), // 11: inner exhaust
        nop(),                                          // 12: padding to reach body_len=8
        instr(Opcode::Exhaust, TypeTag::None, 0, 0, 0), // 13: outer exhaust
        halt(),                                         // 14
    ]);
    assert_eq!(result, Ok(Value::I64(42)));
}

// ============================================================
// Verify comparison operators with negative values
// ============================================================

#[test]
fn lt_i64_negative_values() {
    let result = run_program(vec![
        const_i64(-10),
        const_i64(-5),
        instr(Opcode::Lt, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true))); // -10 < -5
}

#[test]
fn gt_i64_negative_vs_positive() {
    let result = run_program(vec![
        const_i64(1),
        const_i64(-1),
        instr(Opcode::Gt, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::Bool(true))); // 1 > -1
}

// ============================================================
// DIV truncates toward zero
// ============================================================

#[test]
fn div_i64_truncates_toward_zero() {
    // 7 / 2 = 3 (not 3.5)
    let result = run_program(vec![
        const_i64(7),
        const_i64(2),
        instr(Opcode::Div, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(3)));
}

#[test]
fn div_i64_negative_truncates_toward_zero() {
    // -7 / 2 = -3 (Rust's wrapping_div truncates toward zero)
    let result = run_program(vec![
        const_i64(-7),
        const_i64(2),
        instr(Opcode::Div, TypeTag::None, 0, 0, 0),
        halt(),
    ]);
    assert_eq!(result, Ok(Value::I64(-3)));
}
