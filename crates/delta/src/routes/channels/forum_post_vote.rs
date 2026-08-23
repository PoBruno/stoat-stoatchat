use revolt_database::util::permissions::DatabasePermissionQuery;
use revolt_database::util::reference::Reference;
use revolt_database::{Channel, Database, User};
use revolt_models::v0;
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;

/// # Upvote Forum Post
///
/// Add the current user's upvote. Idempotent: voting twice is a no-op.
#[openapi(tag = "Forum")]
#[put("/<target>/posts/<post>/upvote")]
pub async fn upvote_forum_post(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
) -> Result<Json<v0::ForumPost>> {
    vote(db, user, target, post, true).await
}

/// # Remove Forum Post Upvote
///
/// Withdraw the current user's upvote. Idempotent.
#[openapi(tag = "Forum")]
#[delete("/<target>/posts/<post>/upvote")]
pub async fn remove_forum_post_upvote(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
) -> Result<Json<v0::ForumPost>> {
    vote(db, user, target, post, false).await
}

/// Shared body for both vote directions
async fn vote(
    db: &Database,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
    add: bool,
) -> Result<Json<v0::ForumPost>> {
    let channel = target.as_channel(db).await?;
    if !matches!(&channel, Channel::ForumChannel { .. }) {
        return Err(create_error!(InvalidOperation));
    }

    let existing = db.fetch_forum_post(&post.id).await?;
    if existing.channel != channel.id() {
        return Err(create_error!(NotFound));
    }

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    let permissions = calculate_channel_permissions(&mut query).await;
    // Voting is a read-side interaction: seeing the forum is enough.
    permissions.throw_if_lacking_channel_permission(ChannelPermission::ReadMessageHistory)?;

    if existing.locked {
        return Err(create_error!(InvalidOperation));
    }

    if add {
        db.upvote_forum_post(&existing.id, &user.id).await?;
    } else {
        db.remove_forum_post_upvote(&existing.id, &user.id).await?;
    }

    // Re-read so the response carries the authoritative score.
    let post = db.fetch_forum_post(&existing.id).await?;
    Ok(Json(post.into()))
}
