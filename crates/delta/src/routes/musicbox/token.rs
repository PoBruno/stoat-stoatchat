use revolt_config::config;
use revolt_database::{
    util::reference::Reference, voice::VoiceClient, Database,
};
use revolt_result::{create_error, Result};
use rocket::{serde::json::Json, State};
use serde::{Deserialize, Serialize};

use super::agent::AgentAuth;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DataAgentToken {
    /// Canal cuja chamada o agente vai entrar
    pub channel_id: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AgentTokenResponse {
    pub token: String,
    /// Endereço do LiveKit que o agente deve usar
    pub url: String,
}

/// # Agent Voice Token
///
/// Mints a token letting the music agent publish audio into a call.
///
/// O agente roda fora do servidor e não tem conta. Ele não pode assinar o
/// próprio token porque não conhece — nem deve conhecer — a chave do LiveKit.
#[openapi(tag = "MusicBox")]
#[post("/agent/token", data = "<data>")]
pub async fn agent_token(
    _auth: AgentAuth,
    db: &State<Database>,
    voice_client: &State<VoiceClient>,
    data: Json<DataAgentToken>,
) -> Result<Json<AgentTokenResponse>> {
    if !voice_client.is_enabled() {
        return Err(create_error!(LiveKitUnavailable));
    }

    let channel = Reference::from_unchecked(&data.channel_id)
        .as_channel(db)
        .await?;

    if channel.voice().is_none() {
        return Err(create_error!(NotAVoiceChannel));
    }

    let config = config().await;

    // O nó vem da config e não do agente: deixar o agente escolher seria
    // deixá-lo apontar para um LiveKit qualquer.
    let node = config
        .api
        .livekit
        .nodes
        .keys()
        .next()
        .ok_or_else(|| create_error!(LiveKitUnavailable))?
        .clone();

    let url = config
        .hosts
        .livekit
        .get(&node)
        .ok_or_else(|| create_error!(UnknownNode))?
        .clone();

    // Garante a sala antes de emitir o token. Sem isso o `RoomMetadata` não
    // existe, e o webhook de participante do voice-ingress falha ao lê-lo —
    // uma falha que apareceria só depois, num daemon separado.
    voice_client.create_room(&node, &channel).await?;

    let token = voice_client.create_musicbox_token(&node, &channel).await?;

    Ok(Json(AgentTokenResponse { token, url }))
}
