use revolt_database::events::client::EventV1;
use revolt_database::util::permissions::DatabasePermissionQuery;
use revolt_database::util::reference::Reference;
use revolt_database::{
    Channel, Database, File, ForumComment, ForumCommentSort, PartialForumPost, User, AMQP,
};
use revolt_models::v0;
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::serde::json::Json;
use rocket::State;
use validator::Validate;

/// Resolve the channel and post, rejecting anything that is not a forum post
/// belonging to this channel.
async fn resolve(
    db: &Database,
    target: &Reference<'_>,
    post: &Reference<'_>,
) -> Result<(Channel, revolt_database::ForumPost)> {
    let channel = target.as_channel(db).await?;
    if !matches!(&channel, Channel::ForumChannel { .. }) {
        return Err(create_error!(InvalidOperation));
    }

    let post = db.fetch_forum_post(&post.id).await?;
    if post.channel != channel.id() {
        return Err(create_error!(NotFound));
    }

    Ok((channel, post))
}

/// Quantos caracteres do comentario cabem no corpo da notificacao.
const PREVIEW_LEN: usize = 120;

/// Encurta o comentario para caber numa notificacao.
///
/// Corta por `char`, e nao por byte: `&s[..n]` entra em panico se cair no meio
/// de um caractere multibyte, e acento em portugues ocupa dois bytes.
fn comment_preview(content: &str) -> String {
    let content = content.trim();
    if content.chars().count() <= PREVIEW_LEN {
        return content.to_string();
    }

    let cortado: String = content.chars().take(PREVIEW_LEN).collect();
    format!("{}\u{2026}", cortado.trim_end())
}

/// # Create Forum Comment
///
/// Comment on a post, optionally replying to another comment.
#[openapi(tag = "Forum")]
#[post("/<target>/posts/<post>/comments", data = "<data>")]
pub async fn create_forum_comment(
    db: &State<Database>,
    amqp: &State<AMQP>,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
    data: Json<v0::DataCreateForumComment>,
) -> Result<Json<v0::ForumComment>> {
    let data = data.into_inner();
    data.validate()
        .map_err(|error| create_error!(FailedValidation { error: error.to_string() }))?;

    let (channel, post) = resolve(db, &target, &post).await?;

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    let permissions = calculate_channel_permissions(&mut query).await;
    permissions.throw_if_lacking_channel_permission(ChannelPermission::SendMessage)?;

    // A locked post takes no new comments, not even from the author.
    if post.locked {
        return Err(create_error!(InvalidOperation));
    }

    if data.attachments.as_ref().is_some_and(|v| !v.is_empty()) {
        permissions.throw_if_lacking_channel_permission(ChannelPermission::UploadFiles)?;
    }

    // Build the ancestor chain from the parent's, so depth costs one read.
    let (parent, ancestors) = match &data.parent {
        Some(parent_id) => {
            let parent = db.fetch_forum_comment(parent_id).await?;
            if parent.post != post.id {
                return Err(create_error!(NotFound));
            }
            let mut ancestors = parent.ancestors.clone();
            ancestors.push(parent.id.clone());
            (Some(parent.id), ancestors)
        }
        None => (None, vec![]),
    };

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

    let comment = ForumComment {
        id: id.clone(),
        post: post.id.clone(),
        channel: channel.id().to_string(),
        author: user.id.clone(),
        content: data.content,
        attachments,
        parent,
        ancestors,
        upvoters: vec![],
        score: 0,
        edited: None,
        deleted_by: None,
    };

    db.insert_forum_comment(&comment).await?;

    // Keep the post's counters honest. Counting is cheap at this scale and
    // cannot drift the way an $inc can.
    let count = db.count_forum_comments(&post.id).await?;
    db.update_forum_post(
        &post.id,
        &PartialForumPost {
            comment_count: Some(count),
            last_comment_at: Some(iso8601_timestamp::Timestamp::now_utc().to_string()),
            ..Default::default()
        },
        vec![],
    )
    .await?;

    super::forum_post_create::bump_forum_activity(db, &channel, &id).await?;

    // Whoever follows the post gets a mention-level unread, which is what
    // lights the badge in the sidebar.
    let followers: Vec<String> = post
        .subscribers
        .iter()
        .filter(|id| **id != user.id)
        .cloned()
        .collect();

    // Um por vez, de proposito: `add_mention_to_many_unreads` usa `update_many`
    // com filtro `$in`, e nesse caso o upsert do Mongo nao consegue derivar o
    // `_id.user` — quem nunca abriu o canal ficaria sem registro nenhum.
    for follower in &followers {
        db.add_mention_to_unread(channel.id(), follower, &[id.clone()])
            .await?;
    }

    // Push por cima do nao-lido, pela fila "generic".
    //
    // Nao da para reusar a fila de mensagem: aquele payload exige um `Message`
    // completo, e um comentario de forum nao e uma mensagem — o cliente
    // tentaria arquiva-lo no store de mensagens. A `generic` carrega apenas
    // titulo/corpo/icone, que e exatamente o necessario aqui.
    //
    // Falha de push nao pode derrubar a criacao do comentario: o comentario ja
    // esta gravado e o nao-lido ja foi marcado. Por isso o erro so e logado.
    if !followers.is_empty() {
        let titulo = post.title.clone();
        let corpo = format!(
            "{}: {}",
            user.display_name.as_ref().unwrap_or(&user.username),
            comment_preview(&comment.content)
        );

        for follower in &followers {
            match db.fetch_user(follower).await {
                Ok(destinatario) => {
                    if let Err(error) = amqp
                        .generic_message(
                            &destinatario,
                            titulo.clone(),
                            corpo.clone(),
                            user.avatar.as_ref().map(|f| f.id.clone()),
                        )
                        .await
                    {
                        revolt_config::capture_error(&error);
                    }
                }
                Err(error) => {
                    revolt_config::capture_error(&error);
                }
            }
        }
    }

    let model: v0::ForumComment = comment.into();

    EventV1::ForumCommentCreate(model.clone())
        .p(channel.id().to_string())
        .await;

    Ok(Json(model))
}

/// # Fetch Forum Comments
///
/// Fetch every comment on a post, flat. The client assembles the tree.
#[openapi(tag = "Forum")]
#[get("/<target>/posts/<post>/comments?<options..>")]
pub async fn query_forum_comments(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    post: Reference<'_>,
    options: v0::OptionsQueryForumComments,
) -> Result<Json<v0::ForumCommentsResponse>> {
    let (channel, post) = resolve(db, &target, &post).await?;

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::ReadMessageHistory)?;

    let sort = match options.sort.unwrap_or(v0::ForumCommentSort::Old) {
        v0::ForumCommentSort::Top => ForumCommentSort::Top,
        v0::ForumCommentSort::Old => ForumCommentSort::Old,
        v0::ForumCommentSort::New => ForumCommentSort::New,
    };

    let comments = db.fetch_forum_comments(&post.id, sort).await?;

    let author_ids: Vec<String> = {
        let mut ids: Vec<String> = comments.iter().map(|c| c.author.clone()).collect();
        ids.sort();
        ids.dedup();
        ids
    };

    let users = User::fetch_many_ids_as_mutuals(db, &user, &author_ids).await?;
    let members = db
        .fetch_members(&post.server, &author_ids)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|m| m.into())
        .collect();

    Ok(Json(v0::ForumCommentsResponse {
        comments: comments.into_iter().map(Into::into).collect(),
        users,
        members: Some(members),
    }))
}
