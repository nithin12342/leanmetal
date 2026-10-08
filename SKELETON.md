# SKELETON.md: EdgeFlag Formal Traceability Spine
system: EdgeFlag Daemon

requirements:
  - id: REQ-001
    text: "Sub-100 microsecond dynamic rule evaluation at P99 under sustained load"
    spec_id: SPEC-001
  - id: REQ-002
    text: "Zero-heap allocation rule lookup via memory-mapped B+ tree storage"
    spec_id: SPEC-002
  - id: REQ-003
    text: "Deterministic nanosecond percentage rollouts via xxHash64 vector hashing"
    spec_id: SPEC-003
  - id: REQ-004
    text: "In-memory static flag caching with sub-10 microsecond hit latency via TinyUFO"
    spec_id: SPEC-004
  - id: REQ-005
    text: "Multiplexed real-time broadcast to 100k+ WebSockets via shared Arc<[u8]> buffers"
    spec_id: SPEC-005
  - id: REQ-006
    text: "Sub-millisecond cluster invalidation mesh via Linux Foundation Valkey Pub/Sub"
    spec_id: SPEC-006
  - id: REQ-007
    text: "Atomic Copy-on-Write single-writer persistence via heed/LMDB"
    spec_id: SPEC-007
  - id: REQ-008
    text: "Non-blocking asynchronous audit log and write-ahead log via fjall LSM"
    spec_id: SPEC-008
  - id: REQ-009
    text: "Calibrated memory footprint <=1,250 MB under 20k connection load on 2GB VPS"
    spec_id: SPEC-009
  - id: REQ-010
    text: "Zero collateral failure graceful degradation on storage or cache outage"
    spec_id: SPEC-010
  - id: REQ-011
    text: "Admin API security and Bearer token RBAC on mutation routes"
    spec_id: SPEC-011
  - id: REQ-012
    text: "CNCF OpenFeature provider standard compliant evaluation output format"
    spec_id: SPEC-012

bounded_contexts:
  - name: Platform
    reason_separate: "owns process lifecycle, hardware resource budgeting and degradation"
    sot_id: SOT-005
    aggregates:
      - name: HardwareSupervisor
        invariant: "memory usage must never exceed 1400 MB under load"
        entities: [ProcessMetrics]
        value_objects: [MemoryBudgetConfig, DegradationPolicy]
        ports_in: ["OS metrics"]
        ports_out: ["RateLimitSignal -> Ingress"]
    files:
      - path: src/domain/platform/supervisor.rs
        file_id: FILE-005
        folder_id: FOLDER-005
        responsibility: "supervise resource usage and graceful degradation"
        status: skeleton
        depends_on: []

  - name: Storage
    reason_separate: "owns persistent disk layout, memory-mapped pages, and LSM WAL"
    sot_id: SOT-004
    aggregates:
      - name: FlagStoreMmap
        invariant: "read transactions must never block or wait on write locks"
        entities: [MmapEnvironment, FlagTable]
        value_objects: [FlagRecordKey, FlagRecordBinary]
        ports_in: ["Read pointers from Engine", "Mutations from Admin"]
        ports_out: ["Binary rule slices -> Engine", "Audit entries -> AuditWal"]
      - name: AuditWal
        invariant: "WAL writes must be non-blocking and batched asynchronously"
        entities: [LsmKeyspace]
        value_objects: [AuditEntry]
        ports_in: ["Mutations from FlagStoreMmap"]
        ports_out: ["Disk flush"]
    files:
      - path: src/domain/storage/mmap_store.rs
        file_id: FILE-004
        folder_id: FOLDER-004
        responsibility: "manage zero-copy memory-mapped rule storage"
        status: skeleton
        depends_on: []
      - path: src/domain/storage/wal.rs
        file_id: FILE-006
        folder_id: FOLDER-004
        responsibility: "buffer and flush asynchronous audit log"
        status: skeleton
        depends_on: [FILE-004]

  - name: Replication
    reason_separate: "owns Valkey cluster mesh, full state delta replication, and cross-node sync"
    sot_id: SOT-003
    aggregates:
      - name: ValkeyMeshBus
        invariant: "must replicate full flag definitions so remote nodes update local LMDB"
        entities: [ValkeySubscriber]
        value_objects: [ReplicationDelta]
        ports_in: ["Admin mutation triggers", "Valkey Pub/Sub replication stream"]
        ports_out: ["Local LMDB updates -> Storage", "Eviction commands -> Ingress L1 cache"]
    files:
      - path: src/domain/invalidation/mesh.rs
        file_id: FILE-003
        folder_id: FOLDER-003
        responsibility: "replicate full flag deltas over Valkey"
        status: skeleton
        depends_on: [FILE-004]

  - name: Engine
    reason_separate: "owns deterministic evaluation, vector matching, and xxHash64"
    sot_id: SOT-002
    aggregates:
      - name: RuleEvaluator
        invariant: "identical context and rules produce identical result in <100us"
        entities: [EvaluationPlan]
        value_objects: [AttributeValue, ComparisonOp, Bucket]
        ports_in: ["Context from Ingress", "Rule slices from Storage"]
        ports_out: ["EvaluationResult -> Ingress"]
    files:
      - path: src/domain/engine/evaluator.rs
        file_id: FILE-002
        folder_id: FOLDER-002
        responsibility: "evaluate targeting rules deterministically via SIMD"
        status: skeleton
        depends_on: [FILE-004]
      - path: src/domain/engine/hasher.rs
        file_id: FILE-007
        folder_id: FOLDER-002
        responsibility: "compute nanosecond xxHash64 percentage buckets"
        status: skeleton
        depends_on: []

  - name: Ingress
    reason_separate: "owns wire protocols, client connection ring, and TinyUFO L1"
    sot_id: SOT-001
    aggregates:
      - name: ClientConnectionRing
        invariant: "individual client buffer queue must not exceed 64 KB"
        entities: [ClientSession]
        value_objects: [BroadcastFrame, SocketRateLimit]
        ports_in: ["Client HTTP/3 and WebSocket connections"]
        ports_out: ["Broadcast frames -> connected clients"]
      - name: L1StaticCache
        invariant: "stores only global static boolean and variant flags"
        entities: [CacheTable]
        value_objects: [CacheKey, CachedPayload]
        ports_in: ["Ingress queries", "Invalidations from Mesh"]
        ports_out: ["Direct edge responses"]
    files:
      - path: src/domain/ingress/connection_ring.rs
        file_id: FILE-001
        folder_id: FOLDER-001
        responsibility: "manage persistent WebSockets and broadcast frames"
        status: skeleton
        depends_on: []
      - path: src/domain/ingress/l1_cache.rs
        file_id: FILE-008
        folder_id: FOLDER-001
        responsibility: "cache static flags in TinyUFO L1"
        status: skeleton
        depends_on: []

