//! src/domain/engine/evaluator.rs
//! File ID: FILE-002
//! Responsibility: Evaluate targeting rules deterministically via SIMD (<=7 words)
//! Must Never: Perform disk I/O, make network calls, or block threads.

use crate::domain::engine::hasher::is_in_rollout;
use crate::domain::engine::model::{
    AttributeValue, ComparisonOp, Condition, EvaluationContext, EvaluationResult, FlagDefinition,
    TargetingRule,
};
use semver::Version;

/// Evaluates a feature flag dynamically against a client evaluation context.
pub fn evaluate_flag(flag: &FlagDefinition, context: &EvaluationContext) -> EvaluationResult {
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
            // Check percentage rollout for this rule
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
                // Matched rule conditions, but excluded by percentage rollout
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

    // 3. Fallback to default variant if no targeting rule matched
    EvaluationResult {
        flag_key: flag.key.clone(),
        enabled: true,
        variant: flag.default_variant.clone(),
        matched_rule_index: None,
        reason: "default_fallback".to_string(),
    }
}

/// Evaluates whether all conditions of a targeting rule are satisfied by the context.
#[inline(always)]
fn rule_matches(rule: &TargetingRule, context: &EvaluationContext) -> bool {
    rule.conditions.iter().all(|cond| condition_matches(cond, context))
}

/// Evaluates a single condition against the dynamic context.
#[inline]
fn condition_matches(cond: &Condition, context: &EvaluationContext) -> bool {
    let attr = match context.get_attr(&cond.field) {
        Some(val) => val,
        None => return false, // Field not present in context -> condition fails
    };

    match &cond.op {
        ComparisonOp::Equals(expected) => attr == expected,
        ComparisonOp::NotEquals(expected) => attr != expected,
        ComparisonOp::InSet(set) => match attr {
            AttributeValue::String(s) => set.iter().any(|v| v == s),
            AttributeValue::StringList(list) => list.iter().any(|item| set.contains(item)),
            _ => false,
        },
        ComparisonOp::NotInSet(set) => match attr {
            AttributeValue::String(s) => !set.iter().any(|v| v == s),
            AttributeValue::StringList(list) => !list.iter().any(|item| set.contains(item)),
            _ => true,
        },
        ComparisonOp::SemverGte(target_ver_str) => {
            if let (Some(actual_str), Ok(target_ver)) = (attr.as_str(), Version::parse(target_ver_str)) {
                if let Ok(actual_ver) = Version::parse(actual_str) {
                    actual_ver >= target_ver
                } else {
                    false
                }
            } else {
                false
            }
        }
        ComparisonOp::SemverLte(target_ver_str) => {
            if let (Some(actual_str), Ok(target_ver)) = (attr.as_str(), Version::parse(target_ver_str)) {
                if let Ok(actual_ver) = Version::parse(actual_str) {
                    actual_ver <= target_ver
                } else {
                    false
                }
            } else {
                false
            }
        }
        ComparisonOp::GreaterThan(threshold) => {
            if let Some(val) = attr.as_f64() {
                val > *threshold
            } else {
                false
            }
        }
        ComparisonOp::LessThan(threshold) => {
            if let Some(val) = attr.as_f64() {
                val < *threshold
            } else {
                false
            }
        }
        ComparisonOp::Contains(sub) => match attr {
            AttributeValue::String(s) => s.contains(sub),
            AttributeValue::StringList(list) => list.iter().any(|item| item == sub),
            _ => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_kill_switch() {
        let flag = FlagDefinition {
            key: "beta_checkout".to_string(),
            enabled: false,
            rules: vec![],
            default_variant: Some("v2".to_string()),
        };
        let ctx = EvaluationContext::new("usr_1");
        let res = evaluate_flag(&flag, &ctx);
        assert!(!res.enabled);
        assert_eq!(res.reason, "flag_disabled");
    }

    #[test]
    fn test_semver_and_country_rule() {
        let flag = FlagDefinition {
            key: "new_ui".to_string(),
            enabled: true,
            rules: vec![TargetingRule {
                conditions: vec![
                    Condition {
                        field: "country".to_string(),
                        op: ComparisonOp::InSet(vec!["US".to_string(), "CA".to_string()]),
                    },
                    Condition {
                        field: "app_version".to_string(),
                        op: ComparisonOp::SemverGte("2.1.0".to_string()),
                    },
                ],
                rollout_percentage: 100,
                variant: Some("modern_ui".to_string()),
            }],
            default_variant: Some("classic_ui".to_string()),
        };

        let mut attrs = HashMap::new();
        attrs.insert("country".to_string(), AttributeValue::String("US".to_string()));
        attrs.insert("app_version".to_string(), AttributeValue::String("2.4.0".to_string()));
        let ctx = EvaluationContext {
            user_id: "usr_42".to_string(),
            attributes: attrs,
        };

        let res = evaluate_flag(&flag, &ctx);
        assert!(res.enabled);
        assert_eq!(res.variant.as_deref(), Some("modern_ui"));
    }
}
