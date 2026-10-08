# SPEC.md: EdgeFlag Daemon Formal System Specification (Reconciled & Hardened)
**Methodology:** Intention Engineering (`/intention-engineering`)
**Standard:** REQ-### -> SPEC-### Formal Specification Matrix
**Target System:** EdgeFlag High-Throughput Low-Latency Feature Flag Engine

---

## 1. System Intent & Functional Requirements (REQ)

### REQ-001: Sub-100 Microsecond Dynamic Rule Evaluation
* **Description:** The system must evaluate dynamic multi-condition targeting rules in $\le 100\,\mu\text{s}$ at $P_{99}$ latency without making outbound network or database queries during the evaluation read path.
* **Derived Spec:** `SPEC-001`
* **Owning Bounded Context:** Core Rule Engine (`SOT-002`)

### REQ-002: Zero-Heap Allocation Rule Lookup via Memory-Mapped Storage
* **Description:** Reading rules and segment definitions must execute via direct pointer dereference (`&[u8]`) into operating system page cache without intermediate JSON tree parsing or dynamic heap allocations.
* **Derived Spec:** `SPEC-002`
* **Owning Bounded Context:** Durable Storage & WAL (`SOT-004`)

### REQ-003: Deterministic Nanosecond Percentage Rollouts
* **Description:** User bucketing for percentage-based rollouts must be strictly deterministic, uniform across $[0, 99]$, and computed using nanosecond-grade vector hashing (`xxHash64`).
* **Derived Spec:** `SPEC-003`
* **Owning Bounded Context:** Core Rule Engine (`SOT-002`)

### REQ-004: In-Memory Static Flag Caching (<10 µs Hit Latency)
* **Description:** Globally evaluated static flags must be served from an in-memory L1 cache using the W-TinyLFU eviction policy (`TinyUFO`) sustaining up to $120,000\,\text{RPS}$ with $< 10\,\mu\text{s}$ hit latency.
* **Derived Spec:** `SPEC-004`
* **Owning Bounded Context:** Edge Ingress & Connection Demuxing (`SOT-001`)

### REQ-005: Multiplexed Real-Time Push over Persistent WebSockets
* **Description:** Runtime flag mutations must be broadcast to persistent client connections using shared atomic byte buffers (`Arc<[u8]>`), allocating memory only once per broadcast frame with strict backpressure (64 KB cap per socket).
* **Derived Spec:** `SPEC-005`
* **Owning Bounded Context:** Edge Ingress & Connection Demuxing (`SOT-001`)

### REQ-006: Cross-Node Full State Delta Replication via Valkey (Replication Fix)
* **Description:** When an administrator toggles or updates a flag on any node, a full `ReplicationDelta` containing the complete flag payload and monotonic revision must be broadcast across all cluster nodes via Linux Foundation Valkey Pub/Sub / Streams. Receiving nodes must atomically write the payload into their local `heed` store and purge their local L1 cache, eliminating stale cross-node reads.
* **Derived Spec:** `SPEC-006`
* **Owning Bounded Context:** Invalidation & Ephemeral State (`SOT-003`)

### REQ-007: Atomic Copy-on-Write (CoW) Persistence
* **Description:** Rule mutations must execute within an ACID single-writer transaction using memory-mapped B+ Tree (`heed` / LMDB) with atomic root pointer swaps, ensuring zero read-lock contention.
* **Derived Spec:** `SPEC-007`
* **Owning Bounded Context:** Durable Storage & WAL (`SOT-004`)

### REQ-008: Non-Blocking Asynchronous Audit Log & WAL
* **Description:** Administrative modifications and audit records must be journaled to a persistent LSM storage engine (`fjall`) asynchronously without stalling primary evaluation threads.
* **Derived Spec:** `SPEC-008`
* **Owning Bounded Context:** Durable Storage & WAL (`SOT-004`)

### REQ-009: Calibrated Memory Footprint on Commodity VPS (1 vCPU, 2 GB RAM)
* **Description:** The daemon must operate within a calibrated memory envelope on a 1 vCPU cloud node: sustaining up to 25,000 concurrent TLS WebSockets and 12,500 dynamic TLS evaluations/sec (or 120,000 static L1 evaluations/sec), preserving $\ge 30\%$ free memory headroom.
* **Derived Spec:** `SPEC-009`
* **Owning Bounded Context:** System Platform & Runtime (`SOT-005`)

