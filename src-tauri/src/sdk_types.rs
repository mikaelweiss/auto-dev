#![allow(dead_code)]
//! Typed definitions for the Claude CLI's stream-json protocol.
//!
//! These mirror the SDK's `StdoutMessage` type from `@anthropic-ai/claude-agent-sdk`.
//! The CLI emits one JSON object per line on stdout when invoked with
//! `--output-format stream-json`. Each line deserializes into a [`CliMessage`].

use serde::Deserialize;
use serde_json::Value;

// ── Top-level message ────────────────────────────────────────────────────

/// A single NDJSON line from the Claude CLI's stream-json output.
#[derive(Debug)]
pub enum CliMessage {
    Assistant(AssistantMessage),
    Result(ResultMessage),
    System(SystemMessage),
    ToolProgress(ToolProgressMessage),
    ToolUseSummary(ToolUseSummaryMessage),
    RateLimit(RateLimitMessage),
    /// Messages we recognize but don't need to fully parse.
    Passthrough { msg_type: String, session_id: Option<String> },
    /// Completely unknown or malformed JSON.
    Unknown(String),
}

impl CliMessage {
    /// Parse a single NDJSON line into a typed message.
    pub fn parse(line: &str) -> Self {
        let Ok(json) = serde_json::from_str::<Value>(line) else {
            return if line.trim().is_empty() {
                Self::Passthrough { msg_type: "empty".into(), session_id: None }
            } else {
                Self::Unknown(line.to_string())
            };
        };

        let msg_type = json["type"].as_str().unwrap_or("unknown").to_string();
        let session_id = json["session_id"].as_str().map(String::from);

        match msg_type.as_str() {
            "assistant" => match serde_json::from_value::<AssistantMessage>(json) {
                Ok(m) => Self::Assistant(m),
                Err(_) => Self::Passthrough { msg_type, session_id },
            },
            "result" => match serde_json::from_value::<ResultMessage>(json) {
                Ok(m) => Self::Result(m),
                Err(_) => Self::Passthrough { msg_type, session_id },
            },
            "system" => match serde_json::from_value::<SystemMessage>(json) {
                Ok(m) => Self::System(m),
                Err(_) => Self::Passthrough { msg_type, session_id },
            },
            "tool_progress" => match serde_json::from_value::<ToolProgressMessage>(json) {
                Ok(m) => Self::ToolProgress(m),
                Err(_) => Self::Passthrough { msg_type, session_id },
            },
            "tool_use_summary" => match serde_json::from_value::<ToolUseSummaryMessage>(json) {
                Ok(m) => Self::ToolUseSummary(m),
                Err(_) => Self::Passthrough { msg_type, session_id },
            },
            "rate_limit_event" => match serde_json::from_value::<RateLimitMessage>(json) {
                Ok(m) => Self::RateLimit(m),
                Err(_) => Self::Passthrough { msg_type, session_id },
            },
            // Types we recognize but don't need to fully parse
            "user" | "stream_event" | "auth_status" | "prompt_suggestion"
            | "control_request" | "control_response" => {
                Self::Passthrough { msg_type, session_id }
            }
            _ => Self::Passthrough { msg_type, session_id },
        }
    }

    /// Extract the session_id from any message variant.
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::Assistant(m) => m.session_id.as_deref(),
            Self::Result(m) => m.session_id.as_deref(),
            Self::System(m) => m.session_id(),
            Self::ToolProgress(m) => m.session_id.as_deref(),
            Self::ToolUseSummary(m) => m.session_id.as_deref(),
            Self::RateLimit(m) => m.session_id.as_deref(),
            Self::Passthrough { session_id, .. } => session_id.as_deref(),
            Self::Unknown(_) => None,
        }
    }
}

// ── Assistant Message ────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct AssistantMessage {
    pub message: AssistantMessageBody,
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AssistantMessageBody {
    #[serde(default)]
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },

    #[serde(rename = "thinking")]
    Thinking {
        #[serde(default)]
        thinking: String,
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        #[serde(default)]
        tool_use_id: Option<String>,
        #[serde(default)]
        content: Value,
    },
}

#[derive(Debug, Deserialize, Clone)]
pub struct Usage {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    pub cache_creation_input_tokens: Option<u64>,
}

// ── Result Message ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ResultMessage {
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub is_error: bool,
    #[serde(default)]
    pub result: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub duration_api_ms: Option<u64>,
    #[serde(default)]
    pub num_turns: Option<u64>,
    #[serde(default)]
    pub total_cost_usd: Option<f64>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub errors: Option<Vec<String>>,
}

// ── System Messages (discriminated by subtype) ───────────────────────────

#[derive(Debug)]
pub enum SystemMessage {
    Init(SystemInit),
    Status(SystemStatus),
    ApiRetry(SystemApiRetry),
    TaskStarted(SystemTaskEvent),
    TaskProgress(SystemTaskEvent),
    TaskNotification(SystemTaskEvent),
    SessionStateChanged { state: Option<String>, session_id: Option<String> },
    Other { subtype: String, session_id: Option<String> },
}

