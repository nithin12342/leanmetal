//! src/domain/engine/hasher.rs
//! File ID: FILE-007
//! Responsibility: Compute nanosecond xxHash64 percentage buckets (<=7 words)
//! Must Never: Allocate dynamic heap memory or make system calls.

use xxhash_rust::xxh64::xxh64;

/// Computes a deterministic rollout bucket in the range [0, 99]
/// using 64-bit xxHash without dynamic heap allocations.
///
/// Formula: xxHash64(flag_key + ":" + user_id, seed = 0) % 100
#[inline(always)]
pub fn compute_bucket(flag_key: &str, user_id: &str) -> u8 {
    // Stack-allocated scratch buffer for keys under 128 bytes to prevent heap allocation
    const STACK_BUF_LEN: usize = 128;
    let total_len = flag_key.len() + 1 + user_id.len();

    if total_len <= STACK_BUF_LEN {
        let mut buf = [0u8; STACK_BUF_LEN];
        let k_len = flag_key.len();
        buf[..k_len].copy_from_slice(flag_key.as_bytes());
        buf[k_len] = b':';
        buf[k_len + 1..total_len].copy_from_slice(user_id.as_bytes());

        (xxh64(&buf[..total_len], 0) % 100) as u8
    } else {
        // Fallback for unusually long identifiers using incremental xxh64 state
        let mut hasher = xxhash_rust::xxh64::Xxh64::new(0);
        hasher.update(flag_key.as_bytes());
        hasher.update(b":");
        hasher.update(user_id.as_bytes());
        (hasher.digest() % 100) as u8
    }
}

/// Evaluates whether a given user falls within a rollout percentage [0..100]
#[inline(always)]
pub fn is_in_rollout(flag_key: &str, user_id: &str, percentage: u8) -> bool {
    if percentage == 0 {
        return false;
    }
    if percentage >= 100 {
        return true;
    }
    compute_bucket(flag_key, user_id) < percentage
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determinism() {
        let bucket1 = compute_bucket("checkout_v2", "usr_100293");
        let bucket2 = compute_bucket("checkout_v2", "usr_100293");
        assert_eq!(bucket1, bucket2, "Identical inputs must yield identical buckets");
        assert!(bucket1 < 100, "Bucket must be in range [0, 99]");
    }

    #[test]
    fn test_rollout_edges() {
        assert!(!is_in_rollout("feat", "any_user", 0));
        assert!(is_in_rollout("feat", "any_user", 100));
    }
}
