use std::time::Duration;

use revolt_config::config;
use revolt_database::{
    util::{permissions::DatabasePermissionQuery, reference::Reference},
    Database, User,
};
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::{serde::json::Json, State};
use serde::{Deserialize, Serialize};
use ulid::Ulid;

use super::state::{Command, MusicBoxState, Track};

/// Quanto tempo o usuário espera antes de desistir.
///
/// Resolver uma playlist grande no yt-dlp é lento; 5 s devolveria erro para
/// pedidos que ainda estavam de pé. Sessenta é folgado sem prender a conexão
/// para sempre se o agente sumir no meio do trabalho.
const ESPERA_MAXIMA: Duration = Duration::from_secs(60);

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DataResolve {
    /// Nome para buscar, ou endereço de vídeo ou playlist
    pub query: String,
    /// Teto de faixas devolvidas
    pub limit: Option<u16>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ResolveResponse {
    pub tracks: Vec<Track>,
}

/// # Resolve Music
///
/// Turns a search term, a video link or a playlist link into tracks.
///
/// O trabalho é feito por um agente fora do servidor: o YouTube recusa os IPs
/// desta máquina, então quem extrai é uma conexão residencial.
#[openapi(tag = "MusicBox")]
#[post("/<target>/resolve", data = "<data>")]
pub async fn resolve(
    db: &State<Database>,
    estado: &State<MusicBoxState>,
    user: User,
    target: Reference<'_>,
    data: Json<DataResolve>,
) -> Result<Json<ResolveResponse>> {
    let config = config().await;

    if config.musicbox.agent_secret.is_empty() {
        return Err(create_error!(FeatureDisabled {
            feature: "musicbox".to_string()
        }));
    }

    let channel = target.as_channel(db).await?;
    let mut query = DatabasePermissionQuery::new(db, &user).channel(&channel);
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::UseMusicBox)?;

    let espera_agente = Duration::from_secs(config.musicbox.agent_timeout_seconds);
    if !estado.agent_present(espera_agente) {
        return Err(create_error!(FeatureDisabled {
            feature: "musicbox:agent".to_string()
        }));
    }

    let consulta = data.query.trim().to_string();
    if consulta.is_empty() {
        return Err(create_error!(InvalidOperation));
    }

    let id = Ulid::new().to_string();
    let recebe = estado.submit(Command {
        id: id.clone(),
        kind: "resolve".to_string(),
        query: consulta,
        // O teto existe para uma playlist enorme não virar uma fila que
        // ninguém consegue usar, nem uma resposta gigante.
        limit: data.limit.unwrap_or(25).clamp(1, 200),
        channel_id: None,
        track: None,
    });

    match tokio::time::timeout(ESPERA_MAXIMA, recebe).await {
        Ok(Ok(resultado)) => {
            if let Some(erro) = resultado.error {
                log::warn!("musicbox: o agente falhou: {erro}");
                return Err(create_error!(InternalError));
            }
            Ok(Json(ResolveResponse {
                tracks: resultado.tracks,
            }))
        }
        // O canal morreu sem resposta: o agente caiu no meio do trabalho.
        Ok(Err(_)) => {
            estado.forget(&id);
            Err(create_error!(InternalError))
        }
        // Estourou o tempo. Tirar o pedido da fila importa: sem isso, o mapa
        // de quem espera cresce para sempre com pedidos abandonados.
        Err(_) => {
            estado.forget(&id);
            Err(create_error!(InternalError))
        }
    }
}
