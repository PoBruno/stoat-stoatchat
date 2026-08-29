use livekit_api::{access_token::TokenVerifier, webhooks::WebhookReceiver};
use livekit_protocol::TrackType;
use revolt_database::{
    events::client::EventV1,
    iso8601_timestamp::{Duration, Timestamp},
    util::reference::Reference,
    voice::{
        create_voice_state, delete_channel_voice_state, delete_voice_state,
        get_call_notification_recipients, get_user_moved_from_voice, get_user_moved_to_voice,
        get_voice_channel_members, is_musicbox_participant,
        set_channel_call_started_system_message, take_channel_call_started_system_message,
        update_voice_state_tracks, RoomMetadata, UserVoiceChannel, VoiceClient,
    },
    Channel, Database, PartialMessage, SystemMessage, AMQP,
};
use revolt_models::v0;
use revolt_result::{Result, ToRevoltError};
use rocket::{post, State};
use rocket_empty::EmptyResponse;
use ulid::Ulid;

use crate::guard::AuthHeader;

#[post("/<node>", data = "<body>")]
pub async fn ingress(
    db: &State<Database>,
    voice_client: &State<VoiceClient>,
    amqp: &State<AMQP>,
    node: &str,
    auth_header: AuthHeader<'_>,
    body: &str,
) -> Result<EmptyResponse> {
    log::debug!("received event: {body}");

    let config = revolt_config::config().await;

    let node_info = config
        .api
        .livekit
        .nodes
        .get(node)
        .to_internal_error()
        .inspect_err(|_| {
            log::error!("Unknown node {node}, make sure livekit has the correct node name set and matches `hosts.livekit` and `api.livekit.nodes` in the Revolt config.")
        })?;

    let webhook_receiver = WebhookReceiver::new(TokenVerifier::with_api_key(
        &node_info.key,
        &node_info.secret,
    ));

    let event = webhook_receiver
        .receive(body, &auth_header)
        .to_internal_error()?;

    let channel_id = event.room.as_ref().map(|r| &r.name);
    let user_id = event.participant.as_ref().map(|r| &r.identity);
    let room_metadata = if let Some(room) = event.room.as_ref() {
        serde_json::from_str::<RoomMetadata>(&room.metadata).ok()
    } else {
        None
    };

    // O agente de música é um participante sem conta: publica áudio e nada
    // mais. Tudo daqui para baixo trata `identity` como id de usuário — o
    // `as_user` devolveria NotFound, e `User::limits` faz
    // `Ulid::from_str(...).expect(...)`, que derruba o daemon inteiro em vez
    // de falhar aquele evento.
    //
    // A saída fica antes do `match` de propósito: assim vale também para
    // qualquer evento que venha a ser tratado depois, sem precisar lembrar
    // de repetir a checagem.
    if user_id.is_some_and(|id| is_musicbox_participant(id)) {
        log::debug!("Ignoring voice event for music agent {user_id:?}");
        return Ok(EmptyResponse);
    }

    match event.event.as_str() {
        // User joined a channel
        "participant_joined" => {
            let channel_id = channel_id.to_internal_error()?;
            let user_id = user_id.to_internal_error()?;
            let server_id = room_metadata.to_internal_error()?.server;
            let voice_channel = UserVoiceChannel {
                id: channel_id.clone(),
                server_id: server_id.clone(),
            };

            let channel = Reference::from_unchecked(channel_id).as_channel(db).await?;

            let joined_at = Timestamp::UNIX_EPOCH
                .checked_add(Duration::seconds(event.created_at))
                .unwrap();

            let voice_state = create_voice_state(&voice_channel, user_id, joined_at).await?;

            // Only publish one event when a user is moved from one channel to another.
            if let Some(moved_from) = get_user_moved_to_voice(channel_id, user_id).await? {
                EventV1::VoiceChannelMove {
                    user: user_id.to_string(),
                    from: moved_from.id,
                    to: channel_id.to_string(),
                    state: voice_state,
                }
                .p(channel_id.to_string())
                .await;
            } else {
                EventV1::VoiceChannelJoin {
                    id: channel_id.to_string(),
                    state: voice_state,
                }
                .p(channel_id.to_string())
                .await;
            };

            let participants = voice_client.get_room_participants(node, channel_id).await?;

            // Canal dedicado a voz nao ganha mensagem de "fulano iniciou
            // chamada". Ali nao ha conversa para a mensagem entrar: o canal
            // existe para falar, e o histórico so acumulava avisos que
            // ninguem le.
            //
            // Conversa direta e grupo continuam recebendo: naqueles a chamada
            // acontece dentro de um chat que existe por si, e o aviso e o
            // unico registro de que ela houve.
            let canal_dedicado_a_voz = matches!(
                &channel,
                Channel::TextChannel {
                    voice: Some(_),
                    ..
                }
            );

            if participants.len() == 1 && !canal_dedicado_a_voz {
                let user = Reference::from_unchecked(user_id).as_user(db).await?;
                let message_id = Ulid::from_datetime(
                    Timestamp::UNIX_EPOCH
                        .checked_add(Duration::seconds(event.created_at))
                        .unwrap()
                        .into(),
                )
                .to_string();

                let mut call_started_message = SystemMessage::CallStarted {
                    by: user_id.to_string(),
                    finished_at: None,
                }
                .into_message(channel_id.clone());

                call_started_message.id = message_id;

                set_channel_call_started_system_message(channel_id, &call_started_message.id)
                    .await?;

                call_started_message
                    .send(
                        db,
                        Some(amqp),
                        v0::MessageAuthor::System {
                            username: &user.username,
                            avatar: user.avatar.as_ref().map(|file| file.id.as_ref()),
                        },
                        None,
                        None,
                        &channel,
                        false,
                    )
                    .await?;

                if let Channel::DirectMessage { recipients, .. }
                | Channel::Group { recipients, .. } = channel
                {
                    let call_recipients =
                        get_call_notification_recipients(channel_id, user_id).await?;

                    {
                        let call_recipients = if let Some(user_recipients) = call_recipients.clone()
                        {
                            user_recipients
                                .into_iter()
                                .filter(|user_id| {
                                    recipients.contains(user_id) && user_id != &user.id
                                })
                                .collect::<Vec<_>>()
                        } else {
                            recipients
                                .into_iter()
                                .filter(|user_id| user_id != &user.id)
                                .collect()
                        };

                        for recipient in call_recipients {
                            EventV1::VoiceCallUpdate {
                                initiator_id: user.id.clone(),
                                channel_id: channel_id.clone(),
                                started_at: Some(joined_at),
                                ended: false,
                            }
                            .private(recipient)
                            .await
                        }
                    }

                    if let Err(e) = amqp
                        .dm_call_updated(
                            &user.id,
                            channel_id,
                            Some(&joined_at.format_short()),
                            false,
                            call_recipients,
                        )
                        .await
                    {
                        revolt_config::capture_error(&e);
                    }
                }
            }
        }
        // User left a channel
        "participant_left" => {
            let channel_id = channel_id.to_internal_error()?;
            let user_id = user_id.to_internal_error()?;
            let server_id = room_metadata.to_internal_error()?.server;
            let voice_channel = UserVoiceChannel {
                id: channel_id.clone(),
                server_id: server_id.clone(),
            };

            delete_voice_state(&voice_channel, user_id).await?;

            // Dont send leave event when a user is moved
            if get_user_moved_from_voice(channel_id, user_id)
                .await?
                .is_none()
            {
                EventV1::VoiceChannelLeave {
                    id: channel_id.clone(),
                    user: user_id.clone(),
                }
                .p(channel_id.clone())
                .await;
            };

            // // Update CallStarted system message if everyone has left with the end time
            let members = get_voice_channel_members(&voice_channel).await?;

            if members.is_none_or(|m| m.is_empty()) {
                let channel = Reference::from_unchecked(channel_id).as_channel(db).await?;

                // The channel is empty so send out an "end" notification for ringing
                if matches!(
                    channel,
                    Channel::DirectMessage { .. } | Channel::Group { .. }
                ) {
                    EventV1::VoiceCallUpdate {
                        initiator_id: user_id.clone(),
                        channel_id: channel_id.clone(),
                        started_at: None,
                        ended: true,
                    }
                    .p(channel_id.clone())
                    .await;

                    if let Err(e) = amqp
                        .dm_call_updated(user_id, channel_id, None, true, None)
                        .await
                    {
                        revolt_config::capture_internal_error!(&e);
                    }
                }

                if let Some(system_message_id) =
                    take_channel_call_started_system_message(channel_id).await?
                {
                    // Could have been deleted
                    if let Ok(mut message) = Reference::from_unchecked(&system_message_id)
                        .as_message(db)
                        .await
                    {
                        if let Some(SystemMessage::CallStarted { finished_at, .. }) =
                            &mut message.system
                        {
                            *finished_at = Some(Timestamp::now_utc());

                            message
                                .update(
                                    db,
                                    PartialMessage {
                                        system: message.system.clone(),
                                        ..Default::default()
                                    },
                                    Vec::new(),
                                )
                                .await?;
                        } else {
                            log::error!("Broken State: Call started message ID ({}) does not contain a CallStarted system message.", &message.id)
                        }
                    };
                };
            }
        }
        // Audio/video track was started/stopped/unmuted/muted
        "track_published" | "track_unpublished" | "track_unmuted" | "track_muted" => {
            let channel_id = channel_id.to_internal_error()?;
            let user_id = user_id.to_internal_error()?;
            let track = event.track.as_ref().to_internal_error()?;
            let server_id = room_metadata.to_internal_error()?.server;
            let channel = UserVoiceChannel {
                id: channel_id.clone(),
                server_id: server_id.clone(),
            };

            let user = Reference::from_unchecked(user_id).as_user(db).await?;

            let user_limits = user.limits().await;

            // forbid any size which goes over the limit and also limit the aspect ratio to stop people from making too tall or too wide and bypassing the limit.
            // TODO: figure out how to track audio stream quality

            if event.event == "track_published" {
                let mut disconnect = false;

                // 2026-08-28 — removida a expulsão de quem publicava
                // `TrackType::Data`. O data channel virou uso legítimo: é por
                // ele que trafegam as anotações efêmeras (laser) sobre
                // compartilhamento de tela, e o grant `can_publish_data` foi
                // liberado em `revolt-database` (`voice/voice_client.rs`).
                //
                // Na prática o `if` já era código morto para esse caminho:
                // `publishData()` monta um `DataPacket` e o entrega em
                // `engine.sendDataPacket()`, que escreve direto no
                // RTCDataChannel. Não passa por `AddTrackRequest`, logo não
                // nasce nenhum `TrackInfo` — e `WebhookEvent.track` só é
                // preenchido em evento `track_*`. Ou seja, o webhook
                // `track_published` nunca chegava aqui com `TrackType::Data`.
                //
                // As demais validações abaixo (resolução e aspect ratio de
                // vídeo) seguem valendo, assim como a whitelist de
                // `can_publish_sources` do token, que é quem de fato limita o
                // que cada um pode publicar.

                if track.r#type == TrackType::Video as i32 {
                    if user_limits.video_resolution[0] != 0
                        && user_limits.video_resolution[1] != 0
                        && track.width * track.height
                            > user_limits.video_resolution[0] * user_limits.video_resolution[1]
                    {
                        log::debug!("User published video with out of bounds resolution");
                        disconnect = true;
                    };

                    if user_limits.video_aspect_ratio[0] != user_limits.video_aspect_ratio[1]
                        && !(user_limits.video_aspect_ratio[0]..=user_limits.video_aspect_ratio[1])
                            .contains(&(track.width as f32 / track.height as f32))
                    {
                        log::debug!("User published video with out of bounds aspect ratio");
                        disconnect = true;
                    };
                };

                if disconnect {
                    log::debug!("Removing user {user_id} from channel {channel_id} {event:?} due to forbidden track.");

                    let _ = voice_client.remove_user(node, user_id, channel_id).await;
                    delete_voice_state(&channel, user_id).await?;

                    return Ok(EmptyResponse);
                };
            };

            let partial = update_voice_state_tracks(
                &channel,
                user_id,
                event.event == "track_published" || event.event == "track_unmuted", // to avoid duplicating this entire case twice
                track.source,
            )
            .await?;

            EventV1::UserVoiceStateUpdate {
                id: user_id.clone(),
                channel_id: channel_id.clone(),
                data: partial,
            }
            .p(channel_id.clone())
            .await;
        }
        "room_finished" => {
            let channel_id = channel_id.to_internal_error()?;
            let server_id = room_metadata.to_internal_error()?.server;
            let channel = UserVoiceChannel {
                id: channel_id.clone(),
                server_id: server_id.clone(),
            };

            delete_channel_voice_state(&channel, &[]).await?;
        }
        _ => {}
    };

    Ok(EmptyResponse)
}
