# Zero-Overhead Abstraction, Atomic Engine Crates & Dedicated Custom Mechanisms

This directory houses:
1. **Multi-language SDK packages** (`rust`, `python`, `typescript`, `go`).
2. **Category 2: Dedicated, Custom-Engineered Mechanisms** (`singleflight-barrier`, `zerocopy-broadcast-hub`, `backpressure-pruner`, `async-wal-committer`, `lockfree-timeline-swap`).
3. **Atomic Engine Micro-Packages** (`engine-layout`, `engine-log`, `engine-disruptor`, `engine-wire`, `engine-eda`, `engine-runtime`).
4. **Isolated Feature & Transport Packages** (`kernel-bypass-ingress`, `fast-rule-evaluator`, `simd-json-vector`, `p2p-gossip-mesh`, `pingora-edge-proxy`, `edge-guard-sdk`, `mmap-social-engine`).

---

## Complete Standalone Mechanism & Engine Packages

| Package | Path | Responsibility & Core Invariant | Target Registry |
| :--- | :--- | :--- | :--- |
| **`kernel-bypass-ingress`** | [`packages/kernel-bypass-ingress/`](kernel-bypass-ingress) | Zero-copy UMEM pool, AF_XDP (XSK) driver-mode DMA queues, and descriptor rings. | **crates.io** |
| **`fast-rule-evaluator`** | [`packages/fast-rule-evaluator/`](fast-rule-evaluator) | Inlined xxHash64 rollout bucketing (128-byte stack buffer) and multi-clause contextual rule evaluation. | **crates.io** |
| **`simd-json-vector`** | [`packages/simd-json-vector/`](simd-json-vector) | In-place JSON deserialization (`from_slice_simd`) and serialization via CPU vector SIMD registers. | **crates.io** |
| **`p2p-gossip-mesh`** | [`packages/p2p-gossip-mesh/`](p2p-gossip-mesh) | UDP state-delta broadcast and cache invalidation mesh with monotonic revision tracking. | **crates.io** |
| **`pingora-edge-proxy`** | [`packages/pingora-edge-proxy/`](pingora-edge-proxy) | Ingress edge proxy layer with TinyUFO L1 caching and sliding-window token-bucket rate limiter. | **crates.io** |
| **`edge-guard-sdk`** | [`packages/edge-guard-sdk/`](edge-guard-sdk) | Zero-overhead `FlagGuard` trait contracts, static dispatch guards, and Tower/Axum middleware. | **crates.io** |
| **`mmap-social-engine`** | [`packages/mmap-social-engine/`](mmap-social-engine) | High-concurrency social engine integrating singleflight request coalescing and reverse-chronological feeds. | **crates.io** |
| **`singleflight-barrier`** | [`packages/singleflight-barrier/`](singleflight-barrier) | Coalesces thousands of concurrent reads into a single in-flight future using DashMap and Tokio channels. | **crates.io** |
| **`zerocopy-broadcast-hub`** | [`packages/zerocopy-broadcast-hub/`](zerocopy-broadcast-hub) | Serializes events once into `Arc<[u8]>` and fans out to 50,000+ client connections without per-client copies. | **crates.io** |
| **`backpressure-pruner`** | [`packages/backpressure-pruner/`](backpressure-pruner) | Active eviction loop pruning stalled slow consumers exceeding buffer caps to prevent OOM. | **crates.io** |
| **`async-wal-committer`** | [`packages/async-wal-committer/`](async-wal-committer) | Asynchronous background drain loop committing up to 256 writes per batch with sub-100µs latency. | **crates.io** |
| **`lockfree-timeline-swap`** | [`packages/lockfree-timeline-swap/`](lockfree-timeline-swap) | Atomic pointer replacement via `ArcSwap::store()` for copy-on-write feeds with zero read locks. | **crates.io** |
| **`engine-layout`** | [`packages/engine-layout/`](engine-layout) | Descending alignment structs (`RingEntry`, `ConnectionSlot`, `UnifiedFrameHeader`, `EventEnvelope`) with 0% padding. | **crates.io** |
| **`engine-log`** | [`packages/engine-log/`](engine-log) | Contiguous 64MB segment-based append-only WAL and zero-copy `MmapStore` page readers. | **crates.io** |
| **`engine-disruptor`** | [`packages/engine-disruptor/`](engine-disruptor) | Monotonically sequenced LMAX ring buffer with non-blocking spin-yield stage pipeline barriers. | **crates.io** |
| **`engine-wire`** | [`packages/engine-wire/`](engine-wire) | AVX SIMD manifest NACK ledger, asymmetric NACK dispatcher, and 40-byte slab session registry. | **crates.io** |
| **`engine-eda`** | [`packages/engine-eda/`](engine-eda) | Command processor, zero-lock ArcSwap read projections, and sub-150ns Moka L1 cache. | **crates.io** |
| **`engine-runtime`** | [`packages/engine-runtime/`](engine-runtime) | Application bootstrap coordinator, UDP gossip invalidation mesh, and unikernel boot profile. | **crates.io** |

---

## Multi-Language SDK Packages

| Language | Directory | Key Components |
| :--- | :--- | :--- |
| **Rust** | [`packages/rust/`](rust) | `FlagGuard` trait, `MemoryFlagGuard`, `evaluate_flag_zero_overhead`, C-ABI FFI, Kernel Bypass |
| **Python** | [`packages/python/`](python) | `FlagGuard` ABC, `MemoryFlagGuard`, `EdgeFlagFastGuard`, Kernel Bypass, Singleflight |
| **TypeScript / Node** | [`packages/typescript/`](typescript) | `FlagGuard` interface, `MemoryFlagGuard`, `computeBucket`, Kernel Bypass, Singleflight |
| **Go** | [`packages/go/edgeflag/`](go/edgeflag) | `FlagGuard` interface, `MemoryFlagGuard`, `ComputeBucket`, Kernel Bypass |
