# NoLang Instruction Set Specification v0.1

## 1. Design Philosophy

NoLang is a language designed for LLM generation. Every design decision serves one principle:
**minimize the number of valid ways to express any computation to exactly one.**

This means:
- No variable names (de Bruijn indices)
- No syntactic sugar (one control flow primitive)
- No implicit behavior (all types explicit, all matches exhaustive)
- No style choices (fixed-width encoding, canonical ordering)
- No ambiguity (every bit pattern is either valid or rejected)

## 2. Instruction Encoding

Every instruction is exactly **64 bits (8 bytes)**:

```
Bit layout:
[63..56] [55..48] [47..32]  [31..16]  [15..0]
 opcode   type_tag   arg1      arg2     arg3
  8 bit    8 bit    16 bit    16 bit   16 bit
```

- **opcode** (u8): Which operation. See Section 4.
- **type_tag** (u8): Type context for this instruction. See Section 3. Value 0x00 = not applicable.
- **arg1** (u16): First operand. Meaning depends on opcode.
- **arg2** (u16): Second operand. Meaning depends on opcode.
- **arg3** (u16): Third operand. Meaning depends on opcode.

**Byte order: little-endian.** The opcode is the FIRST byte read.

**Unused argument fields MUST be zero.** A verifier rejects any instruction where an unused arg field is nonzero.

## 3. Type System

### 3.1 Type Tags

| Value | Name      | Description                              | Size (slots) |
|-------|-----------|------------------------------------------|--------------|
| 0x00  | NONE      | No type / not applicable                 | 0            |
| 0x01  | I64       | Signed 64-bit integer                    | 1            |
| 0x02  | U64       | Unsigned 64-bit integer                  | 1            |
| 0x03  | F64       | IEEE 754 64-bit float                    | 1            |
| 0x04  | BOOL      | Boolean (0 = false, 1 = true)            | 1            |
| 0x05  | CHAR      | Unicode codepoint (u32, zero-extended)   | 1            |
| 0x06  | VARIANT   | Tagged union. arg1 = variant tag count   | variable     |
| 0x07  | TUPLE     | Product type. arg1 = field count         | variable     |
| 0x08  | FUNC_TYPE | Function type                            | 0 (metadata) |
| 0x09  | ARRAY     | Fixed-size array. arg1 = element type, arg2 = length | variable |
| 0x0A  | MAYBE     | Optional. Sugar for VARIANT with 2 tags: SOME(0), NONE(1) | variable |
| 0x0B  | RESULT    | Ok/Err. Sugar for VARIANT with 2 tags: OK(0), ERR(1) | variable |
| 0x0C  | UNIT      | Zero-size type. The type of "nothing useful to return." | 0 |

Types 0x0D–0xFF are reserved. A verifier rejects any instruction using a reserved type tag.

### 3.2 Type Rules

- Every value on the stack has exactly one type tag.
- Type tags are assigned at BIND time and checked at REF time.
- VARIANT construction requires specifying which tag (0..N-1) and the payload type.
- MATCH on a VARIANT must have exactly N CASE branches (one per tag). No more, no less.
- MAYBE is structurally identical to `VARIANT(2)` with tags SOME=0, NONE=1.
- RESULT is structurally identical to `VARIANT(2)` with tags OK=0, ERR=1.
- Arithmetic opcodes (ADD, SUB, MUL, DIV, MOD) require both operands to have the same numeric type (I64, U64, or F64). Mixed-type arithmetic is rejected.
- Comparison opcodes (EQ, NEQ, LT, GT, LTE, GTE) return BOOL.

## 4. Opcode Table

### 4.1 Binding & Reference

| Opcode | Value | Args         | Description |
|--------|-------|--------------|-------------|
| BIND   | 0x01  | -            | Pop top of stack, create a new binding. The bound value is now at de Bruijn index 0; all existing indices shift up by 1. |
| REF    | 0x02  | arg1=index   | Push the value at de Bruijn index `arg1` onto the stack. |
| DROP   | 0x03  | -            | Remove the most recent binding (index 0). All indices shift down by 1. |

