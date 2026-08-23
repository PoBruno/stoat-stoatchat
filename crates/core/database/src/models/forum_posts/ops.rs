use revolt_result::Result;

use crate::{ForumPost, PartialForumPost};

#[cfg(feature = "mongodb")]
mod mongodb;
mod reference;

/// How a forum feed is ordered
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumFeedSort {
    /// Score decayed by age
    Hot,
    /// Most recently created
    New,
    /// Highest score
    Top,
    /// Most recently commented on
    Active,
}

/// Query for a forum feed
#[derive(Debug, Clone)]
pub struct ForumFeedQuery {
    /// Channel to read from
    pub channel: String,
    /// Ordering
    pub sort: ForumFeedSort,
    /// Only posts carrying this tag
    pub tag: Option<String>,
    /// Maximum posts to return
    pub limit: i64,
    /// Cursor: return posts following this id in the current ordering
    pub after: Option<String>,
}

#[async_trait]
pub trait AbstractForumPosts: Sync + Send {
    /// Insert a new post
    async fn insert_forum_post(&self, post: &ForumPost) -> Result<()>;

    /// Fetch a post by id
    async fn fetch_forum_post(&self, id: &str) -> Result<ForumPost>;

    /// Fetch a feed of posts
    async fn fetch_forum_posts(&self, query: &ForumFeedQuery) -> Result<Vec<ForumPost>>;

    /// Apply a partial update to a post
    async fn update_forum_post(
        &self,
        id: &str,
        partial: &PartialForumPost,
        remove: Vec<crate::FieldsForumPost>,
    ) -> Result<()>;

    /// Delete a post outright
    async fn delete_forum_post(&self, id: &str) -> Result<()>;

    /// Register an upvote.
    ///
    /// Returns true when the vote was actually recorded; false when the user
    /// had already voted. The guard lives in the query filter so the counter
    /// can never drift from the voter list.
    async fn upvote_forum_post(&self, id: &str, user_id: &str) -> Result<bool>;

    /// Withdraw an upvote. Returns true when a vote was actually removed.
    async fn remove_forum_post_upvote(&self, id: &str, user_id: &str) -> Result<bool>;

    /// Store a freshly computed hot rank
    async fn set_forum_post_hot_rank(&self, id: &str, hot_rank: i64) -> Result<()>;

    /// Record that a user opened this post. Idempotent.
    async fn mark_forum_post_viewed(&self, id: &str, user_id: &str) -> Result<()>;

    /// Follow or unfollow a post
    async fn set_forum_post_subscribed(
        &self,
        id: &str,
        user_id: &str,
        subscribed: bool,
    ) -> Result<()>;

    /// Posts young enough for `hot_rank` to still be changing.
    ///
    /// Bounded by the same seven day window the ranking uses, so the periodic
    /// recompute stays O(recent posts) rather than O(all posts).
    async fn fetch_forum_posts_within_hot_window(&self) -> Result<Vec<ForumPost>>;

    /// Delete every post in a channel, used when the channel goes away
    async fn delete_forum_posts_in_channel(&self, channel_id: &str) -> Result<()>;
}
