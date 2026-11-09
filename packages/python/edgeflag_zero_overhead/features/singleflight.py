"""
Zero-Overhead Singleflight Request Coalescer Feature.
"""

import threading
from typing import Any, Callable, Dict


class Singleflight:
    """Coalesces concurrent in-flight requests sharing the same key."""

    def __init__(self):
        self._lock = threading.Lock()
        self._in_flight: Dict[str, threading.Event] = {}
        self._results: Dict[str, Any] = {}

    def execute(self, key: str, fn: Callable[[], Any]) -> Any:
        with self._lock:
            if key in self._in_flight:
                event = self._in_flight[key]
                first_caller = False
            else:
                event = threading.Event()
                self._in_flight[key] = event
                first_caller = True

        if not first_caller:
            event.wait()
            return self._results.get(key)

        try:
            res = fn()
            self._results[key] = res
            return res
        finally:
            event.set()
            with self._lock:
                self._in_flight.pop(key, None)
                self._results.pop(key, None)
