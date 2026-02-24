//! Process execution programs (5): ex265-ex269.
//!
//! Standalone programs demonstrating EXEC_SPAWN and EXEC_CHECK opcodes.
//!
//! EXEC_SPAWN pops an Array (argument vector of strings) and returns a Result.
//! EXEC_CHECK pops a Tuple (process exit struct) and returns a Result.
//!
//! No witness cases are defined because these programs require a real process
//! execution environment and the WitnessValue enum does not carry Array values.

use super::{Category, ProgramSpec};

pub fn programs() -> Vec<ProgramSpec> {
    vec![
        // ex265: spawn with an empty argument vector
        ProgramSpec {
            id: "ex265_exec_spawn_empty",
            intent: "Attempt to spawn a process with an empty argument vector, returning a Result",
            assembly_template: concat!(
                "ARRAY_NEW STRING 0\n",
                "EXEC_SPAWN\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ProcessOps,
        },

        // ex266: spawn function that takes an ARRAY parameter
        ProgramSpec {
            id: "ex266_exec_spawn_func",
            intent: "Function that takes an Array argument vector and spawns a process",
            assembly_template: concat!(
                "FUNC 1 5\n",
                "PARAM ARRAY\n",
                "REF 0\n",
                "EXEC_SPAWN\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "ARRAY_NEW STRING 0\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ProcessOps,
        },

        // ex267: exec_check function that takes a TUPLE parameter
        ProgramSpec {
            id: "ex267_exec_check_func",
            intent: "Function that takes a process-result Tuple and checks its exit status",
            assembly_template: concat!(
                "FUNC 1 5\n",
                "PARAM TUPLE\n",
                "REF 0\n",
                "EXEC_CHECK\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "ARRAY_NEW STRING 0\n",
                "EXEC_SPAWN\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ProcessOps,
        },

        // ex268: build argv array then spawn
        ProgramSpec {
            id: "ex268_exec_spawn_argv",
            intent: "Build a one-element argument vector and pass it to EXEC_SPAWN",
            assembly_template: concat!(
                "STR_CONST \"echo\"\n",
                "ARRAY_NEW STRING 1\n",
                "EXEC_SPAWN\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ProcessOps,
        },

        // ex269: build two-element argv and spawn
        ProgramSpec {
            id: "ex269_exec_spawn_argv2",
            intent: "Build a two-element argument vector with program and argument, then spawn",
            assembly_template: concat!(
                "STR_CONST \"echo\"\n",
                "STR_CONST \"hello\"\n",
                "ARRAY_NEW STRING 2\n",
                "EXEC_SPAWN\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::ProcessOps,
        },
    ]
}
