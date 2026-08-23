use revolt_database::util::permissions::DatabasePermissionQuery;
use revolt_database::util::reference::Reference;
use revolt_database::{Channel, Database, User};
use revolt_models::v0;
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;

/// # Fetch Forum Post
///
/// Fetch a single post by id.
#[openapi(tag = "Forum")]
#[get("/<target>/posts/<post>")]
pub async fn fetch_forum_post(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
) -> Result<Json<v0::ForumPost>> {
    let channel = target.as_channel(db).await?;

    if !matches!(&channel, Channel::ForumChannel { .. }) {
        return Err(create_error!(InvalidOperation));
    }

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::ReadMessageHistory)?;

    let existing = db.fetch_forum_post(&post.id).await?;

    // Guard against reading a post by id from a channel it does not belong to.
    if existing.channel != channel.id() {
        return Err(create_error!(NotFound));
    }

    // Opening the post is what counts as a view. Recorded as a set, so a
    // refresh does not inflate the number.
    db.mark_forum_post_viewed(&existing.id, &user.id).await?;

    let post = db.fetch_forum_post(&existing.id).await?;
    Ok(Json(post.into()))
}
