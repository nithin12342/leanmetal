# RECONSTRUCTION AND RESTORE GUIDE: leanmetal (Core Engine & Workspace)
**Generated At:** 2026-10-30 11:56:08  
**Origin Path:** `D:\d`  
**Primary Branch:** `feat/horizontal-scaling`  
**HEAD Commit at Backup:** `e70e77c5f1e98ac60514edc6ec97f64118ac30dd`  
**Tracked Files in Archive:** 235  
**Remote Repository:** `nithin12342/leanmetal`  

---

## 1. System Overview & Architecture
* **Role:** Single-node zero-copy infrastructure engine, B+Tree LMDB mmap storage, write-harness, manifest-udp-transport
* **Available Branches:** * feat/horizontal-scaling, main, remotes/origin/HEAD -> origin/main, remotes/origin/feat/horizontal-scaling, remotes/origin/main
* **Language & Runtime:** Rust Edition 2024 / LLVM / Tokio Async Runtime
* **Storage & Memory Model:** Memory-Mapped B+Tree (LMDB / heed) + 4KB Page-Aligned Direct I/O WAL (`write-harness`)

---

## 2. Restoration & Extraction Instructions

This archive contains the **entire source tree** and the **complete `.git` metadata directory** (branches, tags, reflogs, and commit history).

### Step 1: Decompress Archive
Extract this `.zip` archive to any target destination directory:
```powershell
Expand-Archive -Path "leanmetal_backup.zip" -DestinationPath "D:\restored\leanmetal"
cd "D:\restored\leanmetal"
```

### Step 2: Verify Git Repository Integrity
Once extracted, git is immediately fully functional with zero initialization steps required:
```powershell
git status
git log --oneline -n 10
git branch -a
git checkout feat/horizontal-scaling
```
Verify that `git rev-parse HEAD` outputs `e70e77c5f1e98ac60514edc6ec97f64118ac30dd`.

---

## 3. Build & Compilation Verification

### Prerequisites
* Rust toolchain (1.85+ / 2024 edition): `rustup default stable`
* C Compiler / Linker for Windows (`x86_64-pc-windows-gnu` or MSVC) / Linux (`gcc` / `clang`)

### Build Commands
```powershell
# Build optimized release binaries
cargo build --release

# Execute full verification test suite
cargo test --release
```

---

## 4. Remote Synchronization (If applicable)
To synchronize with the upstream GitHub repository:
```powershell
git remote -v
git push origin feat/horizontal-scaling
```

---
*Archival standard: SHA-256 integrity verification, 100% commit graph fidelity, zero data loss.*
