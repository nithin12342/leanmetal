# Complete Package & Capability Catalog

This document consolidates every package and atomic capability housed inside the `leanmetal` repository, categorized by architectural tier, hardware alignment, and dependency footprint.

---

## 1. High-Level Architectural Invariants

1. **Absolute Theoretical Minimum Data Travel:**
   * **In-Process (0 Copies):** Memory is allocated once upfront in contiguous cache-line aligned slabs. Internal modules and microservice pipelines evaluate and project state exclusively via **immutable slices (`&[u8]`) or atomic pointer updates (`ArcSwap`)**. No intermediate serialization, no string allocations, and no buffer copies.
   * **Outside-of-Process (Kafka-Style Sequentiality):** Persistence and network streaming execute strictly sequentially via pre-allocated **64MB append-only disk segments (`SegmentLog`)** mapped directly into the OS page cache (`mmap`), eliminating random seek latency.
2. **Pure Code-Based Zero-Dependency Implementations:**
   * Hand-rolled **64-bit SWAR (SIMD Within A Register)** structural parsing—zero external crates.
   * Cross-platform contiguous **DMA Memory Pool and Power-of-Two Descriptor Ring**—zero OS-specific driver crashes.
   * Monotonically sequenced **LMAX Disruptor Ring Buffer** using single-cycle bitwise masking (`seq & mask`).

---

## 2. Complete Package Inventory

