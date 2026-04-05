use crate::provider::{self, ParsedLine, Provider, ProviderKind, SessionConfig};
use crate::sdk_types::LogEntry;
use serde_json::Value;

/// OpenAI Codex CLI provider.
pub struct CodexProvider;

impl Provider for CodexProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Codex
    }

    fn find_binary(&self) -> Result<String, String> {
        // Check ~/.local/bin first
        if let Ok(home) = std::env::var("HOME") {
            let local_path = format!("{home}/.local/bin/codex");
            if std::path::Path::new(&local_path).exists() {
                return Ok(local_path);
            }
        }

        for path in &["/usr/local/bin/codex", "/opt/homebrew/bin/codex"] {
            if std::path::Path::new(path).exists() {
                return Ok(path.to_string());
            }
        }

        // Fallback: use `which`
        let output = std::process::Command::new("/usr/bin/which")
            .arg("codex")
            .output()
            .map_err(|e| format!("Failed to run which: {e}"))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err("codex CLI not found. Install it from https://github.com/openai/codex".to_string())
        }
    }

    fn build_command(
        &self,
        binary_path: &str,
        config: &SessionConfig,
    ) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(binary_path);

        // Use `codex exec` subcommand for non-interactive mode with JSONL output
        cmd.arg("exec");

        cmd.args(["--model", &config.model]);
        cmd.arg("--json"); // JSONL output for structured parsing

        // Don't use --full-auto: it forces --sandbox workspace-write which
        // blocks all network access (Seatbelt on macOS). The agent needs
        // network to run `gh` CLI for GitHub API calls.
        // Instead, set sandbox_mode and approval_policy in .codex/config.toml
        // which codex exec reads at startup.

        // Working directory
        cmd.args(["-C", &config.worktree_path]);

        // Write .codex/config.toml in the worktree with:
        // - sandbox_mode = danger-full-access (allow network for gh CLI)
        // - approval_policy = on-request (auto-approve safe commands)
        // - shell_environment_policy: don't strip *TOKEN* env vars
        // - MCP server config for state advancement tools
        {
            let codex_dir = std::path::Path::new(&config.worktree_path).join(".codex");
            let _ = std::fs::create_dir_all(&codex_dir);

            let mut config_content = String::from(
                "sandbox_mode = \"danger-full-access\"\n\
                 approval_policy = \"on-request\"\n\
                 \n\
                 [shell_environment_policy]\n\
                 ignore_default_excludes = true\n",
            );

            if let Some(ref mcp_binary) = config.mcp_binary_path {
                config_content.push_str(&format!(
                    "\n[mcp_servers.autodev]\ncommand = \"{}\"\n",
                    mcp_binary.replace('\\', "\\\\").replace('"', "\\\"")
                ));
            }

            let _ = std::fs::write(codex_dir.join("config.toml"), config_content);
        }

        // System prompt via config override
        if config.resume_session_id.is_none() && !config.system_prompt.is_empty() {
            cmd.args(["-c", &format!("instructions=\"{}\"", config.system_prompt.replace('"', "\\\""))]);
        }

        // Resume a previous session
        if let Some(ref resume_id) = config.resume_session_id {
            // Use `exec resume` subcommand
            cmd.arg("resume").arg("--last");
            // The resume_id isn't directly usable the same way as Claude's --resume,
            // but we still attempt to continue via the last session
            let _ = resume_id;
        }

        cmd.arg(&config.user_prompt);

        cmd
    }

    fn parse_line(&self, line: &str) -> ParsedLine {
        // Codex in quiet mode outputs JSON-ND when available, plain text otherwise.
        // Try to parse as JSON first, fall back to treating as text.
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return ParsedLine {
                entries: vec![],
                session_id: None,
                cost_usd: None,
                signal: None,
            };
        }

        if let Ok(json) = serde_json::from_str::<Value>(trimmed) {
            return self.parse_json_line(&json);
        }

        // Plain text output — treat as assistant message
        ParsedLine {
            entries: vec![LogEntry {
                event_type: "message".into(),
                content: trimmed.to_string(),
            }],
            session_id: None,
            cost_usd: None,
            signal: None,
        }
    }
}

