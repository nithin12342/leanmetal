# CONTEXT.md: EdgeFlag Daemon Domain-Driven Design & Bounded Contexts (Hardened)
**Methodology:** Intention Engineering (`/intention-engineering`)
**Standard:** Domain-Driven Design (DDD), Aggregates, Invariants, and Ports

---

## 1. Domain Overview & Ubiquitous Language

The **EdgeFlag** domain addresses the challenge of evaluating dynamic runtime configurations, progressive rollouts, and kill-switches with mechanical sympathy — maximizing throughput and minimizing CPU cycles, memory allocations, and network hops.

### Ubiquitous Language Dictionary
* **Flag Key:** A globally unique ASCII string identifier referencing a feature flag or configuration parameter.
* **Evaluation Context:** The open set of client runtime attributes (`user_id`, `country`, `app_version`, `device_type`, `segment_ids`, custom metrics).
* **Targeting Rule:** A logical clause consisting of conditions (`Equals`, `InSet`, `SemverGte`, `GreaterThan`, `Contains`) and rollout strategies.
* **Deterministic Bucket:** An integer $[0, 99]$ computed by hashing `(FlagKey + UserID)` with `xxHash64`, ensuring a user always falls into the same bucket for a given flag.
* **Zero-Copy Slice:** A raw memory reference (`&[u8]`) mapped directly from the operating system's page cache without intermediate heap allocations.
* **Replication Delta:** A structured synchronization frame broadcast over the Valkey mesh containing the full flag definition, monotonic revision, and source node, enabling remote nodes to update their local `heed` stores.
* **Kill-Switch:** An emergency mutation that forces a flag to an inactive state globally in sub-millisecond time.
* **Invalidation & Replication Mesh:** An asynchronous Linux Foundation Valkey Pub/Sub bus coordinating instant cache purges and state synchronization across distributed edge nodes.
* **Zero-Effect Abstraction:** A software interface pattern where calling through generic traits (`T: SocialStore`, `T: FastCache`) with `#[inline]` compiles to machine instructions identical to direct in-place code, yielding zero CPU cycle or hardware overhead.
* **Reverse-Chronological B+ Tree Key:** A 16-byte composite big-endian key prefixing inverted timestamps `(u64::MAX - timestamp_ms)` to post IDs, sorting records chronologically in disk page layout without runtime sort algorithms.
* **Microservice Guard:** An in-process sub-microsecond evaluation gateway evaluating dynamic context (`country`, `plan`, `version`) to authorize or throttle downstream business logic without network hops.
* **Tier 1 In-Process Cache (W-TinyLFU):** In-memory cache (Moka / RustyCache) providing $50\text{–}150\text{ ns}$ lookups with zero IPC and zero syscalls via `Arc<[u8]>` pointer dereferencing.
* **Tier 2 Embedded Disk Cache (heed LMDB):** Single-file memory-mapped CoW B+ Tree providing $1\text{–}5\,\mu\text{s}$ lookups directly from the OS page cache with zero warm-up penalty on restarts.
* **Tier 3 Daemonless P2P Gossip Mesh (Chitchat / Zenoh):** Async UDP/QUIC peer-to-peer gossip bus broadcasting key evictions across nodes to achieve cluster-wide coherence with zero external daemons.
* **Zero-Overhead Guard SDK:** An ergonomic embedding layer (`EdgeFlagGuard`, `EdgeFlagLayer`) enabling microservices to guard endpoints or methods in $\le 3$ lines of code, compiling via static dispatch into inlined branches with zero heap allocations and zero vtable overhead.

---

## 2. Bounded Contexts (Skeleton-of-Thought Decomposition)

```
┌────────────────────────────────────────────────────────────────────────┐
│                        Bounded Context 1 (SOT-001)                     │
│                  Edge Ingress & Connection Demuxing                    │
│      (Rustls TLS 1.3, TinyUFO L1 Cache, WebSocket Connection Ring)     │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
           ┌────────────────────────┴────────────────────────┐
           ▼                                                 ▼
┌─────────────────────────────────────┐   ┌─────────────────────────────────────┐
│     Bounded Context 2 (SOT-002)     │   │     Bounded Context 6 (SOT-006)     │
│        Microservice Guard &         │   │       Zero-Effect Stateful          │
│          Core Rule Engine           │   │         Data Microservice           │
│ (Vector Matching, xxHash64, Schema) │   │ (Profiles, Natural B+ Tree Timeline)│
└──────────────────┬──────────────────┘   └──────────────────┬──────────────────┘
                   │                                         │
                   ▼                                         ▼
┌─────────────────────────────────────┐   ┌─────────────────────────────────────┐
│     Bounded Context 3 (SOT-003)     │   │     Bounded Context 4 (SOT-004)     │
│    Daemonless P2P Invalidation Mesh │   │        Durable Storage & WAL        │
│   (Chitchat/Zenoh UDP / Valkey Bus) │   │   (heed mmap B+ Tree + fjall LSM)   │
└─────────────────────────────────────┘   └─────────────────────────────────────┘
```

---

