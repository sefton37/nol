# NoLang Example Programs

These programs serve three purposes:
1. **Test cases** — every example must assemble, verify, and execute correctly
2. **Training seeds** — each includes a natural language intent description
3. **Specification grounding** — if the spec is ambiguous, these examples are the tiebreaker

Each example includes: intent, assembly, expected output, and notes.

---

## Example 1: Constant Return

**Intent:** Return the integer 42.

```nol
; The simplest valid program
CONST I64 0 42      ; push I64 value 42 (high=0, low=42)
HALT                 ; stop, return top of stack
```

**Expected output:** `I64(42)`

**Notes:** This is the minimal valid program. The stack has exactly one value at HALT.

---

## Example 2: Addition

**Intent:** Add 5 and 3, return the result.

```nol
CONST I64 0 5        ; push 5
CONST I64 0 3        ; push 3
ADD                   ; pop both, push 8
HALT
```

**Expected output:** `I64(8)`

**Notes:** ADD pops two values. The result type matches the input type (I64).

---

## Example 3: Boolean Match

**Intent:** If true, return 1. If false, return 0.

```nol
CONST BOOL 1 0       ; push true (arg1=1 means true)
MATCH 2              ; match on BOOL, expect 2 cases
  CASE 0 2           ; tag 0 = false, body is 2 instructions
    CONST I64 0 0    ; push 0
    NOP              ; (padding to show body boundaries)
  CASE 1 2           ; tag 1 = true, body is 2 instructions
    CONST I64 0 1    ; push 1
    NOP
EXHAUST              ; all cases covered
HALT
```

**Expected output:** `I64(1)` (because we pushed true)

**Notes:** 
- CASE branches MUST be in ascending tag order (0 before 1)
- Both CASE bodies produce I64 (required: same type)
- Each CASE body leaves exactly one value on the stack

---

## Example 4: Simple Function

**Intent:** Define a function that doubles an I64, then call it with 21.

```nol
; Function: double(x) = x + x
FUNC 1 7             ; 1 parameter, 7 instructions in body
  PRE 3              ; precondition: 3 instructions
    REF 0            ; push the parameter
    TYPEOF I64       ; check type (pushes BOOL, pops+repushes value)
    ASSERT           ; pop BOOL, halt if false
  REF 0              ; push parameter (de Bruijn index 0 = most recent binding)
  REF 0              ; push parameter again
  ADD                ; x + x
  RET                ; return sum
  HASH 0x0000 0x0000 0x0000  ; placeholder — compute with `nolang hash`
ENDFUNC

; Entry point
CONST I64 0 21       ; push 21
CALL 0               ; call function at binding index 0 (the double function)
HALT
```

**Expected output:** `I64(42)`

**Notes:**
- The HASH value shown is a placeholder. The real value must be computed with blake3.
- CALL 0 refers to de Bruijn index 0 — the function was defined first, so it's in the binding environment.
- The function expects 1 parameter already on the stack when called.
- TYPEOF is non-destructive — it pushes BOOL without consuming the value.

---

## Example 5: Maybe Type Handling

**Intent:** Given a MAYBE(I64) that contains a value, extract it and add 10. If empty, return 0.

```nol
; Construct SOME(5)
CONST I64 0 5                  ; push 5 (the payload)
VARIANT_NEW VARIANT 2 0        ; 2 total tags, this is tag 0 (SOME)

; Handle the maybe
MATCH 2                         ; match on 2 variants
  CASE 0 3                     ; tag 0 = SOME: payload is on stack
    BIND                        ; bind the payload
    REF 0                       ; push payload
    CONST I64 0 10              ; push 10
  ; (oops, ADD needs to be inside CASE body)
  CASE 1 1                     ; tag 1 = NONE: no payload
    CONST I64 0 0              ; return 0
EXHAUST
HALT
```

**Wait — this example has a bug.** The CASE 0 body is 3 instructions but we need ADD too. Let me fix it:

```nol
; Construct SOME(5)
CONST I64 0 5                  ; push 5 (the payload)
VARIANT_NEW VARIANT 2 0        ; 2 total tags, this is tag 0 (SOME)

; Handle the maybe
MATCH 2                         ; match on 2 variants
  CASE 0 4                     ; tag 0 = SOME: payload is on stack, 4 instr body
    BIND                        ; bind the payload
    REF 0                       ; push payload
    CONST I64 0 10              ; push 10
    ADD                         ; payload + 10
  CASE 1 1                     ; tag 1 = NONE: no payload, 1 instr body
    CONST I64 0 0              ; return 0
EXHAUST
HALT
```

**Expected output:** `I64(15)`

