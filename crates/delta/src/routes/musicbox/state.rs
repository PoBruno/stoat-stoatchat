use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::{oneshot, Notify};

/// Uma faixa resolvida pelo agente.
///
/// Espelha o formato que o agente devolve. Fica declarado aqui, e não em
/// `revolt_models`, porque nada disso é persistido: é um recado de ida e
/// volta entre o navegador e a máquina que extrai.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Track {
    pub id: String,
    pub provider: String,
    pub title: String,
    pub author: Option<String>,
    /// Duração em segundos; nulo em transmissão ao vivo
    pub duration_s: Option<u32>,
    pub cover_url: Option<String>,
    pub page_url: String,
}

/// Trabalho entregue ao agente.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Command {
    pub id: String,
    /// Nome para buscar, ou URL de vídeo ou playlist
    pub query: String,
    pub limit: u16,
}

/// O que o agente devolve depois de trabalhar.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CommandResult {
    pub id: String,
    #[serde(default)]
    pub tracks: Vec<Track>,
    /// Preenchido quando o agente não conseguiu
    #[serde(default)]
    pub error: Option<String>,
    /// Motivo legível por máquina: bloqueado, nao_encontrado, ...
    #[serde(default)]
    pub reason: Option<String>,
}

struct Inner {
    /// Quando o agente falou pela última vez
    agent_seen: Option<Instant>,
    /// Trabalho esperando alguém pegar
    pending: VecDeque<Command>,
    /// Quem está esperando a resposta de cada comando
    waiting: HashMap<String, oneshot::Sender<CommandResult>>,
}

/// Encontro entre quem pede música e o agente que sabe achá-la.
///
/// Mora na memória do processo, e não no Redis, porque o que se guarda aqui
/// é um `oneshot::Sender` — uma requisição HTTP parada esperando resposta.
/// Isso não atravessa processos: só o processo que está segurando a conexão
/// do usuário pode respondê-la.
///
/// A consequência é que **isto assume um único delta**. Com mais de uma
/// instância atrás de um balanceador, o agente conversaria com uma delas e os
/// pedidos feitos nas outras ficariam sem resposta. Se um dia houver mais de
/// uma, isto precisa virar pub/sub no Redis.
pub struct MusicBoxState {
    inner: Mutex<Inner>,
    /// Acorda o agente que está pendurado esperando trabalho
    chegou_trabalho: Notify,
}

impl Default for MusicBoxState {
    fn default() -> Self {
        Self::new()
    }
}

impl MusicBoxState {
    pub fn new() -> Self {
        MusicBoxState {
            inner: Mutex::new(Inner {
                agent_seen: None,
                pending: VecDeque::new(),
                waiting: HashMap::new(),
            }),
            chegou_trabalho: Notify::new(),
        }
    }

    /// Marca que o agente está vivo.
    pub fn agent_checked_in(&self) {
        let mut inner = self.inner.lock().expect("musicbox state");
        inner.agent_seen = Some(Instant::now());
    }

    /// Há um agente que falou faz pouco tempo?
    pub fn agent_present(&self, timeout: Duration) -> bool {
        let inner = self.inner.lock().expect("musicbox state");
        inner
            .agent_seen
            .map(|quando| quando.elapsed() < timeout)
            .unwrap_or(false)
    }

    /// Enfileira trabalho e devolve por onde a resposta vai chegar.
    pub fn submit(&self, command: Command) -> oneshot::Receiver<CommandResult> {
        let (envia, recebe) = oneshot::channel();

        {
            let mut inner = self.inner.lock().expect("musicbox state");
            inner.waiting.insert(command.id.clone(), envia);
            inner.pending.push_back(command);
        }

        self.chegou_trabalho.notify_one();
        recebe
    }

    /// Pega trabalho, se houver algum agora.
    pub fn take_command(&self) -> Option<Command> {
        let mut inner = self.inner.lock().expect("musicbox state");
        inner.pending.pop_front()
    }