impl CodexProvider {
    /// Parse Codex `exec --json` JSONL output.
    ///
    /// Actual event types from `codex exec --json`:
    ///   thread.started    — { thread_id }
    ///   turn.started      — (no payload)
    ///   item.started      — { item: { id, type, command?, status? } }
    ///   item.completed    — { item: { id, type, text?, command?, aggregated_output?, exit_code? } }
    ///   turn.completed    — { usage: { input_tokens, output_tokens, cached_input_tokens } }
    ///   error             — { status, error: { type, message } }
    fn parse_json_line(&self, json: &Value) -> ParsedLine {
        let msg_type = json["type"].as_str().unwrap_or("");

        // Check for MCP state signals in every line
        let signal = provider::detect_signal_from_json(json);

        match msg_type {
            // Thread started — capture session/thread ID
            "thread.started" => {
                let thread_id = json["thread_id"].as_str().map(String::from);
                ParsedLine {
                    entries: vec![],
                    session_id: thread_id,
                    cost_usd: None,
                    signal,
                }
            }

            // Item completed — the main event for messages and tool results
            "item.completed" => {
                let item = &json["item"];
                let item_type = item["type"].as_str().unwrap_or("");

                match item_type {
                    // Agent text message
                    "agent_message" | "message" => {
                        let text = item["text"].as_str().unwrap_or("").to_string();
                        if text.is_empty() {
                            ParsedLine { entries: vec![], session_id: None, cost_usd: None, signal }
                        } else {
                            ParsedLine {
                                entries: vec![LogEntry {
                                    event_type: "message".into(),
                                    content: text,
                                }],
                                session_id: None,
                                cost_usd: None,
                                signal,
                            }
                        }
                    }

                    // Command execution (tool call)
                    "command_execution" => {
                        let command = item["command"].as_str().unwrap_or("command");
                        let exit_code = item["exit_code"].as_i64();
                        let _status = item["status"].as_str().unwrap_or("");

                        // Truncate command for display
                        let cmd_display: String = command.chars().take(120).collect();

                        let mut entries = vec![LogEntry {
                            event_type: "tool_call".into(),
                            content: format!("Bash: {cmd_display}"),
                        }];

                        // If there's output and the command failed, show it
                        if let Some(code) = exit_code {
                            if code != 0 {
                                let output = item["aggregated_output"].as_str().unwrap_or("");
                                let truncated: String = output.chars().take(500).collect();
                                if !truncated.is_empty() {
                                    entries.push(LogEntry {
                                        event_type: "error".into(),
                                        content: format!("Exit code {code}: {truncated}"),
                                    });
                                }
                            }
                        }

                        ParsedLine { entries, session_id: None, cost_usd: None, signal }
                    }

                    // MCP tool call — log it and check for signals
                    "mcp_tool_call" => {
                        let tool = item["details"]["tool"].as_str().unwrap_or("mcp_tool");
                        let server = item["details"]["server"].as_str().unwrap_or("");
                        ParsedLine {
                            entries: vec![LogEntry {
                                event_type: "tool_call".into(),
                                content: format!("MCP ({server}): {tool}"),
                            }],
                            session_id: None,
                            cost_usd: None,
                            signal,
                        }
                    }

                    // File changes
                    "file_change" | "file_edit" => {
                        let path = item["file_path"]
                            .as_str()
                            .or_else(|| item["path"].as_str())
                            .unwrap_or("file");
                        ParsedLine {
                            entries: vec![LogEntry {
                                event_type: "tool_call".into(),
                                content: format!("Edit: {path}"),
                            }],
                            session_id: None,
                            cost_usd: None,
                            signal,
                        }
                    }

                    // Thinking/reasoning items
                    "reasoning" | "thinking" => {
                        let text = item["text"]
                            .as_str()
                            .or_else(|| item["content"].as_str())
                            .unwrap_or("")
                            .to_string();
                        if text.is_empty() {
                            ParsedLine { entries: vec![], session_id: None, cost_usd: None, signal }
                        } else {
                            ParsedLine {
                                entries: vec![LogEntry {
                                    event_type: "thinking".into(),
                                    content: text,
                                }],
                                session_id: None,
                                cost_usd: None,
                                signal,
                            }
                        }
                    }

                    _ => ParsedLine { entries: vec![], session_id: None, cost_usd: None, signal },
                }
            }

            // Item started — show in-progress tool calls
            "item.started" => {
                let item = &json["item"];
                let item_type = item["type"].as_str().unwrap_or("");

                if item_type == "command_execution" {
                    let command = item["command"].as_str().unwrap_or("command");
                    let cmd_display: String = command.chars().take(120).collect();
                    ParsedLine {
                        entries: vec![LogEntry {
                            event_type: "tool_call".into(),
                            content: format!("Running: {cmd_display}"),
                        }],
                        session_id: None,
                        cost_usd: None,
                        signal,
                    }
                } else {
                    ParsedLine { entries: vec![], session_id: None, cost_usd: None, signal }
                }
            }

            // Turn completed — usage stats
            "turn.completed" => {
                let input = json["usage"]["input_tokens"].as_u64().unwrap_or(0);
                let output = json["usage"]["output_tokens"].as_u64().unwrap_or(0);

                let entries = if input > 0 || output > 0 {
                    vec![LogEntry {
                        event_type: "result".into(),
                        content: format!("{input} in / {output} out tokens"),
                    }]
                } else {
                    vec![]
                };

                ParsedLine { entries, session_id: None, cost_usd: None, signal }
            }

            // Error events
            "error" => {
                // Format: {"type":"error","status":400,"error":{"type":"...","message":"..."}}
                let content = json["error"]["message"]
                    .as_str()
                    .or_else(|| json["message"].as_str())
                    .unwrap_or("Unknown error")
                    .to_string();

                ParsedLine {
                    entries: vec![LogEntry {
                        event_type: "error".into(),
                        content,
                    }],
                    session_id: None,
                    cost_usd: None,
                    signal,
                }
            }

            // turn.started, thread.updated, etc — no useful info to show
            _ => ParsedLine { entries: vec![], session_id: None, cost_usd: None, signal },
        }
    }
}
