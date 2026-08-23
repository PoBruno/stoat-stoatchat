use bson::{doc, Document};
use futures::StreamExt;
use mongodb::options::FindOptions;
use revolt_result::Result;

use crate::{
    FieldsForumComment, ForumComment, ForumCommentSort, IntoDocumentPath, MongoDb,
    PartialForumComment,
};

use super::AbstractForumComments;

static COL: &str = "forum_comments";

/// Ordering for each thread sort. Always ends in `_id` so ties are stable.
fn sort_document(sort: ForumCommentSort) -> Document {
    match sort {
        ForumCommentSort::Top => doc! { "score": -1, "_id": 1 },
        ForumCommentSort::Old => doc! { "_id": 1 },
        ForumCommentSort::New => doc! { "_id": -1 },
    }
}

#[async_trait]
impl AbstractForumComments for MongoDb {
    async fn insert_forum_comment(&self, comment: &ForumComment) -> Result<()> {
        query!(self, insert_one, COL, comment).map(|_| ())
    }

    async fn fetch_forum_comment(&self, id: &str) -> Result<ForumComment> {
        query!(self, find_one_by_id, COL, id)?.ok_or_else(|| create_error!(NotFound))
    }

    async fn fetch_forum_comments(
        &self,
        post_id: &str,
        sort: ForumCommentSort,
    ) -> Result<Vec<ForumComment>> {
        Ok(self
            .col::<ForumComment>(COL)
            .find(doc! { "post": post_id })
            .with_options(FindOptions::builder().sort(sort_document(sort)).build())
            .await
            .map_err(|_| create_database_error!("find", COL))?
            .filter_map(|s| async { s.ok() })
            .collect()
            .await)
    }

    async fn update_forum_comment(
        &self,
        id: &str,
        partial: &PartialForumComment,
        remove: Vec<FieldsForumComment>,
    ) -> Result<()> {
        query!(
            self,
            update_one_by_id,
            COL,
            id,
            partial,
            remove.iter().map(|x| x as &dyn IntoDocumentPath).collect(),
            None
        )
        .map(|_| ())
    }

    async fn delete_forum_comment(&self, id: &str) -> Result<()> {
        query!(self, delete_one_by_id, COL, id).map(|_| ())
    }

    async fn count_forum_comments(&self, post_id: &str) -> Result<i32> {
        self.col::<Document>(COL)
            .count_documents(doc! { "post": post_id })
            .await
            .map(|n| n as i32)
            .map_err(|_| create_database_error!("count_documents", COL))
    }

    async fn forum_comment_has_replies(&self, id: &str) -> Result<bool> {
        self.col::<Document>(COL)
            .count_documents(doc! { "ancestors": id })
            .await
            .map(|n| n > 0)
            .map_err(|_| create_database_error!("count_documents", COL))
    }

    async fn upvote_forum_comment(&self, id: &str, user_id: &str) -> Result<bool> {
        // Same guard as posts: the filter is what stops score drifting.
        let result = self
            .col::<Document>(COL)
            .update_one(
                doc! { "_id": id, "upvoters": { "$ne": user_id } },
                doc! { "$addToSet": { "upvoters": user_id }, "$inc": { "score": 1 } },
            )
            .await
            .map_err(|_| create_database_error!("update_one", COL))?;

        Ok(result.modified_count > 0)
    }

    async fn remove_forum_comment_upvote(&self, id: &str, user_id: &str) -> Result<bool> {
        let result = self
            .col::<Document>(COL)
            .update_one(
                doc! { "_id": id, "upvoters": user_id },
                doc! { "$pull": { "upvoters": user_id }, "$inc": { "score": -1 } },
            )
            .await
            .map_err(|_| create_database_error!("update_one", COL))?;

        Ok(result.modified_count > 0)
    }

    async fn delete_forum_comments_on_post(&self, post_id: &str) -> Result<()> {
        self.col::<Document>(COL)
            .delete_many(doc! { "post": post_id })
            .await
            .map(|_| ())
            .map_err(|_| create_database_error!("delete_many", COL))
    }

    async fn delete_forum_comments_in_channel(&self, channel_id: &str) -> Result<()> {
        self.col::<Document>(COL)
            .delete_many(doc! { "channel": channel_id })
            .await
            .map(|_| ())
            .map_err(|_| create_database_error!("delete_many", COL))
    }
}

impl IntoDocumentPath for FieldsForumComment {
    fn as_path(&self) -> Option<&'static str> {
        Some(match self {
            FieldsForumComment::Attachments => "attachments",
        })
    }
}
