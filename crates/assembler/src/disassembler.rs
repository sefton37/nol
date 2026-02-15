//! Disassembler: binary program → canonical assembly text.
//!
//! Output format is flat text, one instruction per line. No indentation,
//! no comments, no blank lines. CONST_EXT occupies one text line but
//! two instruction slots.

use nolang_common::{Opcode, Program, TypeTag};

/// Disassemble a program into canonical assembly text.
///
/// The output is guaranteed to reassemble to an identical binary
/// (`assemble(disassemble(program)) == program`).
pub fn disassemble(program: &Program) -> String {
    let instrs = &program.instructions;
    let mut lines = Vec::new();
    let mut i = 0;

    while i < instrs.len() {
        let instr = &instrs[i];

        let line = match instr.opcode {
            // Pattern A: No arguments
            Opcode::Bind
            | Opcode::Drop
            | Opcode::Neg
            | Opcode::Add
            | Opcode::Sub
            | Opcode::Mul
            | Opcode::Div
            | Opcode::Mod
            | Opcode::Eq
            | Opcode::Neq
            | Opcode::Lt
            | Opcode::Gt
            | Opcode::Lte
            | Opcode::Gte
            | Opcode::And
            | Opcode::Or
            | Opcode::Not
            | Opcode::Xor
            | Opcode::Shl
            | Opcode::Shr
            | Opcode::ArrayGet
            | Opcode::ArrayLen
            | Opcode::Assert
            | Opcode::Ret
            | Opcode::EndFunc
            | Opcode::Exhaust
            | Opcode::Nop
            | Opcode::Halt => instr.opcode.mnemonic().to_string(),

            // Pattern B/D: Single decimal arg
            Opcode::Ref
            | Opcode::Match
            | Opcode::Call
            | Opcode::Recurse
            | Opcode::Project
            | Opcode::Pre
            | Opcode::Post => {
                format!("{} {}", instr.opcode.mnemonic(), instr.arg1)
            }

            // Pattern C: Two decimal args
            Opcode::Func | Opcode::Case => {
                format!("{} {} {}", instr.opcode.mnemonic(), instr.arg1, instr.arg2)
            }

            // Pattern E: Type name in type_tag field
            Opcode::Param => {
                format!("{} {}", instr.opcode.mnemonic(), instr.type_tag.name())
            }

            // Pattern F: Type name from arg1
            Opcode::Typeof => {
                let tt = TypeTag::try_from(instr.arg1 as u8);
                let name = match tt {
                    Ok(t) => t.name(),
                    Err(_) => "NONE",
                };
                format!("{} {}", instr.opcode.mnemonic(), name)
            }

            // Pattern G: Type + two hex args
            Opcode::Const => {
                format!(
                    "{} {} 0x{:04x} 0x{:04x}",
                    instr.opcode.mnemonic(),
                    instr.type_tag.name(),
                    instr.arg1,
                    instr.arg2
                )
            }

            // Pattern H: CONST_EXT → read next instruction as data, emit single line
            Opcode::ConstExt => {
                let high16 = instr.arg1 as u64;
                if i + 1 < instrs.len() {
                    let next = &instrs[i + 1];
                    let low48 = ((next.arg1 as u64) << 32)
                        | ((next.arg2 as u64) << 16)
                        | (next.arg3 as u64);
                    let full_value = (high16 << 48) | low48;
                    i += 1; // skip data slot
                    format!(
                        "{} {} 0x{:016x}",
                        instr.opcode.mnemonic(),
                        instr.type_tag.name(),
                        full_value
                    )
                } else {
                    // Malformed: no data slot. Emit what we can.
                    format!(
                        "{} {} 0x{:016x}",
                        instr.opcode.mnemonic(),
                        instr.type_tag.name(),
                        high16 << 48
                    )
                }
            }

            // Pattern I: Three hex args
            Opcode::Hash => {
                format!(
                    "{} 0x{:04x} 0x{:04x} 0x{:04x}",
                    instr.opcode.mnemonic(),
                    instr.arg1,
                    instr.arg2,
                    instr.arg3
                )
            }

            // Pattern J: Type + two decimal args
            Opcode::VariantNew => {
                format!(
                    "{} {} {} {}",
                    instr.opcode.mnemonic(),
                    instr.type_tag.name(),
                    instr.arg1,
                    instr.arg2
                )
            }

            // Pattern K: Type + one decimal arg
            Opcode::TupleNew | Opcode::ArrayNew => {
                format!(
                    "{} {} {}",
                    instr.opcode.mnemonic(),
                    instr.type_tag.name(),
                    instr.arg1
                )
            }
        };

        lines.push(line);
        i += 1;
    }

    let mut result = lines.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use nolang_common::Instruction;

    fn instr(opcode: Opcode, type_tag: TypeTag, arg1: u16, arg2: u16, arg3: u16) -> Instruction {
        Instruction::new(opcode, type_tag, arg1, arg2, arg3)
    }

    #[test]
    fn empty_program() {
        let program = Program::new(vec![]);
        assert_eq!(disassemble(&program), "");
    }

    #[test]
    fn pattern_a_no_args() {
        let program = Program::new(vec![
            instr(Opcode::Add, TypeTag::None, 0, 0, 0),
            instr(Opcode::Halt, TypeTag::None, 0, 0, 0),
        ]);
        assert_eq!(disassemble(&program), "ADD\nHALT\n");
    }

    #[test]
    fn pattern_b_ref() {
        let program = Program::new(vec![instr(Opcode::Ref, TypeTag::None, 3, 0, 0)]);
        assert_eq!(disassemble(&program), "REF 3\n");
    }

    #[test]
    fn pattern_c_func() {
        let program = Program::new(vec![instr(Opcode::Func, TypeTag::None, 1, 8, 0)]);
        assert_eq!(disassemble(&program), "FUNC 1 8\n");
    }

    #[test]
    fn pattern_e_param() {
        let program = Program::new(vec![instr(Opcode::Param, TypeTag::I64, 0, 0, 0)]);
        assert_eq!(disassemble(&program), "PARAM I64\n");
    }

    #[test]
    fn pattern_f_typeof() {
        let program = Program::new(vec![instr(
            Opcode::Typeof,
            TypeTag::None,
            TypeTag::I64 as u16,
            0,
            0,
        )]);
        assert_eq!(disassemble(&program), "TYPEOF I64\n");
    }

    #[test]
    fn pattern_g_const_hex() {
        let program = Program::new(vec![instr(Opcode::Const, TypeTag::I64, 0, 42, 0)]);
        assert_eq!(disassemble(&program), "CONST I64 0x0000 0x002a\n");
    }

    #[test]
    fn pattern_h_const_ext() {
        let program = Program::new(vec![
            instr(Opcode::ConstExt, TypeTag::I64, 0x0000, 0, 0),
            instr(Opcode::Nop, TypeTag::None, 0x1234, 0x5678, 0x9abc),
        ]);
        assert_eq!(disassemble(&program), "CONST_EXT I64 0x0000123456789abc\n");
    }

    #[test]
    fn pattern_i_hash() {
        let program = Program::new(vec![instr(
            Opcode::Hash,
            TypeTag::None,
            0xa3f2,
            0x1b4c,
            0x7d9e,
        )]);
        assert_eq!(disassemble(&program), "HASH 0xa3f2 0x1b4c 0x7d9e\n");
    }

    #[test]
    fn pattern_j_variant_new() {
        let program = Program::new(vec![instr(Opcode::VariantNew, TypeTag::Variant, 2, 0, 0)]);
        assert_eq!(disassemble(&program), "VARIANT_NEW VARIANT 2 0\n");
    }

    #[test]
    fn pattern_k_tuple_new() {
        let program = Program::new(vec![instr(Opcode::TupleNew, TypeTag::Tuple, 2, 0, 0)]);
        assert_eq!(disassemble(&program), "TUPLE_NEW TUPLE 2\n");
    }

    #[test]
    fn pattern_k_array_new() {
        let program = Program::new(vec![instr(Opcode::ArrayNew, TypeTag::Array, 3, 0, 0)]);
        assert_eq!(disassemble(&program), "ARRAY_NEW ARRAY 3\n");
    }
}
