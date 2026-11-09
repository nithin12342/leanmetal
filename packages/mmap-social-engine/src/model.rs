//! src/domain/social/model.rs
//! File ID: FILE-011
//! Responsibility: Domain entities for benchmark social application (<=7 words)
//! Must Never: Depend on external persistence libraries or network transports.

use serde::{Deserialize, Serialize};

/// User account profile entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserProfile {
    pub id: u64,
    pub username: String,
    pub email: String,
    pub bio: String,
    pub posts_count: u64,
    pub created_at_ms: u64,
}

impl UserProfile {
    pub fn new(id: u64, username: impl Into<String>, email: impl Into<String>, bio: impl Into<String>) -> Self {
        Self {
            id,
            username: username.into(),
            email: email.into(),
            bio: bio.into(),
            posts_count: 0,
            created_at_ms: 1_700_000_000_000,
        }
    }
}

/// Raw stored post entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PostRecord {
    pub id: u64,
    pub author_id: u64,
    pub content: String,
    pub created_at_ms: u64,
    pub likes_count: u64,
}

/// Enriched post joined with author profile metadata.
/// Replicates `ORDER BY created_at DESC LIMIT 20` with author join in the benchmark.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EnrichedPost {
    pub id: u64,
    pub author_id: u64,
    pub author_username: String,
    pub content: String,
    pub created_at_ms: u64,
    pub likes_count: u64,
}

/// Request payload for creating a new post.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreatePostRequest {
    pub author_id: u64,
    pub content: String,
}

/// Query parameters for timeline pagination.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TimelineQuery {
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Paginated timeline response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TimelineResponse {
    pub posts: Vec<EnrichedPost>,
    pub count: usize,
}
