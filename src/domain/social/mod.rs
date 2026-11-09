//! src/domain/social/mod.rs
//! File ID: FILE-015
//! Responsibility: Social domain module root exports (<=7 words)
//! Must Never: Export private implementation details across module boundary.

pub mod arcswap_timeline;
pub mod engine;
pub mod io_uring_wal;
pub mod model;
pub mod store;
pub mod traits;
pub mod zero_copy;

pub use arcswap_timeline::ArcSwapTimeline;
pub use engine::{SocialEngine, SocialEngineError};
pub use io_uring_wal::IoUringWalCommitter;
pub use model::{
    CreatePostRequest, EnrichedPost, PostRecord, TimelineQuery, TimelineResponse, UserProfile,
};
pub use store::{MmapSocialStore, SocialStorageError};
pub use traits::SocialStore;
pub use zero_copy::{DensePostRecord, InvertedTimelineKey};
