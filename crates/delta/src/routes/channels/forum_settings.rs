use revolt_database::util::permissions::DatabasePermissionQuery;
use revolt_database::util::reference::Reference;
use revolt_database::{Channel, Database, PartialChannel, User};
use revolt_models::v0;
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;
use validator::Validate;

/// # Edit Forum Settings
///
/// Configure the tags a forum offers, its default ordering, and whether a post
/// must carry a tag.
///
/// Kept off `PATCH /channels/<id>` so the generic channel edit does not have to
/// know about forum-only fields.
#[openapi(tag = "Forum")]
#[patch("/<target>/forum", data = "<data>")]
pub async fn edit_forum_settings(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    data: Json<v0::DataEditForumChannel>,
) -> Result<Json<v0::Channel>> {
    let data = data.into_inner();
    data.validate()
        .map_err(|error| create_error!(FailedValidation { error: error.to_string() }))?;

    let mut channel = target.as_channel(db).await?;

    let current = match &channel {
        Channel::ForumChannel { forum, .. } => forum.clone(),
        _ => return Err(create_error!(InvalidOperation)),
    };

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::ManageChannel)?;

    let available_tags = match data.available_tags {
        Some(tags) => {
            // Tag ids have to be unique or a post could reference two of them.
            let mut seen = std::collections::HashSet::new();
            for tag in &tags {
                if !seen.insert(tag.id.clone()) {
                    return Err(create_error!(IncorrectData {
                        with: format!("duplicate tag {}", tag.id)
                    }));
                }
            }
            tags.into_iter().map(Into::into).collect()
        }
        None => current.available_tags.clone(),
    };

    let forum = revolt_database::ForumInformation {
        default_sort: data
            .default_sort
            .map(Into::into)
            .unwrap_or(current.default_sort),
        available_tags,
        require_tag: data.require_tag.unwrap_or(current.require_tag),
        last_activity_id: current.last_activity_id.clone(),
    };

    let partial = PartialChannel {
        forum: Some(forum),
        ..Default::default()
    };

    channel.update(db, partial, vec![]).await?;

    Ok(Json(channel.into()))
}
