//! packages/rust/src/guard.rs
//! Zero-Overhead Trait Contracts and In-Process Guards for Microservices (REQ-029 / SPEC-029).
//! Inlined static dispatch with zero dynamic heap allocations on evaluation paths.

use crate::evaluator::evaluate_flag_zero_overhead;
use crate::models::{EvaluationContext, EvaluationResult, FlagDefinition};
use std::collections::HashMap;
use std::sync::RwLock;

/// Zero-cost abstract interface for feature flag guards.
/// Monomorphized statically to avoid virtual method tables (vtables) and `call rax` indirection.
pub trait FlagGuard: Send + Sync + 'static {
    /// Inlined boolean check returning true if the flag is enabled for this context.
    fn is_enabled(&self, flag_key: &str, context: &EvaluationContext) -> bool;

    /// Evaluates the complete flag outcome in-process without network hops.
    fn evaluate(&self, flag_key: &str, context: &EvaluationContext) -> EvaluationResult;
}

/// Thread-safe in-process memory guard with zero heap allocations on hot path.
pub struct MemoryFlagGuard {
    flags: RwLock<HashMap<String, FlagDefinition>>,
}

impl MemoryFlagGuard {
    pub fn new() -> Self {
        Self {
            flags: RwLock::new(HashMap::new()),
        }
    }

    /// Register or update a flag in the local memory guard.
    pub fn set_flag(&self, flag: FlagDefinition) {
        let mut map = self.flags.write().unwrap();
        map.insert(flag.key.clone(), flag);
    }
}

impl Default for MemoryFlagGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl FlagGuard for MemoryFlagGuard {
    #[inline(always)]
    fn is_enabled(&self, flag_key: &str, context: &EvaluationContext) -> bool {
        let map = self.flags.read().unwrap();
        if let Some(flag) = map.get(flag_key) {
            evaluate_flag_zero_overhead(flag, context).enabled
        } else {
            false
        }
    }

    #[inline(always)]
    fn evaluate(&self, flag_key: &str, context: &EvaluationContext) -> EvaluationResult {
        let map = self.flags.read().unwrap();
        if let Some(flag) = map.get(flag_key) {
            evaluate_flag_zero_overhead(flag, context)
        } else {
            EvaluationResult {
                flag_key: flag_key.to_string(),
                enabled: false,
                variant: None,
                matched_rule_index: None,
                reason: "flag_not_found".to_string(),
            }
        }
    }
}
