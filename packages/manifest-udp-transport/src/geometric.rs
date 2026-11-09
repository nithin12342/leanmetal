//! packages/manifest-udp-transport/src/geometric.rs
//! File ID: FILE-020
//! Responsibility: Geometric Manifest UDP hardware geometry engine (<=7 words)
//! Must Never: Cause MTU fragmentation or unaligned L1 cache line accesses.

use core::alloc::Layout;
use std::alloc::{alloc_zeroed, dealloc};

/// Cache-Engineered MTU Constants:
/// Standard Ethernet MTU = 1,500 bytes.
/// IP Header (20B) + UDP Header (8B) = 28B kernel transport headers.
/// Available UDP Datagram Payload = 1,500 - 28 = 1,472 bytes max.
///
/// Geometric Framing:
/// - Header: Exactly 62 bytes (fits with 2B alignment to 64B).
/// - Payload Chunk: Exactly 1,408 bytes (22 * 64-byte CPU cache lines).
/// - Total Wire Frame: 62 + 1,408 = 1,470 bytes <= 1,472 bytes (0 fragmentation).
pub const CACHE_LINE_BYTES: usize = 64;
pub const CHUNK_CACHE_LINES: usize = 22;
pub const CHUNK_STRIDE_BYTES: usize = CHUNK_CACHE_LINES * CACHE_LINE_BYTES; // 1,408 Bytes
pub const GEOMETRIC_HEADER_BYTES: usize = 62;
pub const MAX_WIRE_FRAME_BYTES: usize = GEOMETRIC_HEADER_BYTES + CHUNK_STRIDE_BYTES; // 1,470 Bytes
pub const ETHERNET_MTU_BYTES: usize = 1500;
pub const MAX_UDP_PAYLOAD_BYTES: usize = 1472;

/// Exactly 62 bytes.
/// Sent upfront to declare memory geometries before payload arrival.
/// Enables receiver to allocate exact DMA arena and pre-calculate target offsets.
#[repr(C, packed)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GeometricManifestHeader {
    pub manifest_id: u64,      // 8B (offset 0..8) - Monotonic batch identifier
    pub total_payload_len: u64,// 8B (offset 8..16) - Complete message byte length
    pub session_token: u64,    // 8B (offset 16..24) - Persistent connection token
    pub timestamp_ns: u64,     // 8B (offset 24..32) - Nanosecond transmission timestamp
    pub correlation_id: u64,   // 8B (offset 32..40) - Request/Response pairing ID
    pub chunk_count: u32,      // 4B (offset 40..44) - Total number of chunks in this manifest
    pub chunk_index: u32,      // 4B (offset 44..48) - 0-based sequence index of this chunk
    pub valid_bytes: u16,      // 2B (offset 48..50) - Actual unpadded data bytes in this chunk
    pub routing_tag: u32,      // 4B (offset 50..54) - Topic or endpoint route
    pub channel_id: u8,        // 1B (offset 54..55) - Virtual stream channel (0..255)
    pub flags: u8,             // 1B (offset 55..56) - Control bits: Bit 0 = IS_MANIFEST_DECLARATION
    pub reserved_zero: [u8; 6],// 6B (offset 56..62) - Explicit zero pad to 62 bytes
}

const _: () = {
    assert!(core::mem::size_of::<GeometricManifestHeader>() == 62, "GeometricManifestHeader must be exactly 62 bytes");
    assert!(MAX_WIRE_FRAME_BYTES <= MAX_UDP_PAYLOAD_BYTES, "Frame must fit in UDP payload without IP fragmentation");
    assert!(CHUNK_STRIDE_BYTES % CACHE_LINE_BYTES == 0, "Chunk stride must be a multiple of 64 bytes");
};

impl Default for GeometricManifestHeader {
    #[inline(always)]
    fn default() -> Self {
        Self {
            manifest_id: 0,
            total_payload_len: 0,
            session_token: 0,
            timestamp_ns: 0,
            correlation_id: 0,
            chunk_count: 0,
            chunk_index: 0,
            valid_bytes: 0,
            routing_tag: 0,
            channel_id: 0,
            flags: 0,
            reserved_zero: [0u8; 6],
        }
    }
}

