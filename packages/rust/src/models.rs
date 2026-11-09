//! packages/rust/src/models.rs
//! Zero-overhead cross-platform data models and OpenFeature specification contracts.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Dynamic attribute value supporting strings, numbers, booleans, and string lists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AttributeValue {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    StringList(Vec<String>),
}

impl AttributeValue {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            AttributeValue::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            AttributeValue::Float(f) => Some(*f),
            AttributeValue::Int(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            AttributeValue::Bool(b) => Some(*b),
            _ => None,
        }
    }
}

/// Dynamic evaluation context.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct EvaluationContext {
    pub user_id: String,
    #[serde(default)]
    pub attributes: HashMap<String, AttributeValue>,
}

impl EvaluationContext {
    pub fn new(user_id: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            attributes: HashMap::new(),
        }
    }

    pub fn with_attribute(mut self, key: impl Into<String>, val: AttributeValue) -> Self {
        self.attributes.insert(key.into(), val);
        self
    }

    pub fn get_attr(&self, key: &str) -> Option<&AttributeValue> {
        self.attributes.get(key)
    }
}

/// Comparison operators for rule conditions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", content = "value")]
pub enum ComparisonOp {
    Equals(AttributeValue),
    NotEquals(AttributeValue),
    InSet(Vec<String>),
    NotInSet(Vec<String>),
    SemverGte(String),
    SemverLte(String),
    GreaterThan(f64),
    LessThan(f64),
    Contains(String),
}

/// Targeting condition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    #[serde(flatten)]
    pub op: ComparisonOp,
}

/// Targeting rule clause.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TargetingRule {
    #[serde(default)]
    pub conditions: Vec<Condition>,
    #[serde(default = "default_rollout")]
    pub rollout_percentage: u8,
    pub variant: Option<String>,
}

fn default_rollout() -> u8 {
    100
}

/// Complete feature flag definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlagDefinition {
    pub key: String,
    pub enabled: bool,
    #[serde(default)]
    pub rules: Vec<TargetingRule>,
    pub default_variant: Option<String>,
}

/// Result returned after evaluating a flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationResult {
    pub flag_key: String,
    pub enabled: bool,
    pub variant: Option<String>,
    pub matched_rule_index: Option<usize>,
    pub reason: String,
}

/// OpenFeature resolution structure.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenFeatureResolution {
    pub flag_key: String,
    pub value: serde_json::Value,
    pub variant: Option<String>,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}

impl EvaluationResult {
    pub fn to_openfeature(&self) -> OpenFeatureResolution {
        OpenFeatureResolution {
            flag_key: self.flag_key.clone(),
            value: serde_json::Value::Bool(self.enabled),
            variant: self.variant.clone(),
            reason: self.reason.clone(),
            error_code: None,
        }
    }
}
