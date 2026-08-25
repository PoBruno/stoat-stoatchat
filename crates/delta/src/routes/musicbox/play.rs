use std::time::Duration;

use revolt_config::config;
use revolt_database::{
    util::{permissions::DatabasePermissionQuery, reference::Reference},
    voice::MUSICBOX_IDENTITY_PREFIX,
    Channel, Database, User,
};
use revolt_permissions::{calculate_channel_permissions, ChannelPermission};
use revolt_result::{create_error, Result};
use rocket::{serde::json::Json, State};
use rocket_empty::EmptyResponse;
use serde::Deserialize;
use ulid::Ulid;

use super::agent::AgentAuth;
use super::queue::{avancar, ChannelQueue, Queues, Repeat};
use super::state::{Command, MusicBoxState, Track};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DataEnqueue {
    /// Faixas a acrescentar, na ordem
    pub tracks: Vec<Track>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DataSettings {
    pub repeat: Option<Repeat>,
    pub shuffle: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DataProgress {
    pub channel_id: String,
    pub position_s: u32,
    /// A faixa chegou ao fim sozinha
    #[serde(default)]
    pub finished: bool,
}

/// Quem pode mexer na musica da chamada.
///
/// `UseMusicBox` e nao `Connect`: entrar na chamada e mandar na musica que
/// todo mundo ouve sao coisas diferentes. Numa chamada de dez pessoas, quem
/// pode escutar nao e necessariamente quem deve poder pular a faixa.
async fn canal_permitido(
    db: &Database,
    user: &User,
    alvo: &Reference<'_>,
) -> Result<Channel> {
    let channel = alvo.as_channel(db).await?;
    let mut query = DatabasePermissionQuery::new(db, user).channel(&channel);
    calculate_channel_permissions(&mut query)
        .await
        .throw_if_lacking_channel_permission(ChannelPermission::UseMusicBox)?;
    Ok(channel)
}

async fn exigir_agente(estado: &MusicBoxState) -> Result<()> {
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

    Ok(())
}

/// Manda o agente tocar o que está como atual, ou parar se não há nada.
///
/// Não espera resposta de propósito. Quem chama isto está atendendo um pedido
/// do navegador ou um aviso do agente, e nenhum dos dois deve ficar preso
/// esperando um download começar do outro lado do mundo.
fn sincronizar(estado: &MusicBoxState, channel_id: &str, fila: &ChannelQueue) {
    let comando = match (&fila.current, fila.playing) {
        (Some(faixa), true) => Command {
            id: Ulid::new().to_string(),
            kind: "play".to_string(),
            query: String::new(),
            limit: 0,
            channel_id: Some(channel_id.to_string()),
            track: Some(faixa.clone()),
        },
        _ => Command {
            id: Ulid::new().to_string(),
            kind: "stop".to_string(),
            query: String::new(),
            limit: 0,
            channel_id: Some(channel_id.to_string()),
            track: None,
        },
    };

    estado.submit_detached(comando);
}

/// Preenche a identidade do agente na sala.
///
/// Com bot configurado, e o id dele; sem bot, a identidade sintetica derivada
/// do canal. E o mesmo calculo do token, e precisa continuar batendo com
/// `create_musicbox_token` -- se divergir, o volume passa a ajustar um
/// participante que nao existe e nada acontece, silenciosamente.
async fn com_identidade(mut fila: ChannelQueue, channel_id: &str) -> ChannelQueue {
    let config = config().await;
    fila.bot_identity = Some(if config.musicbox.bot_user_id.is_empty() {
        format!("{MUSICBOX_IDENTITY_PREFIX}{channel_id}")
    } else {
        config.musicbox.bot_user_id.clone()
    });
    fila
}

/// # Fetch Queue
///
/// The music queue for this channel's call.
#[openapi(tag = "MusicBox")]
#[get("/<target>/queue")]
pub async fn fetch_queue(
    db: &State<Database>,
    filas: &State<Queues>,
    user: User,
    target: Reference<'_>,
) -> Result<Json<ChannelQueue>> {
    let channel = canal_permitido(db, &user, &target).await?;
    Ok(Json(
        com_identidade(filas.get(channel.id()), channel.id()).await,
    ))
}

/// # Enqueue Tracks
///
/// Adds tracks to the queue, starting playback if nothing is playing.
#[openapi(tag = "MusicBox")]
#[post("/<target>/queue", data = "<data>")]
pub async fn enqueue(
    db: &State<Database>,
    estado: &State<MusicBoxState>,
    filas: &State<Queues>,
    user: User,
    target: Reference<'_>,
    data: Json<DataEnqueue>,
) -> Result<Json<ChannelQueue>> {
    exigir_agente(estado).await?;
    let channel = canal_permitido(db, &user, &target).await?;

    if channel.voice().is_none() {
        return Err(create_error!(NotAVoiceChannel));
    }

    let mut comecou = false;
    let fila = filas.update(channel.id(), |f| {
        for faixa in data.into_inner().tracks {
            // Sem nada tocando, o que entra vira a atual em vez de esperar
            // numa fila que ninguém vai puxar.
            if f.current.is_none() {
                f.current = Some(faixa);
                f.position_s = 0;
                f.playing = true;
                comecou = true;
            } else {
                f.queue.push(faixa);
            }
        }
    });

    if comecou {
        sincronizar(estado, channel.id(), &fila);
    }

    Ok(Json(fila))
}

/// # Remove From Queue
///
/// Drops the track at this position in the queue.
#[openapi(tag = "MusicBox")]
#[delete("/<target>/queue/<index>")]
pub async fn dequeue(
    db: &State<Database>,
    filas: &State<Queues>,
    user: User,
    target: Reference<'_>,
    index: usize,
) -> Result<Json<ChannelQueue>> {
    let channel = canal_permitido(db, &user, &target).await?;

    Ok(Json(filas.update(channel.id(), |f| {
        if index < f.queue.len() {
            f.queue.remove(index);
        }
    })))
}

/// # Clear Queue
///
/// Empties the queue without stopping what is playing.
#[openapi(tag = "MusicBox")]
#[delete("/<target>/queue")]
pub async fn clear_queue(
    db: &State<Database>,
    filas: &State<Queues>,
    user: User,
    target: Reference<'_>,
) -> Result<Json<ChannelQueue>> {
    let channel = canal_permitido(db, &user, &target).await?;
    Ok(Json(filas.update(channel.id(), |f| f.queue.clear())))
}

/// # Jump To Queued Track
///
/// Plays a queued track right away, putting the current one back in front.
#[openapi(tag = "MusicBox")]
#[post("/<target>/queue/<index>/play")]
pub async fn play_queued(
    db: &State<Database>,
    estado: &State<MusicBoxState>,
    filas: &State<Queues>,
    user: User,
    target: Reference<'_>,
    index: usize,
) -> Result<Json<ChannelQueue>> {
    exigir_agente(estado).await?;
    let channel = canal_permitido(db, &user, &target).await?;

    let fila = filas.update(channel.id(), |f| {
        if index >= f.queue.len() {
            return;
        }
        let escolhida = f.queue.remove(index);
        if let Some(atual) = f.current.take() {
            f.queue.insert(0, atual);
        }
        f.current = Some(escolhida);
        f.position_s = 0;
        f.playing = true;
    });

    sincronizar(estado, channel.id(), &fila);
    Ok(Json(fila))
}

/// # Skip Track
///
/// Moves on to the next track.
#[openapi(tag = "MusicBox")]
#[post("/<target>/next")]
pub async fn next(
    db: &State<Database>,
    estado: &State<MusicBoxState>,
    filas: &State<Queues>,
    user: User,
    target: Reference<'_>,
) -> Result<Json<ChannelQueue>> {
    exigir_agente(estado).await?;
    let channel = canal_permitido(db, &user, &target).await?;

    // `true` porque isto é um pedido explícito: com repetição de uma faixa,
    // pular precisa pular mesmo.
    let fila = filas.update(channel.id(), |f| {
        avancar(f, true);
    });

    sincronizar(estado, channel.id(), &fila);
    Ok(Json(fila))
}

/// # Pause Or Resume
///
/// Toggles playback of the current track.
#[openapi(tag = "MusicBox")]
#[post("/<target>/toggle")]
pub async fn toggle(
    db: &State<Database>,
    estado: &State<MusicBoxState>,
    filas: &State<Queues>,
    user: User,
    target: Reference<'_>,
) -> Result<Json<ChannelQueue>> {
    exigir_agente(estado).await?;
    let channel = canal_permitido(db, &user, &target).await?;

    let fila = filas.update(channel.id(), |f| {
        if f.current.is_some() {
            f.playing = !f.playing;
        }
    });

    // Pausar aqui é parar de verdade: o agente não guarda a faixa em disco,
    // então retomar recomeça o download. Para música de fundo entre amigos
    // isso é aceitável, e evita segurar um processo parado por tempo
    // indefinido.
    sincronizar(estado, channel.id(), &fila);
    Ok(Json(fila))
}

/// # Stop Playback
///
/// Stops and forgets the queue for this channel.
#[openapi(tag = "MusicBox")]
#[post("/<target>/stop")]
pub async fn stop(
    db: &State<Database>,
    estado: &State<MusicBoxState>,
    filas: &State<Queues>,
    user: User,
    target: Reference<'_>,
) -> Result<EmptyResponse> {
    let channel = canal_permitido(db, &user, &target).await?;

    filas.clear(channel.id());
    sincronizar(estado, channel.id(), &ChannelQueue::default());

    Ok(EmptyResponse)
}

/// # Change Playback Settings
///
/// Sets repeat and shuffle for this channel.
#[openapi(tag = "MusicBox")]
#[patch("/<target>/settings", data = "<data>")]
pub async fn settings(
    db: &State<Database>,
    filas: &State<Queues>,
    user: User,
    target: Reference<'_>,
    data: Json<DataSettings>,
) -> Result<Json<ChannelQueue>> {
    let channel = canal_permitido(db, &user, &target).await?;
    let dados = data.into_inner();

    Ok(Json(filas.update(channel.id(), |f| {
        if let Some(r) = dados.repeat {
            f.repeat = r;
        }
        if let Some(s) = dados.shuffle {
            f.shuffle = s;
        }
    })))
}

/// # Report Progress
///
/// The agent tells the server how far along it is, and when a track ended.
///
/// É o que permite a barra andar e a fila seguir sozinha. Sem isso o servidor
/// só saberia que a música acabou quando alguém reclamasse do silêncio.
#[openapi(tag = "MusicBox")]
#[post("/agent/progress", data = "<data>")]
pub async fn progress(
    _auth: AgentAuth,
    estado: &State<MusicBoxState>,
    filas: &State<Queues>,
    data: Json<DataProgress>,
) -> Result<EmptyResponse> {
    let dados = data.into_inner();
    estado.agent_checked_in();

    if !dados.finished {
        filas.report_position(&dados.channel_id, dados.position_s);
        return Ok(EmptyResponse);
    }

    // `false`: a faixa acabou sozinha, então repetir-uma deve repetir.
    let fila = filas.update(&dados.channel_id, |f| {
        avancar(f, false);
    });

    sincronizar(estado, &dados.channel_id, &fila);
    Ok(EmptyResponse)
}