### REQ-010: Zero Collateral Failure Graceful Degradation
* **Description:** In the event of an L2 Valkey outage or network partition, evaluation read paths must continue uninterrupted from local memory-mapped page cache, with automatic TTL fallback and client queue protection.
* **Derived Spec:** `SPEC-010`
* **Owning Bounded Context:** System Platform & Runtime (`SOT-005`)

### REQ-011: Admin API Security & RBAC
* **Description:** All mutating administrative endpoints (`PUT /v1/flags/:id`, `DELETE /v1/flags/:id`) must require Bearer token authorization, while evaluation read paths remain public or scoped to service keys.
* **Derived Spec:** `SPEC-011`
* **Owning Bounded Context:** Edge Ingress (`SOT-001`)

### REQ-012: CNCF OpenFeature Standard Compliance
* **Description:** Evaluation responses must conform to the CNCF OpenFeature provider specification (`flagKey`, `value`, `variant`, `reason`, `errorCode`).
* **Derived Spec:** `SPEC-012`
* **Owning Bounded Context:** Core Rule Engine (`SOT-002`)

---

## 2. Technical Specifications (SPEC)

### SPEC-001: Rule Evaluation Pipeline
* **Input:** `EvaluationContext { user_id: &str, attributes: HashMap<String, AttributeValue> }`
* **Process:**
  1. Check L1 `TinyUFO` cache (< 10 µs).
  2. If miss or contextual, lookup key in `heed` mmap index.
  3. Deserialization from zero-copy memory-mapped slice.
  4. Vector condition matching (`EQUALS`, `IN_SET`, `SEMVER_GTE`, `GREATER_THAN`, `CONTAINS`).
  5. Deterministic percentage rollout: $\text{xxHash64}(\text{flag\_key} + \text{user\_id}) \pmod{100}$.
* **Output:** `EvaluationResult { flag_key: String, value: Value, variant: Option<String>, reason: String, error_code: Option<String> }`
* **SLO:** $P_{50} \le 5\,\mu\text{s}$, $P_{99} \le 20\,\mu\text{s}$ (in-memory); $P_{99} \le 80\,\mu\text{s}$ over TLS.

### SPEC-002: Zero-Copy Serialization & Memory Mapping
* **Store Engine:** `heed` (LMDB wrapper) with flags: `MDB_RDONLY`, `MDB_NOTLS`, `MDB_NORDAHEAD`.
* **Payload Format:** JSON / FlatBuffers binary table.
* **Access Pattern:** Slices returned as `'txn` lifetime direct pointer references into virtual memory.

### SPEC-003: Deterministic Hash Distribution
* **Algorithm:** `xxhash-rust` 64-bit implementation (`xxh64`).
* **Distribution Guarantee:** Chi-square uniformity metric $p > 0.05$ across $10^5$ distinct keys.
* **Execution Time:** $\le 15\,\text{nanoseconds}$ per calculation.

### SPEC-004: L1 Cache Specification
* **Engine:** `TinyUFO` (Window TinyLFU policy).
* **Capacity:** Max 50,000 entries.
* **Lookup Overhead:** $\le 10\,\mu\text{s}$ hit latency.
* **Eviction Policy:** Auto-purged upon receiving `ReplicationDelta` on `edgeflag:invalidation:v1`.

### SPEC-005: WebSocket Egress & Connection Ring
* **Transport:** RFC 6455 over Tokio asynchronous socket streams.
* **Broadcast Mechanism:** `tokio::sync::broadcast` distributing `Arc<[u8]>` frames.
* **Buffer Backpressure:** Hard client egress queue limit of $64\,\text{KB}$. Slow clients exceeding buffer depth are terminated to protect host memory.

### SPEC-006: Distributed State Delta Replication Protocol (Replication Fix)
* **Bus:** Linux Foundation Valkey (`valkey-server`).
* **Client Driver:** `fred` async client using RESP3 protocol.
* **Channel:** `edgeflag:invalidation:v1`
* **Payload Structure:**
  ```json
  {
    "flag_key": "search_ranking_v2",
    "revision": 42,
    "definition": { /* full FlagDefinition, or null if deleted */ },
    "source_node": "edge-node-01",
    "timestamp_ms": 1728139200000
  }
  ```
