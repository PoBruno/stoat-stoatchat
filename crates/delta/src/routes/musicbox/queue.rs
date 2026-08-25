use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

use serde::{Deserialize, Serialize};

use super::state::Track;

/// Como a repetição se comporta ao terminar uma faixa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Repeat {
    Off,
    One,
    All,
}

impl Default for Repeat {
    fn default() -> Self {
        Repeat::Off
    }
}

/// O que a chamada de um canal está tocando.
///
/// Vive no servidor, e não no navegador de quem pediu, porque é estado
/// **da chamada**: quem entra depois precisa ver a mesma fila, e fechar a aba
/// não pode apagar o que os outros estão ouvindo.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ChannelQueue {
    pub playing: bool,
    pub current: Option<Track>,
    /// Segundos já tocados da faixa atual
    pub position_s: u32,
    pub queue: Vec<Track>,
    pub repeat: Repeat,
    pub shuffle: bool,
}

struct Interno {
    filas: HashMap<String, ChannelQueue>,
    /// Quando a posição de cada canal foi atualizada pela última vez
    marcado: HashMap<String, Instant>,
}

/// Filas de todos os canais.
pub struct Queues {
    inner: Mutex<Interno>,
}

impl Default for Queues {
    fn default() -> Self {
        Self::new()
    }
}

impl Queues {
    pub fn new() -> Self {
        Queues {
            inner: Mutex::new(Interno {
                filas: HashMap::new(),
                marcado: HashMap::new(),
            }),
        }
    }

    /// Estado de um canal, com a posição projetada até agora.
    ///
    /// A posição é calculada e não armazenada a cada segundo: o agente informa
    /// de vez em quando, e entre um aviso e outro o relógio daqui preenche a
    /// lacuna. Guardar a cada segundo obrigaria a transmitir a cada segundo.
    ///
    /// A projeção é limitada pela duração da faixa. Sem esse teto, um agente
    /// que nunca começou a tocar faria a barra correr assim mesmo — e uma
    /// barra que passa do fim da música é pior que uma parada, porque parece
    /// que está tocando.
    pub fn get(&self, channel_id: &str) -> ChannelQueue {
        let inner = self.inner.lock().expect("musicbox queues");
        let mut fila = inner.filas.get(channel_id).cloned().unwrap_or_default();

        if fila.playing {
            if let Some(quando) = inner.marcado.get(channel_id) {
                let projetada = fila.position_s + quando.elapsed().as_secs() as u32;

                fila.position_s = match fila.current.as_ref().and_then(|t| t.duration_s) {
                    // Duração zero ou ausente é transmissão ao vivo: não há fim
                    // para limitar contra.
                    Some(duracao) if duracao > 0 => projetada.min(duracao),
                    _ => projetada,
                };
            }
        }

        fila
    }

    /// Altera a fila de um canal e devolve como ela ficou.
    pub fn update<F>(&self, channel_id: &str, alterar: F) -> ChannelQueue
    where
        F: FnOnce(&mut ChannelQueue),
    {
        let mut inner = self.inner.lock().expect("musicbox queues");
        let fila = inner.filas.entry(channel_id.to_string()).or_default();

        let antes = (fila.current.as_ref().map(|t| t.id.clone()), fila.playing);
        alterar(fila);
        let depois = (fila.current.as_ref().map(|t| t.id.clone()), fila.playing);

        let resultado = fila.clone();

        // Reinicia a contagem quando a faixa muda ou a reprodução para e
        // volta; sem isso a posição projetada continuaria correndo por cima da
        // faixa nova.
        if antes != depois {
            inner.marcado.insert(channel_id.to_string(), Instant::now());
        }

        resultado
    }

    /// Marca a posição informada pelo agente.
    pub fn report_position(&self, channel_id: &str, position_s: u32) {
        let mut inner = self.inner.lock().expect("musicbox queues");
        if let Some(fila) = inner.filas.get_mut(channel_id) {
            fila.position_s = position_s;
        }
        inner.marcado.insert(channel_id.to_string(), Instant::now());
    }

    /// Canais que acreditam estar tocando algo.
    ///
    /// Serve para remandar a música quando o agente volta de um reinício.
    pub fn playing_channels(&self) -> Vec<(String, ChannelQueue)> {
        let inner = self.inner.lock().expect("musicbox queues");
        inner
            .filas
            .iter()
            .filter(|(_, f)| f.playing && f.current.is_some())
            .map(|(id, f)| (id.clone(), f.clone()))
            .collect()
    }

    /// Esquece um canal, quando a chamada acaba.
    pub fn clear(&self, channel_id: &str) {
        let mut inner = self.inner.lock().expect("musicbox queues");
        inner.filas.remove(channel_id);
        inner.marcado.remove(channel_id);
    }
}

