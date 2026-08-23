use revolt_database::util::permissions::DatabasePermissionQuery;
use revolt_database::util::reference::Reference;
use revolt_database::{Channel, Database, ForumFeedQuery, ForumFeedSort, User};
use revolt_models::v0;
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;
use validator::Validate;

fn to_feed_sort(sort: v0::ForumSort) -> ForumFeedSort {
    match sort {
        v0::ForumSort::Hot => ForumFeedSort::Hot,
        v0::ForumSort::New => ForumFeedSort::New,
        v0::ForumSort::Top => ForumFeedSort::Top,
        v0::ForumSort::Active => ForumFeedSort::Active,
    }
}

/// # Fetch Forum Posts
///
/// Fetch a page of posts from a forum channel.
#[openapi(tag = "Forum")]
#[get("/<target>/posts?<options..>")]
pub async fn query_forum_posts(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    options: v0::OptionsQueryForumPosts,
) -> Result<Json<v0::ForumPostsResponse>> {
    options
        .validate()
        .map_err(|error| create_error!(FailedValidation { error: error.to_string() }))?;

    let channel = target.as_channel(db).await?;

    let (channel_id, server_id, forum) = match &channel {
        Channel::ForumChannel {
            id, server, forum, ..
        } => (id.clone(), server.clone(), forum.clone()),
        _ => return Err(create_error!(InvalidOperation)),
    };

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::ReadMessageHistory)?;

    let posts = db
        .fetch_forum_posts(&ForumFeedQuery {
            channel: channel_id,
            // Fall back to whatever the channel is configured to use.
            sort: to_feed_sort(options.sort.unwrap_or(forum.default_sort.into())),
            tag: options.tag,
            limit: options.limit.unwrap_or(50),
            after: options.after,
        })
        .await?;

    // Hydrate authors so the client can render without a second round trip.
    let author_ids: Vec<String> = {
        let mut ids: Vec<String> = posts.iter().map(|p| p.author.clone()).collect();
        ids.sort();
        ids.dedup();
        ids
    };

    let users = User::fetch_many_ids_as_mutuals(db, &user, &author_ids).await?;
    let members = db
        .fetch_members(&server_id, &author_ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.into())
        .collect();

    Ok(Json(v0::ForumPostsResponse {
        posts: posts.into_iter().map(Into::into).collect(),
        users,
        members: Some(members),
    }))
}