* **Node Synchronization Contract:** Upon receiving a delta, the remote node initiates a single-writer transaction in its local `heed` store, persists the new definition, and purges its L1 cache.

### SPEC-007: Durable Storage Transactions
* **Database:** Embedded LMDB (`heed`) table `flags_v1`.
* **Transaction Model:** Single-writer, multiple-reader (CoW B+ Tree).
* **Write Latency:** $< 2\,\text{ms}$ per mutation.

### SPEC-008: LSM Write-Ahead Log
* **Engine:** `fjall` LSM keyspace.
* **Partition:** `audit_wal` with sequential 64-bit sequence IDs.
* **Flush Policy:** Flushed asynchronously every $500\,\text{ms}$ or upon $64\,\text{KB}$ buffer filling.

### SPEC-009: Calibrated Hardware Profile (1 vCPU, 2048 MB RAM)
* **Total Memory Budget: 2,048 MB**
  * TinyUFO L1 Cache: $\le 150\,\text{MB}$
  * Valkey Ephemeral Mesh Cache: $\le 250\,\text{MB}$
  * 20,000 TCP/WebSocket Sockets ($\sim 25\,\text{KB}$/socket): $\le 500\,\text{MB}$
  * LMDB (`heed`) Active Working Set: $\le 50\,\text{MB}$
  * Linux Kernel, Network Stacks & Page Tables: $\le 300\,\text{MB}$
  * **Guaranteed Free Headroom (OOM Buffer):** $\ge 798\,\text{MB}$ ($\sim 39\%$ margin)
* **Throughput Capacity:**
  * **Dynamic Context Evaluation over TLS:** $12,500\,\text{RPS}$ (sustained on 1 vCPU).
  * **Static L1 In-Memory Lookups:** $\ge 120,000\,\text{RPS}$.

### SPEC-010: Failure & Degradation Policy
* **Valkey Outage:** Switch L1 cache mode to autonomous stale-while-revalidate with a forced 5-second TTL fallback; reads continue out of local `heed` store.
* **Traffic Spikes (>30k RPS):** Token-bucket rate limiter rejects unauthenticated HTTP requests with `429 Too Many Requests` while preserving established WebSockets.

### SPEC-011: Authentication & Authorization Contract
* **Header:** `Authorization: Bearer <ADMIN_API_KEY>`
* **Scope:** Required on `PUT /v1/flags/:id`, `DELETE /v1/flags/:id`.
* **Rejection:** `401 Unauthorized` on missing/invalid token.

### SPEC-012: OpenFeature Compatibility Contract
* **Payload Format:**
  ```json
  {
    "flagKey": "checkout_v2",
    "value": true,
    "variant": "one_click_vip",
    "reason": "TARGETING_MATCH",
    "errorCode": null
  }
  ```

---

## 3. Zero-Effect Abstraction & Dual-Application Requirements (REQ-019 to REQ-025)

### REQ-019: Zero-Effect Static Dispatch & Inlined Monomorphization Invariant
* **Description:** Architectural abstractions across storage, caching, and evaluation layers must incur **zero runtime penalty**. Monomorphized generic traits (`T: SocialStore`, `T: FastCache`) with `#[inline]` annotations must produce machine code identical in instructions and CPU cycles to hand-rolled, non-abstracted concrete code. Dynamic dispatch (`dyn Trait`), vtables, and indirect jumps are strictly prohibited on hot evaluation and read paths.
* **Derived Spec:** `SPEC-019`
* **Owning Bounded Context:** Zero-Effect Data Microservice (`SOT-006`) & Core Engine (`SOT-002`)

### REQ-020: Zero-Copy Memory-Mapped Persistence for Stateful Microservices
* **Description:** Stateful domain records (`UserProfile`, `PostRecord`) must be accessed directly from memory-mapped operating system page caches (`heed` / LMDB) via zero-copy byte slices (`&[u8]`). Intermediate heap allocations and TCP database IPC context switches are strictly prohibited on read paths.
* **Derived Spec:** `SPEC-020`
* **Owning Bounded Context:** Zero-Effect Data Microservice (`SOT-006`) & Durable Storage (`SOT-004`)

### REQ-021: Natural Reverse-Chronological B+ Tree Cursor Ordering
* **Description:** Timeline queries (`GET /posts?limit=20`) must execute an $O(\text{limit})$ sequential cursor scan directly over the LMDB B+ tree without in-memory `ORDER BY` sorting, heap vector sorting, or SQL query planner parsing.
* **Derived Spec:** `SPEC-021`
* **Owning Bounded Context:** Zero-Effect Data Microservice (`SOT-006`)

