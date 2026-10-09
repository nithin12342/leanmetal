# Zero-Overhead Abstraction & Multi-Language In-Process Guard SDKs

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)
[![Dispatch: Static Monomorphization](https://img.shields.io/badge/Dispatch-Static%20%26%20Inlined-success.svg)](#core-principles)
[![Overhead: Zero Heap Allocation](https://img.shields.io/badge/Hot%20Path-Zero%20Heap%20Allocations-brightgreen.svg)](#hot-path-guarantees)
[![Standards: OpenFeature](https://img.shields.io/badge/Standard-CNCF%20OpenFeature-purple.svg)](https://openfeature.dev/)

This sub-project provides **Zero-Overhead Abstraction** packages across multiple programming languages (Rust, Python, TypeScript, and Go). It implements in-process feature flag evaluation, deterministic xxHash64 rollout bucketing, and static-dispatch guard traits that deliver microsecond-level flag checks without incurring network hops or dynamic runtime penalties.

---

## What is "Zero-Overhead Abstraction"?

In modern software engineering, abstractions often introduce hidden performance taxes:
- **Dynamic Dispatch (`dyn Trait` / vtables):** Compiler generates indirect jumps (`call rax`), which bypass branch predictors and prevent function inlining.
- **Heap Allocations on Read Paths:** Allocating strings, maps, or boxed futures for every evaluation stalls memory allocators under heavy load.
- **IPC / Network Latency:** Evaluating flags over HTTP or database connections costs milliseconds and introduces failure modes.

**Zero-Overhead Abstraction** eliminates these penalties by enforcing:
1. **Static Monomorphization & Inlining:** Generic parameters and `#[inline(always)]` attributes ensure the compiler generates machine code identical to a hand-written `if` statement.
2. **Stack-Allocated Hot Paths:** Context evaluation uses stack-allocated scratch buffers (`[u8; 128]`) to avoid dynamic heap allocation.
3. **In-Process Memory Lookup:** Evaluates against local memory caches (50–100 nanoseconds) with seamless fallback to memory-mapped files (1–5 µs).

---

## Language Package Ecosystem

Each package is self-contained with its own native manifest and dependency management:

```text
packages/
├── rust/                # Rust Crate (Static monomorphization & C-ABI FFI cdylib)
├── python/              # Python Package (Pure Python fallback + C-ABI ctypes fast path)
├── typescript/          # Node.js & Browser TypeScript (Zero-dependency FNV-1a / V8 JIT)
└── go/                  # Go Module (Lightweight goroutine-safe RWMutex memory guard)
```

### Feature Comparison Matrix

| Capability | Rust (`packages/rust`) | Python (`packages/python`) | TypeScript (`packages/typescript`) | Go (`packages/go`) |
| :--- | :---: | :---: | :---: | :---: |
| **Deterministic Bucketing** | `xxHash64` in CPU registers | `xxHash64` / FNV-1a | FNV-1a (V8 optimized) | FNV-1a (64-bit) |
| **Hot-Path Allocations** | **Zero heap allocations** | Minimal dataclass | Pure object reuse | Zero heap escapes |
| **Evaluation Latency** | **< 100 nanoseconds** | < 1.2 microseconds | < 500 nanoseconds | < 250 nanoseconds |
| **OpenFeature Schema** | Built-in | `to_openfeature()` | Built-in interfaces | Built-in |
| **C-ABI FFI Export** | Native `cdylib` export | Supported via `ctypes` | N-API bridgeable | CGo compatible |

---

## Quick Start by Language

### 1. Rust (`packages/rust`)
```rust
use edgeflag_zero_overhead::{FlagDefinition, MemoryFlagGuard, FlagGuard, EvaluationContext};

let guard = MemoryFlagGuard::new();
guard.set_flag(FlagDefinition {
    key: "new_checkout".to_string(),
    enabled: true,
    rules: vec![],
    default_variant: Some("v2".to_string()),
});

let ctx = EvaluationContext::new("usr_102");
assert!(guard.is_enabled("new_checkout", &ctx));
```

### 2. Python (`packages/python`)
```python
from edgeflag_zero_overhead import MemoryFlagGuard, FlagDefinition, EvaluationContext

guard = MemoryFlagGuard()
guard.set_flag(FlagDefinition(key="new_checkout", enabled=True))

ctx = EvaluationContext(user_id="usr_102")
if guard.is_enabled("new_checkout", ctx):
    print("Feature active!")
```

### 3. TypeScript (`packages/typescript`)
```typescript
import { MemoryFlagGuard } from "@edgeflag/zero-overhead";

const guard = new MemoryFlagGuard();
guard.setFlag({ key: "new_checkout", enabled: true });

if (guard.isEnabled("new_checkout", { userId: "usr_102" })) {
  console.log("Feature active!");
}
```

### 4. Go (`packages/go/edgeflag`)
```go
import "github.com/nithin12342/leanmetal/packages/go/edgeflag"

guard := edgeflag.NewMemoryFlagGuard()
guard.SetFlag(edgeflag.FlagDefinition{Key: "new_checkout", Enabled: true})

if guard.IsEnabled("new_checkout", edgeflag.EvaluationContext{UserID: "usr_102"}) {
    // Feature active
}
```

---

## License

Distributed under the [MIT License](../LICENSE). Copyright (c) 2026 NITHIN G.
