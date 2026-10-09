"""
Zero-overhead Python dataclass representations matching the Rust domain schema.
"""

from dataclasses import dataclass, field
from typing import Any, Dict, List, Optional, Union

AttributeValue = Union[str, int, float, bool, List[str]]


@dataclass
class EvaluationContext:
    user_id: str
    attributes: Dict[str, AttributeValue] = field(default_factory=dict)

    def with_attribute(self, key: str, val: AttributeValue) -> "EvaluationContext":
        self.attributes[key] = val
        return self

    def get_attr(self, key: str) -> Optional[AttributeValue]:
        return self.attributes.get(key)


@dataclass
class ComparisonOp:
    op: str
    value: Any


@dataclass
class Condition:
    field: str
    op: ComparisonOp


@dataclass
class TargetingRule:
    conditions: List[Condition] = field(default_factory=list)
    rollout_percentage: int = 100
    variant: Optional[str] = None


@dataclass
class FlagDefinition:
    key: str
    enabled: bool
    rules: List[TargetingRule] = field(default_factory=list)
    default_variant: Optional[str] = None


@dataclass
class EvaluationResult:
    flag_key: str
    enabled: bool
    variant: Optional[str] = None
    matched_rule_index: Optional[int] = None
    reason: str = "default_fallback"

    def to_openfeature(self) -> Dict[str, Any]:
        return {
            "flagKey": self.flag_key,
            "value": self.enabled,
            "variant": self.variant,
            "reason": self.reason,
        }
