//! src/domain/storage/wal.rs
//! File ID: FILE-006
//! Responsibility: Buffer and flush asynchronous audit log (<=7 words)
//! Must Never: Block primary evaluation threads during disk flushes.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum WalError {
    #[error("Fjall LSM error: {0}")]
    Fjall(#[from] fjall::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// An immutable audit journal entry capturing rule modifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditEntry {
    pub sequence_id: u64,
    pub timestamp_ms: u64,
    pub actor: String,
    pub flag_key: String,
    pub operation: String,
    pub payload_json: String,
}

/// Asynchronous Write-Ahead Log & Audit Journal backed by fjall LSM keyspace.
pub struct AuditWal {
    keyspace: fjall::Keyspace,
    partition: fjall::PartitionHandle,
    seq: AtomicU64,
}

impl AuditWal {
    /// Opens or initializes the LSM audit log at the target directory.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, WalError> {
        let path = path.as_ref();
        if !path.exists() {
            fs::create_dir_all(path)?;
        }

        let keyspace = fjall::Config::new(path).open()?;
        let partition = keyspace.open_partition("audit_wal", Default::default())?;

        // Determine current max sequence ID from existing entries
        let mut max_seq = 0u64;
        for item in partition.iter() {
            let (k, _) = item?;
            if k.len() == 8 {
                let s = u64::from_be_bytes(k[..8].try_into().unwrap());
                if s > max_seq {
                    max_seq = s;
                }
            }
        }

        Ok(Self {
            keyspace,
            partition,
            seq: AtomicU64::new(max_seq),
        })
    }

    /// Records an audit log entry in the LSM memtable without blocking on disk sync.
    pub fn append(
        &self,
        actor: &str,
        flag_key: &str,
        operation: &str,
        payload_json: &str,
    ) -> Result<AuditEntry, WalError> {
        let sequence_id = self.seq.fetch_add(1, Ordering::SeqCst) + 1;
        let timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let entry = AuditEntry {
            sequence_id,
            timestamp_ms,
            actor: actor.to_string(),
            flag_key: flag_key.to_string(),
            operation: operation.to_string(),
            payload_json: payload_json.to_string(),
        };

        let key = sequence_id.to_be_bytes();
        let value = serde_json::to_vec(&entry)?;

        self.partition.insert(key, value)?;

        Ok(entry)
    }

    /// Flushes the LSM write-ahead buffer to persistent disk storage.
    pub fn flush(&self) -> Result<(), WalError> {
        self.keyspace.persist(fjall::PersistMode::SyncAll)?;
        Ok(())
    }

    /// Reads all audit entries recorded in sequence order.
    pub fn read_all(&self) -> Result<Vec<AuditEntry>, WalError> {
        let mut entries = Vec::new();
        for item in self.partition.iter() {
            let (_, v) = item?;
            let entry: AuditEntry = serde_json::from_slice(&v)?;
            entries.push(entry);
        }
        Ok(entries)
    }
}
