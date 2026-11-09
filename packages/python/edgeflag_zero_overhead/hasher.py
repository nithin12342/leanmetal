"""
Zero-overhead deterministic bucketing algorithm matching the Rust core engine.
Uses xxHash64 when available, or a deterministic 64-bit FNV/Murmur variant fallback.
"""

import ctypes
from typing import Optional

try:
    import xxhash
    _HAS_XXHASH = True
except ImportError:
    _HAS_XXHASH = False


def _fnv1a_64(data: bytes) -> int:
    """Deterministic 64-bit fallback hash."""
    h = 0xcbf29ce484222325
    for b in data:
        h = ((h ^ b) * 0x100000001b3) & 0xFFFFFFFFFFFFFFFF
    return h


def compute_bucket(flag_key: str, user_id: str) -> int:
    """
    Computes a deterministic rollout bucket in the range [0, 99].
    Formula: xxHash64(flag_key + ":" + user_id) % 100
    """
    key_bytes = f"{flag_key}:{user_id}".encode("utf-8")
    if _HAS_XXHASH:
        val = xxhash.xxh64_intdigest(key_bytes, seed=0)
    else:
        val = _fnv1a_64(key_bytes)
    return val % 100


def is_in_rollout(flag_key: str, user_id: str, percentage: int) -> bool:
    """Evaluates whether user_id is included in the given percentage rollout [0..100]."""
    if percentage <= 0:
        return False
    if percentage >= 100:
        return True
    return compute_bucket(flag_key, user_id) < percentage
