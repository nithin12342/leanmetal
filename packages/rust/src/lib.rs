//! packages/rust/src/lib.rs
//! Crate Root: Zero-Overhead Abstraction & Multi-Language Foundation for EdgeFlag.

pub mod evaluator;
pub mod features;
pub mod ffi;
pub mod guard;
pub mod hasher;
pub mod models;

pub use evaluator::evaluate_flag_zero_overhead;
pub use features::{
    PacketDescriptor, UmemPool, XdpDescriptorRing, ZeroOverheadL1Cache, ZeroOverheadSingleflight,
};
pub use ffi::*;
pub use guard::{FlagGuard, MemoryFlagGuard};
pub use hasher::{compute_bucket, is_in_rollout};
pub use models::{
    AttributeValue, ComparisonOp, Condition, EvaluationContext, EvaluationResult, FlagDefinition,
    OpenFeatureResolution, TargetingRule,
};
