use revolt_database::util::permissions::DatabasePermissionQuery;
use revolt_database::util::reference::Reference;
use revolt_database::{Channel, Database, User};
use revolt_models::v0;
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;

/// # Follow Forum Post
///
/// Follow a post to keep track of it.
#[openapi(tag = "Forum")]
#[put("/<target>/posts/<post>/subscribe")]
pub async fn subscribe_forum_post(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
) -> Result<Json<v0::ForumPost>> {
    set_subscription(db, user, target, post, true).await
}

/// # Unfollow Forum Post
#[openapi(tag = "Forum")]
#[delete("/<target>/posts/<post>/subscribe")]
pub async fn unsubscribe_forum_post(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
) -> Result<Json<v0::ForumPost>> {
    set_subscription(db, user, target, post, false).await
}

/// Shared body for both directions
async fn set_subscription(
    db: &Database,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
    subscribed: bool,
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
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::ReadMessageHistory)?;

    db.set_forum_post_subscribed(&existing.id, &user.id, subscribed)
        .await?;

    let post = db.fetch_forum_post(&existing.id).await?;
    Ok(Json(post.into()))
}
