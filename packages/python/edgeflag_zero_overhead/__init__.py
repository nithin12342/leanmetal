"""
edgeflag_zero_overhead
~~~~~~~~~~~~~~~~~~~~~~~
Zero-Overhead In-Process Feature Flag & Evaluation Guard Package for Python.
Provides deterministic xxHash64 percentage rollouts and static-path rule evaluations.
"""

from .hasher import compute_bucket, is_in_rollout
from .models import (
    AttributeValue,
    ComparisonOp,
    Condition,
    EvaluationContext,
    EvaluationResult,
    FlagDefinition,
    TargetingRule,
)
from .guard import FlagGuard, MemoryFlagGuard, EdgeFlagFastGuard

__all__ = [
    "compute_bucket",
    "is_in_rollout",
    "AttributeValue",
    "ComparisonOp",
    "Condition",
    "EvaluationContext",
    "EvaluationResult",
    "FlagDefinition",
    "TargetingRule",
    "FlagGuard",
    "MemoryFlagGuard",
    "EdgeFlagFastGuard",
]
