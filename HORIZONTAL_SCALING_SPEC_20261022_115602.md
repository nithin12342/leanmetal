# ARCHITECTURE SPECIFICATION: IN-PROCESS ZERO-COPY B+TREE (LMDB) HORIZONTAL SCALING ENGINE
**Timestamp:** 2026-10-22T11:56:02+05:30  
**Methodology:** Intention Engineering (`/intention-engineering`)  
**Target Branch:** `feat/horizontal-scaling`  
**Target Deployments:** Commodity 1 vCPU / 2GB RAM Virtual Private Servers (Oracle Always Free Ampere A1 / Azure Container Instances)  

---

## 1. Executive Summary & Physical Hardware Budget

This document specifies the end-to-end architecture for a single-node, zero-copy, in-process infrastructure engine that eliminates runtime heap allocations, operating system context switching, and kernel networking overhead.

The storage layer relies on an embedded, memory-mapped (`mmap`) B+Tree engine derived from the Lightning Memory-Mapped Database (LMDB) architecture. It completely replaces cellular sheaf/fiber abstractions with deterministic B-tree page traversals, lock-free pointer swapping via read-copy-update semantics (`ArcSwap`), an asynchronous Write-Ahead Log (WAL) batching coordinator using Linux `io_uring`, and the Manifest-Driven NACK transport protocol.

### 1.1 Physical Hardware Envelope

The engine is strictly designed to operate within the following $12/month commodity virtual private server (VPS) boundaries:

* **Compute:** 1 vCPU (x86_64 or aarch64, nominal 3.0 GHz base frequency, 64-byte L1/L2 cache line size).
* **Random Access Memory (RAM):** 2,048 MiB (2.0 GiB) total physical limit.
* **Storage:** 50 GB NVMe-backed local flash block storage.
* **Network Interface Card (NIC):** 1.0 Gbps virtio-net / PCI-passthrough interface (operational throughput: $100\text{ MB/s}$ usable bidirectional bandwidth).
* **Operating Environment:** Single-process bare-metal Linux (kernel 5.15+ or 6.x) with root access for `CAP_NET_RAW` / `CAP_SYS_ADMIN` (AF_XDP UMEM registration).

```
+-------------------------------------------------------------------------------+
|                       TOTAL PHYSICAL RAM: 2,048 MiB                           |
+-----------------------------------+-------------------------------------------+
| Static Reserved Memory: 455.0 MiB | Dynamic Connection Arena: 1,593.0 MiB     |
|  - Host/Kernel Base:     35.0 MiB |  - Active Slabs:       1,593.0 MiB        |
|  - Ingress UMEM Pool:   128.0 MiB |  - Per-Conn Footprint: 256 Bytes          |
|  - LMDB Mmap Virtual:   180.0 MiB |  - Max Concurrent:     6,524,928 Sessions |
|  - LMAX Ring Slabs:       8.0 MiB |                                           |
|  - OS Kernel Margin:     40.0 MiB |                                           |
|  - Static Dictionary:    64.0 KiB |                                           |
+-----------------------------------+-------------------------------------------+
```

---

## 2. Global Memory Map & Static Struct Layouts

All data structures in this architecture are designed under a **Strict Descending Alignment (Zero-Padding) Invariant**. Fields within composite types must be ordered monotonically from the largest scalar alignment requirement down to the smallest bit-level representations:

$$\text{Field Sorting Order: } \mathbf{64\text{-bit (8B)}} \longrightarrow \mathbf{32\text{-bit (4B)}} \longrightarrow \mathbf{16\text{-bit (2B)}} \longrightarrow \mathbf{8\text{-bit (1B)}} \longrightarrow \mathbf{\text{Sub-byte Bitfields}}$$

Compilers must never be permitted to inject uninitialized padding bytes. Compile-time assertions verifying `size_of::<T>() == N` are mandatory for all core structures.

```
+---------------------------------------------------------------------------------------+
|                STRICT DESCENDING ALIGNMENT MEMORY PACKING                             |
+---------------------+-------------------+-------------------+------------+------------+
| Offset 0..23 (8B)   | Offset 24..27(4B) | Offset 28..31(2B) | Off 32(1B) | Off 33(1B) |
| 3x u64 Fields       | 1x u32 / i32      | 2x u16 Fields     | Opcode u8  | Flags u8   |
+---------------------+-------------------+-------------------+------------+------------+
| Total Struct Size: Exact Multiple of Natural Word Boundary (Zero Padding)             |
+---------------------------------------------------------------------------------------+
```

### 2.1 The 64-Byte Ring Buffer Entry (`RingEntry`)

