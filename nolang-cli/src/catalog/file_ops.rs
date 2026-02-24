//! File and path operation programs (10): ex255-ex264.
//!
//! Standalone programs demonstrating file I/O, directory, and path opcodes:
//! FILE_READ, FILE_WRITE, FILE_APPEND, FILE_EXISTS, FILE_DELETE,
//! DIR_LIST, DIR_MAKE, PATH_JOIN, PATH_PARENT.
//!
//! All programs use functions with PARAM PATH so the type checker can track
//! path values precisely.  The entry point passes CONST PATH (which satisfies
//! the type checker) and calls the function.
//!
//! No witness cases are defined because the WitnessValue enum does not carry
//! Path or Bytes values, and because these programs require a real filesystem
//! to execute meaningfully.

use super::{Category, ProgramSpec};

pub fn programs() -> Vec<ProgramSpec> {
    vec![
        // ex255: FILE_EXISTS — check whether a path exists
        ProgramSpec {
            id: "ex255_file_exists",
            intent: "Check whether a given path exists on the filesystem",
            assembly_template: concat!(
                "FUNC 1 5\n",
                "PARAM PATH\n",
                "REF 0\n",
                "FILE_EXISTS\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST PATH 0x0000 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::FileOps,
        },

        // ex256: FILE_READ — read file contents at a path
        ProgramSpec {
            id: "ex256_file_read",
            intent: "Read the byte contents of a file at a given path, returning a Result",
            assembly_template: concat!(
                "FUNC 1 5\n",
                "PARAM PATH\n",
                "REF 0\n",
                "FILE_READ\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST PATH 0x0000 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::FileOps,
        },

        // ex257: FILE_WRITE — write bytes to a path
        ProgramSpec {
            id: "ex257_file_write",
            intent: "Write a byte buffer to a file at a given path, returning a Result",
            assembly_template: concat!(
                "FUNC 2 7\n",
                "PARAM PATH\n",
                "PARAM BYTES\n",
                "REF 1\n",
                "REF 0\n",
                "FILE_WRITE\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST PATH 0x0000 0x0000\n",
                "STR_CONST \"data\"\n",
                "STR_BYTES\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::FileOps,
        },

        // ex258: FILE_APPEND — append bytes to a path
        ProgramSpec {
            id: "ex258_file_append",
            intent: "Append a byte buffer to a file at a given path, returning a Result",
            assembly_template: concat!(
                "FUNC 2 7\n",
                "PARAM PATH\n",
                "PARAM BYTES\n",
                "REF 1\n",
                "REF 0\n",
                "FILE_APPEND\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST PATH 0x0000 0x0000\n",
                "STR_CONST \"more\"\n",
                "STR_BYTES\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::FileOps,
        },

        // ex259: FILE_DELETE — delete a file at a path
        ProgramSpec {
            id: "ex259_file_delete",
            intent: "Delete the file at a given path, returning a Result",
            assembly_template: concat!(
                "FUNC 1 5\n",
                "PARAM PATH\n",
                "REF 0\n",
                "FILE_DELETE\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST PATH 0x0000 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::FileOps,
        },

        // ex260: DIR_LIST — list directory contents
        ProgramSpec {
            id: "ex260_dir_list",
            intent: "List the entries of a directory at a given path, returning a Result",
            assembly_template: concat!(
                "FUNC 1 5\n",
                "PARAM PATH\n",
                "REF 0\n",
                "DIR_LIST\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST PATH 0x0000 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::FileOps,
        },

        // ex261: DIR_MAKE — create a directory
        ProgramSpec {
            id: "ex261_dir_make",
            intent: "Create a directory at a given path, returning a Result",
            assembly_template: concat!(
                "FUNC 1 5\n",
                "PARAM PATH\n",
                "REF 0\n",
                "DIR_MAKE\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST PATH 0x0000 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::FileOps,
        },

        // ex262: PATH_JOIN — join a path with a component string
        ProgramSpec {
            id: "ex262_path_join",
            intent: "Append a path component string to a base path, yielding a new path",
            assembly_template: concat!(
                "FUNC 2 7\n",
                "PARAM PATH\n",
                "PARAM STRING\n",
                "REF 1\n",
                "REF 0\n",
                "PATH_JOIN\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST PATH 0x0000 0x0000\n",
                "STR_CONST \"subdir\"\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::FileOps,
        },

        // ex263: PATH_PARENT — get the parent of a path
        ProgramSpec {
            id: "ex263_path_parent",
            intent: "Return the parent directory of a given path, as a Maybe(Path)",
            assembly_template: concat!(
                "FUNC 1 5\n",
                "PARAM PATH\n",
                "REF 0\n",
                "PATH_PARENT\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST PATH 0x0000 0x0000\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::FileOps,
        },

        // ex264: PATH_JOIN then FILE_EXISTS — build and check a path
        ProgramSpec {
            id: "ex264_path_join_exists",
            intent: "Join a base path with a component and check whether the resulting path exists",
            assembly_template: concat!(
                "FUNC 2 8\n",
                "PARAM PATH\n",
                "PARAM STRING\n",
                "REF 1\n",
                "REF 0\n",
                "PATH_JOIN\n",
                "FILE_EXISTS\n",
                "RET\n",
                "HASH 0x0000 0x0000 0x0000\n",
                "ENDFUNC\n",
                "CONST PATH 0x0000 0x0000\n",
                "STR_CONST \"readme.txt\"\n",
                "CALL 0\n",
                "HALT\n",
            ),
            witness_cases: vec![],
            category: Category::FileOps,
        },
    ]
}
