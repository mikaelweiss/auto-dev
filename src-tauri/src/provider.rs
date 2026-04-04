use std::process::Stdio;

use tauri::{Emitter, Manager};
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::sdk_types::LogEntry;
use crate::types::*;

// ── Provider Kind ───────────────────────────────────────────────────────

/// Identifies which AI provider a model belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderKind {
    Claude,
    Codex,
}

impl ProviderKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            other => Err(format!("Unknown provider: {other}")),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
        }
    }
}

// ── Model Registry ──────────────────────────────────────────────────────

/// Metadata about a model available from a provider.
pub struct ModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub provider: ProviderKind,
    pub default_effort: &'static str,
    pub effort_levels: &'static [&'static str],
}

/// All models available across all providers.
pub fn all_models() -> Vec<ModelInfo> {
    vec![
        // Claude models
        ModelInfo {
            id: "claude-opus-4-6-max-ctx",
            display_name: "Opus 4.6 1M",
            provider: ProviderKind::Claude,
            default_effort: "high",
            effort_levels: &["low", "medium", "high", "max"],
        },
        ModelInfo {
            id: "claude-opus-4-6",
            display_name: "Opus 4.6",
            provider: ProviderKind::Claude,
            default_effort: "high",
            effort_levels: &["low", "medium", "high", "max"],
        },
        ModelInfo {
            id: "claude-sonnet-4-6",
            display_name: "Sonnet 4.6",
            provider: ProviderKind::Claude,
            default_effort: "high",
            effort_levels: &["low", "medium", "high", "max"],
        },
        ModelInfo {
            id: "claude-haiku-4-5",
            display_name: "Haiku 4.5",
            provider: ProviderKind::Claude,
            default_effort: "high",
            effort_levels: &["low", "medium", "high", "max"],
        },
        // Codex models
        ModelInfo {
            id: "gpt-5.4",
            display_name: "GPT-5.4",
            provider: ProviderKind::Codex,
            default_effort: "medium",
            effort_levels: &["low", "medium", "high"],
        },
        ModelInfo {
            id: "gpt-5.3-codex-spark",
            display_name: "GPT-5.3-Codex-Spark",
            provider: ProviderKind::Codex,
            default_effort: "medium",
            effort_levels: &["low", "medium", "high"],
        },
        ModelInfo {
            id: "gpt-5.3-codex",
            display_name: "GPT-5.3-Codex",
            provider: ProviderKind::Codex,
            default_effort: "medium",
            effort_levels: &["low", "medium", "high"],
        },
        ModelInfo {
            id: "gpt-5.2-codex",
            display_name: "GPT-5.2-Codex",
            provider: ProviderKind::Codex,
            default_effort: "medium",
            effort_levels: &["low", "medium", "high"],
        },
    ]
}

/// Get the provider for a given model ID.
pub fn provider_for_model(model_id: &str) -> Option<ProviderKind> {
    all_models()
        .iter()
        .find(|m| m.id == model_id)
        .map(|m| m.provider)
}

// ── Session Config ──────────────────────────────────────────────────────

/// Everything needed to run a single provider session.
pub struct SessionConfig {
    pub worktree_path: String,
    pub system_prompt: String,
    pub user_prompt: String,
    pub permission_mode: String,
    pub model: String,
    pub effort: String,
    pub resume_session_id: Option<String>,
}

/// The result of running a provider session.
pub struct ProviderSessionResult {
    pub cli_session_id: Option<String>,
    pub cost_usd: Option<f64>,
}

// ── Provider Trait ──────────────────────────────────────────────────────

/// Trait that each AI provider must implement.
pub trait Provider: Send + Sync {
    /// Which provider this is.
    fn kind(&self) -> ProviderKind;

    /// Find the CLI binary on disk.
    fn find_binary(&self) -> Result<String, String>;

    /// Build the CLI command for a session.
    fn build_command(
        &self,
        binary_path: &str,
        config: &SessionConfig,
    ) -> tokio::process::Command;

    /// Parse a single stdout line into log entries.
    /// Returns (log_entries, session_id_if_found, cost_if_found).
    fn parse_line(&self, line: &str) -> ParsedLine;
}

/// Result of parsing a single line from the CLI stdout.
pub struct ParsedLine {
    pub entries: Vec<LogEntry>,
    pub session_id: Option<String>,
    pub cost_usd: Option<f64>,
}

// ── Shared Session Runner ───────────────────────────────────────────────