impl GeometricManifestHeader {
    #[inline(always)]
    pub fn manifest_id(&self) -> u64 {
        self.manifest_id
    }

    #[inline(always)]
    pub fn total_payload_len(&self) -> u64 {
        self.total_payload_len
    }

    #[inline(always)]
    pub fn session_token(&self) -> u64 {
        self.session_token
    }

    #[inline(always)]
    pub fn timestamp_ns(&self) -> u64 {
        self.timestamp_ns
    }

    #[inline(always)]
    pub fn correlation_id(&self) -> u64 {
        self.correlation_id
    }

    #[inline(always)]
    pub fn chunk_count(&self) -> u32 {
        self.chunk_count
    }

    #[inline(always)]
    pub fn chunk_index(&self) -> u32 {
        self.chunk_index
    }

    #[inline(always)]
    pub fn valid_bytes(&self) -> u16 {
        self.valid_bytes
    }

    #[inline(always)]
    pub fn routing_tag(&self) -> u32 {
        self.routing_tag
    }

    #[inline(always)]
    pub fn channel_id(&self) -> u8 {
        self.channel_id
    }

    #[inline(always)]
    pub fn flags(&self) -> u8 {
        self.flags
    }

    #[inline(always)]
    pub fn is_manifest_declaration(&self) -> bool {
        (self.flags & 0x01) != 0
    }

    #[inline(always)]
    pub fn set_manifest_declaration(&mut self, is_decl: bool) {
        if is_decl {
            self.flags |= 0x01;
        } else {
            self.flags &= !0x01;
        }
    }

    /// Calculate required chunks for a given payload size under 1,408-byte strides.
    #[inline(always)]
    pub fn calculate_chunk_count(total_payload_len: u64) -> u32 {
        if total_payload_len == 0 {
            0
        } else {
            ((total_payload_len + (CHUNK_STRIDE_BYTES as u64 - 1)) / (CHUNK_STRIDE_BYTES as u64)) as u32
        }
    }

    /// Encode header to raw bytes with zero copy.
    #[inline(always)]
    pub fn to_bytes(&self, out: &mut [u8; GEOMETRIC_HEADER_BYTES]) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                self as *const Self as *const u8,
                out.as_mut_ptr(),
                GEOMETRIC_HEADER_BYTES,
            );
        }
    }

    /// Decode header from raw bytes.
    #[inline(always)]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < GEOMETRIC_HEADER_BYTES {
            return None;
        }
        let mut hdr = core::mem::MaybeUninit::<Self>::uninit();
        unsafe {
            core::ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                hdr.as_mut_ptr() as *mut u8,
                GEOMETRIC_HEADER_BYTES,
            );
            Some(hdr.assume_init())
        }
    }
}

/// 1-Cycle Hardware NACK Detector:
/// Identifies missing chunk sequence indices using hardware bitwise NOT followed by trailing zero count.
/// Operates on 64-chunk leaves in single-digit clock cycles.
#[inline(always)]
pub fn find_missing_chunks_tzcnt(received_mask: u64, total_chunks: u32) -> u64 {
    let valid_bits = if total_chunks >= 64 {
        u64::MAX
    } else {
        (1u64 << total_chunks) - 1
    };

    (!received_mask) & valid_bits
}

/// Extracts the lowest missing chunk index in 1 CPU cycle using hardware `tzcnt` (trailing zeros).
#[inline(always)]
pub fn next_missing_chunk_index(missing_mask: u64) -> Option<u32> {
    if missing_mask == 0 {
        None
    } else {
        Some(missing_mask.trailing_zeros())
    }
}

/// In-Place DMA Memory Arena
/// Pre-allocated contiguous host memory mapped to 64-byte cache lines.
/// Receives incoming chunks directly into calculated memory offsets:
///
/// Target Memory Address = Base Arena Pointer + (Chunk Index * Chunk Stride)
///
/// Backed by page-aligned zero-initialized virtual memory (Copy-on-Write `empty_zero_page`).
pub struct InPlaceDmaArena {
    ptr: *mut u8,
    layout: Layout,
    manifest_id: u64,
    total_payload_len: usize,
    chunk_count: u32,
    received_leaves: Vec<u64>,
    received_count: u32,
}