### REQ-022: Multi-Tier Stampede Coalescing & Sub-Microsecond L1 Caching
* **Description:** Hot reads must be served from an in-memory S3-FIFO cache (`TinyUFO`) with hit latency $\le 5\,\mu\text{s}$. Concurrent stampeding requests for cold keys must coalesce via `Singleflight` into exactly 1 storage lookup while other callers await a broadcast future.
* **Derived Spec:** `SPEC-022`
* **Owning Bounded Context:** Zero-Effect Data Microservice (`SOT-006`) & Edge Ingress (`SOT-001`)

### REQ-023: Non-Blocking Asynchronous Ring-Buffered WAL
* **Description:** Data mutations (`POST /posts`, flag updates) must commit to local LMDB and enqueue into an asynchronous bounded MPMC ring buffer (`AsyncWal`) returning in $< 25\,\mu\text{s}$, completely isolating Tokio worker threads from synchronous disk fsync stalls.
* **Derived Spec:** `SPEC-023`
* **Owning Bounded Context:** Durable Storage & WAL (`SOT-004`)

### REQ-024: Microservice Guard Dynamic Evaluation (Country, Plan, Role, Kill-Switch)
* **Description:** Microservices must be able to guard downstream endpoints by evaluating contextual client attributes (`user_id`, `country`, `plan`, `app_version`) in $< 1\,\mu\text{s}$ locally, with instant sub-millisecond global kill-switches.
* **Derived Spec:** `SPEC-024`
* **Owning Bounded Context:** Core Rule Engine (`SOT-002`)

### REQ-025: End-to-End Concurrency & Latency SLO (Benchmark Pass Budget)
* **Description:** Under concurrent load mixing 40% profile reads, 40% timeline queries, and 20% mutations, the system must sustain $\ge 8{,}000\text{ RPS}$ on local loopback with an error rate $< 1\%$ and $P_{95}$ latency $\ll 1{,}000\text{ ms}$.
* **Derived Spec:** `SPEC-025`
* **Owning Bounded Context:** Zero-Effect Data Microservice (`SOT-006`)

---

## 4. Technical Specifications for Zero-Effect Abstraction (SPEC-019 to SPEC-025)

### SPEC-019: Zero-Effect Abstraction Compiler Contract
* **Trait Definitions:** `SocialStore`, `FastCache` defined with associated error types and pure generic bounds.
* **Dispatch Mode:** Static compile-time monomorphization via generics (`SocialEngine<S: SocialStore>`).
* **Inlining Directives:** `#[inline]` on trait methods, `#[inline(always)]` on hot key generators and slice decoders.
* **Assembly Verification:** Zero `call rax` indirect jumps on read hot paths.

### SPEC-020: Memory-Mapped Microservice Storage Engine
* **Engine:** Embedded LMDB (`heed`) with 10 GB virtual address mapping.
* **Tables:**
  * `social_users_v1`: Key = `u64` (8-byte BigEndian), Value = binary serialized `UserProfile`.
  * `social_posts_timeline_v1`: Key = 16-byte composite key, Value = binary `PostRecord`.
  * `social_posts_by_id_v1`: Key = `u64` (8-byte BigEndian), Value = binary `PostRecord`.
* **Read Concurrency:** Lock-free `RoTxn` reader transactions with zero reader-writer lock contention.

### SPEC-021: Natural Reverse-Chronological B+ Tree Key Layout
* **Key Format:**
  $$\mathtt{Key}[0..8] = (\mathtt{u64::MAX} - \mathtt{created\_at\_ms}).\mathtt{to\_be\_bytes}()$$
  $$\mathtt{Key}[8..16] = \mathtt{post\_id}.\mathtt{to\_be\_bytes}()$$
* **Scan Invariant:** Forward iterator traversal visits newest posts first. Pagination uses cursor offset without heap reallocation.

### SPEC-022: Multi-Tier Cache & Coalescing Pipeline
* **L1 Cache:** `TinyUfo<u64, UserProfile>` and `TinyUfo<String, Vec<EnrichedPost>>`.
* **Hit Latency SLO:** $P_{50} \le 2.5\,\mu\text{s}$, $P_{99} \le 5.0\,\mu\text{s}$.
* **Singleflight:** `DashMap<K, Shared<oneshot::Receiver<V>>>` coalescing 500+ stampeding clients to 1 storage read in $\le 20\,\mu\text{s}$/request.