**De Bruijn index rules:**
- Index 0 always refers to the most recently bound value.
- Index N refers to the value bound N levels ago.
- REF to an index beyond the current binding depth is a verification error.
- Bindings are immutable. Once bound, a value cannot be changed.

### 4.2 Constants

| Opcode | Value | Args                    | Description |
|--------|-------|-------------------------|-------------|
| CONST  | 0x04  | type_tag, arg1, arg2    | Push a constant. For I64/U64: arg1 is high 16 bits, arg2 is low 16 bits (32-bit range). For BOOL: arg1 is 0 or 1. For CHAR: arg1 is the codepoint. For UNIT: no args needed. |
| CONST_EXT | 0x05 | type_tag, arg1       | Extended constant. The next instruction's full 48-bit payload (type_tag + arg1 + arg2 + arg3) is treated as the low 48 bits. arg1 of CONST_EXT is the high 16 bits. Together: 64-bit constant. |

**Note:** CONST can encode values up to 32 bits. For full 64-bit constants, use CONST_EXT which consumes two instruction slots.

**Sign extension:** For I64, the 32-bit value formed by `(arg1 << 16) | arg2` is **sign-extended** to 64 bits. This allows CONST to represent negative I64 values in the range -2,147,483,648 to 2,147,483,647. For U64, the 32-bit value is **zero-extended**. For F64, CONST is not valid — use CONST_EXT.

### 4.3 Arithmetic

| Opcode | Value | Args | Description |
|--------|-------|------|-------------|
| ADD    | 0x10  | -    | Pop two values, push their sum. Same-type numeric only. |
| SUB    | 0x11  | -    | Pop two values, push (second_popped - first_popped). |
| MUL    | 0x12  | -    | Pop two values, push their product. |
| DIV    | 0x13  | -    | Pop two values, push quotient. Division by zero is a runtime error (VM halts with error). |
| MOD    | 0x14  | -    | Pop two values, push remainder. I64/U64 only (not F64). |
| NEG    | 0x15  | -    | Pop one value, push its negation. I64/F64 only. |

**Float safety:** Any floating-point operation that produces NaN or infinity is a runtime error. The VM halts with an error including the instruction index. This guarantee means NaN and infinity never exist as values in a running NoLang program.

### 4.4 Comparison

| Opcode | Value | Args | Description |
|--------|-------|------|-------------|
| EQ     | 0x20  | -    | Pop two values, push BOOL (1 if equal). |
| NEQ    | 0x21  | -    | Pop two values, push BOOL (1 if not equal). |
| LT     | 0x22  | -    | Pop two, push BOOL (1 if second_popped < first_popped). |
| GT     | 0x23  | -    | Pop two, push BOOL (1 if second_popped > first_popped). |
| LTE    | 0x24  | -    | Pop two, push BOOL (1 if second_popped <= first_popped). |
| GTE    | 0x25  | -    | Pop two, push BOOL (1 if second_popped >= first_popped). |

### 4.5 Logic & Bitwise

| Opcode | Value | Args | Description |
|--------|-------|------|-------------|
| AND    | 0x30  | -    | Bitwise AND for integers. Logical AND for BOOL. |
| OR     | 0x31  | -    | Bitwise OR for integers. Logical OR for BOOL. |
| NOT    | 0x32  | -    | Bitwise NOT for integers. Logical NOT for BOOL. |
| XOR    | 0x33  | -    | Bitwise XOR for integers. Logical XOR for BOOL. |
| SHL    | 0x34  | -    | Shift left. Pop shift amount, pop value, push result. |
| SHR    | 0x35  | -    | Shift right (arithmetic for I64, logical for U64). |

### 4.6 Control Flow — Pattern Matching

**This is the ONLY control flow mechanism.** There are no if/else, while, for, or switch statements.