Every slot within the inter-stage LMAX Disruptor ring buffers maps to an exact 64-byte payload. This matches the CPU hardware cache line size, preventing false sharing across physical cores without requiring dummy array padding.

* **Offset 0..7 (`u64`):** `sequence_number` — Monotonically increasing sequence cursor within the ring.
* **Offset 8..15 (`u64`):** `session_id` — Global 64-bit client connection identifier.
* **Offset 16..23 (`u64`):** `timestamp_ns` — Hardware monotonic timestamp obtained via `CLOCK_MONOTONIC_RAW` or `rdtsc`.
* **Offset 24..27 (`u32`):** `crc32_checksum` — CRC32-C (Castagnoli) checksum computed over the payload using SSE4.2 hardware instructions.
* **Offset 28..29 (`u16`):** `batch_id` — Manifest batch sequence identifier (0..65535).
* **Offset 30..31 (`u16`):** `payload_length` — Valid byte length of the trailing payload buffer (0..30).
* **Offset 32 (`u8`):** `opcode` — Operational directive (`0x01: DIRECT_CALL`, `0x02: DIRECT_REPLY`, `0x03: DUPLEX_STREAM`, `0x04: EVENT_NOTIFY`).
* **Offset 33 (`u8`):** `flags` — Bit-packed control bits:
  * Bit 0: `IS_NACK` (1 if frame represents a negative acknowledgement).
  * Bit 1: `END_OF_BATCH` (1 if frame marks the termination of a manifest batch).
  * Bit 2: `IS_COMPRESSED` (1 if payload is compressed with the pre-shared dictionary).
  * Bits 3..7: Reserved for future routing extensions.
* **Offset 34..63 (`[u8; 30]`):** `payload_inline` — Inline data slice. Payloads exceeding 30 bytes are indexed as a 64-bit UMEM offset placed in offsets 34..41.

$$\text{Total Size} = 8 + 8 + 8 + 4 + 2 + 2 + 1 + 1 + 30 = \mathbf{64\text{ Bytes}}$$

### 2.2 The 40-Byte Contiguous Connection Slot (`ConnectionSlot`)

Client states do not exist as heap-allocated socket objects. They reside in a single pre-allocated contiguous flat memory array.

* **Offset 0..7 (`u64`):** `session_id` — Immutable unique connection key.
* **Offset 8..15 (`u64`):** `last_active_ns` — Nanosecond timestamp of latest ingress activity (used for connection eviction).
* **Offset 16..23 (`u64`):** `rx_sequence_watermark` — Highest contiguous packet sequence received and verified.
* **Offset 24..27 (`u32`):** `peer_ip_v4` — Raw network-order IPv4 address.
* **Offset 28..31 (`i32`):** `socket_descriptor` — Kernel file descriptor or AF_XDP socket index.
* **Offset 32..33 (`u16`):** `peer_port` — Network-order TCP/UDP destination port.
* **Offset 34..35 (`u16`):** `window_size` — Current receiver flow-control credit balance.
* **Offset 36 (`u8`):** `state_flags` — Bit-packed state:
  * Bit 0: `IS_ACTIVE` (Slot currently holds an open session).
  * Bit 1: `IS_TLS` (Session negotiated via kernel-TLS or raw datagrams).
  * Bit 2: `IS_BLOCKED` (Session throttled due to egress backpressure).
  * Bits 3..7: Protocol phase state machine.
* **Offset 37 (`u8`):** `compression_dictionary_id` — Identifier of the active pre-shared compression dictionary.
* **Offset 38 (`u8`):** `stream_channel_id` — Logical multiplexing channel ID.
* **Offset 39 (`u8`):** `qos_priority` — Egress traffic shaping classification.

$$\text{Total Size} = 8 + 8 + 8 + 4 + 4 + 2 + 2 + 1 + 1 + 1 + 1 = \mathbf{40\text{ Bytes}}$$

### 2.3 The 24-Byte Unified Wire Frame Header (`UnifiedFrameHeader`)

This header encapsulates all ingress and egress network packets across raw UDP datagrams or virtio-net frames.

* **Offset 0..7 (`u64`):** `batch_id` — Manifest transaction identifier.
* **Offset 8..15 (`u64`):** `correlation_id` — Unique matching token tying an asynchronous request to its deterministic reply.
* **Offset 16..19 (`u32`):** `session_id_compact` — Lower 32 bits of the active session token for fast-path routing.
* **Offset 20..21 (`u16`):** `topic_or_route` — Resource enumeration representing the target API endpoint or pub/sub channel.
* **Offset 22 (`u8`):** `opcode` — Frame operational command identifier.
* **Offset 23 (`u8`):** `frame_flags` — Transport-layer flags (retransmission flags, manifest indicators).

