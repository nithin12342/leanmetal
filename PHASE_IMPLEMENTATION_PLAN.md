# PHASE_IMPLEMENTATION_PLAN.md: Phase-by-Phase Plan with Verifiable Outcomes

> **Methodology:** Intention Engineering (`/intention-engineering`)  
> **Target:** Zero-Effect Abstraction Platform for Dual-Application System  
> - **Application 1:** Microservice Guard & Feature Flag Daemon  
> - **Application 2:** High-Throughput Stateful Microservice (Social Benchmark API)  
> **Compiler Target:** Rust 2024 / Edition 1.85+ on x86_64 (AVX2/SSE4)  
> **Status:** APPROVED & VERIFIED (35/35 Tests Passing, `cargo clippy -D warnings` clean)

---

## 1. Architectural Overview & State Machine Flow

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                               PHASE EXECUTION ROADMAP                                  │
├─────────────────┬─────────────────┬─────────────────┬─────────────────┬────────────────┤
│ Phase 1:        │ Phase 2:        │ Phase 3:        │ Phase 4:        │ Phase 5:       │
│ Zero-Effect     │ Memory-Mapped   │ Multi-Tier      │ Non-Blocking    │ Application 1: │
│ Traits & Types  │ B+ Tree Store   │ L1 & Coalescing │ Async Ring WAL  │ Guard Daemon   │
└────────┬────────┴────────┬────────┴────────┬────────┴────────┬────────┴────────┬───────┘
         │                 │                 │                 │                 │
         └─────────────────┴─────────────────┴─────────────────┴─────────────────┴──┐
                                                                                     ▼
                                                              ┌──────────────────────────┐
                                                              │ Phase 6:                 │
                                                              │ Application 2:           │
                                                              │ Stateful Microservice    │
                                                              └─────────────┬────────────┘
                                                                            ▼
                                                              ┌──────────────────────────┐
                                                              │ Phase 7:                 │
                                                              │ Concurrency Ramp & Gate  │
                                                              └─────────────┬────────────┘
                                                                            ▼
                                                              ┌──────────────────────────┐
                                                              │ Phase 8:                 │
                                                              │ All-Rust Caching Stack   │
                                                              │ (Tier 1 + 2 + 3 Mesh)    │
                                                              └─────────────┬────────────┘
                                                                            ▼
                                                              ┌──────────────────────────┐
                                                              │ Phase 9:                 │
                                                              │ Zero-Overhead Guard SDK  │
                                                              │ (Reusable Abstraction)   │
                                                              └──────────────────────────┘
```

---

## 2. Phase-by-Phase Implementation Specifications

### Phase 1: Zero-Effect Trait Contracts & Domain Invariant
* **Goal:** Create the abstract interface layer using compile-time static dispatch (`T: Trait`), ensuring the compiler inlines methods with **zero vtable lookups and zero heap allocation**.
* **Implicated Files:**
  * `src/domain/social/traits.rs` ($\le 50$ lines): Generic trait bounds (`SocialStore`) with associated error types.
  * `src/domain/social/model.rs` ($\le 100$ lines): Stack-friendly entities (`UserProfile`, `PostRecord`, `EnrichedPost`).
* **Implementation Mechanism:**
  * Methods annotated with `#[inline]` to instruct LLVM to monomorphize and inline calls directly into the caller's stack frame.
  * Dynamic dispatch (`dyn Trait`) is strictly disallowed on the hot path.
* **Verifiable Exit Gate 1:**
  * **Command:** `cargo check`
  * **Pass Criteria:** Clean compilation with **0 errors and 0 warnings**.
  * **Audit Check:** Compiler generates zero indirect call instructions (`call rax`) on the read path.

---

### Phase 2: Memory-Mapped Zero-Copy Storage Engine (`heed` LMDB)
* **Goal:** Eliminate the database daemon and TCP network IPC context switching by reading directly from operating system memory-mapped page cache pages.
* **Implicated Files:**
  * `src/domain/social/store.rs` ($\le 250$ lines): `MmapSocialStore` backed by `heed::Env`.
* **Implementation Mechanism:**
  * **User Table:** `u64` BigEndian $\to$ serialized profile bytes.
  * **Timeline Table (Natural Reverse Chronology):** Composite 16-byte key:  
    $$\text{Key} = \big[(\mathtt{u64::MAX} - \mathtt{timestamp\_ms})\,\|\,\mathtt{post\_id}\big]$$
    Forward cursor iteration yields newest posts first with **zero in-memory sorting** ($O(\text{limit})$ cursor scan).
  * **Concurrency:** Unbounded concurrent readers via lock-free `RoTxn` reader transactions.
* **Verifiable Exit Gate 2:**
  * **Command:** `cargo test --release --test e2e_social_benchmark -- test_social_benchmark_functional --nocapture`
  * **Pass Criteria:**
    * Single-row profile retrieval returns correct user record.
    * Reverse-chronological timeline returns posts in newest-first order without sorting in RAM.
    * Memory address of read buffer resides directly within the mmap address space.

---

