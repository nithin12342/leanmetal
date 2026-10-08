//! src/domain/social/traits.rs
//! File ID: FILE-012
//! Responsibility: Zero-cost generic traits for social storage (<=7 words)
//! Must Never: Use dynamic dispatch or vtables on the hot path.

use super::model::{EnrichedPost, PostRecord, UserProfile};

/// Zero-cost abstract interface for benchmark social storage.
///
/// Implementations must use static dispatch (monomorphization) to ensure
/// method calls are completely inlined by the compiler with zero vtable pointer dereferencing.
pub trait SocialStore: Send + Sync + 'static {
    type Error: std::error::Error + Send + Sync + 'static;

    /// Fetches a user profile by ID (single-row indexed lookup).
    fn get_user(&self, id: u64) -> Result<Option<UserProfile>, Self::Error>;

    /// Persists or updates a user profile.
    fn put_user(&self, user: &UserProfile) -> Result<(), Self::Error>;

    /// Creates and persists a new post, incrementing the author's post count.
    fn create_post(&self, author_id: u64, content: String) -> Result<PostRecord, Self::Error>;

    /// Retrieves an individual post by ID.
    fn get_post(&self, id: u64) -> Result<Option<PostRecord>, Self::Error>;

    /// Records a like on a post and increments its atomic like count.
    fn like_post(&self, post_id: u64, user_id: u64) -> Result<PostRecord, Self::Error>;

    /// Retrieves the global timeline feed with joined author metadata (`ORDER BY created_at DESC LIMIT 20`).
    fn get_timeline(&self, limit: usize, offset: usize) -> Result<Vec<EnrichedPost>, Self::Error>;

    /// Bulk seeds users, posts, and likes using batched transactions for benchmark initialization.
    fn bulk_seed(
        &self,
        users: &[UserProfile],
        posts: &[PostRecord],
        likes: &[(u64, u64)],
    ) -> Result<(), Self::Error>;
}
