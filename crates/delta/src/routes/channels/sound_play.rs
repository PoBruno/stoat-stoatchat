use revolt_database::{
    events::client::EventV1, util::permissions::DatabasePermissionQuery,
    util::reference::Reference, Database, User,
};
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::State;
use rocket_empty::EmptyResponse;

/// # Announce Soundboard Play
///
/// Tell everyone in the channel that a sound is playing.
///
/// The audio itself travels as a LiveKit track published by the player; this
/// only carries the "who played what" label. It cannot go over LiveKit because
/// the token sets `can_publish_data = false` and voice-ingress disconnects any
/// participant that publishes data.
#[openapi(tag = "Soundboard")]
#[post("/<target>/sounds/<sound>/play")]
pub async fn play_sound(
    db: &State<Database>,
    user: User,
    target: Reference<'_>,
    sound: Reference<'_>,
) -> Result<EmptyResponse> {
    let channel = target.as_channel(db).await?;

    // Only voice channels have anyone to hear it.
    if channel.voice().is_none() {
        return Err(create_error!(InvalidOperation));
    }

    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::UseSoundboard)?;

    // Confirm the sound exists and belongs to this server, so the event can
    // never point at something the recipients are unable to resolve.
    let sound = db.fetch_sound(&sound.id).await?;
    match (&sound.parent, channel.server()) {
        (revolt_database::SoundParent::Server { id }, Some(server)) if id == server => {}
        _ => return Err(create_error!(NotFound)),
    }

    EventV1::SoundPlay {
        channel_id: channel.id().to_string(),
        sound_id: sound.id.clone(),
        user_id: user.id.clone(),
    }
    .p(channel.id().to_string())
    .await;

    Ok(EmptyResponse)
}