| Opcode  | Value | Args                 | Description |
|---------|-------|----------------------|-------------|
| MATCH   | 0x40  | arg1=variant_count   | Pop top of stack (must be VARIANT or BOOL or integer). Begin pattern match block. Exactly `arg1` CASE instructions must follow. |
| CASE    | 0x41  | arg1=tag, arg2=body_len | Match case for variant tag `arg1`. The next `arg2` instructions are the case body. If the matched value's tag equals `arg1`, execute body. |
| EXHAUST | 0x42  | -                    | End of MATCH block. Verification: the number of CASE instructions between MATCH and EXHAUST must equal the variant_count in MATCH. |

**Pattern matching on non-VARIANT types:**
- **BOOL**: variant_count=2. CASE tag 0 = false, CASE tag 1 = true.
- **MAYBE**: variant_count=2. CASE tag 0 = SOME (payload on stack), CASE tag 1 = NONE.
- **RESULT**: variant_count=2. CASE tag 0 = OK (payload on stack), CASE tag 1 = ERR (error on stack).
- **I64/U64**: not directly matchable. Use comparison + BOOL match instead.

**CASE body semantics:**
- If the variant has a payload, it is pushed onto the stack before the body executes.
- The body must leave exactly one value on the stack (the match result).
- All CASE bodies must produce values of the same type.

### 4.7 Functions

| Opcode   | Value | Args                        | Description |
|----------|-------|-----------------------------|-------------|
| FUNC     | 0x50  | arg1=param_count, arg2=body_len | Begin function definition. Parameters are the top `arg1` stack values, bound as de Bruijn indices 0..arg1-1 (last pushed = index 0). `body_len` is the number of instructions in the function (including PRE, POST, body, and HASH). |
| PRE      | 0x51  | arg1=condition_len          | Precondition block. The next `arg1` instructions compute a BOOL. If false at call time, runtime error. |
| POST     | 0x52  | arg1=condition_len          | Postcondition block. The next `arg1` instructions compute a BOOL given the return value (bound at index 0). If false, runtime error. |
| RET      | 0x53  | -                           | Return top of stack from current function. |
| CALL     | 0x54  | arg1=func_ref               | Call the function at de Bruijn index `arg1`. Arguments must already be on the stack. |
| RECURSE  | 0x55  | arg1=depth_limit            | Recursive call to the enclosing function. `arg1` is the maximum recursion depth. Exceeding it is a runtime error, not undefined behavior. |
| ENDFUNC  | 0x56  | -                           | End of function block. |

**Function structure (must appear in this exact order):**
```
FUNC param_count body_len
  PRE condition_len        (0 or more PRE blocks)
    ...condition...
  POST condition_len       (0 or more POST blocks)
    ...condition...
  ...body instructions...
  RET
  HASH hash_value
ENDFUNC
```

### 4.8 Data Construction

| Opcode    | Value | Args                        | Description |
|-----------|-------|-----------------------------|-------------|
| VARIANT_NEW | 0x60 | type_tag=VARIANT, arg1=total_tags, arg2=this_tag | Pop payload from stack, construct variant value with tag `arg2` out of `arg1` possible tags. |
| TUPLE_NEW | 0x61  | type_tag=TUPLE, arg1=field_count | Pop `arg1` values from stack, construct tuple. First popped = last field. |
| PROJECT   | 0x62  | arg1=field_index            | Pop tuple from stack, push the value at field `arg1`. |
| ARRAY_NEW | 0x63  | type_tag=ARRAY, arg1=length | Pop `arg1` values from stack (all same type), construct array. |
| ARRAY_GET | 0x64  | -                           | Pop index (U64), pop array, push element. Out-of-bounds is runtime error. |
| ARRAY_LEN | 0x65  | -                           | Pop array, push its length as U64. |

### 4.9 Verification & Meta

