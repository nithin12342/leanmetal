use engine_disruptor::DisruptorRing;
use engine_eda::{CommandIntent, CommandProcessor, ProjectionStore};
use engine_runtime::{PeerMesh, UdpIngress, UnikraftTarget};
use std::net::SocketAddr;

#[test]
fn test_runtime_bootstrap_flow() {
    let mut ring = DisruptorRing::<512>::new();
    assert_eq!(ring.capacity(), 512);

    let store = ProjectionStore::new();
    let intent = CommandIntent {
        aggregate_id: 42,
        expected_revision: 0,
        event_type: 1,
        origin_node_id: 1,
        payload: [9u8; 16],
    };

    let envelope = CommandProcessor::process_command(intent, 1, 1000).unwrap();
    store.apply_event(&envelope);

    let view = store.load_view();
    assert_eq!(*view.aggregate_versions.get(&42).unwrap(), 1);

    let ingress = UdpIngress::unbonded();
    let polled = ingress.poll_into_ring(&mut ring).unwrap();
    assert_eq!(polled, None);
}

#[test]
fn test_peer_mesh_and_unikraft_profile() {
    let mut mesh = PeerMesh::new(7);
    let addr: SocketAddr = "127.0.0.1:9870".parse().unwrap();
    mesh.add_peer(addr);
    assert_eq!(mesh.peer_count(), 1);

    let pkt = mesh.create_invalidation_packet("flag_killswitch", 100);
    assert_eq!(pkt.origin_node_id, 7);
    assert_eq!(pkt.key, "flag_killswitch");
    assert_eq!(pkt.revision, 100);

    let target = UnikraftTarget::default();
    let args = target.build_qemu_args();
    assert!(args.contains(&"-kernel".to_string()));
    assert!(args.contains(&"edgeflag_engine.raw".to_string()));
}
