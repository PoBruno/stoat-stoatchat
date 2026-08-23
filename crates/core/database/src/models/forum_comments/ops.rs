use revolt_result::Result;

use crate::{FieldsForumComment, ForumComment, PartialForumComment};

#[cfg(feature = "mongodb")]
mod mongodb;
mod reference;

/// How a comment thread is ordered
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForumCommentSort {
    /// Highest score first
    Top,
    /// Oldest first, the natural reading order for a discussion
    Old,
    /// Newest first
    New,
}

#[async_trait]
pub trait AbstractForumComments: Sync + Send {
    /// Insert a new comment
    async fn insert_forum_comment(&self, comment: &ForumComment) -> Result<()>;

    /// Fetch a comment by id
    async fn fetch_forum_comment(&self, id: &str) -> Result<ForumComment>;

    /// Fetch every comment on a post.
    ///
    /// Deliberately unpaged: a thread here is small enough to hold in memory,
    /// and the client needs the whole set to assemble the tree anyway.
    async fn fetch_forum_comments(
        &self,
        post_id: &str,
        sort: ForumCommentSort,
    ) -> Result<Vec<ForumComment>>;

    /// Apply a partial update to a comment
    async fn update_forum_comment(
        &self,
        id: &str,
        partial: &PartialForumComment,
        remove: Vec<FieldsForumComment>,
    ) -> Result<()>;

    /// Delete a comment outright, used only when it has no replies
    async fn delete_forum_comment(&self, id: &str) -> Result<()>;

    /// Count comments on a post
    async fn count_forum_comments(&self, post_id: &str) -> Result<i32>;

    /// Whether any comment names this one as an ancestor
    async fn forum_comment_has_replies(&self, id: &str) -> Result<bool>;

    /// Register an upvote. Returns true when the vote was actually recorded.
    async fn upvote_forum_comment(&self, id: &str, user_id: &str) -> Result<bool>;

    /// Withdraw an upvote. Returns true when a vote was actually removed.
    async fn remove_forum_comment_upvote(&self, id: &str, user_id: &str) -> Result<bool>;

    /// Delete every comment on a post
    async fn delete_forum_comments_on_post(&self, post_id: &str) -> Result<()>;

    /// Delete every comment in a channel, used when the channel goes away
    async fn delete_forum_comments_in_channel(&self, channel_id: &str) -> Result<()>;
}