$$\text{Total Size} = 8 + 8 + 4 + 2 + 1 + 1 = \mathbf{24\text{ Bytes}}$$

---

## 3. Pipelined Horizontal Execution (The HFT Assembly Line)

The execution topology completely rejects the traditional multi-threaded model where a single worker thread executes the full lifecycle of an HTTP request. Instead, execution is decomposed into **four strictly sequential, horizontally sliced execution stages** linked by single-producer, single-consumer lock-free ring buffers.

Each stage is bound to a dedicated execution context, utilizing minimal working sets that fit entirely inside L1/L2 CPU caches.

```
+---------------------------------------------------------------------------------------+
|                     PIPELINED HORIZONTAL EXECUTION STAGES                             |
+---------------------------------------------------------------------------------------+
|  [ Physical NIC ]                                                                     |
|         │                                                                             |
|         ▼ (DMA via AF_XDP / VirtIO Rings)                                             |
|  +---------------------------------------------------------------------------------+  |
|  | Stage 0: Kernel Bypass Ingress Engine                                            |  |
|  |   - Direct memory access to 128 MiB UMEM pool (65,536 chunks @ 2,048B)          |  |
|  |   - Validates 24B UnifiedFrameHeader                                            |  |
|  |   - SIMD AVX2/AVX-512 Bitset Manifest check (NACK-only emission)                |  |
|  +---------------------------------------------------------------------------------+  |
|         │                                                                             |
|         ▼ (LMAX Ring Buffer 0: 65,536 Slots @ 64 Bytes = 4 MiB)                       |
|  +---------------------------------------------------------------------------------+  |
|  | Stage 1: Single-Writer In-Process Domain Engine                                  |  |
|  |   - Read Path: Lock-Free ArcSwap Timeline Read (<150 ns)                        |  |
|  |   - Read Path: In-Process LMDB Zero-Copy Mmap B+Tree Traversals (1.25 µs)        |  |
|  |   - Write Path: Non-blocking enqueue to Lock-Free WAL Channel                   |  |
|  |   - Mutates ArcSwap timeline snapshot using atomic pointer swap                 |  |
|  +---------------------------------------------------------------------------------+  |
|         │                                                                             |
|         ├───────────────────────────────────────┐                                     |
|         │ (LMAX Ring Buffer 1: In-Flight WAL)   │ (LMAX Ring Buffer 2: Outbound Wire) |
|         ▼                                       ▼                                     |
|  +-----------------------------------+   +-----------------------------------------+  |
|  | Stage 2: Asynchronous WAL Batcher |   | Stage 3: Zero-Copy Egress Pacer         |  |
|  |   - Linux io_uring (SQE / CQE)    |   |   - Dictionary Vector Compression       |  |
|  |   - Multi-write grouping (up 256) |   |   - Encodes response into 171B frames   |  |
|  |   - Direct O_DIRECT append        |   |   - DMA TX directly to NIC ring         |  |
|  +-----------------------------------+   +-----------------------------------------+  |
+---------------------------------------------------------------------------------------+
```

---

## 4. Master Implementation & Verification Plan

### Phase 1: Zero-Padding Structural Hardening
* Verify and enforce `#[repr(C)]` on all memory types: `RingEntry` (64B), `ConnectionSlot` (40B), and `UnifiedFrameHeader` (24B).
* Add compile-time `const_assert_eq!(size_of::<T>(), N)` macros in `packages/engine-layout`.

### Phase 2: In-Process LMDB Mmap B+Tree Engine (`packages/engine-eda` & `packages/engine-log`)
* Implement zero-copy B+Tree leaf reader with composite inverted key: `(u64::MAX - timestamp_ms, post_id)`.
* Integrate `ArcSwap` contiguous array snapshot for instantaneous $< 150\text{ ns}$ feed generation.

### Phase 3: Manifest-Driven UDP/Ethernet NACK Ingress (`packages/manifest-udp-transport`)
* Complete line-rate burst receiver using 64-bit ledger word and bit-test-and-set (`bts`).
* Implement selective retransmit buffer for dropped packet indices.

### Phase 4: Linux io_uring Multi-Write Group Committer (`packages/engine-runtime`)
* Configure asynchronous batch coordinator draining up to 256 writes per event loop tick.
* Enforce `O_DIRECT` raw block alignment without page-cache memory duplication.

### Phase 5: End-to-End Benchmark & Container Hardening
* Run automated 50,000+ request high-concurrency onslaught against the new horizontally scaled pipeline.
* Build multi-stage Alpine/Debian scratch container images ready for Oracle Always Free / Azure deployment.