| # | Package Name | Directory | Primary Capabilities & Architectural Invariant | Dependencies | Status |
| :---: | :--- | :--- | :--- | :--- | :---: |
| **1** | **`simd-json-vector`** | [`packages/simd-json-vector/`](packages/simd-json-vector) | **Pure hand-rolled 64-bit SWAR SIMD scanner:** Identifies JSON structural punctuation (`{`, `}`, `[`, `]`, `:`, `,`, `"`) in $< 1\text{ cycle/byte}$ using bitwise carry-less arithmetic (`swar_match_byte`) and quote parity masks without external crates. | **0 (Zero)** | **Verified** |
| **2** | **`dma-ring-buffer`** | [`packages/kernel-bypass-ingress/`](packages/kernel-bypass-ingress) | **Hardware-agnostic contiguous DMA memory pool (`DmaMemoryPool`) & circular descriptor ring (`DmaDescriptorRing`):** Fixed 2048-byte frames, power-of-two bitmask indexing, and zero-allocation frame recycling across Windows, Linux, and bare-metal. | **0 (Zero)** | **Verified** |
| **3** | **`engine-layout`** | [`packages/engine-layout/`](packages/engine-layout) | **Foundational zero-padding memory primitives:** Descending alignment structs (`RingEntry` 64B aligned to 64B cache line, `ConnectionSlot` 40B, `UnifiedFrameHeader` 24B, `EventEnvelope` 64B) verified at compile time via `assert_dense_layout!`. | **0 (`no_std`)** | **Verified** |
| **4** | **`engine-disruptor`** | [`packages/engine-disruptor/`](packages/engine-disruptor) | **Monotonically sequenced circular LMAX ring buffer:** Power-of-two capacity ($2^k$) indexed via `seq & mask`, non-blocking progressive spin-yield wait strategies, and chained stage pipeline barriers. | `engine-layout` | **Verified** |
| **5** | **`engine-log`** | [`packages/engine-log/`](packages/engine-log) | **Kafka-style contiguous 64MB segment WAL (`SegmentLog`) & zero-copy page reader (`MmapStore`):** Appends sequentially without random head seeks; reads dereference immutable slices directly from the OS page cache without `read()` syscalls. | `engine-layout`, `memmap2` | **Verified** |
| **6** | **`engine-wire`** | [`packages/engine-wire/`](packages/engine-wire) | **Manifest-driven NACK transport engine:** Asymmetric NACK transmission (zero return packets on 100% receipt), AVX SIMD bitset tracking, and 40-byte contiguous slab session registry (100k sockets in 4.0 MB flat RAM). | `engine-layout` | **Verified** |
| **7** | **`manifest-udp-transport`** | [`packages/manifest-udp-transport/`](packages/manifest-udp-transport) | **Unified zero-padding Manifesto UDP transport:** Combines HTTP request/reply correlation, WebSocket duplex streaming, and Webhook event pushes over raw UDP with zero TCP head-of-line blocking and 64B cache-line aligned headers. | `engine-layout` | **Verified** |
| **8** | **`engine-eda`** | [`packages/engine-eda/`](packages/engine-eda) | **In-process event-driven state engine:** Command validation, zero-lock `ArcSwap` projections, segment snapshot replaying at memory bus speed, and sub-150ns Moka L1 cache. | `engine-layout`, `engine-disruptor`, `engine-log`, `moka`, `arc-swap` | **Verified** |
| **8** | **`engine-runtime`** | [`packages/engine-runtime/`](packages/engine-runtime) | **Assembly, ingress & host target:** Direct UDP ingress worker into DisruptorRing, P2P UDP gossip mesh without Redis/Valkey daemons, and standalone freestanding kernel profile. | `engine-layout`, `engine-log`, `engine-disruptor`, `engine-wire`, `engine-eda` | **Verified** |
| **9** | **`singleflight-barrier`** | [`packages/singleflight-barrier/`](packages/singleflight-barrier) | **In-flight request coalescing barrier:** Eliminates thundering herd DB stampedes by capturing concurrent reads for cold keys into a single shared memory future. | `dashmap`, `tokio` | **Verified** |
| **10** | **`zerocopy-broadcast-hub`** | [`packages/zerocopy-broadcast-hub/`](packages/zerocopy-broadcast-hub) | **Zero-copy WebSocket fanout hub:** Serializes frames once into `Arc<[u8]>` pointers and broadcasts concurrently to 50k+ connections with zero per-connection heap allocations. | `tokio` | **Verified** |
| **11** | **`backpressure-pruner`** | [`packages/backpressure-pruner/`](packages/backpressure-pruner) | **Slow-consumer active eviction engine:** Non-blocking `try_send` checks to disconnect slow client sessions before buffer bloat triggers out-of-memory crashes. | `tokio` | **Verified** |
| **12** | **`async-wal-committer`** | [`packages/async-wal-committer/`](packages/async-wal-committer) | **Asynchronous batch committer:** Decoupled background drain loop pulling up to 256 pending writes off a lock-free queue to commit in batches, acknowledging writes in $< 100\,\mu\text{s}$. | `tokio` | **Verified** |
| **13** | **`lockfree-timeline-swap`** | [`packages/lockfree-timeline-swap/`](packages/lockfree-timeline-swap) | **Atomic timeline pointer replacement:** `ArcSwap::store()` updates bounded in-memory feeds with zero read-lock contention. | `arc-swap` | **Verified** |
| **14** | **`fast-rule-evaluator`** | [`packages/fast-rule-evaluator/`](packages/fast-rule-evaluator) | **Deterministic xxHash64 rollout bucketing:** 128-byte stack-allocated scratch buffer calculating nanosecond $[0, 99]$ percentage distributions without dynamic heap allocations. | `xxhash-rust`, `serde`, `semver` | **Verified** |
| **15** | **`p2p-gossip-mesh`** | [`packages/p2p-gossip-mesh/`](packages/p2p-gossip-mesh) | **Daemonless P2P UDP state delta broadcast mesh:** Broadcasts timestamped monotonic revision envelopes (`ReplicationDelta`) to synchronize cache evictions across nodes without external Redis/Valkey daemons. | `tokio`, `fast-rule-evaluator` | **Verified** |
| **16** | **`pingora-edge-proxy`** | [`packages/pingora-edge-proxy/`](packages/pingora-edge-proxy) | **Edge reverse proxy layer:** Fronts upstream services with 50,000-slot TinyUFO L1 caching and atomic sliding-window token-bucket rate limiter (`EdgeRateLimiter`). | `TinyUFO`, `tokio` | **Verified** |
| **17** | **`edge-guard-sdk`** | [`packages/edge-guard-sdk/`](packages/edge-guard-sdk) | **In-process feature guard trait contracts & middleware:** Monomorphized `FlagGuard` trait with static dispatch and Tower/Axum middleware. | `fast-rule-evaluator`, `axum`, `tower` | **Verified** |
| **18** | **`mmap-social-engine`** | [`packages/mmap-social-engine/`](packages/mmap-social-engine) | **High-concurrency social domain engine:** Multi-tier cache orchestrator coordinating singleflight anti-stampede and reverse-chronological 16-byte B+ tree composite keys. | `singleflight-barrier`, `async-wal-committer`, `TinyUFO` | **Verified** |
| **19** | **`rust` SDK** | [`packages/rust/`](packages/rust) | Native Rust client library providing `FlagGuard`, C-ABI FFI bindings, and zero-overhead memory evaluations. | Language SDK | **Verified** |
| **20** | **`python` SDK** | [`packages/python/`](packages/python) | High-performance Python client library featuring `FlagGuard` ABC, singleflight deduplication, and memory guard. | Language SDK | **Verified** |
| **21** | **`typescript` SDK** | [`packages/typescript/`](packages/typescript) | TypeScript / Node.js library featuring `FlagGuard` interface, `computeBucket`, and in-process cache. | Language SDK | **Verified** |
| **22** | **`go` SDK** | [`packages/go/edgeflag/`](packages/go/edgeflag) | Pure Go client SDK exporting `FlagGuard` interface, `ComputeBucket`, and zero-copy evaluations. | Language SDK | **Verified** |

