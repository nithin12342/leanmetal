use moka::sync::Cache;
use std::sync::Arc;
use std::time::Duration;

pub struct MokaL1Cache {
    cache: Cache<String, Arc<[u8]>>,
}

impl Default for MokaL1Cache {
    fn default() -> Self {
        Self::new(50_000, Duration::from_secs(300))
    }
}

impl MokaL1Cache {
    pub fn new(max_capacity: u64, ttl: Duration) -> Self {
        let cache = Cache::builder()
            .max_capacity(max_capacity)
            .time_to_live(ttl)
            .build();
        Self { cache }
    }

    #[inline(always)]
    pub fn get(&self, key: &str) -> Option<Arc<[u8]>> {
        self.cache.get(key)
    }

    #[inline(always)]
    pub fn insert(&self, key: String, val: Arc<[u8]>) {
        self.cache.insert(key, val);
    }

    #[inline(always)]
    pub fn invalidate(&self, key: &str) {
        self.cache.invalidate(key);
    }
}
