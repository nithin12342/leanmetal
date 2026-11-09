# LeanMetal: Zero-Overhead Concurrency Mechanisms & Multi-Language SDKs

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/badge/crates.io-ecosystem-orange.svg)](https://crates.io)
[![Packages](https://img.shields.io/badge/packages-5%20standalone%20crates-brightgreen.svg)](packages/)

**LeanMetal** is a collection of high-throughput, bare-metal concurrency mechanisms and zero-overhead multi-language SDK abstractions engineered for low-latency systems.

Each mechanism in `packages/` is a dedicated, zero-dependency or minimal-dependency component that solves a hard concurrency or I/O bottleneck in distributed systems and high-scale APIs.

---

## Standalone Mechanism Packages (`packages/`)

These 5 components provide targeted solutions for hot-path concurrency:

| Package | Path | Architecture & Core Invariant | Target Registry |
| :--- | :--- | :--- | :--- |
| **`singleflight-barrier`** | [`packages/singleflight-barrier/`](packages/singleflight-barrier) | Coalesces thousands of concurrent requests for cold keys into a single in-flight future using `DashMap` and broadcast channels. | **crates.io** |
| **`zerocopy-broadcast-hub`** | [`packages/zerocopy-broadcast-hub/`](packages/zerocopy-broadcast-hub) | Serializes frames once into `Arc<[u8]>` pointers and fans out concurrently to 50,000+ connections with zero per-connection heap allocations. | **crates.io** |
| **`backpressure-pruner`** | [`packages/backpressure-pruner/`](packages/backpressure-pruner) | Active eviction loop utilizing non-blocking `try_send` checks to disconnect slow consumers before server OOM crashes occur. | **crates.io** |
| **`async-wal-committer`** | [`packages/async-wal-committer/`](packages/async-wal-committer) | Lock-free circular ring buffer background committer batching up to 256 pending disk writes for sub-100µs write acknowledgments. | **crates.io** |
| **`lockfree-timeline-swap`** | [`packages/lockfree-timeline-swap/`](packages/lockfree-timeline-swap) | Atomic pointer replacement via `ArcSwap::store()`. Implements copy-on-write feeds with zero read-lock contention. | **crates.io** |

---

## Multi-Language Zero-Overhead SDKs

Zero-overhead interfaces delivering in-process feature guards and low-latency evaluation across multiple languages:

| Language | Directory | Ecosystem & Registry | Key Constructs |
| :--- | :--- | :--- | :--- |
| **Rust** | [`packages/rust/`](packages/rust) | `crates.io` | `FlagGuard`, `MemoryFlagGuard`, C-ABI FFI, AF_XDP Kernel Bypass |
| **Python** | [`packages/python/`](packages/python) | PyPI (`pip install`) | `FlagGuard`, `MemoryFlagGuard`, `Singleflight`, AF_XDP engine |
| **TypeScript** | [`packages/typescript/`](packages/typescript) | npm (`npm install`) | `FlagGuard`, `MemoryFlagGuard`, `computeBucket`, AF_XDP simulation |
| **Go** | [`packages/go/`](packages/go/edgeflag) | `go get` / pkg.go.dev | `FlagGuard`, `MemoryFlagGuard`, `ComputeBucket`, AF_XDP bypass |

---

## Companion Production Repositories

This project has been cleanly decomposed by concept:

1. **`leanmetal`** (This Repository): The core library of zero-overhead concurrency primitives and multi-language client packages.
2. **`edgeflag`**: The standalone bare-metal feature flag daemon powered by Actix Web, LMDB mmap, and Valkey/UDP invalidation mesh.
3. **`social-benchmark`**: The stateful $12/month commodity server social API concurrency benchmark (500k posts, 50k users, sub-millisecond P99).

---

## License

Dual-licensed under MIT / Apache-2.0.