    /// Espera aparecer trabalho, até o limite de tempo.
    ///
    /// Segurar a requisição do agente é o que permite o servidor mandar
    /// trabalho para uma máquina que não aceita conexões de fora.
    pub async fn wait_for_command(&self, limite: Duration) -> Option<Command> {
        if let Some(c) = self.take_command() {
            return Some(c);
        }

        // O `notified()` é criado antes da checagem seguinte de propósito: se
        // um comando chegar entre a tentativa acima e o início da espera, o
        // aviso fica guardado e o `await` retorna na hora, em vez de dormir
        // até o limite com trabalho parado na fila.
        let aviso = self.chegou_trabalho.notified();
        tokio::pin!(aviso);

        match tokio::time::timeout(limite, aviso).await {
            Ok(()) => self.take_command(),
            Err(_) => None,
        }
    }

    /// Entrega o resultado a quem estava esperando.
    ///
    /// Devolve `false` quando ninguém espera mais — o pedido desistiu por
    /// tempo. Vale avisar em vez de engolir: resultado sem dono significa que
    /// o agente demorou mais do que o cliente aguenta.
    pub fn deliver(&self, resultado: CommandResult) -> bool {
        let envia = {
            let mut inner = self.inner.lock().expect("musicbox state");
            inner.waiting.remove(&resultado.id)
        };

        match envia {
            Some(canal) => canal.send(resultado).is_ok(),
            None => false,
        }
    }

    /// Desiste de um pedido, para o mapa não crescer para sempre com pedidos
    /// que ninguém respondeu.
    pub fn forget(&self, id: &str) {
        let mut inner = self.inner.lock().expect("musicbox state");
        inner.waiting.remove(id);
        inner.pending.retain(|c| c.id != id);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn entrega_o_resultado_a_quem_pediu() {
        let estado = MusicBoxState::new();
        let recebe = estado.submit(Command {
            id: "a".to_string(),
            query: "coisa".to_string(),
            limit: 5,
        });

        let pego = estado.take_command().expect("havia trabalho");
        assert_eq!(pego.id, "a");

        assert!(estado.deliver(CommandResult {
            id: "a".to_string(),
            tracks: vec![],
            error: None,
            reason: None,
        }));

        assert_eq!(recebe.await.expect("resposta chegou").id, "a");
    }

    #[tokio::test]
    async fn resultado_sem_dono_e_reportado() {
        let estado = MusicBoxState::new();
        assert!(
            !estado.deliver(CommandResult {
                id: "fantasma".to_string(),
                tracks: vec![],
                error: None,
                reason: None,
            }),
            "entregar para ninguem deve devolver false, nao fingir sucesso"
        );
    }

    #[tokio::test]
    async fn desistir_limpa_a_fila_e_o_mapa() {
        let estado = MusicBoxState::new();
        let _recebe = estado.submit(Command {
            id: "b".to_string(),
            query: "coisa".to_string(),
            limit: 5,
        });

        estado.forget("b");

        assert!(estado.take_command().is_none(), "a fila deveria estar vazia");
        assert!(
            !estado.deliver(CommandResult {
                id: "b".to_string(),
                tracks: vec![],
                error: None,
                reason: None,
            }),
            "ninguem mais espera por b"
        );
    }

    #[tokio::test]
    async fn a_espera_acorda_quando_chega_trabalho() {
        use std::sync::Arc;

        let estado = Arc::new(MusicBoxState::new());
        let outro = estado.clone();

        // Manda trabalho depois que a espera ja comecou.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(50)).await;
            outro.submit(Command {
                id: "c".to_string(),
                query: "coisa".to_string(),
                limit: 5,
            });
        });

        let pego = estado.wait_for_command(Duration::from_secs(5)).await;
        assert_eq!(pego.expect("acordou com trabalho").id, "c");
    }

    #[tokio::test]
    async fn a_espera_desiste_no_tempo_limite() {
        let estado = MusicBoxState::new();
        let pego = estado.wait_for_command(Duration::from_millis(80)).await;
        assert!(pego.is_none(), "sem trabalho, deve voltar vazio");
    }

    #[tokio::test]
    async fn agente_ausente_ate_bater_ponto() {
        let estado = MusicBoxState::new();
        assert!(!estado.agent_present(Duration::from_secs(60)));

        estado.agent_checked_in();
        assert!(estado.agent_present(Duration::from_secs(60)));

        // Com uma janela de tolerancia de zero, nem o batimento recem-feito
        // conta como presente.
        assert!(!estado.agent_present(Duration::from_millis(0)));
    }
}