### Bounded Context 1: Edge Ingress & Connection Demuxing (`SOT-001`)
* **Reason Separate:** Owns network wire protocols, TLS termination, admin authentication middleware, client socket lifecycles, and in-memory L1 cache hits.
* **Aggregates:**
  * **Aggregate:** `ClientConnectionRing`
    * **Invariant:** Active socket count must not exceed 25,000 per 2 GB node; an individual client queue depth must never exceed 64 KB without being forcibly closed.
    * **Entities:** `ClientSession { socket_id: u64, protocol: TransportProto, state: SessionState }`
    * **Value Objects:** `BroadcastFrame(Arc<[u8]>)`, `SocketRateLimit(TokenBucket)`
    * **Ports In:** Incoming TCP/TLS connections from downstream clients.
    * **Ports Out:** Forward contextual evaluation requests to `Core Rule Engine`; stream broadcast updates to connected clients.
  * **Aggregate:** `L1StaticCache`
    * **Invariant:** Only stores globally static boolean/variant flags; must never hold user-specific targeting rules.
    * **Entities:** `CacheTable(TinyUfo)`
    * **Value Objects:** `CacheKey(String)`, `CachedPayload(EvaluationResult)`
    * **Ports In:** Query from Ingress; Invalidation purge events from `SOT-003`.
    * **Ports Out:** Direct edge responses (< 10 µs hit).

---

### Bounded Context 2: Core Rule Engine & Microservice Guard (`SOT-002`)
* **Reason Separate:** Owns deterministic rule evaluation logic, SIMD attribute vector matching, and xxHash64 percentage calculations. Conforms to CNCF OpenFeature format.
* **Aggregates:**
  * **Aggregate:** `RuleEvaluator`
    * **Invariant:** Evaluating the identical context against the identical ruleset must produce the exact same boolean/variant deterministically within $< 100\,\mu\text{s}$ (and $< 1\,\mu\text{s}$ in-process).
    * **Entities:** `EvaluationPlan { flag_key: String, rules: Vec<TargetingRule> }`
    * **Value Objects:** `AttributeValue`, `ComparisonOp`, `Bucket(u8)`
    * **Ports In:** `EvaluationContext` from Ingress (`SOT-001`) or Guard Check; Rule byte slices from `SOT-004`.
    * **Ports Out:** `EvaluationResult` to Ingress (`SOT-001`) or Guard Gate.
  * **Aggregate:** `GuardSdkAbstraction`
    * **Invariant:** Embedding into downstream microservices must require $\le 3$ lines of code and incur zero heap allocations and zero dynamic vtable lookups on evaluation paths.
    * **Entities:** `EdgeFlagGuard<S: FlagStore>`, `EdgeFlagLayer<S>`
    * **Value Objects:** `GuardDecision { allowed: bool, variant: Option<&'static str> }`
    * **Ports In:** Request context from Axum router or downstream business handler.
    * **Ports Out:** Inlined boolean pass/deny decisions ($50\text{–}100\text{ ns}$).

---

### Bounded Context 3: State Replication & Daemonless P2P Invalidation Mesh (`SOT-003`)
* **Reason Separate:** Owns cross-node cluster synchronization, ephemeral user overrides, and distributed state delta replication. Eliminates external database daemons (Valkey/Redis) by operating a native in-process UDP/QUIC peer-to-peer gossip bus.
* **Aggregates:**
  * **Aggregate:** `GossipMeshBus`
    * **Invariant:** Write mutations must broadcast eviction frames (`"evict:key_xyz"`) over async UDP/QUIC peer-to-peer gossip to all sibling instances; cluster coherence must not require an external daemon or TCP loopback context switching.
    * **Entities:** `GossipPeerNode { peer_addr: SocketAddr, state: PeerState }`
    * **Value Objects:** `EvictionFrame { op: u8, key: String, revision: u64 }`
    * **Ports In:** Eviction broadcast triggers from write handlers (`SOT-006`, `SOT-004`); UDP gossip packets from sibling peers.
    * **Ports Out:** Eviction purges to `L1StaticCache` / Moka L1 (`SOT-001`); Delta updates to `FlagStoreMmap` (`SOT-004`).
  * **Aggregate:** `ValkeyMeshBus` (Legacy / External Bus Port)
    * **Invariant:** Outgoing mutations must broadcast the full `ReplicationDelta` (not just an ID); remote nodes must persist the payload into their local `heed` store so all nodes stay synchronized.
    * **Entities:** `ValkeySubscriber { client: fred::clients::RedisClient, status: ConnectionState }`
    * **Value Objects:** `ReplicationDelta { flag_key: String, revision: u64, definition: Option<FlagDefinition>, source_node: String, timestamp_ms: u64 }`
    * **Ports In:** Mutation delta publish triggers from Admin Mutation (`SOT-004`); Pub/Sub event stream from Valkey cluster.
    * **Ports Out:** Cache eviction commands to `L1StaticCache` (`SOT-001`); Atomic writes to local `FlagStoreMmap` (`SOT-004`).

---

