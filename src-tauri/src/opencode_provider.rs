use crate::provider::{ParsedLine, Provider, ProviderKind, SessionConfig};
use crate::sdk_types::LogEntry;
use serde_json::Value;

/// Opencode CLI provider.
pub struct OpencodeProvider;

impl Provider for OpencodeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Opencode
    }

    fn find_binary(&self) -> Result<String, String> {
        if let Ok(home) = std::env::var("HOME") {
            let local_path = format!("{home}/.local/bin/opencode");
            if std::path::Path::new(&local_path).exists() {
                return Ok(local_path);
            }
        }

        for path in &["/usr/local/bin/opencode", "/opt/homebrew/bin/opencode"] {
            if std::path::Path::new(path).exists() {
                return Ok(path.to_string());
            }
        }

        let output = std::process::Command::new("/usr/bin/which")
            .arg("opencode")
            .output()
            .map_err(|e| format!("Failed to run which: {e}"))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err("opencode CLI not found. Install it from https://opencode.ai".to_string())
        }
    }

    fn build_command(
        &self,
        binary_path: &str,
        config: &SessionConfig,
    ) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(binary_path);

        cmd.arg("run");
        cmd.args(["--format", "json"]);
        cmd.args(["--model", &config.model]);
        cmd.args(["--dir", &config.worktree_path]);

        // Map effort to opencode's --variant flag
        if !config.effort.is_empty() {
            cmd.args(["--variant", &config.effort]);
        }

        // Resume a previous session
        if let Some(ref resume_id) = config.resume_session_id {
            cmd.args(["--session", resume_id]);
        }

        // Pass the user prompt as positional message
        cmd.arg(&config.user_prompt);

        cmd
    }

    fn parse_line(&self, line: &str) -> ParsedLine {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return ParsedLine {
                entries: vec![],
                session_id: None,
                cost_usd: None,
            };
        }

        if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
            return self.parse_json_line(&json);
        }

        // Plain text fallback
        ParsedLine {
            entries: vec![LogEntry {
                event_type: "message".into(),
                content: trimmed.to_string(),
            }],
            session_id: None,
            cost_usd: None,
        }
    }
}

impl OpencodeProvider {
    /// Parse opencode `run --format json` JSONL output.
    ///
    /// Event types from `opencode run --format json`:
    ///   text        — { part: { type: "text", text } }
    ///   tool_use    — { part: { tool, state: { status, input, output, error } } }
    ///   step_start  — { part: { type: "step-start" } }
    ///   step_finish — { part: { type: "step-finish" } }
    ///   reasoning   — { part: { type: "reasoning", text } }
    ///   error       — { error: { name, data: { message } } }
    fn parse_json_line(&self, json: &Value) -> ParsedLine {
        let event_type = json["type"].as_str().unwrap_or("");

        // Capture sessionID from any event
        let session_id = json["sessionID"].as_str().map(String::from);

        match event_type {
            // Assistant text response
            "text" => {
                let text = json["part"]["text"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if text.is_empty() {
                    return ParsedLine { entries: vec![], session_id, cost_usd: None };
                }
                ParsedLine {
                    entries: vec![LogEntry {
                        event_type: "message".into(),
                        content: text,
                    }],
                    session_id,
                    cost_usd: None,
                }
            }

            // Tool use completion/error
            "tool_use" => {
                let part = &json["part"];
                let tool_name = part["tool"].as_str().unwrap_or("tool");
                let status = part["state"]["status"].as_str().unwrap_or("");

                if status == "error" {
                    let error = part["state"]["error"]
                        .as_str()
                        .unwrap_or("Unknown error")
                        .to_string();
                    return ParsedLine {
                        entries: vec![LogEntry {
                            event_type: "error".into(),
                            content: format!("{tool_name} failed: {error}"),
                        }],
                        session_id,
                        cost_usd: None,
                    };
                }

                let content = match tool_name {
                    "bash" => {
                        let cmd = part["state"]["input"]["command"]
                            .as_str()
                            .unwrap_or("command");
                        let cmd_display: String = cmd.chars().take(120).collect();
                        format!("Bash: {cmd_display}")
                    }
                    "edit" => {
                        let file = part["state"]["input"]["filePath"]
                            .as_str()
                            .unwrap_or("file");
                        format!("Edit: {file}")
                    }
                    "write" => {
                        let file = part["state"]["input"]["filePath"]
                            .as_str()
                            .unwrap_or("file");
                        format!("Write: {file}")
                    }
                    "read" => {
                        let file = part["state"]["input"]["filePath"]
                            .as_str()
                            .unwrap_or("file");
                        format!("Read: {file}")
                    }
                    "glob" => {
                        let pattern = part["state"]["input"]["pattern"]
                            .as_str()
                            .unwrap_or("*");
                        format!("Glob: {pattern}")
                    }
                    "grep" => {
                        let pattern = part["state"]["input"]["pattern"]
                            .as_str()
                            .unwrap_or("");
                        format!("Grep: {pattern}")
                    }
                    "task" => {
                        let desc = part["state"]["input"]["description"]
                            .as_str()
                            .unwrap_or("task");
                        format!("Task: {desc}")
                    }
                    _ => {
                        let title = part["state"]["title"]
                            .as_str()
                            .unwrap_or(tool_name);
                        title.to_string()
                    }
                };

                ParsedLine {
                    entries: vec![LogEntry {
                        event_type: "tool_call".into(),
                        content,
                    }],
                    session_id,
                    cost_usd: None,
                }
            }

            // Thinking/reasoning
            "reasoning" => {
                let text = json["part"]["text"]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if text.is_empty() {
                    return ParsedLine { entries: vec![], session_id, cost_usd: None };
                }
                ParsedLine {
                    entries: vec![LogEntry {
                        event_type: "thinking".into(),
                        content: text,
                    }],
                    session_id,
                    cost_usd: None,
                }
            }

            // Error
            "error" => {
                let content = json["error"]["data"]["message"]
                    .as_str()
                    .or_else(|| json["error"]["name"].as_str())
                    .unwrap_or("Unknown error")
                    .to_string();
                ParsedLine {
                    entries: vec![LogEntry {
                        event_type: "error".into(),
                        content,
                    }],
                    session_id,
                    cost_usd: None,
                }
            }

            // step_start, step_finish — no useful display info
            _ => ParsedLine { entries: vec![], session_id, cost_usd: None },
        }
    }
}