// Memory pointer safely isolated to one arena instance
unsafe impl Send for InPlaceDmaArena {}
unsafe impl Sync for InPlaceDmaArena {}

impl InPlaceDmaArena {
    /// Allocate an aligned DMA arena sized exactly to the manifest geometry.
    /// Memory is page-aligned (4096 bytes) and zero-backed, ensuring zero-cost virtual memory.
    pub fn new(manifest_id: u64, total_payload_len: usize) -> Result<Self, &'static str> {
        let chunk_count = GeometricManifestHeader::calculate_chunk_count(total_payload_len as u64);
        let alloc_bytes = if chunk_count == 0 {
            CACHE_LINE_BYTES
        } else {
            (chunk_count as usize) * CHUNK_STRIDE_BYTES
        };

        // Align to 4096-byte OS page boundary (which is naturally 64-byte aligned)
        let align = 4096;
        let layout = Layout::from_size_align(alloc_bytes, align)
            .map_err(|_| "Invalid layout alignment for DMA arena")?;

        let ptr = unsafe { alloc_zeroed(layout) };
        if ptr.is_null() {
            return Err("Failed to allocate zero-backed DMA arena memory");
        }

        let num_leaves = if chunk_count == 0 {
            0
        } else {
            (chunk_count as usize + 63) / 64
        };
        let received_leaves = vec![0u64; num_leaves];
        let received_count = 0;

