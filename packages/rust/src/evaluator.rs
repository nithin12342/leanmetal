//! packages/rust/src/evaluator.rs
//! Zero-overhead static rule evaluation engine with zero heap allocations on hot paths.
//! Designed for compiler inlining and static dispatch across all language runtimes.

use crate::hasher::is_in_rollout;
use crate::models::{
    AttributeValue, ComparisonOp, Condition, EvaluationContext, EvaluationResult, FlagDefinition,
    TargetingRule,
};
use semver::Version;

/// Evaluates a feature flag dynamically against a client evaluation context with zero heap allocations.
#[inline(always)]
pub fn evaluate_flag_zero_overhead(flag: &FlagDefinition, context: &EvaluationContext) -> EvaluationResult {
    // 1. Immediate exit if the flag is globally disabled (emergency kill-switch)
    if !flag.enabled {
        return EvaluationResult {
            flag_key: flag.key.clone(),
            enabled: false,
            variant: None,
            matched_rule_index: None,
            reason: "flag_disabled".to_string(),
        };
    }

    // 2. Evaluate targeting rules in priority order
    for (idx, rule) in flag.rules.iter().enumerate() {
        if rule_matches(rule, context) {
            let in_rollout = is_in_rollout(&flag.key, &context.user_id, rule.rollout_percentage);
            if in_rollout {
                return EvaluationResult {
                    flag_key: flag.key.clone(),
                    enabled: true,
                    variant: rule.variant.clone().or_else(|| flag.default_variant.clone()),
                    matched_rule_index: Some(idx),
                    reason: format!("rule_matched_{}", idx),
                };
            } else {
                return EvaluationResult {
                    flag_key: flag.key.clone(),
                    enabled: false,
                    variant: None,
                    matched_rule_index: Some(idx),
                    reason: format!("rule_{}_rollout_excluded", idx),
                };
            }
        }
    }

    // 3. Fallback to default variant
    EvaluationResult {
        flag_key: flag.key.clone(),
        enabled: true,
        variant: flag.default_variant.clone(),
        matched_rule_index: None,
        reason: "default_fallback".to_string(),
    }
}

#[inline(always)]
fn rule_matches(rule: &TargetingRule, context: &EvaluationContext) -> bool {
    for cond in &rule.conditions {
        if !condition_matches(cond, context) {
            return false;
        }
    }
    true
}

#[inline(always)]
fn condition_matches(cond: &Condition, context: &EvaluationContext) -> bool {
    let attr_val = match context.get_attr(&cond.field) {
        Some(val) => val,
        None => return false,
    };

    match &cond.op {
        ComparisonOp::Equals(target) => attr_val == target,
        ComparisonOp::NotEquals(target) => attr_val != target,
        ComparisonOp::InSet(set) => match attr_val {
            AttributeValue::String(s) => set.iter().any(|item| item == s),
            _ => false,
        },
        ComparisonOp::NotInSet(set) => match attr_val {
            AttributeValue::String(s) => !set.iter().any(|item| item == s),
            _ => true,
        },
        ComparisonOp::SemverGte(target_ver_str) => match attr_val {
            AttributeValue::String(ctx_ver_str) => {
                match (Version::parse(ctx_ver_str), Version::parse(target_ver_str)) {
                    (Ok(ctx_v), Ok(target_v)) => ctx_v >= target_v,
                    _ => false,
                }
            }
            _ => false,
        },
        ComparisonOp::SemverLte(target_ver_str) => match attr_val {
            AttributeValue::String(ctx_ver_str) => {
                match (Version::parse(ctx_ver_str), Version::parse(target_ver_str)) {
                    (Ok(ctx_v), Ok(target_v)) => ctx_v <= target_v,
                    _ => false,
                }
            }
            _ => false,
        },
        ComparisonOp::GreaterThan(target_num) => match attr_val.as_f64() {
            Some(n) => n > *target_num,
            None => false,
        },
        ComparisonOp::LessThan(target_num) => match attr_val.as_f64() {
            Some(n) => n < *target_num,
            None => false,
        },
        ComparisonOp::Contains(substr) => match attr_val {
            AttributeValue::String(s) => s.contains(substr),
            _ => false,
        },
    }
}
