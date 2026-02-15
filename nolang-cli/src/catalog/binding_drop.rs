//! Binding and drop programs (10): ex220-ex229.
//!
//! Standalone programs demonstrating BIND, REF, and DROP operations.

use super::{Category, ProgramSpec};

pub fn programs() -> Vec<ProgramSpec> {
    vec![
        // ex220: bind and reference
        ProgramSpec {
            id: "ex220_bind_ref",
            intent: "Bind a value, reference it, drop the binding, and return",
            assembly_template: concat!(
                "CONST I64 0x0000 0x002a\n",
                "BIND\n",
                "REF 0\n",
                "DROP\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::BindingDrop,
        },
        // ex221: bind two values
        ProgramSpec {
            id: "ex221_bind_two",
            intent: "Bind two values, reference both, add them, drop bindings, and return",
            assembly_template: concat!(
                "CONST I64 0x0000 0x000a\n",
                "CONST I64 0x0000 0x0014\n",
                "BIND\n",
                "BIND\n",
                "REF 0\n",
                "REF 1\n",
                "ADD\n",
                "DROP\n",
                "DROP\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::BindingDrop,
        },
        // ex222: bind then push new value
        ProgramSpec {
            id: "ex222_bind_drop",
            intent: "Bind a value, push a new value, drop the binding, and return new value",
            assembly_template: concat!(
                "CONST I64 0x0000 0x002a\n",
                "BIND\n",
                "CONST I64 0x0000 0x0063\n",
                "DROP\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::BindingDrop,
        },
        // ex223: shadowing binding
        ProgramSpec {
            id: "ex223_shadow_binding",
            intent: "Bind two values, reference the most recent, drop both, and return",
            assembly_template: concat!(
                "CONST I64 0x0000 0x000a\n",
                "BIND\n",
                "CONST I64 0x0000 0x0014\n",
                "BIND\n",
                "REF 0\n",
                "DROP\n",
                "DROP\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::BindingDrop,
        },
        // ex224: deep reference
        ProgramSpec {
            id: "ex224_deep_ref",
            intent: "Bind three values, reference the oldest (index 2), drop all, and return",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "BIND\n",
                "CONST I64 0x0000 0x0002\n",
                "BIND\n",
                "CONST I64 0x0000 0x0003\n",
                "BIND\n",
                "REF 2\n",
                "DROP\n",
                "DROP\n",
                "DROP\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::BindingDrop,
        },
        // ex225: bind, use multiple times
        ProgramSpec {
            id: "ex225_bind_use_drop",
            intent: "Bind a value, reference it twice, multiply, drop the binding, and return",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0005\n",
                "BIND\n",
                "REF 0\n",
                "REF 0\n",
                "MUL\n",
                "DROP\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::BindingDrop,
        },
        // ex226: rebind after drop
        ProgramSpec {
            id: "ex226_rebind_after_drop",
            intent: "Bind value, reference it, drop, bind new value, reference twice, add, drop, and return",
            assembly_template: concat!(
                "CONST I64 0x0000 0x000a\n",
                "BIND\n",
                "CONST I64 0x0000 0x0014\n",
                "BIND\n",
                "REF 1\n",
                "DROP\n",
                "REF 0\n",
                "ADD\n",
                "DROP\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::BindingDrop,
        },
        // ex227: three bindings sum
        ProgramSpec {
            id: "ex227_three_binds",
            intent: "Bind three values, reference all, sum them, drop all, and return",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0001\n",
                "BIND\n",
                "CONST I64 0x0000 0x0002\n",
                "BIND\n",
                "CONST I64 0x0000 0x0003\n",
                "BIND\n",
                "REF 0\n",
                "REF 1\n",
                "REF 2\n",
                "ADD\n",
                "ADD\n",
                "DROP\n",
                "DROP\n",
                "DROP\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::BindingDrop,
        },
        // ex228: bind in match case
        ProgramSpec {
            id: "ex228_bind_in_match",
            intent: "Match on Bool, bind value in case body, reference and drop, then return",
            assembly_template: concat!(
                "CONST BOOL 0x0001 0x0000\n",
                "MATCH 2\n",
                "CASE 0 1\n",
                "CONST I64 0x0000 0x0000\n",
                "CASE 1 4\n",
                "CONST I64 0x0000 0x002a\n",
                "BIND\n",
                "REF 0\n",
                "DROP\n",
                "EXHAUST\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::BindingDrop,
        },
        // ex229: nested bind and compute
        ProgramSpec {
            id: "ex229_nested_bind",
            intent: "Bind value, compute square, bind result, add original and square, drop all, return",
            assembly_template: concat!(
                "CONST I64 0x0000 0x0003\n",
                "BIND\n",
                "REF 0\n",
                "REF 0\n",
                "MUL\n",
                "BIND\n",
                "REF 0\n",
                "REF 1\n",
                "ADD\n",
                "DROP\n",
                "DROP\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::BindingDrop,
        },
    ]
}
