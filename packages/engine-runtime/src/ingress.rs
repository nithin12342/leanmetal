use engine_disruptor::DisruptorRing;
use engine_layout::RingEntry;
use engine_wire::UnifiedCodec;
use std::io;
use std::net::UdpSocket;

pub struct UdpIngress {
    socket: Option<UdpSocket>,
}

impl UdpIngress {
    pub fn bind(addr: &str) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr)?;
        socket.set_nonblocking(true)?;
        Ok(Self {
            socket: Some(socket),
        })
    }

    pub fn unbonded() -> Self {
        Self { socket: None }
    }

    /// Polls a single packet from the socket and publishes into the disruptor ring.
    pub fn poll_into_ring<const N: usize>(&self, ring: &mut DisruptorRing<N>) -> io::Result<Option<u64>> {
        let Some(socket) = &self.socket else {
            return Ok(None);
        };

        let mut buf = [0u8; 2048];
        match socket.recv_from(&mut buf) {
            Ok((len, _src)) => {
                if let Some((hdr, _payload)) = UnifiedCodec::decode_header(&buf[..len]) {
                    let mut entry = RingEntry::new();
                    entry.sequence = ring.cursor().get();
                    entry.session_id = hdr.session_token;
                    entry.routing_tag = hdr.routing_tag;
                    entry.opcode = hdr.opcode;
                    entry.payload_len = hdr.payload_len as u32;

                    let seq = ring.publish(entry);
                    Ok(Some(seq))
                } else {
                    Ok(None)
                }
            }
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(e),
        }
    }
}
