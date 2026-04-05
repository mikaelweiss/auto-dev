use crate::provider::{self, ParsedLine, Provider, ProviderKind, SessionConfig};
use crate::sdk_types::CliMessage;

/// Claude Code CLI provider.
pub struct ClaudeProvider;

impl Provider for ClaudeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Claude
    }

    fn find_binary(&self) -> Result<String, String> {
        // Check ~/.local/bin first (common install location)
        if let Ok(home) = std::env::var("HOME") {
            let local_path = format!("{home}/.local/bin/claude");
            if std::path::Path::new(&local_path).exists() {
                return Ok(local_path);
            }
        }

        for path in &["/usr/local/bin/claude", "/opt/homebrew/bin/claude"] {
            if std::path::Path::new(path).exists() {
                return Ok(path.to_string());
            }
        }

        // Fallback: use `which`
        let output = std::process::Command::new("/usr/bin/which")
            .arg("claude")
            .output()
            .map_err(|e| format!("Failed to run which: {e}"))?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            Err("claude CLI not found. Install it from https://claude.ai/cli".to_string())
        }
    }

    fn build_command(
        &self,
        binary_path: &str,
        config: &SessionConfig,
    ) -> tokio::process::Command {
        let mut cmd = tokio::process::Command::new(binary_path);
        cmd.args([
            "-p",
            "--verbose",
            "--output-format",
            "stream-json",
            "--permission-mode",
            &config.permission_mode,
            "--model",
            &config.model,
            "--effort",
            &config.effort,
        ]);

        // Inject MCP config for autodev state advancement tools
        if let Some(ref mcp_binary) = config.mcp_binary_path {
            let mcp_config = serde_json::json!({
                "mcpServers": {
                    "autodev": {
                        "command": mcp_binary,
                        "args": []
                    }
                }
            });
            cmd.arg("--mcp-config").arg(mcp_config.to_string());
            cmd.args([
                "--allowed-tools",
                "mcp__autodev__advance_to_blocked,mcp__autodev__advance_to_in_progress,mcp__autodev__advance_to_review",
            ]);
        }

        // Resume a previous conversation if we have a CLI session ID
        if let Some(ref resume_id) = config.resume_session_id {
            cmd.arg("--resume").arg(resume_id);
        } else {
            // Only set system prompt for new conversations
            cmd.arg("--system-prompt").arg(&config.system_prompt);
        }

        cmd.arg(&config.user_prompt)
            .current_dir(&config.worktree_path);

        cmd
    }

    fn parse_line(&self, line: &str) -> ParsedLine {
        let msg = CliMessage::parse(line);

        let session_id = msg.session_id().map(String::from);

        let cost_usd = if let CliMessage::Result(ref result) = msg {
            result.total_cost_usd
        } else {
            None
        };

        // Detect state advancement signals from MCP tool calls
        let signal = serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|json| provider::detect_signal_from_json(&json));

        let entries = msg.to_log_entries();

        ParsedLine {
            entries,
            session_id,
            cost_usd,
            signal,
        }
    }
}