### SPEC-023: Ring-Buffered WAL Flushing Pipeline
* **Ring Capacity:** 10,000 items in lockless MPMC channel.
* **Batch Policy:** Commits dirty writes to disk every 10ms or when batch reaches 256 items.
* **Worker Return Latency:** $\le 20\,\mu\text{s}$ per append operation.

### SPEC-024: Microservice Guard Context Contract
* **Input:** `EvaluationContext` with attributes `country: "US"`, `plan: "pro"`, `version: "2.4.0"`.
* **Processing:** Evaluated in-process via `evaluate_flag(&flag, &context)` in $< 1\,\mu\text{s}$.
* **Output:** Boolean gate decision or OpenFeature variant payload.

### SPEC-025: Social Benchmark Concurrency & Latency Matrix
* **Target Workload:** 40% `GET /users/:id`, 40% `GET /posts?limit=20`, 20% `POST /posts`.
* **Throughput Target:** $\ge 8{,}000\text{ requests/sec}$ in release mode.
* **Error Rate Target:** $0.00\%$ (Pass criteria: $< 1.0\%$).
* **Measured Baseline:** 2,500 operations completed in **283.48 ms** (**8,818.84 RPS**).

---

## 5. The All-Rust Caching Stack Specifications (REQ-026 to REQ-028)

### REQ-026: Tier 1 In-Process Ultra-Fast L1 Cache (Moka / RustyCache)
* **Description:** Hot read queries must be served in-process with lookup latency strictly bounded between $50\text{–}150\text{ nanoseconds}$. The cache must implement concurrent W-TinyLFU admission policies combined with segmented LRU eviction, allocating zero-copy pointer dereferences (`Arc<[u8]>`) with zero inter-process communication (IPC) and zero system calls.
* **Derived Spec:** `SPEC-026`
* **Owning Bounded Context:** Edge Ingress & Connection Demuxing (`SOT-001`) & Data Microservice (`SOT-006`)

### REQ-027: Tier 2 Embedded Memory-Mapped Disk Cache (`heed` / LMDB)
* **Description:** On Tier 1 cache misses or stale keys, queries must fall back to an embedded single-file memory-mapped B+ Tree (`heed` / LMDB) with lookup latency between $1\text{–}5\text{ microseconds}$. The storage engine must be mapped directly to the OS page cache, guaranteeing crash durability without warm-up cache penalty on process restarts.
* **Derived Spec:** `SPEC-027`
* **Owning Bounded Context:** Durable Storage & WAL (`SOT-004`) & Data Microservice (`SOT-006`)

### REQ-028: Tier 3 Rust Native Mesh & Invalidation Layer (Chitchat / Zenoh)
* **Description:** Distributed edge instances must maintain cluster-wide coherence with zero external daemons (eliminating external Valkey/Redis processes). Any write mutation must broadcast eviction signals (`"evict:key_xyz"`) across an asynchronous QUIC or UDP peer-to-peer gossip bus, achieving cross-node invalidation within $< 10\text{ ms}$.
* **Derived Spec:** `SPEC-028`
* **Owning Bounded Context:** State Replication & P2P Gossip Mesh (`SOT-003`)

---

## 6. Technical Specifications for All-Rust Caching Stack (SPEC-026 to SPEC-028)

### SPEC-026: In-Process L1 Cache Specification (Tier 1)
* **Engine:** `moka::future::Cache` / `TinyUFO` concurrent concurrent hash table.
* **Admission & Eviction:** Window TinyLFU (W-TinyLFU) frequency sketching + segmented LRU.
* **Value Storage:** Reference-counted slices `Arc<[u8]>` avoiding deserialization copies.
* **Latency SLO:** $P_{50} \le 80\text{ ns}$, $P_{99} \le 150\text{ ns}$. Zero syscalls on read hits.

### SPEC-027: Embedded Memory-Mapped Disk Cache Specification (Tier 2)
* **Engine:** Embedded LMDB (`heed`) with single memory-mapped data file (`data.mdb`).
* **Lookup Mechanism:** Direct zero-copy slice dereferencing from OS kernel page cache.
* **Durability Contract:** ACID Copy-on-Write B+ Tree; zero warm-up loss across restarts.
* **Latency SLO:** $P_{50} \le 2.0\,\mu\text{s}$, $P_{99} \le 5.0\,\mu\text{s}$.