        Ok(Self {
            ptr,
            layout,
            manifest_id,
            total_payload_len,
            chunk_count,
            received_leaves,
            received_count,
        })
    }

    #[inline(always)]
    pub fn manifest_id(&self) -> u64 {
        self.manifest_id
    }

    #[inline(always)]
    pub fn total_payload_len(&self) -> usize {
        self.total_payload_len
    }

    #[inline(always)]
    pub fn chunk_count(&self) -> u32 {
        self.chunk_count
    }

    #[inline(always)]
    pub fn received_count(&self) -> u32 {
        self.received_count
    }

    #[inline(always)]
    pub fn is_complete(&self) -> bool {
        self.received_count == self.chunk_count
    }

    /// Calculate target offset from base arena for a chunk index:
    /// Target Memory Address = Base Arena Offset + (Chunk Index * Chunk Stride)
    #[inline(always)]
    pub fn chunk_offset(chunk_index: u32) -> usize {
        (chunk_index as usize) * CHUNK_STRIDE_BYTES
    }

    /// Direct target memory pointer for DMA placement
    #[inline(always)]
    pub fn target_memory_address(&self, chunk_index: u32) -> *mut u8 {
        assert!(chunk_index < self.chunk_count, "Chunk index out of bounds");
        unsafe { self.ptr.add(Self::chunk_offset(chunk_index)) }
    }

    /// Enforces that target address strictly matches a 64-byte CPU cache line boundary.
    #[inline(always)]
    pub fn is_aligned_64(&self, chunk_index: u32) -> bool {
        let addr = self.target_memory_address(chunk_index) as usize;
        addr % CACHE_LINE_BYTES == 0
    }

    /// Direct slice to the 1,408-byte chunk stride in host RAM
    #[inline(always)]
    pub fn chunk_slice(&self, chunk_index: u32) -> Option<&[u8]> {
        if chunk_index >= self.chunk_count {
            return None;
        }
        unsafe {
            let chunk_ptr = self.target_memory_address(chunk_index);
            Some(core::slice::from_raw_parts(chunk_ptr, CHUNK_STRIDE_BYTES))
        }
    }

    /// Direct mutable slice to the 1,408-byte chunk stride in host RAM
    #[inline(always)]
    pub fn chunk_slice_mut(&mut self, chunk_index: u32) -> Option<&mut [u8]> {
        if chunk_index >= self.chunk_count {
            return None;
        }
        unsafe {
            let chunk_ptr = self.target_memory_address(chunk_index);
            Some(core::slice::from_raw_parts_mut(chunk_ptr, CHUNK_STRIDE_BYTES))
        }
    }

    /// Write an incoming wire chunk directly into its DMA memory offset.
    /// Dual-layer strategy:
    /// - Zero Wire Padding: Wire datagram transmits only `header.valid_bytes`.
    /// - 64-Byte Host Stride: In-memory chunk occupies full 1,408-byte stride.
    pub fn write_chunk(
        &mut self,
        header: &GeometricManifestHeader,
        wire_payload: &[u8],
    ) -> Result<bool, &'static str> {
        if header.manifest_id != self.manifest_id {
            return Err("Manifest ID mismatch");
        }
        if header.chunk_index >= self.chunk_count {
            return Err("Chunk index exceeds allocated manifest count");
        }
        let valid_bytes = header.valid_bytes as usize;
        if valid_bytes > CHUNK_STRIDE_BYTES {
            return Err("Chunk valid_bytes exceeds 1,408 byte stride");
        }
        if wire_payload.len() < valid_bytes {
            return Err("Wire payload shorter than declared valid_bytes");
        }

        // Direct memory placement via calculated offset
        let target_ptr = self.target_memory_address(header.chunk_index);
        unsafe {
            core::ptr::copy_nonoverlapping(wire_payload.as_ptr(), target_ptr, valid_bytes);
        }

        // Update L1 leaf bitmask
        let leaf_idx = (header.chunk_index / 64) as usize;
        let bit_idx = header.chunk_index % 64;
        let leaf_mask = &mut self.received_leaves[leaf_idx];

        if (*leaf_mask & (1u64 << bit_idx)) == 0 {
            *leaf_mask |= 1u64 << bit_idx;
            self.received_count += 1;
        }

        Ok(self.is_complete())
    }

    /// Retrieve all missing chunk indices using 1-cycle `_tzcnt_u64` per missing chunk.
    pub fn missing_chunks(&self) -> Vec<u32> {
        let mut missing = Vec::new();
        for (leaf_idx, &received_mask) in self.received_leaves.iter().enumerate() {
            let remaining_chunks_in_manifest = self.chunk_count.saturating_sub((leaf_idx as u32) * 64);
            let chunks_in_this_leaf = remaining_chunks_in_manifest.min(64);

            let mut missing_mask = find_missing_chunks_tzcnt(received_mask, chunks_in_this_leaf);
            while let Some(offset) = next_missing_chunk_index(missing_mask) {
                let chunk_idx = (leaf_idx as u32) * 64 + offset;
                missing.push(chunk_idx);
                // Clear the lowest set bit
                missing_mask &= missing_mask - 1;
            }
        }
        missing
    }

    /// Reassembles the full unpadded contiguous payload once all chunks are present.
    pub fn assemble_payload(&self, out: &mut [u8]) -> Result<usize, &'static str> {
        if !self.is_complete() {
            return Err("Cannot assemble payload: manifest batch is incomplete");
        }
        if out.len() < self.total_payload_len {
            return Err("Output buffer too small for payload assembly");
        }

        let mut write_pos = 0;
        let mut bytes_left = self.total_payload_len;

        for chunk_idx in 0..self.chunk_count {
            let chunk_data = self.chunk_slice(chunk_idx).unwrap();
            let to_copy = bytes_left.min(CHUNK_STRIDE_BYTES);
            out[write_pos..write_pos + to_copy].copy_from_slice(&chunk_data[..to_copy]);
            write_pos += to_copy;
            bytes_left -= to_copy;
        }

        Ok(write_pos)
    }

    /// Convenience reassembly into an allocated `Vec<u8>`.
    pub fn to_contiguous_vec(&self) -> Result<Vec<u8>, &'static str> {
        let mut buf = vec![0u8; self.total_payload_len];
        self.assemble_payload(&mut buf)?;
        Ok(buf)
    }
}

impl Drop for InPlaceDmaArena {
    fn drop(&mut self) {
        if !self.ptr.is_null() {
            unsafe {
                dealloc(self.ptr, self.layout);
            }
            self.ptr = core::ptr::null_mut();
        }
    }
}

/// Codec for Geometric Wire Frames:
/// Transmits header (62B) + exact unpadded payload without wire padding.
pub struct GeometricFrameCodec;

