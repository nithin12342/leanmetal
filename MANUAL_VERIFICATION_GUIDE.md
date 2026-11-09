# MANUAL_VERIFICATION_GUIDE.md: Audit & Verification Protocol
**Methodology:** Intention Engineering (`/intention-engineering`) — Mandatory Invariant #4

---

## 1. Overview & Verification Invariants

In Intention Engineering, **nothing is marked done on a claim**. Every unit of work must produce unambiguous, verifiable proof of execution against real inputs. 

This guide defines the explicit manual audit commands, expected artifacts, and delta checks required to independently verify each layer of the **EdgeFlag Daemon**.

---

## 2. Environment Verification Commands

Run the following commands in PowerShell or Bash to verify the underlying toolchains:

```powershell
# 1. Verify Rust & Cargo version
cargo --version
# Expected Output: cargo 1.98.1+

# 2. Verify GCC compiler & linker (WinLibs MinGW-w64)
gcc --version
# Expected Output: gcc (MinGW-W64 x86_64-ucrt-posix-seh...) 16.2.0

# 3. Verify FlatBuffers Schema Compiler
flatc --version
# Expected Output: flatc version 25.12.19

# 4. Verify CMake build system
cmake --version
# Expected Output: cmake version 4.4.2+
```

---

## 3. Build & Compilation Verification (Phase 3 Gate)

Run the full dependency check and crate compilation:

```powershell
cargo check
```
* **Expected Result:** `Finished dev profile [unoptimized + debuginfo] target(s) in ...` with **0 errors**.
* **Audit Check:** Ensure no unresolved external crates or compiler warnings exist.

---

## 4. Layer-by-Layer Verification Protocols

### A. Layer 2: Core Rule Engine & xxHash64 Bucketing (`METHOD-001`)
* **Verification Objective:** Verify deterministic hashing and sub-microsecond calculation time.
* **Command:**
  ```powershell
  cargo test --bin edgeflag -- test_xxhash_bucketing
  ```
* **Verification Check:**
  - Hashes 100,000 arbitrary `(flag_key, user_id)` combinations.
  - Bucket values fall strictly in the range $[0, 99]$.
  - Chi-Square uniformity test yields $p > 0.05$.
  - Average execution time $\le 15\,\text{ns}$ per bucket calculation.

### B. Layer 4: Zero-Copy Memory-Mapped Persistence (`METHOD-004`)
* **Verification Objective:** Verify direct memory pointer dereferencing without heap allocation.
* **Command:**
  ```powershell
  cargo test --bin edgeflag -- test_mmap_zero_copy_read
  ```
* **Verification Check:**
  - Writes a FlatBuffers serialized rule table to a temporary LMDB database (`heed`).
  - Reads back the rule slice `&[u8]`.
  - Memory address of the returned slice resides within the OS virtual memory page cache mapping.
  - Heap allocation count during read is exactly `0`.

### C. Layer 3: Linux Foundation Valkey Invalidation Mesh (`METHOD-006`)
* **Verification Objective:** Verify sub-millisecond Pub/Sub invalidation between edge nodes.
* **Prerequisite:** Start local Valkey daemon (`valkey-server` on Linux/WSL).
* **Command:**
  ```powershell
  cargo test --bin edgeflag -- test_valkey_invalidation_mesh
  ```
* **Verification Check:**
  - Subscriber connects to channel `edgeflag:invalidation:v1`.
  - Admin publisher mutates flag `beta_feature_checkout`.
  - Subscriber receives payload within $< 5\,\text{ms}$ and purges local TinyUFO L1 cache entry.

---

## 5. Traceability Matrix & Hash Audit Table

| Traceability ID | Target Artifact / File | Verifiable Metric | Verification Method |
| :--- | :--- | :--- | :--- |
| `VERIFY-001` | `src/domain/engine/hasher.rs` | $10^5$ uniform buckets in $[0, 99]$ | `cargo test test_xxhash_bucketing` |
| `VERIFY-002` | `src/domain/engine/evaluator.rs` | Evaluation latency $\le 100\,\mu\text{s}$ ($P_{99}$) | Synthetic benchmark harness |
| `VERIFY-003` | `src/domain/invalidation/mesh.rs` | Invalidation delivery $< 5\,\text{ms}$ | Valkey Pub/Sub multi-client test |
| `VERIFY-004` | `src/domain/storage/mmap_store.rs` | 0 heap allocations on read path | Pointer address vs page map check |
| `VERIFY-005` | `src/domain/ingress/connection_ring.rs`| Single buffer memory allocation per broadcast | `Arc<[u8]>` pointer equality test |
| `VERIFY-019` | `tests/e2e_social_benchmark.rs` | Stampede coalescing $\le 20\,\mu\text{s}$/req | `cargo test --release --test e2e_social_benchmark` |
| `VERIFY-020` | `tests/e2e_social_benchmark.rs` | $O(\text{limit})$ B+ tree timeline cursor scan | Reverse chronological sort verification |
| `VERIFY-021` | `tests/e2e_social_benchmark.rs` | Sustained throughput $\ge 8{,}000\text{ RPS}$, error rate $0.00\%$ | Concurrent load simulation (2,500 operations) |
| `VERIFY-026` | `src/domain/ingress/l1_cache.rs` | Tier 1 lookup latency $50\text{–}150\text{ ns}$, zero syscalls | `cargo test --release --test e2e_social_benchmark` |
| `VERIFY-027` | `src/domain/social/store.rs` | Tier 2 mmap B+ tree $1\text{–}5\,\mu\text{s}$, zero restart loss | `cargo test --release --test e2e_social_benchmark` |
| `VERIFY-028` | `src/domain/invalidation/mesh.rs`| Tier 3 P2P gossip eviction $< 10\text{ ms}$, zero daemons | `cargo test --release --test e2e_invalidation` |
| `VERIFY-029` | `src/domain/engine/evaluator.rs` | Guard evaluation $\le 80\text{ ns}$, 0 vtables, 0 heap allocs | `cargo test --release --test e2e_evaluator` |