### Bounded Context 4: Durable Storage & Write-Ahead Log (`SOT-004`)
* **Reason Separate:** Owns persistence, memory-mapped B+ Tree tables (`heed`), and asynchronous LSM Write-Ahead Logging (`fjall` & `AsyncWal`).
* **Aggregates:**
  * **Aggregate:** `FlagStoreMmap`
    * **Invariant:** Read transactions must never block or wait on write locks; write transactions must execute sequentially via single-writer CoW.
    * **Entities:** `MmapEnvironment(heed::Env)`, `FlagTable(heed::Database)`
    * **Value Objects:** `FlagRecordKey(&str)`, `FlagRecordBinary(&[u8])`
    * **Ports In:** Read pointer dereferences from `SOT-002`; Mutation commands from Administrative API and Replication Mesh (`SOT-003`).
    * **Ports Out:** Zero-copy binary slices to `SOT-002`; Audit logs to `AuditWal`.
  * **Aggregate:** `AuditWal` & `AsyncWal`
    * **Invariant:** WAL write buffering must be lock-free and flushed asynchronously without stalling request workers.
    * **Entities:** `LsmKeyspace(fjall::Keyspace)`, `AsyncRing(tokio::sync::mpsc)`
    * **Value Objects:** `AsyncWriteItem`, `AuditEntry`
    * **Ports In:** Mutation events from `FlagStoreMmap` and `SocialStore`.
    * **Ports Out:** Durable append writes to disk.

---

### Bounded Context 5: System Platform & Runtime (`SOT-005`)
* **Reason Separate:** Owns system lifecycle, hardware resource budgeting (memory caps, CPU thread allocation), graceful degradation, and kernel bypass rings.
* **Aggregates:**
  * **Aggregate:** `HardwareSupervisor`
    * **Invariant:** Total process memory consumption must remain bounded within calibrated limits ($< 1,250\,\text{MB}$ at 20,000 connections), reserving $\ge 798\,\text{MB}$ free safety margin.
    * **Entities:** `ProcessMetrics { resident_ram: usize, active_fds: usize, cpu_load: f32 }`
    * **Value Objects:** `MemoryBudgetConfig`, `DegradationPolicy`
    * **Ports In:** Operating system resource metrics.
    * **Ports Out:** Trigger shedding/rate-limiting on Ingress (`SOT-001`).

---

### Bounded Context 6: Zero-Effect Stateful Data Microservice (`SOT-006`)
* **Reason Separate:** Owns high-concurrency stateful entities (user profiles, reverse-chronological timeline feeds, and post records) utilizing zero-cost trait abstractions.
* **Aggregates:**
  * **Aggregate:** `MmapSocialStore`
    * **Invariant:** Read transactions must never block on write locks; timeline feeds must execute an $O(\text{limit})$ cursor scan on mmap disk pages without in-memory `ORDER BY` sorting or SQL query parsing.
    * **Entities:** `MmapSocialStore { env: Arc<Env>, users_db, posts_timeline_db, posts_by_id_db }`
    * **Value Objects:** `UserProfile`, `PostRecord`, `EnrichedPost`
    * **Ports In:** Queries and mutations from `SocialEngine`.
    * **Ports Out:** Direct memory dereferenced slices and enriched joined records.
  * **Aggregate:** `SocialEngine`
    * **Invariant:** Composing L1 S3-FIFO caching and Singleflight request coalescing must introduce 0 wrapper allocations and $\le 20\,\mu\text{s}$ stampede overhead.
    * **Entities:** `SocialEngine<S: SocialStore>`
    * **Value Objects:** `TimelineResponse`, `CreatePostRequest`
    * **Ports In:** HTTP requests from Axum benchmark handlers (`GET /users/:id`, `GET /posts`, `POST /posts`).
    * **Ports Out:** Responses to clients; Async log items to `AsyncWal` (`SOT-004`).

---

## 3. Data Flow Total Order

$$\text{SOT-005 (Platform)} \longrightarrow \text{SOT-004 (Tier 2 heed Storage)} \longleftrightarrow \text{SOT-003 (Tier 3 P2P Mesh)} \longleftrightarrow \text{SOT-006 (Data Service)} \longleftrightarrow \text{SOT-001 (Tier 1 Moka Ingress)}$$

1. **Storage (`SOT-004`) [Tier 2]:** Initializes memory-mapped `data.mdb` B+ Tree tables and background WAL ring workers ($1\text{–}5\,\mu\text{s}$ reads).
2. **P2P Gossip Mesh (`SOT-003`) [Tier 3]:** Broadcasts `"evict:key"` across UDP/QUIC peer nodes on writes, ensuring coherence with zero external daemons.
3. **Data Microservice (`SOT-006`):** Coordinates zero-cost trait lookups, Singleflight coalescing, and reverse-chronological timeline feeds.
4. **Ingress & L1 Cache (`SOT-001`) [Tier 1]:** Serves hot reads directly in $50\text{–}150\text{ ns}$ via in-process W-TinyLFU (`Arc<[u8]>`) with zero syscalls and zero IPC.


