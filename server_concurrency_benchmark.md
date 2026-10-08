# Server Concurrency Benchmark: Which Language Handles the Most Users on a $12 Server?

> **Reference:** Based on the benchmark experiment by Arjay McCandless:  
> [Which Programming Language Can Handle the Most Users on a $12 Server?](https://www.youtube.com/watch?v=sQXFhh_PiG4)

---

## 1. Test Setup & Constraints

The experiment investigated the maximum concurrency sustainable on severely constrained, budget cloud infrastructure.

* **Hardware:** Single **\$12/month DigitalOcean Droplet** (1 vCPU, 2 GB RAM).
* **Network & Ingress:** Incoming traffic reverse-proxied via **Nginx** into the local application server process.
* **Baseline Database:** **PostgreSQL** running locally on the same instance.
* **Pass / Fail Criteria:**
  * Sustained concurrent Virtual Users (VUs) ramped incrementally.
  * **$P_{95}$ Latency:** $< 1{,}000\text{ ms}$.
  * **Error Rate:** $< 1\%$ (any HTTP 500, 502, or 504 timeouts count against the budget).

---

## 2. Application Domain & Operations

The benchmark implemented a standardized RESTful API simulating a realistic social networking workload. The load generator executed user journeys across three core endpoints:

```text
[ Load Generator / Virtual Users ]
  │
  ├─── 1. Authenticate / Fetch Profile (GET /users/:id)
  ├─── 2. Fetch User Timeline Feed     (GET /feed or /posts)
  └─── 3. Create / Like a Post         (POST /posts)
```

### Endpoint Breakdown

1. **Authentication / Profile Read (`GET /users/:id`)**
   * **Behavior:** Keyed lookup querying user credentials, username, and account metadata.
   * **Stresses:** Single-row relational indexed lookups and connection pool acquisition speed.

2. **Timeline / Feed Read (`GET /posts` with pagination)**
   * **Behavior:** Fetches recent posts joined with author metadata (`ORDER BY created_at DESC LIMIT 20`).
   * **Stresses:** Data serialization, memory allocation, and query execution planning.

3. **Write / Mutation (`POST /posts`)**
   * **Behavior:** Inserts a new record and updates counter tallies.
   * **Stresses:** Database write locks, transaction commit latency, and Write-Ahead Log (WAL) throughput.

---

## 3. Benchmark Results & Leaderboard

Across 8 runtime implementations of the exact same specification on the \$12 instance:

| Stack / Runtime | Database | Max Concurrent Users | Limiting Factor |
| :--- | :--- | :---: | :--- |
| **Laravel (PHP)** | PostgreSQL | ~750 | Framework boot overhead per request |
| **Python (FastAPI)** | PostgreSQL | ~2,150 | Async event-loop + DB pool starvation |
| **PHP (Raw / Octane)** | PostgreSQL | ~2,700 | Bootstrapping eliminated; worker model overhead |
| **Node.js (Express)** | PostgreSQL | ~3,250 | Single-threaded event loop queuing |
| **Bun** | PostgreSQL | ~4,200 | High-performance JavaScript engine runtime |
| **C# (.NET Core)** | PostgreSQL | ~4,400 | Optimized Kestrel HTTP server |
| **Java (Spring Boot)** | PostgreSQL | ~5,100 | High JVM multi-threading throughput |
| **Go** | PostgreSQL | ~6,500 | Lightweight goroutines, minimal memory footprint |
| **Rust (Axum)** | PostgreSQL | ~6,900 | Zero-cost abstractions, efficient CPU utilization |
| **Rust (Axum)** | SQLite (Embedded) | **14,050** | External database daemon & network IPC eliminated |

---

## 4. Key Takeaways & Architectural Evolution

### A. The "PostgreSQL Tax" on Single-Core Hardware
On a single vCPU, PostgreSQL introduces severe overhead. The solitary core must constantly time-slice between:
1. **Nginx** (reverse proxying and TLS termination).
2. **Application Runtime** (Python, Node, Go, Rust, etc.).
3. **PostgreSQL Server Process** (connection backends, query planner, lock manager, TCP/socket IPC).

Because PostgreSQL runs as separate processes communicating over TCP loopback or Unix domain sockets, aggressive context switching consumes most of the available CPU. For example, Python (FastAPI) bottlenecked at ~2,150 users not strictly because of Python execution overhead, but because its connection pool timed out waiting on PostgreSQL scheduling.

### B. The "SQLite Effect" (14,050 Users)
Switching the database backend from PostgreSQL to SQLite in Rust increased throughput from **6,900 to 14,050 concurrent users** on the exact same \$12 hardware:
* **Zero IPC Overhead:** Eliminates the external daemon, socket communication, and connection pool starvation.
* **In-Memory Access:** The database lives directly within the application's memory space.

### C. Pushing Further: Replacing SQLite with LMDB (`heed`) + `ArcSwap`
While SQLite delivered over 14,000 users, it still hit physical boundaries under heavy load:
* **Single-File Lock Contention:** SQLite uses file-level locking during writes, causing concurrent writes to block or stall readers.
* **C-FFI & SQL Overhead:** Bridging across the C Foreign Function Interface (FFI) and runtime SQL query string parsing consume valuable cycles.

By transitioning to **heed (LMDB)** + **ArcSwap**:
* **Zero SQL Parsing:** Reads become direct pointer dereferences into memory-mapped OS pages (mmap).
* **Copy-On-Write (COW) B+ Tree:** Readers never wait on writer locks, achieving completely lock-free read operations.
* **Outcome:** Extracts maximum single-core utilization by eliminating both network IPC and concurrency locking overhead.
