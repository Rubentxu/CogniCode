//! WebSocket handler for live diagram updates
//!
//! Provides a WebSocket endpoint at `/ws/diagrams/live` that streams
//! file change events to connected clients for live diagram updates.

use axum::{
    extract::{ws::{Message, WebSocket, WebSocketUpgrade}, State},
    response::IntoResponse,
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;

use crate::watch_service::{is_diagram_relevant, FileWatcher};

/// WebSocket state shared across connections
#[derive(Clone)]
pub struct WsState {
    /// Shared file watcher (using tokio Mutex for async compatibility)
    pub watcher: Arc<TokioMutex<Option<FileWatcher>>>,
    /// Debounce duration in milliseconds
    pub debounce_ms: u64,
    /// Project path being watched
    pub project_path: String,
}

impl WsState {
    pub fn new(project_path: String, debounce_ms: u64) -> Self {
        Self {
            watcher: Arc::new(TokioMutex::new(None)),
            debounce_ms,
            project_path,
        }
    }

    /// Initialize the file watcher
    pub async fn init_watcher(&self) -> Result<(), notify::Error> {
        let mut guard = self.watcher.lock().await;
        if guard.is_none() {
            let watcher = FileWatcher::new(&self.project_path, self.debounce_ms)?;
            *guard = Some(watcher);
        }
        Ok(())
    }
}

/// WebSocket message types sent to clients
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsServerMessage {
    /// File change event
    Change {
        change_type: String,
        file_path: String,
        timestamp: String,
    },
    /// Acknowledgment of client registration
    Registered { project_path: String },
    /// Error message
    Error { message: String },
    /// Heartbeat ping response
    Pong,
}

/// WebSocket message types received from clients
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum WsClientMessage {
    /// Register for updates (with optional project path)
    Register { project_path: Option<String> },
    /// Pause live updates
    Pause,
    /// Resume live updates
    Resume,
    /// Ping heartbeat
    Ping,
    /// Request current diagram state
    Refresh,
}

/// Handle WebSocket connections at `/ws/diagrams/live`
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<WsState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_socket(socket, state))
}

/// Handle an individual WebSocket connection
async fn handle_socket(socket: WebSocket, state: WsState) {
    let (mut sender, mut receiver) = socket.split();

    // Initialize watcher if not already done
    if let Err(e) = state.init_watcher().await {
        let msg = WsServerMessage::Error {
            message: format!("Failed to initialize file watcher: {}", e),
        };
        let _ = sender.send(Message::Text(serde_json::to_string(&msg).unwrap())).await;
        return;
    }

    // Send registration acknowledgment
    let ack = WsServerMessage::Registered {
        project_path: state.project_path.clone(),
    };
    let _ = sender.send(Message::Text(serde_json::to_string(&ack).unwrap())).await;

    // Track pause state
    let mut paused = false;

    // Get broadcast receiver for file events
    let watcher_guard = state.watcher.lock().await;
    let mut rx = match watcher_guard.as_ref() {
        Some(watcher) => watcher.subscribe(),
        None => {
            let msg = WsServerMessage::Error {
                message: "File watcher not initialized".to_string(),
            };
            let _ = sender.send(Message::Text(serde_json::to_string(&msg).unwrap())).await;
            return;
        }
    };
    drop(watcher_guard);

    loop {
        tokio::select! {
            // Handle messages from client
            msg = receiver.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(client_msg) = serde_json::from_str::<WsClientMessage>(&text) {
                            match client_msg {
                                WsClientMessage::Register { project_path } => {
                                    // Re-acknowledge with potentially new project path
                                    let ack = WsServerMessage::Registered {
                                        project_path: project_path.unwrap_or_else(|| state.project_path.clone()),
                                    };
                                    let _ = sender.send(Message::Text(serde_json::to_string(&ack).unwrap())).await;
                                }
                                WsClientMessage::Pause => {
                                    paused = true;
                                }
                                WsClientMessage::Resume => {
                                    paused = false;
                                }
                                WsClientMessage::Ping => {
                                    let pong = WsServerMessage::Pong;
                                    let _ = sender.send(Message::Text(serde_json::to_string(&pong).unwrap())).await;
                                }
                                WsClientMessage::Refresh => {
                                    // Client requested a refresh - would trigger analysis here
                                    // For now just send a heartbeat
                                    let heartbeat = WsServerMessage::Pong;
                                    let _ = sender.send(Message::Text(serde_json::to_string(&heartbeat).unwrap())).await;
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        break;
                    }
                    Some(Ok(Message::Ping(data))) => {
                        let _ = sender.send(Message::Pong(data)).await;
                    }
                    _ => {}
                }
            }

            // Handle broadcast events from file watcher
            _ = rx.recv() => {
                if !paused {
                    // Process the event and send to client
                    if let Ok(change_event) = rx.try_recv() {
                        if is_diagram_relevant(&change_event.file_path) {
                            let msg = WsServerMessage::Change {
                                change_type: format!("{:?}", change_event.change_type).to_lowercase(),
                                file_path: change_event.file_path.display().to_string(),
                                timestamp: change_event.timestamp.to_rfc3339(),
                            };
                            let _ = sender.send(Message::Text(serde_json::to_string(&msg).unwrap())).await;
                        }
                    }
                }
            }
        }
    }
}

/// Create the WebSocket router for the dashboard server
pub fn create_ws_router(state: WsState) -> Router {
    Router::new()
        .route("/ws/diagrams/live", get(ws_handler))
        .with_state(state)
}
