//! packages/fast-rule-evaluator/src/lib.rs
//! High-Performance Deterministic Feature Flag Rule Evaluator & xxHash64 Bucketing Engine.

pub mod hasher;
pub mod model;
pub mod evaluator;

pub use hasher::{compute_bucket, is_in_rollout};
pub use model::{
    AttributeValue, ComparisonOp, Condition, EvaluationContext, EvaluationResult,
    FlagDefinition, TargetingRule, OpenFeatureResolution,
};
pub use evaluator::evaluate_flag;