/// Escolhe a próxima faixa e a coloca como atual.
///
/// Devolve `true` se há algo tocando depois disso.
///
/// `saltou` distingue "a faixa acabou" de "alguém apertou próxima". Só no
/// primeiro caso a repetição de uma faixa a repete: repetir depois de um
/// pedido explícito seria ignorar o comando.
pub fn avancar(fila: &mut ChannelQueue, saltou: bool) -> bool {
    if !saltou && fila.repeat == Repeat::One {
        if fila.current.is_some() {
            fila.position_s = 0;
            fila.playing = true;
            return true;
        }
    }

    let anterior = fila.current.take();

    if fila.queue.is_empty() {
        // Com repetição de tudo e nada na fila, a única faixa volta ao começo.
        if fila.repeat == Repeat::All {
            if let Some(faixa) = anterior {
                fila.current = Some(faixa);
                fila.position_s = 0;
                fila.playing = true;
                return true;
            }
        }
        fila.playing = false;
        fila.position_s = 0;
        return false;
    }

    let indice = if fila.shuffle {
        // Aleatório sem dependência nova: o relógio basta para um grupo de
        // amigos escolhendo música.
        (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(0))
            % fila.queue.len()
    } else {
        0
    };

    let proxima = fila.queue.remove(indice);

    if fila.repeat == Repeat::All {
        if let Some(faixa) = anterior {
            fila.queue.push(faixa);
        }
    }

    fila.current = Some(proxima);
    fila.position_s = 0;
    fila.playing = true;
    true
}

#[cfg(test)]
mod test {
    use super::*;

    fn faixa(id: &str) -> Track {
        Track {
            id: id.to_string(),
            provider: "youtube".to_string(),
            title: id.to_string(),
            author: None,
            duration_s: Some(100),
            cover_url: None,
            page_url: format!("https://exemplo/{id}"),
        }
    }

    #[test]
    fn avanca_puxando_da_frente() {
        let mut f = ChannelQueue {
            current: Some(faixa("a")),
            queue: vec![faixa("b"), faixa("c")],
            ..Default::default()
        };

        assert!(avancar(&mut f, false));
        assert_eq!(f.current.as_ref().unwrap().id, "b");
        assert_eq!(f.queue.len(), 1);
        assert!(f.playing);
    }

    #[test]
    fn fila_vazia_para_a_reproducao() {
        let mut f = ChannelQueue {
            current: Some(faixa("a")),
            playing: true,
            ..Default::default()
        };

        assert!(!avancar(&mut f, false));
        assert!(f.current.is_none());
        assert!(!f.playing, "sem nada para tocar, nao pode seguir 'tocando'");
    }

    #[test]
    fn repetir_uma_so_vale_no_fim_natural() {
        let mut f = ChannelQueue {
            current: Some(faixa("a")),
            queue: vec![faixa("b")],
            repeat: Repeat::One,
            ..Default::default()
        };

        // Fim natural: repete a mesma.
        avancar(&mut f, false);
        assert_eq!(f.current.as_ref().unwrap().id, "a");
        assert_eq!(f.queue.len(), 1, "a fila nao foi tocada");

        // Pedido explicito: ganha da repeticao.
        avancar(&mut f, true);
        assert_eq!(
            f.current.as_ref().unwrap().id,
            "b",
            "apertar proxima deve pular mesmo com repetir-uma"
        );
    }

    #[test]
    fn repetir_tudo_devolve_a_faixa_ao_fim_da_fila() {
        let mut f = ChannelQueue {
            current: Some(faixa("a")),
            queue: vec![faixa("b")],
            repeat: Repeat::All,
            ..Default::default()
        };

        avancar(&mut f, false);
        assert_eq!(f.current.as_ref().unwrap().id, "b");
        assert_eq!(f.queue.len(), 1);
        assert_eq!(f.queue[0].id, "a", "a que saiu volta para o fim");
    }

    #[test]
    fn repetir_tudo_com_uma_so_faixa_recomeca() {
        let mut f = ChannelQueue {
            current: Some(faixa("a")),
            repeat: Repeat::All,
            ..Default::default()
        };

        assert!(avancar(&mut f, false));
        assert_eq!(f.current.as_ref().unwrap().id, "a");
        assert_eq!(f.position_s, 0);
    }

    #[test]
    fn posicao_projetada_so_corre_tocando() {
        let filas = Queues::new();

        filas.update("c1", |f| {
            f.current = Some(faixa("a"));
            f.playing = false;
            f.position_s = 30;
        });

        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert_eq!(
            filas.get("c1").position_s,
            30,
            "pausado, a posicao nao pode andar"
        );
    }

    #[test]
    fn posicao_projetada_nao_passa_do_fim_da_faixa() {
        let filas = Queues::new();

        filas.update("c1", |f| {
            f.current = Some(Track {
                duration_s: Some(1),
                ..faixa("curta")
            });
            f.playing = true;
            f.position_s = 0;
        });

        std::thread::sleep(std::time::Duration::from_millis(2100));
        assert_eq!(
            filas.get("c1").position_s,
            1,
            "a barra nao pode passar do fim: pareceria que ainda toca"
        );
    }

    #[test]
    fn ao_vivo_nao_tem_teto() {
        let filas = Queues::new();

        filas.update("c1", |f| {
            f.current = Some(Track {
                duration_s: None,
                ..faixa("ao-vivo")
            });
            f.playing = true;
            f.position_s = 0;
        });

        std::thread::sleep(std::time::Duration::from_millis(1100));
        assert!(
            filas.get("c1").position_s >= 1,
            "sem duracao conhecida, a contagem segue"
        );
    }

    #[test]
    fn canal_desconhecido_devolve_vazio() {
        let filas = Queues::new();
        let f = filas.get("nunca-visto");
        assert!(f.current.is_none());
        assert!(!f.playing);
        assert!(f.queue.is_empty());
    }
}
