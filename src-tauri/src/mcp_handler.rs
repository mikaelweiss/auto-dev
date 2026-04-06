use std::collections::HashMap;
use std::sync::Mutex;

use tauri::{Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::types::*;
use crate::{db, AppState};

// ── State Signals ──────────────────────────────────────────────────────

/// A state transition signal received from the MCP server via HTTP callback.
#[derive(Debug, Clone)]
pub enum StateSignal {
    AdvanceToBlocked,
    AdvanceToInProgress,
    AdvanceToReview,
}

/// Thread-safe storage for MCP signals keyed by session DB id.
pub type SignalStore = Mutex<HashMap<i64, StateSignal>>;

/// Take (remove) the stored signal for a session, if any.
pub fn take_signal(app: &tauri::AppHandle, session_db_id: i64) -> Option<StateSignal> {
    let state = app.state::<AppState>();
    let mut signals = state.mcp_signals.lock().ok()?;
    signals.remove(&session_db_id)
}

// ── Callback Server ────────────────────────────────────────────────────

/// Run the MCP callback HTTP server on an already-bound std listener.
/// The Tauri app starts this in a background task during setup.
/// Accepts a std listener because the Tokio runtime isn't available in setup().
pub async fn run_callback_server(std_listener: std::net::TcpListener, app_handle: tauri::AppHandle) {
    let listener = match TcpListener::from_std(std_listener) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to convert MCP callback listener to tokio: {e}");
            return;
        }
    };
    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let app = app_handle.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, &app).await {
                        eprintln!("MCP callback error: {e}");
                    }
                });
            }
            Err(e) => {
                eprintln!("MCP callback accept error: {e}");
            }
        }
    }
}

async fn handle_connection(
    mut stream: tokio::net::TcpStream,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let mut buf = vec![0u8; 16384];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| format!("Read: {e}"))?;
    let request = String::from_utf8_lossy(&buf[..n]).to_string();

    // Extract body from HTTP request
    let body = request.split("\r\n\r\n").nth(1).unwrap_or("");

    let response = match handle_signal(body, app) {
        Ok(msg) => format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{msg}",
            msg.len()
        ),
        Err(e) => {
            let err = serde_json::json!({"error": e}).to_string();
            format!(
                "HTTP/1.1 400 Bad Request\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{err}",
                err.len()
            )
        }
    };

    stream
        .write_all(response.as_bytes())
        .await
        .map_err(|e| format!("Write: {e}"))?;
    Ok(())
}

fn handle_signal(body: &str, app: &tauri::AppHandle) -> Result<String, String> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|e| format!("Invalid JSON: {e}"))?;

    let action = json["action"]
        .as_str()
        .ok_or("Missing action field")?;
    let session_id: i64 = json["session_id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| json["session_id"].as_i64())
        .ok_or("Missing or invalid session_id")?;
    let repo_id: i64 = json["repo_id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| json["repo_id"].as_i64())
        .ok_or("Missing or invalid repo_id")?;
    let issue_number: i64 = json["issue_number"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .or_else(|| json["issue_number"].as_i64())
        .ok_or("Missing or invalid issue_number")?;

    let (signal, column, message) = match action {
        "advance_to_blocked" => (
            StateSignal::AdvanceToBlocked,
            "blocked",
            "State updated to blocked. AutoDev has been notified and will present your questions to the user. Stop working now and wait for a response.",
        ),
        "advance_to_in_progress" => (
            StateSignal::AdvanceToInProgress,
            "in_progress",
            "State updated to in_progress. AutoDev will automatically start the implementation phase in a new session. Stop working now.",
        ),
        "advance_to_review" => (
            StateSignal::AdvanceToReview,
            "review",
            "State updated to review. The code is now marked as ready for human review. Stop working now.",
        ),
        _ => return Err(format!("Unknown action: {action}")),
    };

    // 1. Update issue column in DB
    {
        let state = app.state::<AppState>();
        let db_conn = state.db.lock().map_err(|e| format!("DB lock: {e}"))?;
        db::set_issue_column(&db_conn, repo_id, issue_number, column)
            .map_err(|e| format!("DB error: {e}"))?;
    }

    // 2. Emit column change event to frontend
    let _ = app.emit(
        "issue-column-changed",
        serde_json::json!({
            "repo_id": repo_id,
            "issue_number": issue_number,
            "column_id": column,
        }),
    );

    // 3. Insert log entry
    {
        let state = app.state::<AppState>();
        let db_result = state.db.lock();
        if let Ok(db_conn) = db_result {
            let _ = db::insert_session_log(&db_conn, session_id, "status_change", message);
        }
    }

    // 4. Emit log entry to frontend
    let _ = app.emit(
        "session-log",
        SessionLogEvent {
            session_id: session_id.to_string(),
            entry: SessionLogEntry {
                id: uuid::Uuid::new_v4().to_string(),
                session_id: session_id.to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                event_type: "status_change".to_string(),
                content: message.to_string(),
            },
        },
    );

    // 5. Store signal for post-session handling (auto-start implement, etc.)
    {
        let state = app.state::<AppState>();
        let mut signals = state
            .mcp_signals
            .lock()
            .map_err(|e| format!("Signal lock: {e}"))?;
        signals.insert(session_id, signal);
    }

    Ok(serde_json::json!({"status": "ok", "message": message}).to_string())
}
