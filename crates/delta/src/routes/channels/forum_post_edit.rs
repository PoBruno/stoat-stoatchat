use revolt_database::util::permissions::DatabasePermissionQuery;
use revolt_database::util::reference::Reference;
use revolt_database::{Channel, Database, PartialForumPost, User};
use revolt_models::v0;
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;
use rocket_empty::EmptyResponse;
use validator::Validate;

/// # Edit Forum Post
///
/// Edit a post, or moderate it by pinning / locking.
///
/// Editing the body requires being the author. Pinning and locking require
/// `ManageMessages`.
#[openapi(tag = "Forum")]
#[patch("/<target>/posts/<post>", data = "<data>")]
pub async fn edit_forum_post(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
    data: Json<v0::DataEditForumPost>,
) -> Result<Json<v0::ForumPost>> {
    let data = data.into_inner();
    data.validate()
        .map_err(|error| create_error!(FailedValidation { error: error.to_string() }))?;

    let channel = target.as_channel(db).await?;
    let forum = match &channel {
        Channel::ForumChannel { forum, .. } => forum.clone(),
        _ => return Err(create_error!(InvalidOperation)),
    };

    let post = db.fetch_forum_post(&post.id).await?;
    if post.channel != channel.id() {
        return Err(create_error!(NotFound));
    }

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    let permissions = calculate_channel_permissions(&mut query).await;
    permissions.throw_if_lacking_channel_permission(ChannelPermission::ReadMessageHistory)?;

    let is_author = post.author == user.id;
    let can_moderate = permissions
        .has_channel_permission(ChannelPermission::ManageMessages);

    // Content edits belong to the author alone; a moderator may remove a post
    // but may not put words in someone else's mouth.
    let edits_content = data.title.is_some()
        || data.content.is_some()
        || data.tags.is_some()
        || data.attachments.is_some()
        || data.remove.is_some();

    if edits_content && !is_author {
        return Err(create_error!(CannotEditMessage));
    }

    if (data.pinned.is_some() || data.locked.is_some()) && !can_moderate {
        return Err(create_error!(MissingPermission {
            permission: ChannelPermission::ManageMessages.to_string()
        }));
    }

    if post.locked && edits_content && !can_moderate {
        return Err(create_error!(InvalidOperation));
    }

    if let Some(tags) = &data.tags {
        let known: Vec<&str> = forum.available_tags.iter().map(|t| t.id.as_str()).collect();
        if let Some(unknown) = tags.iter().find(|t| !known.contains(&t.as_str())) {
            return Err(create_error!(IncorrectData {
                with: format!("unknown tag {unknown}")
            }));
        }
        if forum.require_tag && tags.is_empty() {
            return Err(create_error!(IncorrectData { with: "tags".into() }));
        }
    }

    // Appended, not replaced: pasting an image while editing must not drop
    // what was already attached.
    let attachments = match data.attachments {
        Some(ids) if !ids.is_empty() => {
            permissions.throw_if_lacking_channel_permission(ChannelPermission::UploadFiles)?;

            let mut files = post.attachments.clone().unwrap_or_default();
            for attachment_id in ids {
                files.push(
                    revolt_database::File::use_attachment(db, &attachment_id, &post.id, &user.id)
                        .await?,
                );
            }
            Some(files)
        }
        _ => None,
    };

    let mut partial = PartialForumPost {
        attachments,
        title: data.title,
        content: data.content,
        tags: data.tags,
        pinned: data.pinned,
        locked: data.locked,
        ..Default::default()
    };

    if edits_content {
        partial.edited = Some(iso8601_timestamp::Timestamp::now_utc().to_string());
    }

    let remove: Vec<revolt_database::FieldsForumPost> = data
        .remove
        .unwrap_or_default()
        .into_iter()
        .map(|field| match field {
            v0::FieldsForumPost::Content => revolt_database::FieldsForumPost::Content,
            v0::FieldsForumPost::Attachments => revolt_database::FieldsForumPost::Attachments,
        })
        .collect();

    db.update_forum_post(&post.id, &partial, remove).await?;

    // Re-read rather than patching the local copy: removals and the stored
    // edit timestamp are easier to trust from the source.
    let post = db.fetch_forum_post(&post.id).await?;

    let model: v0::ForumPost = post.into();
    Ok(Json(model))
}

/// # Delete Forum Post
///
/// Delete a post. The author may delete their own; anyone else needs
/// `ManageMessages`.
#[openapi(tag = "Forum")]
#[delete("/<target>/posts/<post>")]
pub async fn delete_forum_post(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
) -> Result<EmptyResponse> {
    let channel = target.as_channel(db).await?;
    if !matches!(&channel, Channel::ForumChannel { .. }) {
        return Err(create_error!(InvalidOperation));
    }

    let post = db.fetch_forum_post(&post.id).await?;
    if post.channel != channel.id() {
        return Err(create_error!(NotFound));
    }

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    let permissions = calculate_channel_permissions(&mut query).await;
    permissions.throw_if_lacking_channel_permission(ChannelPermission::ReadMessageHistory)?;

    if post.author != user.id {
        permissions.throw_if_lacking_channel_permission(ChannelPermission::ManageMessages)?;
    }

    // Comments live in their own collection; dropping the post does not take
    // them with it.
    db.delete_forum_comments_on_post(&post.id).await?;
    db.delete_forum_post(&post.id).await?;

    Ok(EmptyResponse)
}
