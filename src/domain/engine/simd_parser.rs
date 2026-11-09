//! src/domain/engine/simd_parser.rs
//! File ID: FILE-008
//! Responsibility: Parse JSON via CPU vector registers (<= 7 words)
//! Must Never: Allocate unnecessary heap memory or panic on malformed input.

use serde::de::DeserializeOwned;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SimdJsonError {
    #[error("SIMD JSON parse error: {0}")]
    Simd(#[from] simd_json::Error),
    #[error("Standard JSON fallback error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Parses JSON bytes in-place using AVX2/SSE4 vector SIMD registers (REQ-014 / SPEC-014).
/// Operates directly on mutable slices (&mut [u8]) for maximum register throughput.
#[inline(always)]
pub fn from_slice_simd<T: DeserializeOwned>(bytes: &mut [u8]) -> Result<T, SimdJsonError> {
    simd_json::from_slice(bytes).map_err(SimdJsonError::Simd)
}

/// Reads immutable byte slice into a scratch buffer and parses via SIMD registers.
#[inline]
pub fn from_slice_scratch<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SimdJsonError> {
    let mut scratch = bytes.to_vec();
    simd_json::from_slice(&mut scratch).map_err(SimdJsonError::Simd)
}

/// Serializes a struct into vector bytes using SIMD-optimized formatting.
#[inline(always)]
pub fn to_vec_simd<T: Serialize>(value: &T) -> Result<Vec<u8>, SimdJsonError> {
    simd_json::to_vec(value).map_err(SimdJsonError::Simd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct BenchmarkModel {
        id: String,
        enabled: bool,
        rollout: u32,
    }

    #[test]
    fn test_simd_json_roundtrip() {
        let model = BenchmarkModel {
            id: "flag_search_v2".to_string(),
            enabled: true,
            rollout: 50,
        };

        let mut bytes = to_vec_simd(&model).unwrap();
        let parsed: BenchmarkModel = from_slice_simd(&mut bytes).unwrap();
        assert_eq!(model, parsed);
    }
}
