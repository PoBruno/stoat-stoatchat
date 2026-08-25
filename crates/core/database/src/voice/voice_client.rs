use crate::{
    models::{Channel, User},
    voice::RoomMetadata,
    Database,
};
use livekit_api::{
    access_token::{AccessToken, VideoGrants},
    services::room::{CreateRoomOptions, RoomClient as InnerRoomClient, UpdateParticipantOptions},
};
use livekit_protocol::{ParticipantInfo, ParticipantPermission, Room};
use revolt_config::{config, LiveKitNode};
use revolt_permissions::{ChannelPermission, PermissionValue};
use revolt_result::{create_error, Result, ToRevoltError};
use std::{collections::HashMap, time::Duration};

use super::{get_allowed_sources, MUSICBOX_IDENTITY_PREFIX};

#[derive(Debug)]
pub struct RoomClient {
    pub client: InnerRoomClient,
    pub node: LiveKitNode,
}

#[derive(Debug)]
pub struct VoiceClient {
    pub rooms: HashMap<String, RoomClient>,
}

impl VoiceClient {
    pub fn new(nodes: HashMap<String, LiveKitNode>) -> Self {
        Self {
            rooms: nodes
                .into_iter()
                .map(|(name, node)| {
                    (
                        name,
                        RoomClient {
                            client: InnerRoomClient::with_api_key(
                                &node.url,
                                &node.key,
                                &node.secret,
                            ),
                            node,
                        },
                    )
                })
                .collect(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        !self.rooms.is_empty()
    }

    pub async fn from_revolt_config() -> Self {
        let config = config().await;

        Self::new(config.api.livekit.nodes.clone())
    }

    pub fn get_node(&self, name: &str) -> Result<&RoomClient> {
        self.rooms
            .get(name)
            .ok_or_else(|| create_error!(UnknownNode))
    }

    pub async fn create_token(
        &self,
        node: &str,
        db: &Database,
        user: &User,
        permissions: PermissionValue,
        channel: &Channel,
    ) -> Result<String> {
        let room = self.get_node(node)?;

        let limits = user.limits().await;
        let allowed_sources = get_allowed_sources(&limits, permissions);

        AccessToken::with_api_key(&room.node.key, &room.node.secret)
            .with_name(&format!("{}#{}", user.username, user.discriminator))
            .with_identity(&user.id)
            .with_metadata(
                &serde_json::to_string(&user.clone().into(db, None).await).to_internal_error()?,
            )
            .with_ttl(Duration::from_secs(10))
            .with_grants(VideoGrants {
                room_join: true,
                can_publish: true,
                can_publish_data: false,
                can_publish_sources: allowed_sources
                    .into_iter()
                    .map(ToString::to_string)
                    .collect(),
                can_subscribe: permissions.has_channel_permission(ChannelPermission::Listen),
                room: channel.id().to_string(),
                ..Default::default()
            })
            .to_jwt()
            .to_internal_error()
    }

    /// Token para o agente de música entrar numa sala.
    ///
    /// Com uma conta de bot configurada, o agente entra **como aquele
    /// usuário**: mesma identidade, mesmo nome, mesmos metadados que uma
    /// pessoa teria. É o que faz a interface desenhá-lo com nome e avatar sem
    /// nenhum caso especial, e o que permite controlá-lo pelas permissões do
    /// canal como se controla qualquer um.
    ///
    /// Sem conta configurada, cai numa identidade sintética com prefixo. Ela
    /// toca igual, mas aparece sem rosto — e obriga o resto do código de voz
    /// a reconhecê-la para não tratá-la como usuário.
    ///
    /// Em nenhum dos casos usa `hidden`. Seria o campo óbvio para tirá-lo da
    /// lista de participantes, mas no LiveKit oculto significa oculto de
    /// verdade: as faixas não são entregues a ninguém, e a música não toca.
    pub async fn create_musicbox_token(
        &self,
        node: &str,
        db: &Database,
        channel: &Channel,
    ) -> Result<String> {
        let room = self.get_node(node)?;
        let config = config().await;

        let bot = if config.musicbox.bot_user_id.is_empty() {
            None
        } else {
            // Bot configurado que sumiu do banco não é motivo para a música
            // parar: registra e segue com a identidade sintética.
            match db.fetch_user(&config.musicbox.bot_user_id).await {
                Ok(user) => Some(user),
                Err(_) => {
                    log::warn!(
                        "musicbox: bot_user_id {} não existe; entrando sem identidade",
                        config.musicbox.bot_user_id
                    );
                    None
                }
            }
        };

        let mut token = AccessToken::with_api_key(&room.node.key, &room.node.secret)
            // Folgado em relação aos 10s de uma pessoa: o agente pode estar do
            // outro lado de uma conexão residencial, e o token só serve para o
            // aperto de mão.
            .with_ttl(Duration::from_secs(60));

        token = match &bot {
            Some(user) => token
                .with_identity(&user.id)
                .with_name(&format!("{}#{}", user.username, user.discriminator))
                .with_metadata(
                    &serde_json::to_string(&user.clone().into(db, None).await)
                        .to_internal_error()?,
                ),
            None => token
                .with_identity(&format!("{MUSICBOX_IDENTITY_PREFIX}{}", channel.id()))
                .with_name("MusicBox"),
        };

        token
            .with_grants(VideoGrants {
                room_join: true,
                can_publish: true,
                can_publish_data: false,
                // A mesma fonte do soundboard: áudio que não é microfone.
                can_publish_sources: vec!["unknown".to_string()],
                // O agente toca, não escuta. Assinar as outras faixas gastaria
                // banda da casa de quem hospeda para nada.
                can_subscribe: false,
                room: channel.id().to_string(),
                ..Default::default()
            })
            .to_jwt()
            .to_internal_error()
    }

    pub async fn create_room(&self, node: &str, channel: &Channel) -> Result<Room> {        let room = self.get_node(node)?;

        let metadata = RoomMetadata {
            server: channel.server().map(|id| id.to_string()),
        };

        room.client
            .create_room(
                channel.id(),
                CreateRoomOptions {
                    empty_timeout: 5 * 60, // 5 minutes,
                    metadata: serde_json::to_string(&metadata).to_internal_error()?,
                    ..Default::default()
                },
            )
            .await
            .to_internal_error()
    }

    pub async fn update_permissions(
        &self,
        node: &str,
        user: &User,
        channel_id: &str,
        new_permissions: ParticipantPermission,
    ) -> Result<ParticipantInfo> {
        let room = self.get_node(node)?;

        room.client
            .update_participant(
                channel_id,
                &user.id,
                UpdateParticipantOptions {
                    permission: Some(new_permissions),
                    ..Default::default()
                },
            )
            .await
            .to_internal_error()
    }

    pub async fn remove_user(&self, node: &str, user_id: &str, channel_id: &str) -> Result<()> {
        let room = self.get_node(node)?;

        room.client
            .remove_participant(channel_id, user_id)
            .await
            .to_internal_error()
    }

    pub async fn delete_room(&self, node: &str, channel_id: &str) -> Result<()> {
        let room = self.get_node(node)?;

        room.client
            .delete_room(channel_id)
            .await
            .to_internal_error()
    }

    pub async fn get_room_participants(
        &self,
        node: &str,
        channel_id: &str,
    ) -> Result<Vec<ParticipantInfo>> {
        let room = self.get_node(node)?;

        room.client
            .list_participants(channel_id)
            .await
            .to_internal_error()
    }
}
