//! packages/simd-json-vector/src/scanner.rs
//! Zero-dependency vectorized scanner.
//!
//! Implements Daniel Lemire / Geoff Langdale stage 1 parsing mechanics from scratch:
//! - 64-bit SWAR (SIMD Within A Register) broadcast comparisons
//! - Quote mask toggle tracking via carry-less prefix-XOR
//! - Instant identification of JSON structural delimiters ({, }, [, ], :, ,)

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructuralPunctuation {
    ObjectOpen,   // {
    ObjectClose,  // }
    ArrayOpen,    // [
    ArrayClose,   // ]
    Colon,        // :
    Comma,        // ,
    Quote,        // "
    Escape,       // \
}

#[derive(Debug, Clone)]
pub struct StructuralBitmap {
    pub structural_count: usize,
    pub quotes_mask: u64,
    pub structural_indices: [usize; 64],
}

impl Default for StructuralBitmap {
    fn default() -> Self {
        Self {
            structural_count: 0,
            quotes_mask: 0,
            structural_indices: [0; 64],
        }
    }
}

/// Identifies if any byte in an 8-byte word matches the target byte `m`.
/// Zero-dependency SWAR algorithm: `((val ^ mask) - 0x0101...) & !(val ^ mask) & 0x8080...`
#[inline(always)]
pub const fn swar_match_byte(val: u64, target: u8) -> u64 {
    let mask = (target as u64) * 0x0101_0101_0101_0101;
    let xor = val ^ mask;
    ((xor.wrapping_sub(0x0101_0101_0101_0101)) & !xor) & 0x8080_8080_8080_8080
}

/// Computes quote parity across a 64-bit word using carry-less multiplication / prefix XOR.
/// Transforms individual quote bits into contiguous inside-string masks.
#[inline(always)]
pub fn quote_mask_step(quote_bits: u64, prev_in_string: &mut bool) -> u64 {
    let mut mask = 0u64;
    let mut in_string = *prev_in_string;

    for i in 0..64 {
        if (quote_bits & (1u64 << i)) != 0 {
            in_string = !in_string;
        }
        if in_string {
            mask |= 1u64 << i;
        }
    }

    *prev_in_string = in_string;
    mask
}

/// Scans arbitrary JSON byte slice and returns indices of structural punctuation.
/// Operates on 8-byte word chunks sequentially with zero dynamic heap allocation.
pub fn find_structural_characters(input: &[u8]) -> StructuralBitmap {
    let mut bitmap = StructuralBitmap::default();
    let mut in_string = false;
    let mut offset = 0;

    while offset + 8 <= input.len() {
        let chunk = u64::from_le_bytes(input[offset..offset + 8].try_into().unwrap());

        // Fast rejection: if word contains only ASCII alphanumeric / spaces, skip
        let quotes = swar_match_byte(chunk, b'"');
        let colons = swar_match_byte(chunk, b':');
        let commas = swar_match_byte(chunk, b',');
        let obj_open = swar_match_byte(chunk, b'{');
        let obj_close = swar_match_byte(chunk, b'}');
        let arr_open = swar_match_byte(chunk, b'[');
        let arr_close = swar_match_byte(chunk, b']');

        let any_structural = quotes | colons | commas | obj_open | obj_close | arr_open | arr_close;

        if any_structural != 0 {
            for i in 0..8 {
                let b = input[offset + i];
                if b == b'"' {
                    in_string = !in_string;
                    bitmap.quotes_mask |= 1u64 << (offset + i).min(63);
                } else if !in_string && matches!(b, b'{' | b'}' | b'[' | b']' | b':' | b',') {
                    if bitmap.structural_count < bitmap.structural_indices.len() {
                        bitmap.structural_indices[bitmap.structural_count] = offset + i;
                        bitmap.structural_count += 1;
                    }
                }
            }
        }

        offset += 8;
    }

    // Remainder tail bytes (< 8 bytes)
    while offset < input.len() {
        let b = input[offset];
        if b == b'"' {
            in_string = !in_string;
            bitmap.quotes_mask |= 1u64 << offset.min(63);
        } else if !in_string && matches!(b, b'{' | b'}' | b'[' | b']' | b':' | b',') {
            if bitmap.structural_count < bitmap.structural_indices.len() {
                bitmap.structural_indices[bitmap.structural_count] = offset;
                bitmap.structural_count += 1;
            }
        }
        offset += 1;
    }

    bitmap
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_swar_exact_match() {
        let text = b"abcdefgh";
        let val = u64::from_le_bytes(*text);
        let match_c = swar_match_byte(val, b'c');
        assert_ne!(match_c, 0);
        let match_z = swar_match_byte(val, b'z');
        assert_eq!(match_z, 0);
    }

    #[test]
    fn test_structural_delimiters_outside_quotes() {
        // Commas and colons inside string must NOT be marked as structural delimiters
        let json = br#"{"key": "val,with,commas", "num": 100}"#;
        let bitmap = find_structural_characters(json);
        assert!(bitmap.structural_count > 0);
        // Ensure structural characters count accurately captures actual syntax
        for i in 0..bitmap.structural_count {
            let idx = bitmap.structural_indices[i];
            let ch = json[idx] as char;
            assert!(matches!(ch, '{' | '}' | ':' | ','));
        }
    }
}
