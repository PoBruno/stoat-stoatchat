use revolt_database::util::permissions::DatabasePermissionQuery;
use revolt_database::util::reference::Reference;
use revolt_database::{Channel, Database, PartialForumComment, PartialForumPost, User};
use revolt_models::v0;
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;
use rocket_empty::EmptyResponse;
use validator::Validate;

/// # Edit Forum Comment
///
/// Edit a comment. Only the author may change the body.
#[openapi(tag = "Forum")]
#[patch("/<target>/comments/<comment>", data = "<data>")]
pub async fn edit_forum_comment(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    comment: Reference<'_>,
    data: Json<v0::DataEditForumComment>,
) -> Result<Json<v0::ForumComment>> {
    let data = data.into_inner();
    data.validate()
        .map_err(|error| create_error!(FailedValidation { error: error.to_string() }))?;

    let channel = target.as_channel(db).await?;
    if !matches!(&channel, Channel::ForumChannel { .. }) {
        return Err(create_error!(InvalidOperation));
    }

    let comment = db.fetch_forum_comment(&comment.id).await?;
    if comment.channel != channel.id() {
        return Err(create_error!(NotFound));
    }

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::ReadMessageHistory)?;

    // Moderators can remove a comment but never rewrite it.
    if comment.author != user.id {
        return Err(create_error!(CannotEditMessage));
    }

    // Appended, not replaced, same as posts.
    let attachments = match data.attachments {
        Some(ids) if !ids.is_empty() => {
            let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
            calculate_channel_permissions(&mut query)
                .await
                .throw_if_lacking_channel_permission(ChannelPermission::UploadFiles)?;

            let mut files = comment.attachments.clone().unwrap_or_default();
            for attachment_id in ids {
                files.push(
                    revolt_database::File::use_attachment(
                        db,
                        &attachment_id,
                        &comment.id,
                        &user.id,
                    )
                    .await?,
                );
            }
            Some(files)
        }
        _ => None,
    };

    let partial = PartialForumComment {
        content: data.content,
        attachments,
        edited: Some(iso8601_timestamp::Timestamp::now_utc().to_string()),
        ..Default::default()
    };

    let remove: Vec<revolt_database::FieldsForumComment> = data
        .remove
        .unwrap_or_default()
        .into_iter()
        .map(|field| match field {
            v0::FieldsForumComment::Attachments => {
                revolt_database::FieldsForumComment::Attachments
            }
        })
        .collect();

    db.update_forum_comment(&comment.id, &partial, remove).await?;

    let comment = db.fetch_forum_comment(&comment.id).await?;
    Ok(Json(comment.into()))
}

/// # Delete Forum Comment
///
/// Delete a comment. The author may delete their own; anyone else needs
/// `ManageMessages`.
///
/// A comment that has replies is tombstoned rather than removed, so the
/// subtree below it is not orphaned.
#[openapi(tag = "Forum")]
#[delete("/<target>/comments/<comment>")]
pub async fn delete_forum_comment(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    comment: Reference<'_>,
) -> Result<EmptyResponse> {
    let channel = target.as_channel(db).await?;
    if !matches!(&channel, Channel::ForumChannel { .. }) {
        return Err(create_error!(InvalidOperation));
    }

    let comment = db.fetch_forum_comment(&comment.id).await?;
    if comment.channel != channel.id() {
        return Err(create_error!(NotFound));
    }

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    let permissions = calculate_channel_permissions(&mut query).await;
    permissions.throw_if_lacking_channel_permission(ChannelPermission::ReadMessageHistory)?;

    if comment.author != user.id {
        permissions.throw_if_lacking_channel_permission(ChannelPermission::ManageMessages)?;
    }

    let post_id = comment.post.clone();

    if db.forum_comment_has_replies(&comment.id).await? {
        // Tombstone: keep the node so its children still have a parent.
        db.update_forum_comment(
            &comment.id,
            &PartialForumComment {
                content: Some(String::new()),
                deleted_by: Some(user.id.clone()),
                ..Default::default()
            },
            vec![revolt_database::FieldsForumComment::Attachments],
        )
        .await?;
    } else {
        db.delete_forum_comment(&comment.id).await?;
    }

    let count = db.count_forum_comments(&post_id).await?;
    db.update_forum_post(
        &post_id,
        &PartialForumPost {
            comment_count: Some(count),
            ..Default::default()
        },
        vec![],
    )
    .await?;

    Ok(EmptyResponse)
}

/// # Upvote Forum Comment
#[openapi(tag = "Forum")]
#[put("/<target>/comments/<comment>/upvote")]
pub async fn upvote_forum_comment(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    comment: Reference<'_>,
) -> Result<Json<v0::ForumComment>> {
    vote(db, user, target, comment, true).await
}

/// # Remove Forum Comment Upvote
#[openapi(tag = "Forum")]
#[delete("/<target>/comments/<comment>/upvote")]
pub async fn remove_forum_comment_upvote(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    comment: Reference<'_>,
) -> Result<Json<v0::ForumComment>> {
    vote(db, user, target, comment, false).await
}

/// Shared body for both vote directions
async fn vote(
    db: &Database,
    user: User,
    target: Reference<'_>,
    comment: Reference<'_>,
    add: bool,
) -> Result<Json<v0::ForumComment>> {
    let channel = target.as_channel(db).await?;
    if !matches!(&channel, Channel::ForumChannel { .. }) {
        return Err(create_error!(InvalidOperation));
    }

    let existing = db.fetch_forum_comment(&comment.id).await?;
    if existing.channel != channel.id() {
        return Err(create_error!(NotFound));
    }

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::ReadMessageHistory)?;

    let post = db.fetch_forum_post(&existing.post).await?;
    if post.locked {
        return Err(create_error!(InvalidOperation));
    }

    if add {
        db.upvote_forum_comment(&existing.id, &user.id).await?;
    } else {
        db.remove_forum_comment_upvote(&existing.id, &user.id)
            .await?;
    }

    let comment = db.fetch_forum_comment(&existing.id).await?;
    Ok(Json(comment.into()))
}
