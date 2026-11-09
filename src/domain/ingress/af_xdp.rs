//! src/domain/ingress/af_xdp.rs
//! File ID: FILE-015
//! Responsibility: Bind native AF_XDP XSK sockets with fallback (<= 7 words)
//! Must Never: Abort boot or panic when NIC/XDP unavailable; degrade gracefully.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// True when this binary contains the native XSK bind path.
const NATIVE_BUILD: bool = cfg!(all(target_os = "linux", feature = "af-xdp"));

#[cfg(all(target_os = "linux", feature = "af-xdp"))]
use xsk_rs::{CompQueue, FillQueue, FrameDesc, RxQueue, Socket, TxQueue, Umem};

pub const DEFAULT_FRAME_COUNT: u32 = 1024;
/// Conservative TX cap: fits any default UMEM frame (>= 2 KiB) and any MTU.
pub const MAX_TX_PAYLOAD: usize = 1500;

/// Requested XDP attach mode for `xsk_socket__create` (via xsk-rs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum XdpMode {
    /// Zero-copy driver mode (`XDP_FLAGS_DRV_MODE`). Requires NIC XDP support.
    #[default]
    Drv,
    /// Generic SKB mode (`XDP_FLAGS_SKB_MODE`). Works on any NIC, still
    /// bypasses the filter stack but copies via `sk_buff`.
    Skb,
}

/// Native bind configuration (REQ-020 / SPEC-020).
#[derive(Debug, Clone)]
pub struct AfXdpConfig {
    pub ifname: String,
    pub queue_id: u32,
    pub frame_count: u32,
    pub mode: XdpMode,
    /// When `mode == Drv` fails (no NIC support), retry once in SKB mode.
    pub skb_fallback: bool,
}

impl Default for AfXdpConfig {
    fn default() -> Self {
        Self {
            ifname: "eth0".to_string(),
            queue_id: 0,
            frame_count: DEFAULT_FRAME_COUNT,
            mode: XdpMode::Drv,
            skb_fallback: true,
        }
    }
}

