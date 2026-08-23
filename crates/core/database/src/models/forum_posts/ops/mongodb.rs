use bson::{doc, Document};
use futures::StreamExt;
use mongodb::options::FindOptions;
use revolt_result::Result;

use crate::{
    FieldsForumPost, ForumFeedQuery, ForumFeedSort, ForumPost, IntoDocumentPath, MongoDb,
    PartialForumPost,
};

use super::AbstractForumPosts;

static COL: &str = "forum_posts";

/// Ordering for each feed sort.
///
/// Every one ends in `_id` so paging is by cursor, never by skip. Pinned posts
/// always float to the top.
fn sort_document(sort: ForumFeedSort) -> Document {
    match sort {
        ForumFeedSort::Hot => doc! { "pinned": -1, "hot_rank": -1, "_id": -1 },
        ForumFeedSort::New => doc! { "pinned": -1, "_id": -1 },
        ForumFeedSort::Top => doc! { "pinned": -1, "score": -1, "_id": -1 },
        ForumFeedSort::Active => doc! { "pinned": -1, "last_comment_at": -1, "_id": -1 },
    }
}

#[async_trait]
impl AbstractForumPosts for MongoDb {
    async fn insert_forum_post(&self, post: &ForumPost) -> Result<()> {
        query!(self, insert_one, COL, post).map(|_| ())
    }

    async fn fetch_forum_post(&self, id: &str) -> Result<ForumPost> {
        query!(self, find_one_by_id, COL, id)?.ok_or_else(|| create_error!(NotFound))
    }

    async fn fetch_forum_posts(&self, query: &ForumFeedQuery) -> Result<Vec<ForumPost>> {
        let mut filter = doc! { "channel": &query.channel };

        if let Some(tag) = &query.tag {
            filter.insert("tags", tag);
        }

        // Cursor paging: for id-ordered feeds this is exact. For the ranked
        // feeds it is approximate, which is fine at this instance's volume -
        // a feed never spans more than a page or two.
        if let Some(after) = &query.after {
            filter.insert("_id", doc! { "$lt": after });
        }

        Ok(self
            .col::<ForumPost>(COL)
            .find(filter)
            .with_options(
                FindOptions::builder()
                    .sort(sort_document(query.sort))
                    .limit(query.limit)
                    .build(),
            )
            .await
            .map_err(|_| create_database_error!("find", COL))?
            .filter_map(|s| async { s.ok() })
            .collect()
            .await)
    }

    async fn update_forum_post(
        &self,
        id: &str,
        partial: &PartialForumPost,
        remove: Vec<FieldsForumPost>,
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

    async fn delete_forum_post(&self, id: &str) -> Result<()> {
        query!(self, delete_one_by_id, COL, id).map(|_| ())
    }

    async fn upvote_forum_post(&self, id: &str, user_id: &str) -> Result<bool> {
        // The `$ne` guard is what keeps `score` honest: without it a repeat
        // vote would no-op on $addToSet while still running $inc.
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

    async fn remove_forum_post_upvote(&self, id: &str, user_id: &str) -> Result<bool> {
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

    async fn set_forum_post_hot_rank(&self, id: &str, hot_rank: i64) -> Result<()> {
        self.col::<Document>(COL)
            .update_one(doc! { "_id": id }, doc! { "$set": { "hot_rank": hot_rank } })
            .await
            .map(|_| ())
            .map_err(|_| create_database_error!("update_one", COL))
    }

    async fn mark_forum_post_viewed(&self, id: &str, user_id: &str) -> Result<()> {
        self.col::<Document>(COL)
            .update_one(
                doc! { "_id": id },
                doc! { "$addToSet": { "viewers": user_id } },
            )
            .await
            .map(|_| ())
            .map_err(|_| create_database_error!("update_one", COL))
    }

    async fn set_forum_post_subscribed(
        &self,
        id: &str,
        user_id: &str,
        subscribed: bool,
    ) -> Result<()> {
        let change = if subscribed {
            doc! { "$addToSet": { "subscribers": user_id } }
        } else {
            doc! { "$pull": { "subscribers": user_id } }
        };

        self.col::<Document>(COL)
            .update_one(doc! { "_id": id }, change)
            .await
            .map(|_| ())
            .map_err(|_| create_database_error!("update_one", COL))
    }

    async fn fetch_forum_posts_within_hot_window(&self) -> Result<Vec<ForumPost>> {
        // Ids are ULIDs, so the id itself carries the timestamp: a bound on
        // _id is a bound on creation time, and it uses the primary index.
        let cutoff = ulid::Ulid::from_parts(
            (iso8601_timestamp::Timestamp::now_utc()
                .duration_since(iso8601_timestamp::Timestamp::UNIX_EPOCH)
                .whole_milliseconds() as u64)
                .saturating_sub(7 * 24 * 60 * 60 * 1000),
            0,
        )
        .to_string();

        Ok(self
            .col::<ForumPost>(COL)
            .find(doc! { "_id": { "$gte": cutoff } })
            .await
            .map_err(|_| create_database_error!("find", COL))?
            .filter_map(|s| async { s.ok() })
            .collect()
            .await)
    }

    async fn delete_forum_posts_in_channel(&self, channel_id: &str) -> Result<()> {
        self.col::<Document>(COL)
            .delete_many(doc! { "channel": channel_id })
            .await
            .map(|_| ())
            .map_err(|_| create_database_error!("delete_many", COL))
    }
}

impl IntoDocumentPath for FieldsForumPost {
    fn as_path(&self) -> Option<&'static str> {
        Some(match self {
            FieldsForumPost::Content => "content",
            FieldsForumPost::Attachments => "attachments",
        })
    }
}
