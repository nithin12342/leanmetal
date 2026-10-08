//! src/domain/storage/mod.rs
//! Durable storage and LSM Write-Ahead Logging module

pub mod async_wal;
pub mod mmap_store;
pub mod wal;

pub use async_wal::AsyncWal;
pub use mmap_store::{MmapFlagStore, StorageError};
pub use wal::{AuditEntry, AuditWal, WalError};
