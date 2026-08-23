use revolt_database::events::client::EventV1;
use revolt_database::util::permissions::DatabasePermissionQuery;
use revolt_database::util::reference::Reference;
use revolt_database::{Channel, Database, File, ForumPost, User};
use revolt_models::v0;
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;
use validator::Validate;

/// Record the newest activity on a forum channel.
///
/// Shared by post and comment creation: both count as "something new" for the
/// unread indicator.
pub async fn bump_forum_activity(
    db: &Database,
    channel: &Channel,
    activity_id: &str,
) -> Result<()> {
    let forum = match channel {
        Channel::ForumChannel { forum, .. } => forum,
        _ => return Ok(()),
    };

    let mut updated = forum.clone();
    updated.last_activity_id = Some(activity_id.to_string());

    let mut channel = channel.clone();
    channel
        .update(
            db,
            revolt_database::PartialChannel {
                forum: Some(updated),
                ..Default::default()
            },
            vec![],
        )
        .await
}

/// # Create Forum Post
///
/// Create a new post in a forum channel.
#[openapi(tag = "Forum")]
#[post("/<target>/posts", data = "<data>")]
pub async fn create_forum_post(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    data: Json<v0::DataCreateForumPost>,
) -> Result<Json<v0::ForumPost>> {
    let data = data.into_inner();
    data.validate()
        .map_err(|error| create_error!(FailedValidation { error: error.to_string() }))?;

    let channel = target.as_channel(db).await?;

    // Only forum channels hold posts.
    let (channel_id, server_id, forum) = match &channel {
        Channel::ForumChannel {
            id, server, forum, ..
        } => (id.clone(), server.clone(), forum.clone()),
        _ => return Err(create_error!(InvalidOperation)),
    };

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    let permissions = calculate_channel_permissions(&mut query).await;
    permissions.throw_if_lacking_channel_permission(ChannelPermission::SendMessage)?;

    if data.attachments.as_ref().is_some_and(|v| !v.is_empty()) {
        permissions.throw_if_lacking_channel_permission(ChannelPermission::UploadFiles)?;
    }

    // Tags must be declared on the channel.
    let known: Vec<&str> = forum.available_tags.iter().map(|t| t.id.as_str()).collect();
    if let Some(unknown) = data.tags.iter().find(|t| !known.contains(&t.as_str())) {
        return Err(create_error!(IncorrectData {
            with: format!("unknown tag {unknown}")
        }));
    }

    if forum.require_tag && data.tags.is_empty() {
        return Err(create_error!(IncorrectData { with: "tags".into() }));
    }

    let id = ulid::Ulid::new().to_string();

    let attachments = if let Some(ids) = data.attachments {
        let mut files = Vec::with_capacity(ids.len());
        for attachment_id in ids {
            files.push(
                File::use_attachment(db, &attachment_id, &id, &user.id)
                    .await?
                    .into(),
            );
        }
        Some(files)
    } else {
        None
    };

    let now = iso8601_timestamp::Timestamp::now_utc();

    let post = ForumPost {
        id: id.clone(),
        channel: channel_id.clone(),
        server: server_id,
        author: user.id.clone(),
        title: data.title,
        content: data.content,
        attachments,
        tags: data.tags,
        upvoters: vec![],
        score: 0,
        comment_count: 0,
        // The author has obviously seen it, and follows it by default so
        // replies are trackable.
        viewers: vec![user.id.clone()],
        subscribers: vec![user.id.clone()],
        // A brand new post is the freshest thing in the feed; seed the rank so
        // it does not sit at zero until the first vote.
        hot_rank: revolt_database::hot_rank(
            0,
            ulid::Ulid::from_string(&id).map(|u| u.timestamp_ms() as i64).unwrap_or(0),
            now.duration_since(iso8601_timestamp::Timestamp::UNIX_EPOCH).whole_milliseconds() as i64,
        ),
        last_comment_at: None,
        pinned: false,
        locked: false,
        edited: None,
        deleted_by: None,
    };

    db.insert_forum_post(&post).await?;

    // The read marker compares against this, so a new post has to bump it or
    // the sidebar never lights up.
    bump_forum_activity(db, &channel, &id).await?;

    let model: v0::ForumPost = post.into();

    EventV1::ForumPostCreate(model.clone())
        .p(channel_id)
        .await;

    Ok(Json(model))
}
