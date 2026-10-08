//! src/domain/engine/mod.rs
//! Core rule evaluation, models, and bucketing engine

pub mod evaluator;
pub mod guard_sdk;
pub mod hasher;
pub mod model;
pub mod simd_parser;
pub mod singleflight;

pub use evaluator::evaluate_flag;
pub use guard_sdk::{EdgeFlagGuard, EdgeFlagLayer, FlagGuard};
pub use hasher::{compute_bucket, is_in_rollout};
pub use model::{
    AttributeValue, ComparisonOp, Condition, EvaluationContext, EvaluationResult, FlagDefinition,
    OpenFeatureResolution, TargetingRule,
};
pub use simd_parser::{from_slice_scratch, from_slice_simd, to_vec_simd, SimdJsonError};
pub use singleflight::Singleflight;

