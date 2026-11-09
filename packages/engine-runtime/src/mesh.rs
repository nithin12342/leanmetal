use std::collections::HashSet;
use std::net::SocketAddr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GossipPacket {
    pub origin_node_id: u32,
    pub key: String,
    pub revision: u64,
}

pub struct PeerMesh {
    node_id: u32,
    peers: HashSet<SocketAddr>,
}

impl PeerMesh {
    pub fn new(node_id: u32) -> Self {
        Self {
            node_id,
            peers: HashSet::new(),
        }
    }

    #[inline]
    pub fn node_id(&self) -> u32 {
        self.node_id
    }

    pub fn add_peer(&mut self, addr: SocketAddr) {
        self.peers.insert(addr);
    }

    pub fn peer_count(&self) -> usize {
        self.peers.len()
    }

    pub fn create_invalidation_packet(&self, key: &str, revision: u64) -> GossipPacket {
        GossipPacket {
            origin_node_id: self.node_id,
            key: key.to_string(),
            revision,
        }
    }
}
