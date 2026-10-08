//! src/domain/social/mod.rs
//! File ID: FILE-015
//! Responsibility: Social domain module root exports (<=7 words)
//! Must Never: Export private implementation details across module boundary.

pub mod engine;
pub mod model;
pub mod store;
pub mod traits;

pub use engine::{SocialEngine, SocialEngineError};
pub use model::{
    CreatePostRequest, EnrichedPost, PostRecord, TimelineQuery, TimelineResponse, UserProfile,
};
pub use store::{MmapSocialStore, SocialStorageError};
pub use traits::SocialStore;
