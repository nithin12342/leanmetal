# Zero-Overhead Abstraction Multi-Language Packages

This directory houses the zero-overhead in-process SDK abstractions, deterministic xxHash64 percentage bucketing, and targeting rule evaluators for multiple programming language ecosystems.

## Architecture & Principles
- **Zero Runtime Tax:** Direct compile-time monomorphization and compiler inlining with zero dynamic dispatch (`dyn Trait`) and zero vtable hops.
- **Zero Heap Allocations on Evaluation Path:** Stack-allocated scratch buffers (`[u8; 128]`) for fast-path bucketing and boolean flag checks.
- **Sub-Microsecond Latency:** Nanosecond hash generation ($< 50\text{ ns}$) and memory-mapped reads ($< 5\,\mu\text{s}$).

## Language Packages

| Language | Directory | Key Components |
| :--- | :--- | :--- |
| **Rust** | [`packages/rust/`](file:///d:/d/packages/rust) | `FlagGuard` trait, `MemoryFlagGuard`, `evaluate_flag_zero_overhead`, C-ABI FFI |
| **Python** | [`packages/python/`](file:///d:/d/packages/python) | `FlagGuard` ABC, `MemoryFlagGuard`, `EdgeFlagFastGuard` (FFI + Pure Python), `hasher.py` |
| **TypeScript / Node** | [`packages/typescript/`](file:///d:/d/packages/typescript) | `FlagGuard` interface, `MemoryFlagGuard`, `computeBucket`, OpenFeature types |
| **Go** | [`packages/go/edgeflag/`](file:///d:/d/packages/go/edgeflag) | `FlagGuard` interface, `MemoryFlagGuard`, `ComputeBucket`, zero-alloc evaluation |
