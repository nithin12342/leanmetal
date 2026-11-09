//! src/domain/invalidation/mod.rs
//! Distributed invalidation mesh backed by Linux Foundation Valkey

pub mod mesh;

pub use mesh::{
    GossipMeshBus, InvalidationError, InvalidationMessage, ReplicationDelta, ValkeyMeshBus,
    INVALIDATION_CHANNEL,
};

