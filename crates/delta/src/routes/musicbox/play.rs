use std::time::Duration;

use revolt_config::config;
use revolt_database::{
    util::{permissions::DatabasePermissionQuery, reference::Reference},
    Database, User,
};
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::{serde::json::Json, State};
use rocket_empty::EmptyResponse;
use serde::Deserialize;
use ulid::Ulid;

use super::state::{Command, MusicBoxState, Track};

/// Mais curto que o da busca: mandar tocar não espera download nenhum, só o
/// agente confirmar que começou.
const ESPERA_MAXIMA: Duration = Duration::from_secs(30);

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DataPlay {
    pub track: Track,
}

/// Permissão para mandar no MusicBox.
///
/// Usa `Connect` — quem pode entrar na chamada pode mexer na música dela. Um
/// bit próprio (`UseMusicBox`) daria controle mais fino, mas gastar um bit de
/// permissão antes de a feature provar que é usada é otimizar cedo demais.
async fn exigir_permissao(
    db: &Database,
    user: &User,
    canal: &Reference<'_>,
) -> Result<revolt_database::Channel> {
    let channel = canal.as_channel(db).await?;
    let mut query = DatabasePermissionQuery::new(db, user).channel(&channel);
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::Connect)?;
    Ok(channel)
}

async fn mandar(estado: &MusicBoxState, comando: Command) -> Result<EmptyResponse> {
    let config = config().await;

    if config.musicbox.agent_secret.is_empty() {
        return Err(create_error!(FeatureDisabled {
            feature: "musicbox".to_string()
        }));
    }

    if !estado.agent_present(Duration::from_secs(config.musicbox.agent_timeout_seconds)) {
        return Err(create_error!(FeatureDisabled {
            feature: "musicbox:agent".to_string()
        }));
    }

    let id = comando.id.clone();
    let recebe = estado.submit(comando);

    match tokio::time::timeout(ESPERA_MAXIMA, recebe).await {
        Ok(Ok(resultado)) => {
            if let Some(erro) = resultado.error {
                log::warn!("musicbox: o agente não conseguiu tocar: {erro}");
                return Err(create_error!(InternalError));
            }
            Ok(EmptyResponse)
        }
        _ => {
            estado.forget(&id);
            Err(create_error!(InternalError))
        }
    }
}

/// # Play Track
///
/// Plays a track into this channel's call.
///
/// O áudio vai da máquina do agente direto para o servidor de voz; esta rota
/// só carrega o pedido.
#[openapi(tag = "MusicBox")]
#[post("/<target>/play", data = "<data>")]
pub async fn play(
    db: &State<Database>,
    estado: &State<MusicBoxState>,
    user: User,
    target: Reference<'_>,
    data: Json<DataPlay>,
) -> Result<EmptyResponse> {
    let channel = exigir_permissao(db, &user, &target).await?;

    if channel.voice().is_none() {
        return Err(create_error!(NotAVoiceChannel));
    }

    mandar(
        estado,
        Command {
            id: Ulid::new().to_string(),
            kind: "play".to_string(),
            query: String::new(),
            limit: 0,
            channel_id: Some(channel.id().to_string()),
            track: Some(data.into_inner().track),
        },
    )
    .await
}

/// # Stop Playback
///
/// Stops whatever the MusicBox is playing in this channel.
#[openapi(tag = "MusicBox")]
#[post("/<target>/stop")]
pub async fn stop(
    db: &State<Database>,
    estado: &State<MusicBoxState>,
    user: User,
    target: Reference<'_>,
) -> Result<EmptyResponse> {
    let channel = exigir_permissao(db, &user, &target).await?;

    mandar(
        estado,
        Command {
            id: Ulid::new().to_string(),
            kind: "stop".to_string(),
            query: String::new(),
            limit: 0,
            channel_id: Some(channel.id().to_string()),
            track: None,
        },
    )
    .await
}
