//! src/domain/social/store.rs
//! File ID: FILE-013
//! Responsibility: LMDB mmap B+ tree social storage engine (<=7 words)
//! Must Never: Lock read operations or copy redundant buffers on read path.

use super::model::{EnrichedPost, PostRecord, UserProfile};
use super::traits::SocialStore;
use heed::byteorder::BigEndian;
use heed::types::{Bytes, U64};
use heed::{Database, Env, EnvOpenOptions};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, thiserror::Error)]
pub enum SocialStorageError {
    #[error("LMDB error: {0}")]
    Heed(#[from] heed::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("User with ID {0} not found")]
    UserNotFound(u64),
    #[error("Post with ID {0} not found")]
    PostNotFound(u64),
}

/// Zero-cost, memory-mapped social database engine backed by heed / LMDB.
#[derive(Clone)]
pub struct MmapSocialStore {
    env: Arc<Env>,
    users_db: Database<U64<BigEndian>, Bytes>,
    posts_timeline_db: Database<Bytes, Bytes>,
    posts_by_id_db: Database<U64<BigEndian>, Bytes>,
    seq_counter: Arc<AtomicU64>,
}

impl MmapSocialStore {
    /// Opens or initializes an LMDB social storage environment.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SocialStorageError> {
        let path = path.as_ref();
        if !path.exists() {
            fs::create_dir_all(path)?;
        }

        let env = unsafe {
            EnvOpenOptions::new()
                .map_size(10 * 1024 * 1024 * 1024) // 10 GB virtual address space
                .max_dbs(10)
                .open(path)?
        };

        let mut wtxn = env.write_txn()?;
        let users_db = env.create_database(&mut wtxn, Some("social_users_v1"))?;
        let posts_timeline_db = env.create_database(&mut wtxn, Some("social_posts_timeline_v1"))?;
        let posts_by_id_db = env.create_database(&mut wtxn, Some("social_posts_by_id_v1"))?;
        wtxn.commit()?;

        // Discover initial sequence ID by scanning highest existing post ID
        let rtxn = env.read_txn()?;
        let mut max_id = 0u64;
        let iter = posts_by_id_db.iter(&rtxn)?;
        for (id, _) in iter.flatten() {
            if id > max_id {
                max_id = id;
            }
        }
        drop(rtxn);

        Ok(Self {
            env: Arc::new(env),
            users_db,
            posts_timeline_db,
            posts_by_id_db,
            seq_counter: Arc::new(AtomicU64::new(max_id)),
        })
    }

    /// Generates a 16-byte reverse-chronological composite key:
    /// `[ (u64::MAX - timestamp_ms).to_be_bytes() (8 bytes) || post_id.to_be_bytes() (8 bytes) ]`
    #[inline(always)]
    fn make_timeline_key(timestamp_ms: u64, post_id: u64) -> [u8; 16] {
        let mut key = [0u8; 16];
        let inverted_time = u64::MAX - timestamp_ms;
        key[0..8].copy_from_slice(&inverted_time.to_be_bytes());
        key[8..16].copy_from_slice(&post_id.to_be_bytes());
        key
    }
}

impl SocialStore for MmapSocialStore {
    type Error = SocialStorageError;

