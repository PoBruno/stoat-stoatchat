use revolt_result::Result;

use crate::{
    FieldsForumPost, ForumFeedQuery, ForumFeedSort, ForumPost, PartialForumPost, ReferenceDb,
};

use super::AbstractForumPosts;

#[async_trait]
impl AbstractForumPosts for ReferenceDb {
    async fn insert_forum_post(&self, post: &ForumPost) -> Result<()> {
        let mut posts = self.forum_posts.lock().await;
        if posts.contains_key(&post.id) {
            Err(create_database_error!("insert", "forum_post"))
        } else {
            posts.insert(post.id.to_string(), post.clone());
            Ok(())
        }
    }

    async fn fetch_forum_post(&self, id: &str) -> Result<ForumPost> {
        let posts = self.forum_posts.lock().await;
        posts.get(id).cloned().ok_or_else(|| create_error!(NotFound))
    }

    async fn fetch_forum_posts(&self, query: &ForumFeedQuery) -> Result<Vec<ForumPost>> {
        let posts = self.forum_posts.lock().await;

        let mut found: Vec<ForumPost> = posts
            .values()
            .filter(|p| p.channel == query.channel)
            .filter(|p| match &query.tag {
                Some(tag) => p.tags.contains(tag),
                None => true,
            })
            .filter(|p| match &query.after {
                Some(after) => &p.id < after,
                None => true,
            })
            .cloned()
            .collect();

        // Mirror the Mongo ordering so tests behave the same on both drivers.
        found.sort_by(|a, b| {
            b.pinned
                .cmp(&a.pinned)
                .then_with(|| match query.sort {
                    ForumFeedSort::Hot => b.hot_rank.cmp(&a.hot_rank),
                    ForumFeedSort::New => std::cmp::Ordering::Equal,
                    ForumFeedSort::Top => b.score.cmp(&a.score),
                    ForumFeedSort::Active => b.last_comment_at.cmp(&a.last_comment_at),
                })
                .then_with(|| b.id.cmp(&a.id))
        });

        found.truncate(query.limit.max(0) as usize);
        Ok(found)
    }

    async fn update_forum_post(
        &self,
        id: &str,
        partial: &PartialForumPost,
        remove: Vec<FieldsForumPost>,
    ) -> Result<()> {
        let mut posts = self.forum_posts.lock().await;
        let post = posts.get_mut(id).ok_or_else(|| create_error!(NotFound))?;

        for field in remove {
            match field {
                FieldsForumPost::Content => post.content = None,
                FieldsForumPost::Attachments => post.attachments = None,
            }
        }

        post.apply_options(partial.clone());
        Ok(())
    }

    async fn delete_forum_post(&self, id: &str) -> Result<()> {
        let mut posts = self.forum_posts.lock().await;
        posts.remove(id);
        Ok(())
    }

    async fn upvote_forum_post(&self, id: &str, user_id: &str) -> Result<bool> {
        let mut posts = self.forum_posts.lock().await;
        let post = posts.get_mut(id).ok_or_else(|| create_error!(NotFound))?;

        if post.upvoters.iter().any(|u| u == user_id) {
            return Ok(false);
        }

        post.upvoters.push(user_id.to_string());
        post.score += 1;
        Ok(true)
    }

    async fn remove_forum_post_upvote(&self, id: &str, user_id: &str) -> Result<bool> {
        let mut posts = self.forum_posts.lock().await;
        let post = posts.get_mut(id).ok_or_else(|| create_error!(NotFound))?;

        let before = post.upvoters.len();
        post.upvoters.retain(|u| u != user_id);

        if post.upvoters.len() == before {
            return Ok(false);
        }

        post.score -= 1;
        Ok(true)
    }

    async fn set_forum_post_hot_rank(&self, id: &str, hot_rank: i64) -> Result<()> {
        let mut posts = self.forum_posts.lock().await;
        let post = posts.get_mut(id).ok_or_else(|| create_error!(NotFound))?;
        post.hot_rank = hot_rank;
        Ok(())
    }

    async fn mark_forum_post_viewed(&self, id: &str, user_id: &str) -> Result<()> {
        let mut posts = self.forum_posts.lock().await;
        if let Some(post) = posts.get_mut(id) {
            if !post.viewers.iter().any(|u| u == user_id) {
                post.viewers.push(user_id.to_string());
            }
            Ok(())
        } else {
            Err(create_error!(NotFound))
        }
    }

    async fn set_forum_post_subscribed(
        &self,
        id: &str,
        user_id: &str,
        subscribed: bool,
    ) -> Result<()> {
        let mut posts = self.forum_posts.lock().await;
        if let Some(post) = posts.get_mut(id) {
            post.subscribers.retain(|u| u != user_id);
            if subscribed {
                post.subscribers.push(user_id.to_string());
            }
            Ok(())
        } else {
            Err(create_error!(NotFound))
        }
    }

    async fn fetch_forum_posts_within_hot_window(&self) -> Result<Vec<ForumPost>> {
        let posts = self.forum_posts.lock().await;
        Ok(posts.values().cloned().collect())
    }

    async fn delete_forum_posts_in_channel(&self, channel_id: &str) -> Result<()> {
        let mut posts = self.forum_posts.lock().await;
        posts.retain(|_, p| p.channel != channel_id);
        Ok(())
    }
}