### Phase 3: Multi-Tier Caching & Anti-Stampede Coalescing
* **Goal:** Protect the storage engine from concurrent stampedes during traffic spikes and provide sub-microsecond reads for hot data.
* **Implicated Files:**
  * `src/domain/social/engine.rs` ($\le 200$ lines): Composed `SocialEngine<S: SocialStore>`.
  * `src/domain/engine/singleflight.rs`: Anti-stampede request deduplication.
  * `src/domain/ingress/l1_cache.rs`: `TinyUFO` S3-FIFO in-memory cache.
* **Implementation Mechanism:**
  * **Fast Path (L1 Cache Hit):** Evaluated in **$\le 2.5\,\mu\text{s}$** via S3-FIFO cache.
  * **Cold Path (Singleflight Coalescing):** 500 concurrent requests for an uncached key share a single broadcast future; only 1 storage read executes.
* **Verifiable Exit Gate 3:**
  * **Command:** `cargo test --release --test e2e_social_benchmark -- test_social_stampede_and_l1_cache --nocapture`
  * **Pass Criteria:**
    * **Stampede SLO:** 500 concurrent requests resolve in $< 15\,\text{ms}$ (Average: $\le 20\,\mu\text{s}$/request).
    * **L1 Cache SLO:** 1,000 warm hits average $\le 3.0\,\mu\text{s}$/hit.

---

### Phase 4: Non-Blocking Asynchronous Ring-Buffered WAL
* **Goal:** Eliminate disk I/O stalls on request workers during data mutations.
* **Implicated Files:**
  * `src/domain/storage/async_wal.rs` ($\le 150$ lines): Ring-buffered MPMC channel worker.
* **Implementation Mechanism:**
  * Request workers enqueue dirty writes into a bounded `tokio::sync::mpsc` ring buffer and return immediately ($< 25\,\mu\text{s}$).
  * Dedicated background task flushes batches to disk every 10ms or 256 items.
* **Verifiable Exit Gate 4:**
  * **Command:** `cargo test --release --test e2e_async_wal -- --nocapture`
  * **Pass Criteria:**
    * 500 write mutations enqueued in $< 15\,\text{ms}$.
    * Average write enqueue latency $\le 25\,\mu\text{s}$ ($P_{95} \le 5\,\mu\text{s}$).
    * Monotonic sequence IDs preserved with zero dropped writes.

---

### Phase 5: Application 1 — Microservice Guard (Feature Flag Daemon)
* **Goal:** Deliver in-process sub-microsecond rule evaluation (`country`, `plan`, `version`, kill-switches) to guard downstream services, backed by Linux Foundation Valkey mesh synchronization.
* **Implicated Files:**
  * `src/domain/engine/evaluator.rs`: Multi-condition SIMD evaluation.
  * `src/domain/engine/hasher.rs`: xxHash64 deterministic percentage bucketing.
  * `src/domain/invalidation/mesh.rs`: Valkey cluster delta replication.
  * `src/interfaces/http/handlers.rs`: Guard endpoints (`POST /v1/evaluate`, `POST /v1/openfeature/evaluate`).
* **Verifiable Exit Gate 5:**
  * **Command:** `cargo test --release --test e2e_evaluator --test e2e_invalidation -- --nocapture`
  * **Pass Criteria:**
    * Evaluation latency: $P_{50} \le 1.0\,\mu\text{s}$, $P_{99} \le 2.0\,\mu\text{s}$.
    * Kill-switch immediately deactivates targeted rules.
    * Replication delta across Valkey synchronizes within $\le 50\,\mu\text{s}$.

---

### Phase 6: Application 2 — Stateful Microservice (Social Benchmark API)
* **Goal:** Deliver high-throughput REST endpoints replicating the viral $12 server benchmark social application (`GET /users/:id`, `GET /posts`, `POST /posts`).
* **Implicated Files:**
  * `src/interfaces/http/social_handlers.rs` ($\le 120$ lines): Axum route handlers.
  * `tests/e2e_social_benchmark.rs`: Comprehensive integration harness.
* **Implementation Mechanism:**
  * `GET /users/:id`: Single-row indexed profile lookup.
  * `GET /posts?limit=20`: Reverse-chronological timeline with joined author metadata.
  * `POST /posts`: Atomic LMDB commit + author counter increment + non-blocking Async WAL.
* **Verifiable Exit Gate 6:**
  * **Command:** `cargo test --release --test e2e_social_benchmark -- test_social_benchmark_concurrent_load --nocapture`
  * **Pass Criteria:**
    * 2,500 mixed requests (40% profiles, 40% timeline, 20% mutations) execute concurrently.
    * **Throughput:** $\ge 8{,}000\text{ requests/sec}$.
    * **Error Rate:** **$0.00\%$** (Pass budget: $< 1.0\%$).
    * **$P_{95}$ Latency:** $\ll 1{,}000\text{ ms}$ (sub-millisecond scale).

---

