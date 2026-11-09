use edgeflag::domain::ingress::af_xdp::{AfXdpConfig, AfXdpError, NativeXskEngine};
use edgeflag::{DaemonMetrics, KernelBypassEngine};

fn valid_config(ifname: &str) -> AfXdpConfig {
    AfXdpConfig {
        ifname: ifname.to_string(),
        queue_id: 0,
        frame_count: 1024,
        mode: edgeflag::XdpMode::Drv,
        skb_fallback: true,
    }
}

#[test]
fn test_af_xdp_config_validation_is_platform_independent() {
    assert!(AfXdpConfig::default().validate().is_ok());
    assert!(valid_config("eth0").validate().is_ok());
    for bad_if in ["", "   ", "this-interface-name-is-far-too-long"] {
        let err = valid_config(bad_if).validate().unwrap_err();
        assert!(matches!(err, AfXdpError::InvalidConfig(_)), "{bad_if}");
    }
    for bad_frames in [0u32, 15, 1000, 100_000] {
        let cfg = AfXdpConfig {
            frame_count: bad_frames,
            ..valid_config("eth0")
        };
        assert!(cfg.validate().is_err(), "frames={bad_frames}");
    }
}

// Without the native bind compiled in, open must fail as Unsupported —
// never as a NIC error — proving validation runs before any bind attempt.
#[cfg(not(all(target_os = "linux", feature = "af-xdp")))]
#[test]
fn test_af_xdp_open_reports_unsupported_without_native_build() {
    assert!(!NativeXskEngine::is_native_build());
    match NativeXskEngine::open(valid_config("eth0")) {
        Err(AfXdpError::Unsupported(_)) => {}
        Err(e) => panic!("expected Unsupported, got {e}"),
        Ok(_) => panic!("expected Unsupported, bind unexpectedly succeeded"),
    }
    assert!(NativeXskEngine::try_open_from_env().is_none());
    assert!(!NativeXskEngine::probe(&valid_config("eth0")));
}

// With the native bind compiled in, a bogus interface must fail at the NIC
// layer (Socket/InvalidConfig) — proving xsk_socket__create was attempted.
#[cfg(all(target_os = "linux", feature = "af-xdp"))]
#[test]
fn test_af_xdp_native_bind_fails_cleanly_without_nic() {
    assert!(NativeXskEngine::is_native_build());
    match NativeXskEngine::open(valid_config("definitely-not-an-iface")) {
        Err(AfXdpError::Socket(_, _, _)) | Err(AfXdpError::InvalidConfig(_)) => {}
        Err(e) => panic!("expected NIC-layer error, got {e}"),
        Ok(_) => panic!("bind to bogus interface unexpectedly succeeded"),
    }
    assert!(!NativeXskEngine::probe(&valid_config("definitely-not-an-iface")));
}

#[test]
fn test_af_xdp_metrics_series_present() {
    let m = DaemonMetrics::new();
    m.record_xdp_rx(3, 300);
    let out = m.render_prometheus(0, 0, false);
    assert!(out.contains("edgeflag_xdp_rx_packets_total 3"), "{out}");
    assert!(out.contains("edgeflag_xdp_rx_bytes_total 300"), "{out}");
}

#[test]
fn test_simulated_umem_engine_unaffected_by_native_module() {
    let mut engine = KernelBypassEngine::new(8, 2048);
    let desc = engine.inject_packet(b"af-xdp-regression").expect("inject");
    let seen = engine.process_packet(&desc, |b| b.to_vec());
    assert_eq!(seen, b"af-xdp-regression");
    assert!(engine.poll_rx().is_some());
    engine.complete_packet(desc);
}