**Notes:**
- VARIANT_NEW with tag 0 out of 2 total tags creates SOME
- In CASE 0, the variant's payload is automatically pushed onto the stack
- We BIND it so we can REF it (otherwise it's just a loose stack value)
- Both branches produce I64

---

## Example 6: Recursive Factorial

**Intent:** Compute factorial(5) recursively.

```nol
; Function: factorial(n)
; if n == 0, return 1
; else return n * factorial(n - 1)
FUNC 1 18            ; 1 parameter, 18 instructions
  ; Check: is n == 0?
  REF 0              ; push n
  CONST I64 0 0      ; push 0
  EQ                 ; push BOOL (n == 0?)

  ; Branch on the boolean
  MATCH 2            ; match on BOOL
    CASE 0 2         ; false: n != 0
      NOP            ; (body below)
      NOP
    CASE 1 2         ; true: n == 0
      CONST I64 0 1  ; return 1
      NOP
  EXHAUST

  ; Wait — this structure doesn't work cleanly because we need
  ; the false branch to do recursion. Let me restructure.
ENDFUNC
```

**Let me write this more carefully:**

```nol
; Function: factorial(n) -> I64
; Base case: n <= 1 → return 1
; Recursive: n * factorial(n-1)
FUNC 1 17            ; 1 parameter, 17 instructions in body
  ; Compare n <= 1
  REF 0              ; push n
  CONST I64 0 1      ; push 1
  LTE                ; push BOOL (n <= 1?)

  ; Branch
  MATCH 2            ; match on BOOL
    CASE 0 8         ; false branch (n > 1): compute n * fact(n-1)
      REF 0          ; push n
      REF 0          ; push n again
      CONST I64 0 1  ; push 1
      SUB             ; n - 1
      RECURSE 100    ; factorial(n-1), max depth 100
      MUL             ; n * factorial(n-1)
      NOP             ; padding
      NOP
    CASE 1 2         ; true branch (n <= 1): return 1
      CONST I64 0 1  ; push 1
      NOP
  EXHAUST

  RET
  HASH 0x0000 0x0000 0x0000  ; placeholder
ENDFUNC

; Entry point
CONST I64 0 5        ; push 5
CALL 0               ; call factorial
HALT
```

**Expected output:** `I64(120)`

**Notes:**
- RECURSE 100 means max recursion depth is 100
- RECURSE reuses the enclosing function — no need to reference it by index
- The CASE body lengths must account for ALL instructions in each branch
- Both CASE bodies must leave exactly one value on the stack
- NOP padding ensures body lengths are correct (both branches must produce same stack effect)
- **IMPORTANT**: The NOP padding issue suggests we may want CASE bodies to not require fixed instruction counts, but rather be delimited. This is a spec refinement candidate — for now, padding with NOP is the canonical approach.

---

## Example 7: Tuple Construction and Projection

**Intent:** Create a pair (3, 7), extract the second element, return it.

```nol
CONST I64 0 3        ; push 3
CONST I64 0 7        ; push 7
TUPLE_NEW TUPLE 2    ; pop 2 values, make tuple. First popped (7) = last field.
PROJECT 1            ; extract field index 1 (second field = 7)
HALT
```

**Wait — need to verify the field ordering.** Per SPEC: "First popped = last field."
So stack is [3, 7] (7 on top). Pop 7 first → field 1. Pop 3 → field 0.
`PROJECT 1` gets field 1, which is 7. ✓

**Expected output:** `I64(7)`

---

## Example 8: Array Operations

**Intent:** Create an array [10, 20, 30], get the element at index 1, return it.

```nol
CONST I64 0 10       ; push 10
CONST I64 0 20       ; push 20
CONST I64 0 30       ; push 30
ARRAY_NEW ARRAY 3    ; pop 3 values, make array
CONST U64 0 1        ; push index 1 (indices are U64)
ARRAY_GET             ; pop index, pop array, push element
HALT
```

**Expected output:** `I64(20)`

**Notes:** Array indices are U64. Out-of-bounds is a runtime error.

---

## Example 9: Function with Pre and Post Conditions

**Intent:** Define an absolute value function for I64. Precondition: input is I64. Postcondition: result >= 0.

```nol
; Function: abs(x) -> I64
FUNC 1 19
  PRE 3                        ; input is I64
    REF 0
    TYPEOF I64
    ASSERT
  POST 4                       ; result >= 0
    REF 0                      ; return value is at index 0 in POST context
    CONST I64 0 0
    GTE                        ; result >= 0?
    ASSERT

  ; Body: if x < 0, negate it
  REF 0                        ; push x
  CONST I64 0 0                ; push 0
  LT                           ; x < 0?

  MATCH 2
    CASE 0 1                   ; false (x >= 0): return x as-is
      REF 0                    ; push x
    CASE 1 2                   ; true (x < 0): negate
      REF 0                    ; push x
      NEG                      ; -x
  EXHAUST

  RET
  HASH 0x0000 0x0000 0x0000   ; placeholder
ENDFUNC

; Entry point
CONST I64 0xFFFF 0xFFF3       ; push -13 (two's complement: 0xFFFFFFF3 = -13 in 32-bit)
CALL 0
HALT
```

**Expected output:** `I64(13)`

**Notes:**
- Negative constants in 32-bit require careful encoding
- For full 64-bit negative values, use CONST_EXT
- POST condition: the return value is available at de Bruijn index 0

---

## Summary: Test Matrix

| Example | Opcodes Tested | Key Feature |
|---------|---------------|-------------|
| 1       | CONST, HALT | Minimal program |
| 2       | CONST, ADD, HALT | Arithmetic |
| 3       | CONST, MATCH, CASE, EXHAUST, HALT | Boolean branching |
| 4       | FUNC, PRE, TYPEOF, ASSERT, REF, ADD, RET, CALL, HALT | Functions with contracts |
| 5       | VARIANT_NEW, MATCH, CASE, BIND, REF, ADD, HALT | Maybe/Optional handling |
| 6       | FUNC, REF, CONST, LTE, MATCH, CASE, SUB, RECURSE, MUL, RET, CALL, HALT | Recursion |
| 7       | CONST, TUPLE_NEW, PROJECT, HALT | Tuple construction |
| 8       | CONST, ARRAY_NEW, ARRAY_GET, HALT | Array operations |
| 9       | FUNC, PRE, POST, TYPEOF, ASSERT, REF, LT, MATCH, CASE, NEG, RET, CALL, HALT | Full contracts |

**Missing coverage (write additional programs for these):**
- CONST_EXT (64-bit constants)
- DROP (releasing bindings)
- MOD, NEQ, GT, GTE (arithmetic/comparison completeness)
- AND, OR, NOT, XOR, SHL, SHR (logic/bitwise)
- ARRAY_LEN
- RESULT type (Ok/Err handling)
- Multiple functions calling each other
- Programs that should FAIL verification (negative test cases)