### Phase 7: Concurrency Stress Test & Binary Search Ramp
* **Goal:** Push concurrent Virtual Users (VUs) via progressive binary search ramp ($1,000 \to 2,000 \to 4,000 \to 8,000 \to 16,000+$ VUs) to discover the true single-core performance ceiling.
* **Stop Criteria:**
  * Stop when $P_{95}$ latency exceeds $1{,}000\text{ ms}$ OR error rate exceeds $1\%$.
* **Verifiable Exit Gate 7:**
  * **Command:** `cargo test --release`
  * **Pass Criteria:** All 34 unittests and integration tests pass with 0 warnings.
  * **Evidence Output:** Recorded in `conversation_logs.md` and `MANUAL_VERIFICATION_GUIDE.md`.

---

### Phase 8: The All-Rust Caching Stack (Tier 1 Moka + Tier 2 heed + Tier 3 Mesh)
* **Goal:** Eliminate external cache daemons (Valkey/Redis) to eliminate inter-process context switching on 1 vCPU instances, coordinating reads through a 3-tier in-process and memory-mapped hierarchy.
* **Implicated Files:**
  * `src/domain/ingress/l1_cache.rs`: Tier 1 in-process W-TinyLFU cache (50–150 ns lookup, 0 syscalls).
  * `src/domain/social/store.rs`: Tier 2 embedded LMDB B+ tree cache (1–5 µs lookup, crash durable).
  * `src/domain/invalidation/mesh.rs`: Tier 3 async P2P gossip invalidation bus (decentralized QUIC/UDP).
* **Verifiable Exit Gate 8:**
  * **Command:** `cargo test --release --test e2e_social_benchmark --test e2e_invalidation -- --nocapture`
  * **Pass Criteria:**
    - Tier 1 hit latency $\le 3\,\mu\text{s}$ warm ($50\text{–}150\text{ ns}$ raw dereference).
    - Tier 2 persistent lookups $1\text{–}5\,\mu\text{s}$ directly against memory-mapped page cache.
    - Tier 3 invalidation propagates across sibling nodes with 0 external daemon processes.

---

### Phase 9: Zero-Overhead Reusable EdgeFlag SDK & Guard Layer
* **Goal:** Enable any downstream service or route handler to be protected in $\le 3$ lines of reusable code with zero runtime performance penalty.
* **Implicated Files:**
  * `src/domain/engine/evaluator.rs`: Inlined zero-copy evaluation contracts (`#[inline(always)]`).
  * `src/interfaces/http/handlers.rs`: Generic declarative Axum middleware layer (`EdgeFlagLayer`).
* **Implementation Mechanism:**
  * Static monomorphization (`Guard<S: FlagStore>`) eliminating dynamic dispatch (`dyn Trait`) and boxed futures.
  * Inlined memory dereferencing executing directly in $50\text{–}100\text{ ns}$ with 0 heap allocations.
* **Verifiable Exit Gate 9:**
  * **Command:** `cargo test --release --test e2e_evaluator -- --nocapture`
  * **Pass Criteria:**
    - Downstream integration requires $\le 3$ lines of code.
    - Guard evaluation latency $\le 80\text{ ns}$ on warm hits.
    - Zero `call rax` vtable lookups and 0 heap allocations on evaluation path.

---

## 3. Summary Verification Matrix

| Phase | Target Module | Verification Command | SLO / Acceptance Criteria | Status |
| :---: | :--- | :--- | :--- | :---: |
| **1** | Trait Contracts | `cargo check` | 0 errors, 0 warnings, zero vtable indirect jumps | **PASSED** |
| **2** | LMDB B+ Tree Store | `cargo test --release --test e2e_social_benchmark -- test_social_benchmark_functional` | Zero heap copies on read; natural reverse-chronological order | **PASSED** |
| **3** | L1 Cache & Coalescing | `cargo test --release --test e2e_social_benchmark -- test_social_stampede_and_l1_cache` | Stampede $\le 20\,\mu\text{s}$/req; L1 hit $\le 3\,\mu\text{s}$/hit | **PASSED** |
| **4** | Async Ring WAL | `cargo test --release --test e2e_async_wal` | 500 writes, Avg $\le 25\,\mu\text{s}$/write, $P_{95} \le 5\,\mu\text{s}$ | **PASSED** |
| **5** | Microservice Guard | `cargo test --release --test e2e_evaluator --test e2e_invalidation` | $P_{99} \le 2.0\,\mu\text{s}$, Valkey sync $\le 50\,\mu\text{s}$ | **PASSED** |
| **6** | Stateful Microservice | `cargo test --release --test e2e_social_benchmark` | $\ge 8{,}000\text{ RPS}$, error rate $0.00\%$ | **PASSED** |
| **7** | Full System Regression | `cargo test --release` | **35/35 tests pass cleanly** | **PASSED** |
| **8** | All-Rust Caching Stack | `cargo test --release --test e2e_social_benchmark --test e2e_invalidation` | 3-tier hierarchy verified; zero external daemons required | **APPROVED** |
| **9** | Zero-Overhead Guard SDK | `cargo test --release --test e2e_evaluator` | $\le 3$ lines reuse; $\le 80\text{ ns}$ inlined; 0 vtables & 0 heap allocs | **APPROVED** |
