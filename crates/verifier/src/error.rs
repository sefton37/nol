//! Verification errors for the NoLang verifier.
//!
//! Every error includes an instruction index (`at`) for precise error reporting.
//! The verifier collects ALL errors, not just the first.

use nolang_common::TypeTag;
use thiserror::Error;

/// Errors found during static verification.
///
/// Each variant corresponds to a specific verification check from
/// ARCHITECTURE.md. All variants include location information.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VerifyError {
    // --- Structural ---
    /// Program does not end with HALT.
    #[error("program does not end with HALT")]
    MissingHalt,

    /// FUNC without matching ENDFUNC (or vice versa).
    #[error("unmatched FUNC at instruction {at}")]
    UnmatchedFunc { at: usize },

    /// MATCH without matching EXHAUST (or vice versa).
    #[error("unmatched MATCH at instruction {at}")]
    UnmatchedMatch { at: usize },

    /// FUNC block nested inside another FUNC block.
    #[error("nested FUNC at instruction {at}")]
    NestedFunc { at: usize },

    /// CASE branches not in ascending tag order.
    #[error(
        "CASE order violation at instruction {at}: expected tag {expected_tag}, found {found_tag}"
    )]
    CaseOrderViolation {
        at: usize,
        expected_tag: u16,
        found_tag: u16,
    },

    /// Unused argument field is nonzero.
    #[error("non-zero unused field at instruction {at}")]
    NonZeroUnusedField { at: usize },

    // --- Type Safety ---
    /// Types do not match where they should.
    #[error("type mismatch at instruction {at}: expected {expected:?}, found {found:?}")]
    TypeMismatch {
        at: usize,
        expected: TypeTag,
        found: TypeTag,
    },

    /// REF to a de Bruijn index beyond the current binding depth.
    #[error("unresolvable REF at instruction {at}: index {index}, binding depth {binding_depth}")]
    UnresolvableRef {
        at: usize,
        index: u16,
        binding_depth: u16,
    },

    // --- Exhaustion ---
    /// MATCH does not have the right number of CASE branches.
    #[error("non-exhaustive match at instruction {at}: expected {expected} cases, found {found}")]
    NonExhaustiveMatch {
        at: usize,
        expected: u16,
        found: u16,
    },

    /// Duplicate CASE tag in a MATCH block.
    #[error("duplicate CASE tag {tag} at instruction {at}")]
    DuplicateCase { at: usize, tag: u16 },

    // --- Hash ---
    /// HASH value does not match recomputed hash.
    #[error(
        "hash mismatch at instruction {at}: expected {expected:02x?}, computed {computed:02x?}"
    )]
    HashMismatch {
        at: usize,
        expected: [u8; 6],
        computed: [u8; 6],
    },

    /// FUNC block has no HASH instruction.
    #[error("missing HASH in FUNC at instruction {func_at}")]
    MissingHash { func_at: usize },

    // --- Contracts ---
    /// PRE block does not produce a BOOL.
    #[error("PRE condition does not produce BOOL at instruction {at}")]
    PreConditionNotBool { at: usize },

    /// POST block does not produce a BOOL.
    #[error("POST condition does not produce BOOL at instruction {at}")]
    PostConditionNotBool { at: usize },

    // --- Reachability ---
    /// Instruction is unreachable from any entry point.
    #[error("unreachable instruction at {at}")]
    UnreachableInstruction { at: usize },

    // --- Stack ---
    /// Stack underflow detected statically.
    #[error("stack underflow at instruction {at}")]
    StackUnderflow { at: usize },

    /// Stack has wrong depth at HALT.
    #[error("unbalanced stack at HALT (instruction {at_halt}): depth {depth}, expected 1")]
    UnbalancedStack { at_halt: usize, depth: usize },

    // --- Limits ---
    /// Program exceeds maximum size.
    #[error("program too large: {size} instructions (max 65536)")]
    ProgramTooLarge { size: usize },

    /// REF index exceeds limit.
    #[error("REF index {index} too deep at instruction {at}")]
    RefTooDeep { at: usize, index: u16 },

    /// RECURSE depth limit exceeds maximum.
    #[error("recursion limit {limit} too high at instruction {at}")]
    RecursionLimitTooHigh { at: usize, limit: u16 },

    // --- PARAM ---
    /// PARAM count does not match FUNC param_count.
    #[error("PARAM count mismatch in FUNC at {at}: expected {expected}, found {found}")]
    ParamCountMismatch {
        at: usize,
        expected: u16,
        found: u16,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_variants_display() {
        // Ensure all variants have valid Display implementations
        let errors: Vec<VerifyError> = vec![
            VerifyError::MissingHalt,
            VerifyError::UnmatchedFunc { at: 0 },
            VerifyError::UnmatchedMatch { at: 0 },
            VerifyError::NestedFunc { at: 0 },
            VerifyError::CaseOrderViolation {
                at: 0,
                expected_tag: 0,
                found_tag: 1,
            },
            VerifyError::NonZeroUnusedField { at: 0 },
            VerifyError::TypeMismatch {
                at: 0,
                expected: TypeTag::I64,
                found: TypeTag::Bool,
            },
            VerifyError::UnresolvableRef {
                at: 0,
                index: 5,
                binding_depth: 2,
            },
            VerifyError::NonExhaustiveMatch {
                at: 0,
                expected: 3,
                found: 2,
            },
            VerifyError::DuplicateCase { at: 0, tag: 1 },
            VerifyError::HashMismatch {
                at: 0,
                expected: [0; 6],
                computed: [1; 6],
            },
            VerifyError::MissingHash { func_at: 0 },
            VerifyError::PreConditionNotBool { at: 0 },
            VerifyError::PostConditionNotBool { at: 0 },
            VerifyError::UnreachableInstruction { at: 0 },
            VerifyError::StackUnderflow { at: 0 },
            VerifyError::UnbalancedStack {
                at_halt: 0,
                depth: 3,
            },
            VerifyError::ProgramTooLarge { size: 70000 },
            VerifyError::RefTooDeep { at: 0, index: 5000 },
            VerifyError::RecursionLimitTooHigh { at: 0, limit: 2000 },
            VerifyError::ParamCountMismatch {
                at: 0,
                expected: 2,
                found: 1,
            },
        ];

        for error in &errors {
            let display = error.to_string();
            assert!(!display.is_empty(), "empty display for {error:?}");
        }
        // 21 variants from ARCHITECTURE.md + ParamCountMismatch
        assert_eq!(errors.len(), 21);
    }
}