---

## 3. Data Flow & Zero-Copy Mechanical Pipeline

```
                                      ┌────────────────────────────────────────────────────────────┐
                                      │              Physical Hardware / Network Ingress           │
                                      └─────────────────────────────┬──────────────────────────────┘
                                                                    │
                                                                    ▼
                                      ┌────────────────────────────────────────────────────────────┐
                                      │             DmaMemoryPool (dma-ring-buffer)                │
                                      │          Pre-allocated 2048-Byte Contiguous Frames         │
                                      └─────────────────────────────┬──────────────────────────────┘
                                                                    │  (Direct In-Place Pointer: &[u8])
                                                                    ▼
                                      ┌────────────────────────────────────────────────────────────┐
                                      │            Pure SWAR Scanner (simd-json-vector)            │
                                      │     64-Bit Word Parallel Bitwise Punctuation & Tape Index  │
                                      │          Zero Heap Allocation / Zero Dynamic Copies        │
                                      └─────────────────────────────┬──────────────────────────────┘
                                                                    │  (Direct Monomorphic Mapping)
                                                                    ▼
                                      ┌────────────────────────────────────────────────────────────┐
                                      │          DisruptorRing<N> (engine-disruptor)               │
                                      │     Contiguous Array of 64-Byte Cache-Line RingEntry Slots │
                                      │              Single-Cycle Bitmask: seq & mask              │
                                      └─────────────────────────────┬──────────────────────────────┘
                                                                    │
                                     ┌──────────────────────────────┴──────────────────────────────┐
                                     ▼                                                             ▼
┌────────────────────────────────────────────────────────────┐       ┌────────────────────────────────────────────────────────────┐
│         Inside Process Boundary: Zero Data Travel          │       │       Outside Process Boundary: Sequential Hardware        │
├────────────────────────────────────────────────────────────┤       ├────────────────────────────────────────────────────────────┤
│ • engine-eda (ProjectionStore): Updates ArcSwap atomic ptr │       │ • engine-log (SegmentLog): Sequential append to 64MB       │
│ • singleflight-barrier: Coalesces in-flight reads in RAM   │       │   contiguous disk segments (Kafka-style, zero head seeks)  │
│ • edge-guard-sdk: Inlined branch gate in 50–100 ns         │       │ • MmapStore: Zero-copy read slices straight from OS        │
│ • Result: 0 bytes copied, 0 heap allocations on hot path   │       │   page cache lines without read() syscalls                 │
└────────────────────────────────────────────────────────────┘       └────────────────────────────────────────────────────────────┘
```