impl AfXdpConfig {
    /// Pure-Rust validation: runs on every platform, no NIC access.
    pub fn validate(&self) -> Result<(), AfXdpError> {
        let name = self.ifname.trim();
        if name.is_empty() || name.len() > 15 {
            return Err(AfXdpError::InvalidConfig(format!(
                "interface name must be 1-15 chars, got '{}'",
                self.ifname
            )));
        }
        if !(16..=65_536).contains(&self.frame_count) || !self.frame_count.is_power_of_two() {
            return Err(AfXdpError::InvalidConfig(format!(
                "frame_count must be a power of two in 16..=65536, got {}",
                self.frame_count
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AfXdpError {
    #[error("native AF_XDP unavailable in this build: {0}")]
    Unsupported(&'static str),
    #[error("invalid AF_XDP config: {0}")]
    InvalidConfig(String),
    #[error("AF_XDP UMEM setup failed on '{0}': {1}")]
    Umem(String, String),
    #[error("AF_XDP socket bind failed on '{0}' queue {1}: {2}")]
    Socket(String, u32, String),
    #[error("AF_XDP transmit failed: {0}")]
    Transmit(String),
    #[error("AF_XDP receive failed: {0}")]
    Receive(String),
}

/// Snapshot of native datapath counters.
#[derive(Debug, Clone, Copy, Default)]
pub struct XdpStats {
    pub rx_packets: u64,
    pub rx_bytes: u64,
    pub tx_packets: u64,
    pub downgraded_to_skb: bool,
}

/// Native XSK engine. Owns one UMEM + RX/TX/Fill/Completion rings bound to a
/// single `(interface, queue)` pair. When the NIC, privileges, or build lack
/// XDP support, `open` returns `Err` and callers must use `KernelBypassEngine`.
pub struct NativeXskEngine {
    config: AfXdpConfig,
    downgraded_to_skb: AtomicBool,
    rx_packets: AtomicU64,
    rx_bytes: AtomicU64,
    tx_packets: AtomicU64,
    #[cfg(all(target_os = "linux", feature = "af-xdp"))]
    inner: std::sync::Mutex<NativeInner>,
}

#[cfg(all(target_os = "linux", feature = "af-xdp"))]
struct NativeInner {
    umem: Umem,
    tx_q: TxQueue,
    rx_q: RxQueue,
    fill_q: FillQueue,
    comp_q: CompQueue,
    /// Descriptors recycled from RX path, ready for TX writes.
    tx_free: Vec<FrameDesc>,
    rx_buf: Vec<FrameDesc>,
}

impl NativeXskEngine {
    /// Returns true only for Linux binaries built with `--features af-xdp`.
    pub fn is_native_build() -> bool {
        NATIVE_BUILD
    }

    /// Validates config, then binds `xsk_socket__create` on `(ifname, queue)`.
    /// DRV mode is tried first; on failure retries once in SKB mode when
    /// `skb_fallback` is set. Never panics; all NIC errors become `Err`.
    pub fn open(config: AfXdpConfig) -> Result<Self, AfXdpError> {
        config.validate()?;
        #[cfg(all(target_os = "linux", feature = "af-xdp"))]
        {
            Self::native_open(config)
        }
        #[cfg(not(all(target_os = "linux", feature = "af-xdp")))]
        {
            let _ = config;
            Err(AfXdpError::Unsupported(
                "rebuild with --features af-xdp on Linux for native bind",
            ))
        }
    }

    /// Non-throwing availability check: binds a transient socket and drops it.
    pub fn probe(config: &AfXdpConfig) -> bool {
        Self::open(config.clone()).is_ok()
    }

    /// Opt-in boot hook. Returns `None` (never errors) when `EDGEFLAG_XDP_IFACE`
    /// is unset or the bind fails, so daemon boot always falls back cleanly.
    /// Optional env: `EDGEFLAG_XDP_QUEUE`, `EDGEFLAG_XDP_FRAMES`, `EDGEFLAG_XDP_MODE`
    /// (`drv`|`skb`), `EDGEFLAG_XDP_SKB_FALLBACK` (`0` disables downgrade).
    pub fn try_open_from_env() -> Option<Self> {
        let ifname = std::env::var("EDGEFLAG_XDP_IFACE").ok()?;
        let config = AfXdpConfig {
            ifname,
            queue_id: std::env::var("EDGEFLAG_XDP_QUEUE")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(0),
            frame_count: std::env::var("EDGEFLAG_XDP_FRAMES")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_FRAME_COUNT),
            mode: match std::env::var("EDGEFLAG_XDP_MODE")
                .unwrap_or_default()
                .to_ascii_lowercase()
                .as_str()
            {
                "skb" => XdpMode::Skb,
                _ => XdpMode::Drv,
            },
            skb_fallback: std::env::var("EDGEFLAG_XDP_SKB_FALLBACK")
                .map(|v| v != "0")
                .unwrap_or(true),
        };
        match Self::open(config) {
            Ok(engine) => {
                tracing::info!(
                    "AF_XDP native bind active on '{}' (downgraded_to_skb={})",
                    engine.config.ifname,
                    engine.downgraded()
                );
                Some(engine)
            }
            Err(e) => {
                tracing::warn!(
                    "AF_XDP native bind unavailable, using simulation: {}",
                    e
                );
                None
            }
        }
    }

    pub fn config(&self) -> &AfXdpConfig {
        &self.config
    }

    pub fn downgraded(&self) -> bool {
        self.downgraded_to_skb.load(Ordering::Relaxed)
    }

    pub fn stats(&self) -> XdpStats {
        XdpStats {
            rx_packets: self.rx_packets.load(Ordering::Relaxed),
            rx_bytes: self.rx_bytes.load(Ordering::Relaxed),
            tx_packets: self.tx_packets.load(Ordering::Relaxed),
            downgraded_to_skb: self.downgraded(),
        }
    }

    #[cfg(all(target_os = "linux", feature = "af-xdp"))]
    fn native_open(config: AfXdpConfig) -> Result<Self, AfXdpError> {
        use xsk_rs::config::{SocketConfigBuilder, UmemConfig, XdpFlags};

        let frame_count = config.frame_count;
        let frame_count_nz = frame_count.try_into().map_err(|_| {
            AfXdpError::InvalidConfig(format!("frame_count {frame_count} out of range"))
        })?;
        let (umem, descs) = Umem::new(UmemConfig::default(), frame_count_nz, false)
            .map_err(|e| AfXdpError::Umem(config.ifname.clone(), format!("{e:?}")))?;

        let iface: xsk_rs::config::Interface =
            config.ifname.parse().map_err(|e| {
                AfXdpError::InvalidConfig(format!("unparsable interface: {e:?}"))
            })?;

        // Try DRV (zero-copy) first, then SKB fallback — each attempt is a
        // real `xsk_socket__create` against the NIC driver.
        let mut attempts = vec![config.mode];
        if config.mode == XdpMode::Drv && config.skb_fallback {
            attempts.push(XdpMode::Skb);
        }
        let mut last_err = String::from("no bind attempts made");
        for mode in attempts {
            let flags = match mode {
                XdpMode::Drv => {
                    XdpFlags::XDP_FLAGS_DRV_MODE | XdpFlags::XDP_FLAGS_UPDATE_IF_NOEXIST
                }
                XdpMode::Skb => {
                    XdpFlags::XDP_FLAGS_SKB_MODE | XdpFlags::XDP_FLAGS_UPDATE_IF_NOEXIST
                }
            };
            let mut builder = SocketConfigBuilder::new();
            builder.xdp_flags(flags);
            let sock_cfg = builder.build();
            // SAFETY: this engine owns the sole socket for (ifname, queue);
            // UMEM outlives all queues via the enclosing struct.
            match unsafe { Socket::new(sock_cfg, &umem, &iface, config.queue_id) } {
                Ok((tx_q, rx_q, Some((mut fill_q, comp_q)))) => {
                    // Partition frames: first half feeds RX fill ring, second
                    // half is the TX free pool (same UMEM, disjoint frames).
                    let mid = (descs.len() / 2).max(1);
                    let mut descs = descs;
                    let tx_part = descs.split_off(mid);
                    unsafe { fill_q.produce(&descs) };
                    drop(descs);
                    let rx_buf = vec![FrameDesc::default(); mid];
                    let downgraded = mode == XdpMode::Skb && config.mode == XdpMode::Drv;
                    return Ok(Self {
                        config,
                        downgraded_to_skb: AtomicBool::new(downgraded),
                        rx_packets: AtomicU64::new(0),
                        rx_bytes: AtomicU64::new(0),
                        tx_packets: AtomicU64::new(0),
                        inner: std::sync::Mutex::new(NativeInner {
                            umem,
                            tx_q,
                            rx_q,
                            fill_q,
                            comp_q,
                            tx_free: tx_part,
                            rx_buf,
                        }),
                    });
                }
                Ok((_, _, None)) => {
                    last_err = "kernel returned no fill/completion rings".to_string();
                }
                Err(e) => {
                    last_err = format!("{e:?}");
                }
            }
        }
        Err(AfXdpError::Socket(
            config.ifname.clone(),
            config.queue_id,
            last_err,
        ))
    }

    /// Polls RX once (blocking up to ~100ms), copies packet bytes out of UMEM,
    /// recycles descriptors to the fill ring, and updates counters.
    /// `out` is cleared first; returns packets delivered.
    #[cfg(all(target_os = "linux", feature = "af-xdp"))]
    pub fn poll_rx(&self, out: &mut Vec<Vec<u8>>) -> Result<usize, AfXdpError> {
        out.clear();
        let mut inner = self.inner.lock().map_err(|e| {
            AfXdpError::Receive(format!("ring lock poisoned: {e}"))
        })?;
        let NativeInner { umem, rx_q, fill_q, rx_buf, .. } = &mut *inner;
        // SAFETY: descriptors come from our own UMEM; bytes are copied out
        // before descriptors are recycled to the fill ring.
        let n = unsafe { rx_q.poll_and_consume(rx_buf, 100) }
            .map_err(|e| AfXdpError::Receive(format!("{e:?}")))?;
        let mut bytes = 0u64;
        for desc in rx_buf.iter().take(n) {
            let data = unsafe { umem.data(desc) };
            bytes += data.contents().len() as u64;
            out.push(data.contents().to_vec());
        }
        unsafe { fill_q.produce(&rx_buf[..n]) };
        drop(inner);
        self.rx_packets.fetch_add(n as u64, Ordering::Relaxed);
        self.rx_bytes.fetch_add(bytes, Ordering::Relaxed);
        Ok(n)
    }

    #[cfg(not(all(target_os = "linux", feature = "af-xdp")))]
    pub fn poll_rx(&self, _out: &mut Vec<Vec<u8>>) -> Result<usize, AfXdpError> {
        Err(AfXdpError::Unsupported(
            "rebuild with --features af-xdp on Linux for native bind",
        ))
    }

    /// Transmits one packet via the TX ring (payload capped at MTU).
    #[cfg(all(target_os = "linux", feature = "af-xdp"))]
    pub fn transmit(&self, payload: &[u8]) -> Result<(), AfXdpError> {
        use std::io::Write as _;
        if payload.len() > MAX_TX_PAYLOAD {
            return Err(AfXdpError::Transmit(format!(
                "payload {}B exceeds {}B cap",
                payload.len(),
                MAX_TX_PAYLOAD
            )));
        }
        let mut inner = self.inner.lock().map_err(|e| {
            AfXdpError::Transmit(format!("ring lock poisoned: {e}"))
        })?;
        if inner.tx_free.is_empty() {
            Self::reclaim_tx_inner(&mut inner);
            if inner.tx_free.is_empty() {
                return Err(AfXdpError::Transmit("no free TX frames".to_string()));
            }
        }
        let mut desc = inner.tx_free.pop().expect("checked above");
        let NativeInner { umem, tx_q, .. } = &mut *inner;
        // SAFETY: desc belongs to our UMEM and is not in any ring.
        unsafe {
            umem
                .data_mut(&mut desc)
                .cursor()
                .write_all(payload)
                .map_err(|e| AfXdpError::Transmit(format!("umem write: {e}")))?;
            tx_q
                .produce_and_wakeup(&[desc])
                .map_err(|e| AfXdpError::Transmit(format!("{e:?}")))?;
        }
        drop(inner);
        self.tx_packets.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    #[cfg(not(all(target_os = "linux", feature = "af-xdp")))]
    pub fn transmit(&self, _payload: &[u8]) -> Result<(), AfXdpError> {
        Err(AfXdpError::Unsupported(
            "rebuild with --features af-xdp on Linux for native bind",
        ))
    }

    /// Returns completed TX descriptors to the free pool. Called automatically
    /// on TX pressure; also safe to call periodically from a poll loop.
    #[cfg(all(target_os = "linux", feature = "af-xdp"))]
    pub fn reclaim_tx(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            Self::reclaim_tx_inner(&mut inner);
        }
    }

    #[cfg(all(target_os = "linux", feature = "af-xdp"))]
    fn reclaim_tx_inner(inner: &mut NativeInner) {
        let mut done = vec![FrameDesc::default(); 32];
        // SAFETY: completion queue yields our own transmitted descriptors.
        let n = unsafe { inner.comp_q.consume(&mut done) };
        inner.tx_free.extend_from_slice(&done[..n]);
    }

    #[cfg(not(all(target_os = "linux", feature = "af-xdp")))]
    pub fn reclaim_tx(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_af_xdp_config_defaults_validate() {
        let cfg = AfXdpConfig::default();
        assert!(cfg.validate().is_ok());
        assert_eq!(cfg.ifname, "eth0");
    }

    #[test]
    fn test_af_xdp_config_rejects_bad_input() {
        for bad in [
            AfXdpConfig { ifname: String::new(), ..AfXdpConfig::default() },
            AfXdpConfig { ifname: "   ".to_string(), ..AfXdpConfig::default() },
            AfXdpConfig { ifname: "this-name-is-way-too-long".to_string(), ..AfXdpConfig::default() },
            AfXdpConfig { frame_count: 0, ..AfXdpConfig::default() },
            AfXdpConfig { frame_count: 1000, ..AfXdpConfig::default() },
            AfXdpConfig { frame_count: 1_000_000, ..AfXdpConfig::default() },
        ] {
            assert!(bad.validate().is_err(), "must reject {bad:?}");
        }
    }

    #[test]
    fn test_af_xdp_error_display_is_stable() {
        let e = AfXdpError::Socket("eth0".to_string(), 0, "EOPNOTSUPP".to_string());
        assert!(e.to_string().contains("eth0"));
    }
}
