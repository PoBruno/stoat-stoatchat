use revolt_result::Result;

use crate::{
    FieldsForumComment, ForumComment, ForumCommentSort, PartialForumComment, ReferenceDb,
};

use super::AbstractForumComments;

#[async_trait]
impl AbstractForumComments for ReferenceDb {
    async fn insert_forum_comment(&self, comment: &ForumComment) -> Result<()> {
        let mut comments = self.forum_comments.lock().await;
        if comments.contains_key(&comment.id) {
            Err(create_database_error!("insert", "forum_comment"))
        } else {
            comments.insert(comment.id.to_string(), comment.clone());
            Ok(())
        }
    }

    async fn fetch_forum_comment(&self, id: &str) -> Result<ForumComment> {
        let comments = self.forum_comments.lock().await;
        comments
            .get(id)
            .cloned()
            .ok_or_else(|| create_error!(NotFound))
    }

    async fn fetch_forum_comments(
        &self,
        post_id: &str,
        sort: ForumCommentSort,
    ) -> Result<Vec<ForumComment>> {
        let comments = self.forum_comments.lock().await;

        let mut found: Vec<ForumComment> = comments
            .values()
            .filter(|c| c.post == post_id)
            .cloned()
            .collect();

        // Mirror the Mongo ordering so tests behave the same on both drivers.
        match sort {
            ForumCommentSort::Top => {
                found.sort_by(|a, b| b.score.cmp(&a.score).then(a.id.cmp(&b.id)))
            }
            ForumCommentSort::Old => found.sort_by(|a, b| a.id.cmp(&b.id)),
            ForumCommentSort::New => found.sort_by(|a, b| b.id.cmp(&a.id)),
        }

        Ok(found)
    }

    async fn update_forum_comment(
        &self,
        id: &str,
        partial: &PartialForumComment,
        remove: Vec<FieldsForumComment>,
    ) -> Result<()> {
        let mut comments = self.forum_comments.lock().await;
        if let Some(comment) = comments.get_mut(id) {
            for field in remove {
                #[allow(clippy::single_match)]
                match field {
                    FieldsForumComment::Attachments => comment.attachments = None,
                }
            }
            comment.apply_options(partial.clone());
            Ok(())
        } else {
            Err(create_error!(NotFound))
        }
    }

    async fn delete_forum_comment(&self, id: &str) -> Result<()> {
        let mut comments = self.forum_comments.lock().await;
        comments
            .remove(id)
            .map(|_| ())
            .ok_or_else(|| create_error!(NotFound))
    }

    async fn count_forum_comments(&self, post_id: &str) -> Result<i32> {
        let comments = self.forum_comments.lock().await;
        Ok(comments.values().filter(|c| c.post == post_id).count() as i32)
    }

    async fn forum_comment_has_replies(&self, id: &str) -> Result<bool> {
        let comments = self.forum_comments.lock().await;
        Ok(comments
            .values()
            .any(|c| c.ancestors.iter().any(|a| a == id)))
    }

    async fn upvote_forum_comment(&self, id: &str, user_id: &str) -> Result<bool> {
        let mut comments = self.forum_comments.lock().await;
        if let Some(comment) = comments.get_mut(id) {
            if comment.upvoters.iter().any(|u| u == user_id) {
                return Ok(false);
            }
            comment.upvoters.push(user_id.to_string());
            comment.score += 1;
            Ok(true)
        } else {
            Err(create_error!(NotFound))
        }
    }

    async fn remove_forum_comment_upvote(&self, id: &str, user_id: &str) -> Result<bool> {
        let mut comments = self.forum_comments.lock().await;
        if let Some(comment) = comments.get_mut(id) {
            let before = comment.upvoters.len();
            comment.upvoters.retain(|u| u != user_id);
            if comment.upvoters.len() == before {
                return Ok(false);
            }
            comment.score -= 1;
            Ok(true)
        } else {
            Err(create_error!(NotFound))
        }
    }

    async fn delete_forum_comments_on_post(&self, post_id: &str) -> Result<()> {
        let mut comments = self.forum_comments.lock().await;
        comments.retain(|_, c| c.post != post_id);
        Ok(())
    }

    async fn delete_forum_comments_in_channel(&self, channel_id: &str) -> Result<()> {
        let mut comments = self.forum_comments.lock().await;
        comments.retain(|_, c| c.channel != channel_id);
        Ok(())
    }
}
