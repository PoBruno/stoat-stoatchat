use std::time::Duration;

use revolt_config::config;
use revolt_result::{create_error, Error, Result};
use revolt_rocket_okapi::{
    gen::OpenApiGenerator,
    request::{OpenApiFromRequest, RequestHeaderInput},
    revolt_okapi::openapi3::{Parameter, ParameterValue},
    OpenApiError,
};
use rocket::{
    http::Status,
    request::{FromRequest, Outcome, Request},
    serde::json::Json,
    State,
};
use rocket_empty::EmptyResponse;
use schemars::schema::{InstanceType, SchemaObject, SingleOrVec};

use super::state::{Command, CommandResult, MusicBoxState};

/// Prova de que quem chama é o agente de música.
///
/// O segredo é comparado inteiro, sem atalho: não vale sair mais cedo na
/// primeira letra diferente, porque o tempo da resposta contaria ao chamador
/// quantas letras ele já acertou.
pub struct AgentAuth;
/// Comparação em tempo constante.
fn iguais(a: &str, b: &str) -> bool {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[async_trait]
impl<'r> FromRequest<'r> for AgentAuth {
    type Error = Error;

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let esperado = config().await.musicbox.agent_secret.clone();

        // Segredo vazio significa recurso desligado. Sem isto, uma instância
        // que não configurou nada aceitaria qualquer agente que mandasse um
        // cabeçalho vazio.
        if esperado.is_empty() {
            return Outcome::Error((Status::NotFound, create_error!(NotFound)));
        }

        match req.headers().get_one("x-musicbox-secret") {
            Some(dado) if iguais(dado, &esperado) => Outcome::Success(AgentAuth),
            _ => Outcome::Error((Status::Unauthorized, create_error!(InvalidCredentials))),
        }
    }
}

impl OpenApiFromRequest<'_> for AgentAuth {
    fn from_request_input(
        _gen: &mut OpenApiGenerator,
        _name: String,
        _required: bool,
    ) -> Result<RequestHeaderInput, OpenApiError> {
        Ok(RequestHeaderInput::Parameter(Parameter {
            name: "X-MusicBox-Secret".to_string(),
            description: Some("Shared secret proving this is the music agent.".to_string()),
            allow_empty_value: false,
            required: true,
            deprecated: false,
            extensions: schemars::Map::new(),
            location: "header".to_string(),
            value: ParameterValue::Schema {
                allow_reserved: false,
                example: None,
                examples: None,
                explode: None,
                style: None,
                schema: SchemaObject {
                    instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::String))),
                    ..Default::default()
                },
            },
        }))
    }
}

/// # Agent Heartbeat
///
/// Tells the server an extraction agent is alive and reachable.
///
/// Quando o agente volta depois de sumir, isto remanda o que cada canal
/// acredita estar tocando. Sem esse reencontro, um reinício do agente deixaria
/// o servidor anunciando música sobre o silêncio — e a barra correndo sozinha.
#[openapi(tag = "MusicBox")]
#[post("/agent/heartbeat")]
pub async fn heartbeat(
    _auth: AgentAuth,
    estado: &State<MusicBoxState>,
    filas: &State<super::queue::Queues>,
) -> Result<EmptyResponse> {
    let config = config().await;
    let voltou = estado.agent_checked_in_with_timeout(Duration::from_secs(
        config.musicbox.agent_timeout_seconds,
    ));

    if voltou {
        for (canal, fila) in filas.playing_channels() {
            if let Some(faixa) = fila.current {
                log::info!("musicbox: agente voltou; retomando {canal}");
                estado.submit_detached(Command {
                    id: ulid::Ulid::new().to_string(),
                    kind: "play".to_string(),
                    query: String::new(),
                    limit: 0,
                    channel_id: Some(canal),
                    track: Some(faixa),
                });
            }
        }
    }

    Ok(EmptyResponse)
}

/// # Take Work
///
/// Long-polls for a command. Responds with `null` when there is nothing to do.
///
/// O agente vive atrás de NAT e não aceita conexões, então é ele quem procura
/// o servidor e fica pendurado esperando. Devolver logo "nada a fazer" faria o
/// agente perguntar de novo em seguida, e a busca só sairia no próximo ciclo.
///
/// A resposta é sempre 200 com corpo — `null` quando não há trabalho. Um
/// `Option` nu viraria 404 no Rocket, que é exatamente o código que o agente
/// usa para concluir "esta rota não existe no servidor": ele se desligaria por
/// meio minuto a cada espera vazia.
#[openapi(tag = "MusicBox")]
#[get("/agent/commands")]
pub async fn take_work(
    _auth: AgentAuth,
    estado: &State<MusicBoxState>,
) -> Result<Json<Option<Command>>> {
    let config = config().await;

    // Pendurar-se também conta como sinal de vida: um agente esperando
    // trabalho está tão presente quanto um batendo ponto.
    estado.agent_checked_in();

    let espera = Duration::from_secs(config.musicbox.poll_seconds);
    Ok(Json(estado.wait_for_command(espera).await))
}

/// # Hand In Result
///
/// Returns the tracks the agent resolved for a command.
#[openapi(tag = "MusicBox")]
#[post("/agent/result", data = "<data>")]
pub async fn hand_in(
    _auth: AgentAuth,
    estado: &State<MusicBoxState>,
    data: Json<CommandResult>,
) -> Result<EmptyResponse> {
    estado.agent_checked_in();

    if !estado.deliver(data.into_inner()) {
        // Não é erro do agente: quem pediu desistiu de esperar. Vale registrar
        // porque significa que a extração demorou mais do que o pedido aguenta.
        log::info!("musicbox: resultado chegou sem ninguém esperando por ele");
    }

    Ok(EmptyResponse)
}

#[cfg(test)]
mod test {
    use super::iguais;

    #[test]
    fn comparacao_de_segredo() {
        assert!(iguais("abc", "abc"));
        assert!(!iguais("abc", "abd"));
        assert!(!iguais("abc", "ab"), "tamanhos diferentes nunca sao iguais");
        assert!(!iguais("", "a"));
        assert!(iguais("", ""));
    }
}