---

## 6. Social Concurrency Benchmark Verification Protocol

### Command to Execute:
```powershell
cargo test --release --test e2e_social_benchmark -- --nocapture
```

### Verification Criteria & Audit Checklist:
1. **Functional API Conformance:**
   - `GET /health` returns HTTP 200 `healthy`.
   - `GET /users/:id` returns user account profile via single-row indexed lookup.
   - `POST /posts` persists post, updates author post counts, and enqueues to async WAL.
   - `GET /posts?limit=10&offset=0` returns recent posts joined with author profile metadata in reverse-chronological order without in-memory `ORDER BY` sorting.
2. **Stampede Coalescing & L1 Cache:**
   - 500 concurrent stampede requests resolve in $< 10\text{ ms}$ (Average: $\le 20\,\mu\text{s}$/request).
   - Warm L1 cache hits average $\le 3\,\mu\text{s}$/hit.
3. **High-Concurrency Load:**
   - 2,500 mixed requests (40% profile reads, 40% timeline feeds, 20% mutations) execute concurrently.
   - Error rate must be **$0.00\%$** (Budget: $< 1\%$).
   - Sustained throughput exceeds **$8{,}000\text{ requests/sec}$** on local loopback.

---

## 7. The All-Rust Caching Stack Verification Protocol

### A. Tier 1: In-Process Ultra-Fast L1 Cache (`VERIFY-026`)
* **Objective:** Validate sub-microsecond in-process lookup latency and zero-copy pointer dereference.
* **Audit Command:**
  ```powershell
  cargo test --release --test e2e_social_benchmark -- test_social_stampede_and_l1_cache --nocapture
  ```
* **Success Criteria:**
  - 1,000 warm read hits complete in $\le 3.0\,\mu\text{s}$ per hit (individual in-process dereference: $50\text{–}150\text{ ns}$).
  - Pointer equality check confirms zero intermediate heap allocation (`Arc<[u8]>` returned directly).

### B. Tier 2: Embedded Memory-Mapped Disk Cache (`VERIFY-027`)
* **Objective:** Validate zero-IPC OS page cache reads and instant restart durability without warm-up penalty.
* **Audit Command:**
  ```powershell
  cargo test --release --test e2e_social_benchmark -- test_social_benchmark_functional --nocapture
  ```
* **Success Criteria:**
  - Single-row profile reads and reverse-chronological timeline feeds execute in $1\text{–}5\,\mu\text{s}$.
  - State survives process restart by opening pre-existing `data.mdb` file directly.

### C. Tier 3: Rust Native Mesh & Invalidation Layer (`VERIFY-028`)
* **Objective:** Validate cluster-wide key eviction via P2P gossip bus with zero external daemons.
* **Audit Command:**
  ```powershell
  cargo test --release --test e2e_invalidation -- --nocapture
  ```
* **Success Criteria:**
  - Mutation on Node A transmits binary framed eviction datagram `[OpCode: 1B | KeyLen: 2B | Key: &[u8]]`.
  - Node B purges local Tier 1 L1 cache within $< 10\text{ ms}$.
  - Zero external process dependencies (no background `redis-server` or `valkey-server` daemon container required).

---

## 8. Zero-Overhead Reusable EdgeFlag Abstraction Verification Protocol

### A. Ergonomic Reusability Audit ($\le 3$ Lines of Code)
* **Objective:** Ensure the Guard SDK can be integrated into downstream routers or business logic without framework lock-in or boilerplate.
* **Audit Command:**
  ```powershell
  cargo test --release --test e2e_evaluator -- --nocapture
  ```
* **Success Criteria:**
  - Route protection integrates declaratively via `.layer(EdgeFlagLayer::require("flag_name", guard))`.
  - Hot-path direct call evaluates via `guard.is_enabled("flag_name", &ctx)`.

### B. Zero-Overhead Performance Audit (`VERIFY-029`)
* **Objective:** Prove the abstraction incurs zero performance penalty compared to hand-coded in-place checks.
* **Audit Checks:**
  1. **Latency SLO:** Inlined guard evaluation completes in $\le 80\text{ ns}$ ($P_{99} \le 120\text{ ns}$).
  2. **Heap Allocation:** Heap allocation count on the evaluation path is strictly **0**.
  3. **Assembly Contract:** Binary disassembly contains zero indirect vtable calls (`call rax`) on the evaluation path.