impl SystemMessage {
    pub fn session_id(&self) -> Option<&str> {
        match self {
            Self::Init(m) => m.session_id.as_deref(),
            Self::Status(m) => m.session_id.as_deref(),
            Self::ApiRetry(m) => m.session_id.as_deref(),
            Self::TaskStarted(m) | Self::TaskProgress(m) | Self::TaskNotification(m) => {
                m.session_id.as_deref()
            }
            Self::SessionStateChanged { session_id, .. } => session_id.as_deref(),
            Self::Other { session_id, .. } => session_id.as_deref(),
        }
    }
}

// Custom deserialize for SystemMessage since it needs two-level tag dispatch
impl<'de> serde::Deserialize<'de> for SystemMessage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        let subtype = value["subtype"].as_str().unwrap_or("unknown").to_string();
        let session_id = value["session_id"].as_str().map(String::from);

        Ok(match subtype.as_str() {
            "init" => match serde_json::from_value::<SystemInit>(value) {
                Ok(m) => Self::Init(m),
                Err(_) => Self::Other { subtype, session_id },
            },
            "status" => match serde_json::from_value::<SystemStatus>(value) {
                Ok(m) => Self::Status(m),
                Err(_) => Self::Other { subtype, session_id },
            },
            "api_retry" => match serde_json::from_value::<SystemApiRetry>(value) {
                Ok(m) => Self::ApiRetry(m),
                Err(_) => Self::Other { subtype, session_id },
            },
            "task_started" => match serde_json::from_value::<SystemTaskEvent>(value) {
                Ok(m) => Self::TaskStarted(m),
                Err(_) => Self::Other { subtype, session_id },
            },
            "task_progress" => match serde_json::from_value::<SystemTaskEvent>(value) {
                Ok(m) => Self::TaskProgress(m),
                Err(_) => Self::Other { subtype, session_id },
            },
            "task_notification" => match serde_json::from_value::<SystemTaskEvent>(value) {
                Ok(m) => Self::TaskNotification(m),
                Err(_) => Self::Other { subtype, session_id },
            },
            "session_state_changed" => Self::SessionStateChanged {
                state: value["state"].as_str().map(String::from),
                session_id,
            },
            _ => Self::Other { subtype, session_id },
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct SystemInit {
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub tools: Option<Vec<String>>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub claude_code_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SystemStatus {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SystemApiRetry {
    #[serde(default)]
    pub attempt: Option<u32>,
    #[serde(default)]
    pub max_retries: Option<u32>,
    #[serde(default)]
    pub retry_delay_ms: Option<u64>,
    #[serde(default)]
    pub error_status: Option<u32>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

/// Unified type for task_started, task_progress, and task_notification.
#[derive(Debug, Deserialize)]
pub struct SystemTaskEvent {
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub task_type: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub last_tool_name: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

// ── Tool Progress ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ToolProgressMessage {
    #[serde(default)]
    pub tool_use_id: Option<String>,
    #[serde(default)]
    pub tool_name: Option<String>,
    #[serde(default)]
    pub parent_tool_use_id: Option<String>,
    #[serde(default)]
    pub elapsed_time_seconds: Option<f64>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
}

// ── Tool Use Summary ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ToolUseSummaryMessage {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub preceding_tool_use_ids: Option<Vec<String>>,
    #[serde(default)]
    pub session_id: Option<String>,
}

// ── Rate Limit ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct RateLimitMessage {
    #[serde(default)]
    pub rate_limit_info: Option<RateLimitInfo>,
    #[serde(default)]
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RateLimitInfo {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub resets_at: Option<f64>,
    #[serde(default)]
    pub utilization: Option<f64>,
    #[serde(default)]
    pub rate_limit_type: Option<String>,
}

// ── Helpers for converting parsed messages into log entries ──────────────

/// A processed log entry ready for storage and emission to the frontend.
pub struct LogEntry {
    pub event_type: String,
    pub content: String,
}

impl CliMessage {
    /// Convert this message into zero or more log entries for the activity feed.
    pub fn to_log_entries(&self) -> Vec<LogEntry> {
        match self {
            Self::Assistant(msg) => {
                let mut entries = Vec::new();
                for block in &msg.message.content {
                    match block {
                        ContentBlock::Text { text } if !text.trim().is_empty() => {
                            entries.push(LogEntry {
                                event_type: "message".into(),
                                content: text.clone(),
                            });
                        }
                        ContentBlock::ToolUse { name, input, .. } => {
                            entries.push(LogEntry {
                                event_type: "tool_call".into(),
                                content: format_tool_summary(name, input),
                            });
                        }
                        ContentBlock::Thinking { thinking } if !thinking.trim().is_empty() => {
                            entries.push(LogEntry {
                                event_type: "thinking".into(),
                                content: thinking.clone(),
                            });
                        }
                        _ => {}
                    }
                }
                entries
            }
            Self::Result(msg) => {
                let mut entries = Vec::new();
                // Emit the result text as a message
                if let Some(ref text) = msg.result {
                    if !text.trim().is_empty() {
                        entries.push(LogEntry {
                            event_type: "message".into(),
                            content: text.clone(),
                        });
                    }
                }
                // Emit cost/duration as a result summary
                if let Some(cost) = msg.total_cost_usd {
                    let duration = msg.duration_ms.map(|ms| ms / 1000).unwrap_or(0);
                    let turns = msg.num_turns.unwrap_or(0);
                    entries.push(LogEntry {
                        event_type: "result".into(),
                        content: format!(
                            "${:.4} | {}s | {} turns",
                            cost, duration, turns
                        ),
                    });
                }
                // Emit errors
                if let Some(ref errors) = msg.errors {
                    for err in errors {
                        entries.push(LogEntry {
                            event_type: "error".into(),
                            content: err.clone(),
                        });
                    }
                }
                entries
            }
            Self::System(msg) => match msg {
                SystemMessage::ApiRetry(retry) => {
                    let attempt = retry.attempt.unwrap_or(0);
                    let max = retry.max_retries.unwrap_or(0);
                    let err = retry.error.as_deref().unwrap_or("unknown error");
                    vec![LogEntry {
                        event_type: "api_retry".into(),
                        content: format!("API retry {attempt}/{max}: {err}"),
                    }]
                }
                SystemMessage::TaskStarted(ev) => {
                    let desc = ev.description.as_deref().unwrap_or("task");
                    vec![LogEntry {
                        event_type: "task_progress".into(),
                        content: format!("Started: {desc}"),
                    }]
                }
                SystemMessage::TaskProgress(ev) => {
                    if let Some(ref summary) = ev.summary {
                        vec![LogEntry {
                            event_type: "task_progress".into(),
                            content: summary.clone(),
                        }]
                    } else {
                        vec![]
                    }
                }
                SystemMessage::TaskNotification(ev) => {
                    let status = ev.status.as_deref().unwrap_or("done");
                    let summary = ev.summary.as_deref().unwrap_or("");
                    vec![LogEntry {
                        event_type: "task_progress".into(),
                        content: format!("Task {status}: {summary}"),
                    }]
                }
                SystemMessage::Status(st) => {
                    if st.status.as_deref() == Some("compacting") {
                        vec![LogEntry {
                            event_type: "status_change".into(),
                            content: "Compacting context...".into(),
                        }]
                    } else {
                        vec![]
                    }
                }
                _ => vec![],
            },
            Self::RateLimit(msg) => {
                if let Some(ref info) = msg.rate_limit_info {
                    let status = info.status.as_deref().unwrap_or("unknown");
                    if status == "allowed" {
                        return vec![];
                    }
                    let util = info.utilization.map(|u| format!(" ({:.0}%)", u * 100.0)).unwrap_or_default();
                    vec![LogEntry {
                        event_type: "rate_limit".into(),
                        content: format!("Rate limit: {status}{util}"),
                    }]
                } else {
                    vec![]
                }
            }
            Self::ToolProgress(msg) => {
                let name = msg.tool_name.as_deref().unwrap_or("tool");
                let secs = msg.elapsed_time_seconds.unwrap_or(0.0);
                if secs > 10.0 {
                    vec![LogEntry {
                        event_type: "tool_progress".into(),
                        content: format!("{name}: {secs:.0}s elapsed"),
                    }]
                } else {
                    vec![]
                }
            }
            Self::ToolUseSummary(msg) => {
                if let Some(ref summary) = msg.summary {
                    vec![LogEntry {
                        event_type: "tool_call".into(),
                        content: summary.clone(),
                    }]
                } else {
                    vec![]
                }
            }
            Self::Passthrough { .. } | Self::Unknown(_) => vec![],
        }
    }
}

/// Format a human-readable summary for a tool call.
fn format_tool_summary(name: &str, input: &Value) -> String {
    match name {
        "Read" | "Write" => {
            let path = input["file_path"].as_str().unwrap_or("");
            format!("{name}: {path}")
        }
        "Edit" => {
            let path = input["file_path"].as_str().unwrap_or("");
            format!("Edit: {path}")
        }
        "Bash" => {
            let cmd = input["command"].as_str().unwrap_or("");
            let truncated: String = cmd.chars().take(100).collect();
            format!("Bash: {truncated}")
        }
        "Grep" => {
            let pattern = input["pattern"].as_str().unwrap_or("");
            format!("Grep: {pattern}")
        }
        "Glob" => {
            let pattern = input["pattern"].as_str().unwrap_or("");
            format!("Glob: {pattern}")
        }
        "Agent" => {
            let desc = input["description"].as_str().unwrap_or("subagent");
            format!("Agent: {desc}")
        }
        _ => name.to_string(),
    }
}