impl GeometricFrameCodec {
    /// Encode frame to wire datagram buffer.
    #[inline(always)]
    pub fn encode(
        header: &GeometricManifestHeader,
        payload_chunk: &[u8],
        wire_buf: &mut [u8],
    ) -> Result<usize, &'static str> {
        let valid_bytes = header.valid_bytes as usize;
        let total_frame_bytes = GEOMETRIC_HEADER_BYTES + valid_bytes;

        if total_frame_bytes > MAX_WIRE_FRAME_BYTES {
            return Err("Frame size exceeds cache-engineered MTU boundary");
        }
        if wire_buf.len() < total_frame_bytes {
            return Err("Wire buffer insufficient for geometric frame");
        }
        if payload_chunk.len() < valid_bytes {
            return Err("Payload slice smaller than header.valid_bytes");
        }

        header.to_bytes((&mut wire_buf[..GEOMETRIC_HEADER_BYTES]).try_into().unwrap());
        wire_buf[GEOMETRIC_HEADER_BYTES..total_frame_bytes]
            .copy_from_slice(&payload_chunk[..valid_bytes]);

        Ok(total_frame_bytes)
    }

    /// Decode geometric frame from incoming datagram.
    #[inline(always)]
    pub fn decode<'a>(
        wire_buf: &'a [u8],
    ) -> Result<(GeometricManifestHeader, &'a [u8]), &'static str> {
        if wire_buf.len() < GEOMETRIC_HEADER_BYTES {
            return Err("Datagram truncated: smaller than geometric header");
        }

        let header = GeometricManifestHeader::from_bytes(&wire_buf[..GEOMETRIC_HEADER_BYTES])
            .ok_or("Failed to parse geometric header")?;

        let valid_bytes = header.valid_bytes as usize;
        let expected_frame_bytes = GEOMETRIC_HEADER_BYTES + valid_bytes;

        if wire_buf.len() < expected_frame_bytes {
            return Err("Datagram truncated: smaller than header.valid_bytes");
        }

        let payload = &wire_buf[GEOMETRIC_HEADER_BYTES..expected_frame_bytes];
        Ok((header, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometric_header_size_and_packing() {
        assert_eq!(core::mem::size_of::<GeometricManifestHeader>(), 62);
        let mut hdr = GeometricManifestHeader::default();
        hdr.manifest_id = 0x0102030405060708;
        hdr.total_payload_len = 100_000;
        hdr.chunk_count = 72;
        hdr.chunk_index = 5;
        hdr.valid_bytes = 1408;
        hdr.set_manifest_declaration(true);

        assert!(hdr.is_manifest_declaration());

        let mut bytes = [0u8; 62];
        hdr.to_bytes(&mut bytes);

        let parsed = GeometricManifestHeader::from_bytes(&bytes).unwrap();
        assert_eq!(parsed.manifest_id(), 0x0102030405060708);
        assert_eq!(parsed.total_payload_len(), 100_000);
        assert_eq!(parsed.chunk_count(), 72);
        assert_eq!(parsed.chunk_index(), 5);
        assert_eq!(parsed.valid_bytes(), 1408);
        assert!(parsed.is_manifest_declaration());
    }

    #[test]
    fn test_mtu_and_cacheline_geometry() {
        // Assert zero fragmentation within 1,500 byte Ethernet MTU
        assert!(MAX_WIRE_FRAME_BYTES <= MAX_UDP_PAYLOAD_BYTES);
        assert_eq!(CHUNK_STRIDE_BYTES, 1408);
        assert_eq!(CHUNK_STRIDE_BYTES % CACHE_LINE_BYTES, 0);
        assert_eq!(CHUNK_CACHE_LINES, 22);
    }

    #[test]
    fn test_1_cycle_tzcnt_nack_detector() {
        // Test with 10 total chunks
        // Chunks received: 0, 1, 3, 4 (missing: 2, 5, 6, 7, 8, 9)
        let received_mask: u64 = (1 << 0) | (1 << 1) | (1 << 3) | (1 << 4);
        let mut missing_mask = find_missing_chunks_tzcnt(received_mask, 10);

        // First missing chunk must be index 2
        assert_eq!(next_missing_chunk_index(missing_mask), Some(2));
        missing_mask &= missing_mask - 1; // Clear bit 2

        // Next missing chunk must be index 5
        assert_eq!(next_missing_chunk_index(missing_mask), Some(5));
        missing_mask &= missing_mask - 1; // Clear bit 5

        assert_eq!(next_missing_chunk_index(missing_mask), Some(6));
    }

    #[test]
    fn test_in_place_dma_arena_offset_placement_and_padding() {
        let manifest_id = 9988;
        // Total payload of 3,500 bytes:
        // Chunk 0: 1,408 bytes
        // Chunk 1: 1,408 bytes
        // Chunk 2: 684 bytes
        // Total chunks = 3
        let total_len = 3500;
        let mut arena = InPlaceDmaArena::new(manifest_id, total_len).unwrap();

        assert_eq!(arena.chunk_count(), 3);
        assert!(!arena.is_complete());

        // Verify 64-byte CPU cache alignment for all chunk targets
        for idx in 0..3 {
            assert!(arena.is_aligned_64(idx));
            assert_eq!(InPlaceDmaArena::chunk_offset(idx) % CACHE_LINE_BYTES, 0);
        }

        // Generate synthetic test data
        let mut full_source = vec![0u8; total_len];
        for (i, byte) in full_source.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }

        // Deliver chunks out of order: Chunk 1 first, then Chunk 2, then Chunk 0
        let c1_header = GeometricManifestHeader {
            manifest_id,
            total_payload_len: total_len as u64,
            chunk_count: 3,
            chunk_index: 1,
            valid_bytes: 1408,
            ..Default::default()
        };
        let c1_wire_payload = &full_source[1408..2816];
        assert_eq!(arena.write_chunk(&c1_header, c1_wire_payload).unwrap(), false);

        // Missing chunks should report chunks 0 and 2
        assert_eq!(arena.missing_chunks(), vec![0, 2]);

        // Deliver chunk 2 (partial chunk with zero wire padding: only 684 bytes transmitted)
        let c2_header = GeometricManifestHeader {
            manifest_id,
            total_payload_len: total_len as u64,
            chunk_count: 3,
            chunk_index: 2,
            valid_bytes: 684,
            ..Default::default()
        };
        let c2_wire_payload = &full_source[2816..3500];
        assert_eq!(arena.write_chunk(&c2_header, c2_wire_payload).unwrap(), false);

        // Missing chunk is now only chunk 0
        assert_eq!(arena.missing_chunks(), vec![0]);

        // Finally deliver chunk 0
        let c0_header = GeometricManifestHeader {
            manifest_id,
            total_payload_len: total_len as u64,
            chunk_count: 3,
            chunk_index: 0,
            valid_bytes: 1408,
            ..Default::default()
        };
        let c0_wire_payload = &full_source[0..1408];
        assert_eq!(arena.write_chunk(&c0_header, c0_wire_payload).unwrap(), true);

        // Complete! No missing chunks
        assert!(arena.is_complete());
        assert!(arena.missing_chunks().is_empty());

        // Verify full payload reassembly
        let assembled = arena.to_contiguous_vec().unwrap();
        assert_eq!(assembled.len(), total_len);
        assert_eq!(assembled, full_source);
    }

    #[test]
    fn test_wire_frame_codec_zero_wire_padding() {
        let header = GeometricManifestHeader {
            manifest_id: 12345,
            total_payload_len: 200,
            chunk_count: 1,
            chunk_index: 0,
            valid_bytes: 200,
            routing_tag: 42,
            ..Default::default()
        };

        let payload = [0xABu8; 200];
        let mut wire_buffer = [0u8; 512];

        // Encodes only 62B header + 200B unpadded wire payload = 262 bytes
        let encoded_len = GeometricFrameCodec::encode(&header, &payload, &mut wire_buffer).unwrap();
        assert_eq!(encoded_len, 62 + 200);

        // Decode roundtrip
        let (decoded_hdr, decoded_payload) =
            GeometricFrameCodec::decode(&wire_buffer[..encoded_len]).unwrap();
        assert_eq!(decoded_hdr.manifest_id(), 12345);
        assert_eq!(decoded_hdr.valid_bytes(), 200);
        assert_eq!(decoded_payload, &payload[..]);
    }
}
