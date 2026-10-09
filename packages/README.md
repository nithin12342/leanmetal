# Zero-Overhead Abstraction & Dedicated Custom Mechanisms

This directory houses:
1. Multi-language SDK packages (**Rust**, **Python**, **TypeScript**, and **Go**).
2. **Category 2: Dedicated, Custom-Engineered Mechanisms ("Newly Created Ones")**, extracted into individual standalone production packages.

---

## Category 2: Standalone Mechanism Packages

| Mechanism | Package Directory | Core Invariant & Custom Engineering | Target Registry |
| :--- | :--- | :--- | :--- |
| **1. Singleflight Coalescing Barrier** | [`packages/singleflight-barrier/`](file:///d:/d/packages/singleflight-barrier) | Built from scratch using DashMap and Tokio broadcast channels. Captures 5,000 simultaneous cold queries and coalesces them into 1 in-flight future. | **crates.io** (`singleflight-barrier`) |
| **2. Zero-Copy WebSocket Broadcast Hub** | [`packages/zerocopy-broadcast-hub/`](file:///d:/d/packages/zerocopy-broadcast-hub) | Serializes a mutation event exactly once into an `Arc<[u8]>` buffer and fans it out concurrently to 50,000+ connections with zero per-connection allocations. | **crates.io** (`zerocopy-broadcast-hub`) |
| **3. Slow-Consumer Pruning & Backpressure Engine** | [`packages/backpressure-pruner/`](file:///d:/d/packages/backpressure-pruner) | Non-blocking channel checks (`try_send`). Proactively isolates and prunes stalled sessions exceeding 64KB / 64 frames to safeguard server memory from OOM. | **crates.io** (`backpressure-pruner`) |
| **4. Asynchronous WAL Batch Committer** | [`packages/async-wal-committer/`](file:///d:/d/packages/async-wal-committer) | Asynchronous background drain loop pulling up to 256 pending writes off the lock-free ring buffer to commit in single batches, returning in $< 100\,\mu\text{s}$. | **crates.io** (`async-wal-committer`) |
| **5. Lock-Free Mutable Timeline Replacement** | [`packages/lockfree-timeline-swap/`](file:///d:/d/packages/lockfree-timeline-swap) | Atomic pointer replacement via `ArcSwap::store()`. Clones, prepends, bounds to 50 items, and swaps pointer with zero read-lock contention. | **crates.io** (`lockfree-timeline-swap`) |

---

## Multi-Language SDK Packages

| Language | Directory | Key Components |
| :--- | :--- | :--- |
| **Rust** | [`packages/rust/`](file:///d:/d/packages/rust) | `FlagGuard` trait, `MemoryFlagGuard`, `evaluate_flag_zero_overhead`, C-ABI FFI, Kernel Bypass |
| **Python** | [`packages/python/`](file:///d:/d/packages/python) | `FlagGuard` ABC, `MemoryFlagGuard`, `EdgeFlagFastGuard`, Kernel Bypass, Singleflight |
| **TypeScript / Node** | [`packages/typescript/`](file:///d:/d/packages/typescript) | `FlagGuard` interface, `MemoryFlagGuard`, `computeBucket`, Kernel Bypass, Singleflight |
| **Go** | [`packages/go/edgeflag/`](file:///d:/d/packages/go/edgeflag) | `FlagGuard` interface, `MemoryFlagGuard`, `ComputeBucket`, Kernel Bypass |