| Opcode  | Value | Args              | Description |
|---------|-------|-------------------|-------------|
| HASH    | 0x70  | arg1, arg2, arg3  | The 48-bit truncated blake3 hash of all instructions in the enclosing FUNC block (excluding the HASH instruction itself and ENDFUNC). Concatenate arg1, arg2, arg3 as big-endian for the 48-bit value. |
| ASSERT  | 0x71  | -                 | Pop BOOL from stack. If false, runtime error with instruction index. |
| TYPEOF  | 0x72  | arg1=expected_tag | Pop value, push BOOL (1 if value's type tag matches `arg1`). Non-destructive: value is pushed back. |

### 4.10 VM Control

| Opcode | Value | Args | Description |
|--------|-------|------|-------------|
| HALT   | 0xFE  | -    | Stop execution. Top of stack is the program result. |
| NOP    | 0xFF  | -    | No operation. Exists for alignment. Must have all arg fields = 0. |

### 4.11 Reserved Ranges

| Range       | Purpose                    |
|-------------|----------------------------|
| 0x00        | ILLEGAL — always rejected  |
| 0x06–0x0F   | Reserved: future binding ops |
| 0x16–0x1F   | Reserved: future arithmetic |
| 0x26–0x2F   | Reserved: future comparison |
| 0x36–0x3F   | Reserved: future logic ops  |
| 0x43–0x4F   | Reserved: future control flow |
| 0x57–0x5F   | Reserved: future function ops |
| 0x66–0x6F   | Reserved: future data ops   |
| 0x73–0x7F   | Reserved: future meta ops   |
| 0x80–0xFD   | Reserved: future expansion  |

Any instruction with a reserved opcode is rejected by the verifier.

## 5. Program Structure

A valid NoLang program is a sequence of 64-bit instructions with the following structure:

```
[top-level function definitions]
[entry point instructions]
HALT
```

**Rules:**
1. The program must end with HALT.
2. Function definitions (FUNC..ENDFUNC) may only appear at the top level, not nested.
3. Every FUNC block must contain exactly one HASH instruction as its second-to-last instruction (before ENDFUNC).
4. Every FUNC block must contain exactly one RET instruction.
5. The entry point is the first instruction after the last ENDFUNC (or the first instruction if there are no functions).
6. The stack must contain exactly one value when HALT is reached.

## 6. Hash Computation

HASH values are computed as follows:

1. Collect all instruction bytes in the FUNC block from FUNC (inclusive) through the instruction before HASH (inclusive).
2. Compute blake3 hash of these bytes.
3. Truncate to 48 bits (first 6 bytes of the hash output).
4. Store as: arg1 = bytes[0..2] big-endian, arg2 = bytes[2..4] big-endian, arg3 = bytes[4..6] big-endian.

The verifier recomputes this hash and rejects any FUNC block where it doesn't match.

## 7. Canonical Ordering Rules

To ensure exactly one representation per computation:

1. **CASE branches in a MATCH must appear in ascending tag order.** CASE tag=0 before CASE tag=1 before CASE tag=2.
2. **PRE blocks appear before POST blocks in a function.**
3. **Function definitions appear before the entry point.**
4. **Constants use the smallest encoding.** If a value fits in CONST (32-bit), CONST_EXT must not be used.
5. **No dead code.** Every instruction must be reachable. Unreachable instructions are a verification error.

## 8. Stack Discipline

- The stack starts empty.
- Every instruction's effect on stack depth is deterministic and knowable statically.
- At the end of each CASE body, the stack depth must be exactly: (depth at MATCH entry - 1 + 1). That is, the matched value is consumed and one result is produced.
- At HALT, the stack must contain exactly one value.
- Stack underflow is a verification error (caught statically), never a runtime error.

## 9. Limits

| Limit                   | Value   | Rationale                            |
|-------------------------|---------|--------------------------------------|
| Max program size        | 65,536 instructions | 512KB. Keeps programs tractable for LLM generation. |
| Max stack depth         | 4,096 slots | Prevents unbounded memory use. |
| Max binding depth       | 4,096   | Same as stack depth.                 |
| Max recursion depth     | 1,024   | Per RECURSE instruction arg.         |
| Max function params     | 256     | Limited by arg1 field (u16, but practical limit). |
| Max variant tags        | 256     | Practical limit for exhaustive matching. |
| Max tuple fields        | 256     | Practical limit.                     |
| Max array length        | 65,535  | Limited by u16 arg field.            |
