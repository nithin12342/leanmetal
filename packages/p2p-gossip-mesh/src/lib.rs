//! packages/p2p-gossip-mesh/src/lib.rs
//! Daemonless P2P UDP state delta broadcast and cache invalidation mesh.

pub mod mesh;

pub use mesh::{
    GossipMeshBus, InvalidationError, InvalidationMessage, ReplicationDelta, ValkeyMeshBus,
    INVALIDATION_CHANNEL,
};
