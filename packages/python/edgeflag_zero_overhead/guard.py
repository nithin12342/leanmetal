"""
In-process guard abstractions and FFI bindings for Python.
"""

from abc import ABC, abstractmethod
import json
import os
import ctypes
from typing import Dict, Optional
from .models import EvaluationContext, EvaluationResult, FlagDefinition
from .hasher import is_in_rollout


class FlagGuard(ABC):
    """Abstract zero-overhead flag guard protocol."""

    @abstractmethod
    def is_enabled(self, flag_key: str, context: EvaluationContext) -> bool:
        pass

    @abstractmethod
    def evaluate(self, flag_key: str, context: EvaluationContext) -> EvaluationResult:
        pass


class MemoryFlagGuard(FlagGuard):
    """Pure Python fast in-process memory guard."""

    def __init__(self):
        self._flags: Dict[str, FlagDefinition] = {}

    def set_flag(self, flag: FlagDefinition) -> None:
        self._flags[flag.key] = flag

    def is_enabled(self, flag_key: str, context: EvaluationContext) -> bool:
        res = self.evaluate(flag_key, context)
        return res.enabled

    def evaluate(self, flag_key: str, context: EvaluationContext) -> EvaluationResult:
        flag = self._flags.get(flag_key)
        if not flag or not flag.enabled:
            return EvaluationResult(
                flag_key=flag_key,
                enabled=False,
                reason="flag_disabled" if flag else "flag_not_found",
            )

        for idx, rule in enumerate(flag.rules):
            if is_in_rollout(flag.key, context.user_id, rule.rollout_percentage):
                return EvaluationResult(
                    flag_key=flag.key,
                    enabled=True,
                    variant=rule.variant or flag.default_variant,
                    matched_rule_index=idx,
                    reason=f"rule_matched_{idx}",
                )

        return EvaluationResult(
            flag_key=flag.key,
            enabled=True,
            variant=flag.default_variant,
            reason="default_fallback",
        )


class EdgeFlagFastGuard(FlagGuard):
    """
    Sub-microsecond FFI guard backed by compiled Rust dynamic library if present,
    falling back seamlessly to pure Python in-process memory evaluation.
    """

    def __init__(self, lib_path: Optional[str] = None):
        self._fallback = MemoryFlagGuard()
        self._ffi_lib = None

        if lib_path and os.path.exists(lib_path):
            try:
                self._ffi_lib = ctypes.CDLL(lib_path)
                self._ffi_lib.edgeflag_is_in_rollout.argtypes = [
                    ctypes.c_char_p,
                    ctypes.c_char_p,
                    ctypes.c_ubyte,
                ]
                self._ffi_lib.edgeflag_is_in_rollout.restype = ctypes.c_ubyte
            except Exception:
                self._ffi_lib = None

    def set_flag(self, flag: FlagDefinition) -> None:
        self._fallback.set_flag(flag)

    def is_enabled(self, flag_key: str, context: EvaluationContext) -> bool:
        if self._ffi_lib:
            k = flag_key.encode("utf-8")
            u = context.user_id.encode("utf-8")
            return bool(self._ffi_lib.edgeflag_is_in_rollout(k, u, 100))
        return self._fallback.is_enabled(flag_key, context)

    def evaluate(self, flag_key: str, context: EvaluationContext) -> EvaluationResult:
        return self._fallback.evaluate(flag_key, context)
