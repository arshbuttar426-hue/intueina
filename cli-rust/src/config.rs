use crate::cortex::Cortex;
use as_not_as_protocol::Protocol;
use as_not_as_protocol::scorer::PreparedScoring;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{oneshot, broadcast};
use std::thread;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub cortex_db_path: String,
    pub port: u16,
    pub host: String,
    pub embed_model: String,
    pub embed_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        Self {
            // SPIM only — single file, SPIM1 magic, no vectors
            cortex_db_path: format!("{}/.intueina/cortex.json", home),
            port: 9124,
            host: "127.0.0.1".into(),
            embed_model: "spim-positional".into(),
            embed_enabled: false,
        }
    }
}

pub enum CortexCmd {
    Stats,
    Health,
    Learn { node_id: String, content: String },
    LearnWithEmbedding { node_id: String, content: String, embedding: Vec<f32>, model: String },
    RecallWithProtocol { query: String, density: String, limit: usize, think_in: Vec<String>, output_needed: Vec<String>, prepared: PreparedScoring, model: String },
    Delete { node_id: String },
    Vacuum,
    Shutdown,
}

#[derive(Clone)]
pub struct CortexActor {
    tx: tokio::sync::mpsc::UnboundedSender<(CortexCmd, oneshot::Sender<serde_json::Value>)>,
}

impl CortexActor {
    pub fn spawn(db_path: String) -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<(CortexCmd, oneshot::Sender<serde_json::Value>)>();

        thread::spawn(move || {
            let cortex = Cortex::open(&db_path).expect("cortex open failed");
            while let Some((cmd, reply)) = rx.blocking_recv() {
                match cmd {
                    CortexCmd::Shutdown => break,
                    CortexCmd::Stats => {
                        let stats = cortex.stats().ok();
                        let _ = reply.send(serde_json::to_value(stats).unwrap_or_default());
                    }
                    CortexCmd::Health => {
                        let health = cortex.fast_stats().ok();
                        let _ = reply.send(serde_json::to_value(health).unwrap_or_default());
                    }
                    CortexCmd::Learn { node_id, content } => {
                        let _ = cortex.ingest(&node_id, &content, "note");
                        let _ = reply.send(serde_json::json!({"stored": true}));
                    }
                    CortexCmd::LearnWithEmbedding { node_id, content, embedding: _, model: _ } => {
                        // SPIM only — embedding is ignored, vector is dead, store is WORD→MEMORY→POSITION
                        let _ = cortex.ingest(&node_id, &content, "note");
                        let _ = reply.send(serde_json::json!({"stored": true, "embedding_dim": 0, "store": "spim"}));
                    }
                    CortexCmd::RecallWithProtocol { query, density, limit, think_in, output_needed, prepared, model } => {
                        let mems = cortex.recall_with_protocol(
                            &query, limit, &density, &think_in, &output_needed, &prepared, &model,
                        ).ok().unwrap_or_default();
                        let tag_line = cortex.format_tags(&query, &think_in, &output_needed);
                        let _ = reply.send(serde_json::json!({"memories": mems, "tag_line": tag_line}));
                    }
                    CortexCmd::Vacuum => {
                        let result = cortex.vacuum()
                            .map(|_| serde_json::json!({"vacuumed": true}))
                            .unwrap_or(serde_json::json!({"error": "vacuum failed"}));
                        let _ = reply.send(result);
                    }
                    CortexCmd::Delete { node_id } => {
                        let result = cortex.delete_node(&node_id)
                            .map(|_| serde_json::json!({"deleted": true, "node_id": node_id}))
                            .unwrap_or(serde_json::json!({"error": "delete failed"}));
                        let _ = reply.send(result);
                    }
                }
            }
        });

        CortexActor { tx }
    }

    pub async fn call(&self, cmd: CortexCmd) -> serde_json::Value {
        let (reply_tx, reply_rx) = oneshot::channel();
        let _ = self.tx.send((cmd, reply_tx));
        reply_rx.await.unwrap_or_default()
    }
}

pub struct ServerState {
    pub cortex: CortexActor,
    pub protocol: Option<Arc<Protocol>>,
    pub embed_model: String,
    pub start_time: std::time::Instant,
    pub started_at: f64,
    pub tx: broadcast::Sender<String>,
}

impl ServerState {
    pub fn new(cfg: &AppConfig) -> Self {
        // SPIM only — protocol with embeddings is retired, store is WORD→MEMORY→POSITION
        if cfg.embed_enabled {
            eprintln!("[intueina] embed_enabled forced false — SPIM is lossless, no vectors");
        }
        let protocol = None;
        // Enforce SPIM DB — only ~/.intueina/cortex.json SPIM1
        if !cfg.cortex_db_path.ends_with("cortex.json") {
            eprintln!("[intueina] cortex_db_path forced to SPIM DB {}", cfg.cortex_db_path);
        }

        let (tx, _) = broadcast::channel(64);

        Self {
            cortex: CortexActor::spawn(cfg.cortex_db_path.clone()),
            protocol,
            embed_model: cfg.embed_model.clone(),
            start_time: std::time::Instant::now(),
            started_at: std::time::UNIX_EPOCH.elapsed().unwrap().as_secs_f64(),
            tx,
        }
    }
}