/// Run a session using any provider. This is the shared process lifecycle:
/// spawn -> read stdout -> parse lines -> emit events -> wait for exit.
pub async fn run_provider_session(
    provider: &dyn Provider,
    config: SessionConfig,
    session_db_id: i64,
    app_handle: &tauri::AppHandle,
) -> Result<ProviderSessionResult, String> {
    let binary_path = provider.find_binary()?;
    let mut cmd = provider.build_command(&binary_path, &config);

    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    // Create a new process group so we can kill all children together
    #[cfg(unix)]
    unsafe {
        cmd.pre_exec(|| {
            libc::setpgid(0, 0);
            Ok(())
        });
    }

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {e}", provider.kind().display_name()))?;

    // Register the PID for cancellation support
    if let Some(pid) = child.id() {
        register_pid(app_handle, session_db_id, pid);
    }

    let stdout = child
        .stdout
        .take()
        .ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take();

    let mut reader = BufReader::new(stdout).lines();
    let mut cli_session_id: Option<String> = None;
    let mut cost_usd: Option<f64> = None;

    while let Some(line) = reader
        .next_line()
        .await
        .map_err(|e| format!("Failed to read output: {e}"))?
    {
        let parsed = provider.parse_line(&line);

        // Capture session ID
        if cli_session_id.is_none() {
            if let Some(ref sid) = parsed.session_id {
                cli_session_id = Some(sid.clone());
                save_cli_session_id(app_handle, session_db_id, sid);
            }
        }

        // Capture cost
        if let Some(cost) = parsed.cost_usd {
            cost_usd = Some(cost);
        }

        // Emit log entries
        for entry in parsed.entries {
            insert_log_via_app(app_handle, session_db_id, &entry.event_type, &entry.content);

            let _ = app_handle.emit(
                "session-log",
                SessionLogEvent {
                    session_id: session_db_id.to_string(),
                    entry: SessionLogEntry {
                        id: uuid::Uuid::new_v4().to_string(),
                        session_id: session_db_id.to_string(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        event_type: entry.event_type,
                        content: entry.content,
                    },
                },
            );
        }
    }

    // Unregister PID
    unregister_pid(app_handle, session_db_id);

    let status = child
        .wait()
        .await
        .map_err(|e| format!("Failed to wait for process: {e}"))?;

    // Exit code 143 = SIGTERM (128+15), meaning user stopped the session.
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if status.signal() == Some(libc::SIGTERM) {
            return Ok(ProviderSessionResult {
                cli_session_id,
                cost_usd,
            });
        }
    }

    if !status.success() {
        let mut stderr_output = String::new();
        if let Some(stderr) = stderr {
            let mut stderr_reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = stderr_reader.next_line().await {
                stderr_output.push_str(&line);
                stderr_output.push('\n');
            }
        }
        let provider_name = provider.kind().display_name();
        let detail = if stderr_output.trim().is_empty() {
            format!("{provider_name} exited with {status}")
        } else {
            format!(
                "{provider_name} exited with {status}: {}",
                stderr_output.trim()
            )
        };
        return Err(detail);
    }

    Ok(ProviderSessionResult {
        cli_session_id,
        cost_usd,
    })
}

// ── Provider Registry ───────────────────────────────────────────────────

/// Get the appropriate provider for a model ID.
pub fn get_provider(model_id: &str) -> Result<Box<dyn Provider>, String> {
    let kind = provider_for_model(model_id).ok_or_else(|| {
        format!("Unknown model: {model_id}")
    })?;

    match kind {
        ProviderKind::Claude => Ok(Box::new(crate::claude_provider::ClaudeProvider)),
        ProviderKind::Codex => Ok(Box::new(crate::codex_provider::CodexProvider)),
    }
}

/// Get a provider by kind (for when you know the provider but not the model).
pub fn get_provider_by_kind(kind: ProviderKind) -> Box<dyn Provider> {
    match kind {
        ProviderKind::Claude => Box::new(crate::claude_provider::ClaudeProvider),
        ProviderKind::Codex => Box::new(crate::codex_provider::CodexProvider),
    }
}

// ── Helpers (shared with sessions.rs) ───────────────────────────────────

pub fn register_pid(app_handle: &tauri::AppHandle, session_db_id: i64, pid: u32) {
    let state = app_handle.state::<crate::AppState>();
    let Ok(mut pids) = state.active_pids.lock() else { return };
    pids.insert(session_db_id, pid);
}

pub fn unregister_pid(app_handle: &tauri::AppHandle, session_db_id: i64) {
    let state = app_handle.state::<crate::AppState>();
    let Ok(mut pids) = state.active_pids.lock() else { return };
    pids.remove(&session_db_id);
}

fn save_cli_session_id(app_handle: &tauri::AppHandle, session_db_id: i64, cli_id: &str) {
    let state = app_handle.state::<crate::AppState>();
    let Ok(db) = state.db.lock() else { return };
    let _ = crate::db::update_session_cli_id(&db, session_db_id, cli_id);
}

fn insert_log_via_app(app_handle: &tauri::AppHandle, session_db_id: i64, event_type: &str, content: &str) {
    let state = app_handle.state::<crate::AppState>();
    let Ok(db) = state.db.lock() else { return };
    let _ = crate::db::insert_session_log(&db, session_db_id, event_type, content);
}
