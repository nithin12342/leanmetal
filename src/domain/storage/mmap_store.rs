//! src/domain/storage/mmap_store.rs
//! File ID: FILE-004
//! Responsibility: Manage zero-copy memory-mapped rule storage (<=7 words)
//! Must Never: Lock read operations or copy data into heap on read paths.

use crate::domain::engine::model::FlagDefinition;
use heed::types::{Bytes, Str};
use heed::{Database, Env, EnvOpenOptions, RoTxn};
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Heed LMDB error: {0}")]
    Heed(#[from] heed::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Zero-copy memory-mapped feature flag store backed by heed/LMDB.
#[derive(Clone)]
pub struct MmapFlagStore {
    env: Arc<Env>,
    db: Database<Str, Bytes>,
}

impl MmapFlagStore {
    /// Opens or creates an LMDB environment at the specified directory path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, StorageError> {
        let path = path.as_ref();
        if !path.exists() {
            fs::create_dir_all(path)?;
        }

        // Configure LMDB environment: 10GB virtual address mapping, 10 databases
        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(10 * 1024 * 1024 * 1024) // 10 GB virtual space (sparse)
                .max_dbs(10)
                .open(path)?
        };

        let mut wtxn = env.write_txn()?;
        let db = env.create_database(&mut wtxn, Some("flags_v1"))?;
        wtxn.commit()?;

        Ok(Self {
            env: Arc::new(env),
            db,
        })
    }

    /// Acquires a lock-free, read-only transaction for reading directly from the mmap page cache.
    pub fn read_txn(&self) -> Result<RoTxn<'_>, StorageError> {
        Ok(self.env.read_txn()?)
    }

    /// Returns a direct zero-copy byte slice (&'a [u8]) mapped directly from operating system RAM.
    /// Incurs zero heap allocations.
    #[inline(always)]
    pub fn get_raw<'a>(&self, rtxn: &'a RoTxn<'_>, key: &str) -> Result<Option<&'a [u8]>, StorageError> {
        Ok(self.db.get(rtxn, key)?)
    }

    /// Deserializes a flag definition from the memory-mapped slice.
    pub fn get_flag(&self, rtxn: &RoTxn<'_>, key: &str) -> Result<Option<FlagDefinition>, StorageError> {
        match self.get_raw(rtxn, key)? {
            Some(bytes) => {
                let flag: FlagDefinition = serde_json::from_slice(bytes)?;
                Ok(Some(flag))
            }
            None => Ok(None),
        }
    }

    /// Atomically persists a feature flag within a single-writer Copy-on-Write transaction.
    pub fn put_flag(&self, flag: &FlagDefinition) -> Result<(), StorageError> {
        let serialized = serde_json::to_vec(flag)?;
        let mut wtxn = self.env.write_txn()?;
        self.db.put(&mut wtxn, &flag.key, &serialized)?;
        wtxn.commit()?;
        Ok(())
    }

    /// Deletes a feature flag atomically.
    pub fn delete_flag(&self, key: &str) -> Result<bool, StorageError> {
        let mut wtxn = self.env.write_txn()?;
        let deleted = self.db.delete(&mut wtxn, key)?;
        wtxn.commit()?;
        Ok(deleted)
    }

    /// Returns all flag keys currently indexed in the store.
    pub fn list_keys(&self, rtxn: &RoTxn<'_>) -> Result<Vec<String>, StorageError> {
        let mut keys = Vec::new();
        let iter = self.db.iter(rtxn)?;
        for item in iter {
            let (k, _) = item?;
            keys.push(k.to_string());
        }
        Ok(keys)
    }
}
