//! packages/mmap-social-engine/src/lib.rs
//! High-Throughput Multi-Tier Social Engine with Singleflight Anti-Stampede & Reverse-Chronological Ordering.

pub mod model;
pub mod traits;
pub mod engine;

pub use model::{CreatePostRequest, EnrichedPost, PostRecord, TimelineQuery, TimelineResponse, UserProfile};
pub use traits::SocialStore;
pub use engine::{SocialEngine, SocialEngineError};