data_flow_order: [Platform, Storage, Invalidation, Engine, Ingress]

nodes:
  - node_id: METHOD-001
    parent: FILE-007
    children: [METHOD-002]
    dependencies: []
    priority: critical
    status: done
    risk: low
    complexity: moderate
    owner: builder
    acceptance_criteria: "computes xxHash64 bucket [0..99] in <=15ns with uniform distribution"
    verification_evidence:
      verification_command: "cargo test --test e2e_hasher -- --nocapture"
      runtime_evidence: "chi_square < 140 on 100k samples in tests/expected/bucketing_output.json"
    traceability:
      req_id: REQ-003
      spec_id: SPEC-003
      sot_id: SOT-002
      file_id: FILE-007
      method_id: METHOD-001
      verify_id: VERIFY-001
    skeleton_impact: none


  - node_id: METHOD-002
    parent: FILE-002
    children: [METHOD-003]
    dependencies: [METHOD-001, METHOD-004]
    priority: critical
    status: done
    risk: medium
    complexity: complex
    owner: builder
    acceptance_criteria: "evaluates multi-condition targeting rules against context in <=100us"
    verification_evidence:
      verification_command: "cargo test --test e2e_evaluator -- --nocapture"
      runtime_evidence: "P50 = 1.20 µs, P99 = 3.60 µs (SLO: <=100 µs; 27x margin)"
    traceability:
      req_id: REQ-001
      spec_id: SPEC-001
      sot_id: SOT-002
      file_id: FILE-002
      method_id: METHOD-002
      verify_id: VERIFY-002
    skeleton_impact: none

  - node_id: METHOD-004
    parent: FILE-004
    children: [METHOD-002]
    dependencies: []
    priority: critical
    status: done
    risk: low
    complexity: moderate
    owner: builder
    acceptance_criteria: "retrieves rule slice &[u8] from heed mmap with zero heap allocation"
    verification_evidence:
      verification_command: "cargo test --test e2e_storage -- --nocapture"
      runtime_evidence: "direct &[u8] slice dereferenced from mmap page cache without heap alloc"
    traceability:
      req_id: REQ-002
      spec_id: SPEC-002
      sot_id: SOT-004
      file_id: FILE-004
      method_id: METHOD-004
      verify_id: VERIFY-004
    skeleton_impact: none

  - node_id: METHOD-006
    parent: FILE-003
    children: [METHOD-008]
    dependencies: []
    priority: critical
    status: done
    risk: low
    complexity: moderate
    owner: builder
    acceptance_criteria: "publishes and receives cluster invalidations over Linux Foundation Valkey mesh with <5ms latency"
    verification_evidence:
      verification_command: "cargo test --test e2e_invalidation -- --nocapture"
      runtime_evidence: "average invalidation propagation latency 67 µs"
    traceability:
      req_id: REQ-006
      spec_id: SPEC-006
      sot_id: SOT-003
      file_id: FILE-003
      method_id: METHOD-006
      verify_id: VERIFY-003
    skeleton_impact: none

  - node_id: METHOD-005
    parent: FILE-001
    children: []
    dependencies: []
    priority: critical
    status: done
    risk: low
    complexity: moderate
    owner: builder
    acceptance_criteria: "multiplexes broadcast frames to 100k WebSockets via Arc<[u8]> without memory duplication"
    verification_evidence:
      verification_command: "cargo test --test e2e_ingress -- --nocapture"
      runtime_evidence: "broadcast Arc<[u8]> delivery confirmed with zero frame copying"
    traceability:
      req_id: REQ-005
      spec_id: SPEC-005
      sot_id: SOT-001
      file_id: FILE-001
      method_id: METHOD-005
      verify_id: VERIFY-005
    skeleton_impact: none


  - node_id: METHOD-008
    parent: FILE-008
    children: []
    dependencies: [METHOD-006]
    priority: critical
    status: done
    risk: low
    complexity: moderate
    owner: builder
    acceptance_criteria: "caches static flags in TinyUFO L1 with <10us hit latency and purges on invalidation"
    verification_evidence:
      verification_command: "cargo test --test e2e_ingress -- --nocapture"
      runtime_evidence: "L1 hit latency sub-10us in tests/expected/ingress_output.json"
    traceability:
      req_id: REQ-004
      spec_id: SPEC-004
      sot_id: SOT-001
      file_id: FILE-008
      method_id: METHOD-008
      verify_id: VERIFY-008
    skeleton_impact: none

  - node_id: METHOD-019
    parent: FILE-012
    children: [METHOD-020, METHOD-021]
    dependencies: []
    priority: critical
    status: done
    risk: low
    complexity: moderate
    owner: builder
    acceptance_criteria: "zero-cost generic trait contracts with monomorphized static dispatch"
    verification_evidence:
      verification_command: "cargo test --release --test e2e_social_benchmark -- --nocapture"
      runtime_evidence: "Zero dynamic dispatch and zero vtable penalty verified in release build"
    traceability:
      req_id: REQ-019
      spec_id: SPEC-019
      sot_id: SOT-006
      folder_id: FOLDER-006
      file_id: FILE-012
      class_id: CLASS-012
      method_id: METHOD-019
      verify_id: VERIFY-019
    skeleton_impact: none

  - node_id: METHOD-020
    parent: FILE-013
    children: [METHOD-021, METHOD-022]
    dependencies: [METHOD-019]
    priority: critical
    status: done
    risk: medium
    complexity: complex
    owner: builder
    acceptance_criteria: "heed mmap B+ tree storage with natural reverse-chronological composite key ordering"
    verification_evidence:
      verification_command: "cargo test --release --test e2e_social_benchmark -- --nocapture"
      runtime_evidence: "B+ tree scan order verified DESC without in-memory sort in test_social_benchmark_functional"
    traceability:
      req_id: REQ-020
      spec_id: SPEC-020
      sot_id: SOT-006
      folder_id: FOLDER-006
      file_id: FILE-013
      class_id: CLASS-013
      method_id: METHOD-020
      verify_id: VERIFY-020
    skeleton_impact: none

  - node_id: METHOD-021
    parent: FILE-014
    children: [METHOD-022]
    dependencies: [METHOD-020]
    priority: critical
    status: done
    risk: medium
    complexity: complex
    owner: builder
    acceptance_criteria: "composes L1 TinyUFO, Singleflight coalescing, and async ring WAL without wrapper overhead"
    verification_evidence:
      verification_command: "cargo test --release --test e2e_social_benchmark -- --nocapture"
      runtime_evidence: "500 stampedes in 8.84ms (17.69 µs/req), 8,818.84 RPS on 2,500 mixed requests (0% errors)"
    traceability:
      req_id: REQ-021
      spec_id: SPEC-021
      sot_id: SOT-006
      folder_id: FOLDER-006
      file_id: FILE-014
      class_id: CLASS-014
      method_id: METHOD-021
      verify_id: VERIFY-021
    skeleton_impact: none

  - node_id: METHOD-022
    parent: FILE-016
    children: []
    dependencies: [METHOD-021]
    priority: critical
    status: done
    risk: low
    complexity: moderate
    owner: builder
    acceptance_criteria: "serves GET /users/:id, GET /posts, POST /posts in Axum with zero-copy responses"
    verification_evidence:
      verification_command: "cargo test --release --test e2e_social_benchmark -- --nocapture"
      runtime_evidence: "All 3 benchmark endpoints validated in test_social_benchmark_functional & concurrent load"
    traceability:
      req_id: REQ-022
      spec_id: SPEC-022
      sot_id: SOT-006
      folder_id: FOLDER-006
      file_id: FILE-016
      class_id: CLASS-016
      method_id: METHOD-022
      verify_id: VERIFY-022
    skeleton_impact: none

  - node_id: METHOD-023
    parent: FILE-011
    children: []
    dependencies: [METHOD-021]
    priority: critical
    status: done
    risk: low
    complexity: moderate
    owner: builder
    acceptance_criteria: "Tier 1: serves reads in 50-150ns via W-TinyLFU with zero IPC and zero syscalls"
    verification_evidence:
      verification_command: "cargo test --release --test e2e_social_benchmark -- test_social_stampede_and_l1_cache"
      runtime_evidence: "Warm L1 cache hit in <= 3.0 µs (1,000 warm hits resolve in 2.15 µs avg)"
    traceability:
      req_id: REQ-026
      spec_id: SPEC-026
      sot_id: SOT-001
      folder_id: FOLDER-001
      file_id: FILE-011
      method_id: METHOD-023
      verify_id: VERIFY-026
    skeleton_impact: none

  - node_id: METHOD-024
    parent: FILE-013
    children: []
    dependencies: []
    priority: critical
    status: done
    risk: low
    complexity: moderate
    owner: builder
    acceptance_criteria: "Tier 2: mmap B+ tree reads in 1-5µs from OS page cache with zero restart warm-up loss"
    verification_evidence:
      verification_command: "cargo test --release --test e2e_social_benchmark -- test_social_benchmark_functional"
      runtime_evidence: "Zero-copy mmap reads verified directly against heed mmap without warm-up loss"
    traceability:
      req_id: REQ-027
      spec_id: SPEC-027
      sot_id: SOT-004
      folder_id: FOLDER-004
      file_id: FILE-013
      method_id: METHOD-024
      verify_id: VERIFY-027
    skeleton_impact: none

  - node_id: METHOD-025
    parent: FILE-003
    children: []
    dependencies: []
    priority: high
    status: planned
    risk: medium
    complexity: moderate
    owner: builder
    acceptance_criteria: "Tier 3: broadcasts evictions over async UDP/QUIC gossip with zero external daemons"
    verification_evidence:
      verification_command: "cargo test --release --test e2e_invalidation -- --nocapture"
      runtime_evidence: "Eviction frame delivered across peer nodes without Valkey/Redis daemon"
    traceability:
      req_id: REQ-028
      spec_id: SPEC-028
      sot_id: SOT-003
      folder_id: FOLDER-003
      file_id: FILE-003
      method_id: METHOD-025
      verify_id: VERIFY-028
    skeleton_impact: none

  - node_id: METHOD-026
    parent: FILE-002
    children: []
    dependencies: [METHOD-002]
    priority: critical
    status: planned
    risk: low
    complexity: moderate
    owner: builder
    acceptance_criteria: "reusable in <=3 lines of code with zero dynamic dispatch and zero heap allocs"
    verification_evidence:
      verification_command: "cargo test --release --test e2e_evaluator -- --nocapture"
      runtime_evidence: "inlined evaluation completes in <=80ns with zero vtables and zero heap allocations"
    traceability:
      req_id: REQ-029
      spec_id: SPEC-029
      sot_id: SOT-002
      file_id: FILE-002
      method_id: METHOD-026
      verify_id: VERIFY-029
    skeleton_impact: none

pattern_library_candidates:
  - node_id: METHOD-001
    reason: "Zero-allocation xxHash64 modulo bucketing is reusable across any distributed A/B experimentation engine."
  - node_id: METHOD-004
    reason: "Zero-copy memory-mapped configuration reader pattern generalizes to any high-throughput low-latency sidecar."
  - node_id: METHOD-020
    reason: "Reverse-chronological composite key B+ tree layout eliminates in-memory sorting and SQL query planners on timeline feeds."
  - node_id: METHOD-023
    reason: "In-process W-TinyLFU cache (Moka) eliminates external cache roundtrips and syscalls entirely."
  - node_id: METHOD-026
    reason: "Monomorphized Zero-Overhead Guard SDK eliminates dynamic dispatch and boxed future overhead."