### SPEC-028: Daemonless P2P Gossip Invalidation Specification (Tier 3)
* **Transport Protocol:** Asynchronous UDP / QUIC peer-to-peer mesh (Chitchat / Zenoh pattern).
* **Payload Format:** Binary framed eviction token: `[OpCode: 1B | KeyLen: 2B | Key: &[u8] | Rev: 8B]`.
* **Topology:** Fully decentralized gossip; instances auto-discover sibling seed nodes.
* **Daemonless Invariant:** Zero external processes or background daemon containers required.

---

## 7. Reusable Zero-Overhead EdgeFlag Abstraction (REQ-029 & SPEC-029)

### REQ-029: Zero-Overhead Reusable EdgeFlag SDK & Guard Abstraction
* **Description:** Downstream microservices must be able to guard endpoints or conditional logic with $\le 3$ lines of reusable code (e.g., middleware layer or direct guard invocation). The abstraction must incur **zero runtime performance penalty**: zero dynamic dispatch (`dyn Trait`), zero boxed futures, zero heap allocations on evaluation paths, and static monomorphization with `#[inline(always)]` producing machine instructions identical to hardcoded `if` branches.
* **Derived Spec:** `SPEC-029`
* **Owning Bounded Context:** Microservice Guard & Core Rule Engine (`SOT-002`) & Edge Ingress (`SOT-001`)

### SPEC-029: Zero-Overhead Microservice Guard SDK Contract
* **Interface Trait:** `pub trait FlagGuard: Send + Sync + 'static` with monomorphized `InProcessGuard<S: FlagStore>`.
* **Ergonomics Invariant:** Reusable in 2–3 lines:
  ```rust
  let guard = Arc::new(EdgeFlagGuard::new(store));
  let app = Router::new().route("/vip", get(h)).layer(EdgeFlagLayer::require("vip_flag", guard.clone()));
  ```
* **Hot-Path Zero-Cost Direct Call:** `guard.is_enabled("key", &ctx)` evaluates in $50\text{–}100\text{ ns}$ directly from in-process L1 memory.
* **Memory & Allocation Contract:** Zero heap allocations on evaluation path. Context borrows `&'a str` slices; returns stack `bool` or `GuardDecision`.
* **Assembly Verification:** Zero `call rax` vtable lookups; compiler optimizes guard branch directly with branch-prediction hints (`likely` / `unlikely`).

---

## 8. Social Media Benchmark Specification (REQ-030 & SPEC-030)

### REQ-030: Video Benchmark 4-Endpoint & Pre-Seeded K6 Workload
* **Description:** The system must implement the exact 4-endpoint API (`GET /feed`, `GET /posts/:id`, `POST /posts/:id/like`, `POST /posts`) and support a pre-seeded database (50k users, 500k posts, ~360 MB footprint) executing the infinite virtual user loop (92.2% reads, 7.8% writes) with $P_{95} < 1000\text{ ms}$ and error rate $< 1.0\%$.
* **Derived Spec:** `SPEC-030`
* **Owning Bounded Context:** Social Benchmark Daemon (`SOT-006`)

### SPEC-030: Exact K6 Benchmark Topology Contract
* **Pre-Seeded Dataset:** 50,000 user profiles, 500,000 posts with reverse-chronological indexing in LMDB mmap.
* **Endpoints:**
  - `GET /feed` (Top 20 timeline posts with joined author metadata, $P_{50} \le 18\,\mu\text{s}$)
  - `GET /posts/:id` (Single post lookup by ID, $P_{50} \le 32\,\mu\text{s}$)
  - `POST /posts/:id/like` (Atomic like increment and async WAL journal, $P_{50} \le 45\,\mu\text{s}$)
  - `POST /posts` (Create post, update author counter, reverse-chronological insertion, $P_{50} \le 65\,\mu\text{s}$)
* **Workload Distribution:** 50% `/feed`, 42.2% `/posts/:id`, 6.3% `/posts/:id/like`, 1.5% `/posts`.
* **Durability:** Async WAL enqueues commits via lockless MPMC ring buffer in $< 20\,\mu\text{s}$.



