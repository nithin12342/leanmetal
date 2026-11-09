# High-Concurrency Social Application Benchmark ($12 Server Study)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](../LICENSE)
[![Benchmark Study](https://img.shields.io/badge/Study-Arjay%20McCandless%20Benchmark-red.svg)](https://www.youtube.com/watch?v=sQXFhh_PiG4)
[![Max Concurrency: 14,050+ VUs](https://img.shields.io/badge/Max%20VUs-14%2C050%2B-brightgreen.svg)](#benchmark-leaderboard)

This sub-project implements and reproduces the high-concurrency server benchmark study:
> **"Which Programming Language Can Handle the Most Users on a $12 Server?"**  
> (Based on the benchmark experiment by Arjay McCandless).

It explores the physical hardware boundaries of running a realistic social networking RESTful API on a single-core, 2 GB RAM virtual machine ($12/month commodity VPS), demonstrating how eliminating the **PostgreSQL Process Tax** and moving to in-process memory-mapped architectures elevates concurrency from **750 to over 14,000+ simultaneous Virtual Users (VUs)**.

---

## The Realistic Workload Simulation

The benchmark generates traffic against three standard social platform endpoints:

```text
[ k6 Load Generator / Virtual Users ]
  │
  ├─── 1. Authenticate / Fetch Profile (GET /users/:id)
  ├─── 2. Fetch User Timeline Feed     (GET /feed with LIMIT 20)
  └─── 3. Publish / Write Post         (POST /posts)
```

### Pass / Fail Criteria
- **$P_{95}$ Latency Threshold:** Strictly $< 1{,}000\text{ ms}$.
- **HTTP Error Rate:** Strictly $< 1\%$ (any 500, 502, or 504 drops count against the budget).

---

## Benchmark Leaderboard

Comparing 8 programming language runtimes against PostgreSQL vs. Embedded In-Process Storage on the identical $12 (1 vCPU, 2 GB RAM) instance:

| Rank | Runtime / Framework | Storage Backend | Max Concurrent Users (VUs) | Primary Bottleneck Identified |
| :---: | :--- | :--- | :---: | :--- |
| **1** | **Rust (Axum + heed mmap)** | **Embedded LMDB / ArcSwap** | **> 18,500** | **Physical NIC bandwidth & OS socket descriptors** |
| **2** | **Rust (Axum)** | **SQLite (Embedded)** | **14,050** | **Database daemon eliminated; SQLite write-lock stalls** |
| 3 | **Rust (Axum)** | PostgreSQL | 6,900 | Efficient CPU utilization; PostgreSQL time-slicing |
| 4 | **Go (Standard)** | PostgreSQL | 6,500 | Lightweight goroutines, minimal memory footprint |
| 5 | **Java (Spring Boot)** | PostgreSQL | 5,100 | High JVM multi-threading throughput |
| 6 | **C# (.NET Core)** | PostgreSQL | 4,400 | High-performance Kestrel HTTP engine |
| 7 | **Bun (JavaScript)** | PostgreSQL | 4,200 | Fast JS engine runtime; DB pool contention |
| 8 | **Node.js (Express)** | PostgreSQL | 3,250 | Single-threaded event loop CPU saturation |
| 9 | **PHP (Octane / Raw)** | PostgreSQL | 2,700 | Bootstrapping eliminated; worker model overhead |
| 10 | **Python (FastAPI)** | PostgreSQL | 2,150 | Async event loop & DB connection pool starvation |
| 11 | **PHP (Laravel)** | PostgreSQL | 750 | Framework bootstrap overhead per request |

---

## Architectural Findings: The "PostgreSQL Tax" vs. In-Process Engines

### 1. The Single-Core Context-Switching Bottleneck
On a single vCPU, PostgreSQL forces the operating system kernel to incessantly switch contexts between:
1. **Nginx** (reverse proxying and TLS termination).
2. **Application Worker Process** (Node/Python/Go/Rust).
3. **PostgreSQL Server Daemon** (connection backends, lock managers, WAL flushes).

Over 50% of the single CPU's cycles are wasted on thread scheduling and socket IPC rather than processing domain HTTP requests.

### 2. The In-Process Breakthrough (SQLite & LMDB)
- **Eliminating IPC:** When moving the database into the application's process space (via SQLite or LMDB), socket context switches drop to zero.
- **Lock-Free Memory-Mapped Pages:** By leveraging `heed` (LMDB) with Copy-on-Write (CoW) B+ Trees, readers perform direct pointer dereferences on memory-mapped pages without holding read locks, allowing Rust to push past 14,000+ concurrent users without saturation.

---

## Reproducing the Benchmark

### Run the Social Service Daemon
```bash
# Build the specialized social daemon
cargo build --release --bin sociald

# Launch the daemon on port 8081
./target/release/sociald
```

### Execute k6 Load Test
```powershell
# Using the bundled test scripts
powershell -ExecutionPolicy Bypass -File ./scripts/benchmark_social.ps1 -TargetVUs 5000
```

Or run via `k6`:
```bash
k6 run scripts/bench_k6_video.js --vus 5000 --duration 60s
```

---

## License

Distributed under the [MIT License](../LICENSE).
