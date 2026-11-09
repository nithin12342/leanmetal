use engine_layout::{OpCode, UnifiedFrameHeader};

pub struct UnifiedCodec;

impl UnifiedCodec {
    pub const HEADER_LEN: usize = core::mem::size_of::<UnifiedFrameHeader>(); // 24 bytes

    /// Decodes a 24-byte header directly from a raw slice without copying.
    #[inline(always)]
    pub fn decode_header(buf: &[u8]) -> Option<(UnifiedFrameHeader, &[u8])> {
        if buf.len() < Self::HEADER_LEN {
            return None;
        }

        let header = unsafe {
            core::ptr::read_unaligned(buf.as_ptr() as *const UnifiedFrameHeader)
        };

        let payload_len = header.payload_len as usize;
        if buf.len() < Self::HEADER_LEN + payload_len {
            return None;
        }

        let payload = &buf[Self::HEADER_LEN..Self::HEADER_LEN + payload_len];
        Some((header, payload))
    }

    /// Encodes a header into a pre-allocated byte slice.
    #[inline(always)]
    pub fn encode_header(header: &UnifiedFrameHeader, dst: &mut [u8]) -> bool {
        if dst.len() < Self::HEADER_LEN {
            return false;
        }
        let src_bytes = unsafe {
            core::slice::from_raw_parts(
                header as *const UnifiedFrameHeader as *const u8,
                Self::HEADER_LEN,
            )
        };
        dst[..Self::HEADER_LEN].copy_from_slice(src_bytes);
        true
    }

    #[inline(always)]
    pub fn make_header(correlation_id: u64, session_token: u64, routing_tag: u32, opcode: OpCode, payload_len: u16) -> UnifiedFrameHeader {
        UnifiedFrameHeader {
            correlation_id,
            session_token,
            routing_tag,
            payload_len,
            opcode: opcode as u8,
            flags: 0,
        }
    }
}