    #[inline]
    fn get_user(&self, id: u64) -> Result<Option<UserProfile>, Self::Error> {
        let rtxn = self.env.read_txn()?;
        match self.users_db.get(&rtxn, &id)? {
            Some(bytes) => {
                let user: UserProfile = serde_json::from_slice(bytes)?;
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    #[inline]
    fn put_user(&self, user: &UserProfile) -> Result<(), Self::Error> {
        let serialized = serde_json::to_vec(user)?;
        let mut wtxn = self.env.write_txn()?;
        self.users_db.put(&mut wtxn, &user.id, &serialized)?;
        wtxn.commit()?;
        Ok(())
    }

    #[inline]
    fn create_post(&self, author_id: u64, content: String) -> Result<PostRecord, Self::Error> {
        let post_id = self.seq_counter.fetch_add(1, Ordering::SeqCst) + 1;
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let post = PostRecord {
            id: post_id,
            author_id,
            content,
            created_at_ms: now_ms,
            likes_count: 0,
        };

        let post_bytes = serde_json::to_vec(&post)?;
        let timeline_key = Self::make_timeline_key(now_ms, post_id);

        let mut wtxn = self.env.write_txn()?;

        // Verify author exists and increment user posts count
        if let Some(user_bytes) = self.users_db.get(&wtxn, &author_id)? {
            let mut user: UserProfile = serde_json::from_slice(user_bytes)?;
            user.posts_count += 1;
            let updated_user_bytes = serde_json::to_vec(&user)?;
            self.users_db.put(&mut wtxn, &author_id, &updated_user_bytes)?;
        } else {
            return Err(SocialStorageError::UserNotFound(author_id));
        }

        // Store post in reverse-chronological timeline index
        self.posts_timeline_db.put(&mut wtxn, &timeline_key, &post_bytes)?;
        // Store post in ID index
        self.posts_by_id_db.put(&mut wtxn, &post_id, &post_bytes)?;

        wtxn.commit()?;
        Ok(post)
    }

    #[inline]
    fn get_post(&self, id: u64) -> Result<Option<PostRecord>, Self::Error> {
        let rtxn = self.env.read_txn()?;
        match self.posts_by_id_db.get(&rtxn, &id)? {
            Some(bytes) => {
                let post: PostRecord = serde_json::from_slice(bytes)?;
                Ok(Some(post))
            }
            None => Ok(None),
        }
    }

    #[inline]
    fn like_post(&self, post_id: u64, _user_id: u64) -> Result<PostRecord, Self::Error> {
        let mut wtxn = self.env.write_txn()?;
        let post_bytes = self
            .posts_by_id_db
            .get(&wtxn, &post_id)?
            .ok_or(SocialStorageError::PostNotFound(post_id))?;

        let mut post: PostRecord = serde_json::from_slice(post_bytes)?;
        post.likes_count += 1;

        let updated_bytes = serde_json::to_vec(&post)?;
        self.posts_by_id_db.put(&mut wtxn, &post_id, &updated_bytes)?;

        // Update in timeline index as well
        let timeline_key = Self::make_timeline_key(post.created_at_ms, post.id);
        self.posts_timeline_db.put(&mut wtxn, &timeline_key, &updated_bytes)?;

        wtxn.commit()?;
        Ok(post)
    }

    #[inline]
    fn get_timeline(&self, limit: usize, offset: usize) -> Result<Vec<EnrichedPost>, Self::Error> {
        let rtxn = self.env.read_txn()?;
        let mut results = Vec::with_capacity(limit);

        // B+ tree iteration: forward iteration over inverted keys is naturally reverse-chronological!
        let iter = self.posts_timeline_db.iter(&rtxn)?;
        let mut skipped = 0;

        for item in iter {
            let (_key, val_bytes) = item?;
            if skipped < offset {
                skipped += 1;
                continue;
            }

            let post: PostRecord = serde_json::from_slice(val_bytes)?;

            // Join author username from users_db
            let author_username = match self.users_db.get(&rtxn, &post.author_id)? {
                Some(u_bytes) => {
                    let user: UserProfile = serde_json::from_slice(u_bytes)?;
                    user.username
                }
                None => "unknown".to_string(),
            };

            results.push(EnrichedPost {
                id: post.id,
                author_id: post.author_id,
                author_username,
                content: post.content,
                created_at_ms: post.created_at_ms,
                likes_count: post.likes_count,
            });

            if results.len() >= limit {
                break;
            }
        }

        Ok(results)
    }

    fn bulk_seed(
        &self,
        users: &[UserProfile],
        posts: &[PostRecord],
        _likes: &[(u64, u64)],
    ) -> Result<(), Self::Error> {
        let mut wtxn = self.env.write_txn()?;
        for u in users {
            let bytes = serde_json::to_vec(u)?;
            self.users_db.put(&mut wtxn, &u.id, &bytes)?;
        }
        for p in posts {
            let bytes = serde_json::to_vec(p)?;
            let tkey = Self::make_timeline_key(p.created_at_ms, p.id);
            self.posts_timeline_db.put(&mut wtxn, &tkey, &bytes)?;
            self.posts_by_id_db.put(&mut wtxn, &p.id, &bytes)?;
        }
        wtxn.commit()?;
        Ok(())
    }
}
