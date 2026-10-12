//! packages/simd-json-vector/src/lib.rs
//! Pure zero-dependency vectorized structural byte scanner and JSON tape indexer.
//!
//! Hand-rolled SWAR (SIMD Within A Register) and x86_64 AVX2/SSE2 intrinsics.
//! Strictly zero heap allocations on the scan path. Scans 64-bit/256-bit words
//! simultaneously to locate structural punctuation (`{`, `}`, `[`, `]`, `:`, `,`, `"`, `\`).

pub mod scanner;

pub use scanner::{
    find_structural_characters, quote_mask_step, StructuralBitmap, StructuralPunctuation,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_dependency_structural_scan() {
        let json = br#"{"user_id": 49102, "active": true, "roles": ["admin"]}"#;
        let bitmap = find_structural_characters(json);
        assert!(bitmap.structural_count > 0);
        // Verify key quotes and punctuation were pinpointed
        assert!(bitmap.quotes_mask != 0);
    }
}
