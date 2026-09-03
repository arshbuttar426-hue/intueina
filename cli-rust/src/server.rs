use std::sync::Arc;
use axum::{
    Router, routing::{delete, get, post}, Json, extract::{Path, State},
    response::IntoResponse,
};
use axum::extract::ws::{WebSocketUpgrade, WebSocket, Message};
use futures::{SinkExt, StreamExt};
use serde_json::{json, Value};
use crate::config::{ServerState, AppConfig, CortexCmd};
use uuid::Uuid;

pub async fn serve(config: AppConfig) {
    let state = Arc::new(ServerState::new(&config));

    let app = Router::new()
        .route("/health", get(health))
        .route("/ws", get(ws_handler))
        .route("/v1/learn", post(learn))
        .route("/v1/recall", post(recall))
        .route("/v1/vacuum", post(vacuum))
        .route("/v1/nodes/{node_id}", delete(delete_node))
        .route("/v1/stats", get(stats))
        .with_state(state);

    let addr = format!("{}:{}", config.host, config.port);
    println!("[intueina] memory server listening on http://{}", addr);
    match tokio::net::TcpListener::bind(&addr).await {
        Ok(listener) => {
            if let Err(e) = axum::serve(listener, app).await {
                eprintln!("[intueina] server error: {e}");
            }
        }
        Err(e) => {
            eprintln!("[intueina] failed to bind {addr}: {e}");
        }
    }
}

async fn health(state: State<Arc<ServerState>>) -> Json<Value> {
    let cortex_health = state.cortex.call(CortexCmd::Health).await;
    Json(json!({
        "version": "0.4.0",
        "cortex": cortex_health,
        "protocol_loaded": state.protocol.is_some(),
        "uptime_seconds": state.start_time.elapsed().as_secs_f64(),
        "started_at": state.started_at,
        "status": "healthy",
    }))
}

#[derive(serde::Deserialize)]
struct LearnRequest {
    content: String,
}

async fn learn(
    state: State<Arc<ServerState>>,
    Json(body): Json<LearnRequest>,
) -> Json<Value> {
    let node_id = format!("learn_{}", Uuid::new_v4());

    if let Some(ref protocol) = state.protocol {
        let protocol = Arc::clone(protocol);
        let embed_model = state.embed_model.clone();
        let content = body.content.clone();
        let node_id_clone = node_id.clone();

        let emb = tokio::task::spawn_blocking(move || {
            protocol.embed(&content).map_err(|e| e.to_string())
        }).await;

        match emb {
            Ok(Ok(vec)) => {
                let _ = state.cortex.call(CortexCmd::LearnWithEmbedding {
                    node_id: node_id_clone.clone(),
                    content: body.content.clone(),
                    embedding: vec,
                    model: embed_model,
                }).await;
            }
            _ => {
                let _ = state.cortex.call(CortexCmd::Learn {
                    node_id: node_id_clone,
                    content: body.content.clone(),
                }).await;
            }
        }
    } else {
        let _ = state.cortex.call(CortexCmd::Learn {
            node_id: node_id.clone(),
            content: body.content.clone(),
        }).await;
    }

    // Broadcast learn event
    let _ = state.tx.send(json!({
        "event": "learn",
        "data": {
            "node_id": node_id,
            "content_preview": body.content.chars().take(50).collect::<String>(),
        }
    }).to_string());

    Json(json!({
        "stored": true,
        "node_id": node_id,
        "embedding_enabled": state.protocol.is_some(),
    }))
}

#[derive(serde::Deserialize)]
struct RecallRequest {
    query: String,
    limit: Option<usize>,
    density: Option<String>,
}

#[axum::debug_handler]
async fn recall(
    state: State<Arc<ServerState>>,
    Json(body): Json<RecallRequest>,
) -> Json<Value> {
    let result = {
        // SPIM only — protocol with embeddings is retired, but recall must still work
        let prepared = state.protocol.as_ref()
            .and_then(|p| p.prepare(&body.query).ok())
            .unwrap_or_else(|| as_not_as_protocol::scorer::PreparedScoring { query_vec: vec![0.0; 1], term_embeddings: vec![] });
        state.cortex.call(CortexCmd::RecallWithProtocol {
            query: body.query.clone(),
            density: body.density.unwrap_or_else(|| "balanced".into()),
            limit: body.limit.unwrap_or(5),
            think_in: vec![],
            output_needed: vec![],
            prepared,
            model: state.embed_model.clone(),
        }).await
    };

    // Broadcast recall event
    let _ = state.tx.send(json!({
        "event": "recall",
        "data": {
            "query": body.query,
            "count": result.get("memories").and_then(|m| m.as_array()).map(|a| a.len()).unwrap_or(0),
        }
    }).to_string());

    Json(json!({
        "memories": result.get("memories"),
        "tag_line": result.get("tag_line"),
    }))
}

async fn vacuum(state: State<Arc<ServerState>>) -> Json<Value> {
    let result = state.cortex.call(CortexCmd::Vacuum).await;
    Json(json!({
        "cortex": result,
    }))
}

async fn delete_node(
    state: State<Arc<ServerState>>,
    Path(node_id): Path<String>,
) -> Json<Value> {
    let result = state.cortex.call(CortexCmd::Delete { node_id }).await;
    Json(result)
}

async fn stats(state: State<Arc<ServerState>>) -> Json<Value> {
    let cortex_stats = state.cortex.call(CortexCmd::Stats).await;
    Json(json!({
        "cortex": cortex_stats,
        "uptime_seconds": state.start_time.elapsed().as_secs_f64(),
        "started_at": state.started_at,
    }))
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<ServerState>>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_ws(socket, state))
}

async fn handle_ws(socket: WebSocket, state: Arc<ServerState>) {
    let (mut sender, mut receiver) = socket.split();
    let mut rx = state.tx.subscribe();

    // Send boot state
    let boot = json!({
        "event": "boot",
        "data": {
            "version": "0.4.0",
            "embed_model": state.embed_model,
        }
    });
    let _ = sender.send(Message::Text(boot.to_string().into())).await;

    // Forward broadcast messages to client
    let mut send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            if sender.send(Message::Text(msg.into())).await.is_err() {
                break;
            }
        }
    });

    // Client messages (heartbeat/pong)
    let mut recv_task = tokio::spawn(async move {
        while let Some(Ok(_)) = receiver.next().await {}
    });

    // Wait for either task to finish
    tokio::select! {
        _ = &mut send_task => recv_task.abort(),
        _ = &mut recv_task => send_task.abort(),
    }
}
