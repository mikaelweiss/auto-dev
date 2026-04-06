use std::io::{self, BufRead, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

use serde_json::{json, Value};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut out = stdout.lock();

    loop {
        match read_message(&mut reader) {
            Ok(Some(request)) => {
                let response = handle_request(&request);
                if let Some(resp) = response {
                    let _ = write_message(&mut out, &resp);
                }
            }
            Ok(None) => break, // EOF
            Err(_) => break,
        }
    }
}

fn read_message(reader: &mut impl BufRead) -> io::Result<Option<Value>> {
    let mut line = String::new();
    let bytes = reader.read_line(&mut line)?;
    if bytes == 0 {
        return Ok(None); // EOF
    }
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    serde_json::from_str(trimmed)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_message(writer: &mut impl Write, msg: &Value) -> io::Result<()> {
    let body = serde_json::to_string(msg)?;
    writeln!(writer, "{body}")?;
    writer.flush()
}

fn handle_request(request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request
        .get("method")
        .and_then(|m| m.as_str())
        .unwrap_or("");

    match method {
        "initialize" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2025-11-25",
                "serverInfo": {
                    "name": "autodev",
                    "version": "0.1.0"
                },
                "capabilities": {
                    "tools": {}
                }
            }
        })),

        // Notifications have no id and need no response
        "notifications/initialized" | "notifications/cancelled" => None,

        "tools/list" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [
                    {
                        "name": "advance_to_blocked",
                        "description": "Signal that this issue needs human input before the AI can proceed. Call this when you have blocking questions or need clarification from the user.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    },
                    {
                        "name": "advance_to_in_progress",
                        "description": "Signal that the specification is complete and implementation should begin automatically. Call this after you have posted the spec as a comment on the issue.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    },
                    {
                        "name": "advance_to_review",
                        "description": "Signal that implementation is complete and the code is ready for human review. Call this after you have committed all changes.",
                        "inputSchema": {
                            "type": "object",
                            "properties": {},
                            "required": []
                        }
                    }
                ]
            }
        })),

        "tools/call" => {
            let tool_name = request
                .pointer("/params/name")
                .and_then(|n| n.as_str())
                .unwrap_or("");

            match call_autodev(tool_name) {
                Ok(msg) => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{
                            "type": "text",
                            "text": msg
                        }]
                    }
                })),
                Err(e) => Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "content": [{
                            "type": "text",
                            "text": format!("Error: {e}")
                        }],
                        "isError": true
                    }
                })),
            }
        }

        _ => {
            if id.is_some() {
                Some(json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Method not found: {method}")
                    }
                }))
            } else {
                None // Ignore unknown notifications
            }
        }
    }
}

/// Call back to the Tauri app's HTTP server to perform the actual state change.
fn call_autodev(action: &str) -> Result<String, String> {
    let port = std::env::var("AUTODEV_CALLBACK_PORT")
        .map_err(|_| "AUTODEV_CALLBACK_PORT not set — MCP server not connected to AutoDev app")?;
    let session_id = std::env::var("AUTODEV_SESSION_ID")
        .map_err(|_| "AUTODEV_SESSION_ID not set")?;
    let repo_id = std::env::var("AUTODEV_REPO_ID")
        .map_err(|_| "AUTODEV_REPO_ID not set")?;
    let issue_number = std::env::var("AUTODEV_ISSUE_NUMBER")
        .map_err(|_| "AUTODEV_ISSUE_NUMBER not set")?;

    let body = json!({
        "action": action,
        "session_id": session_id,
        "repo_id": repo_id,
        "issue_number": issue_number,
    })
    .to_string();

    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}"))
        .map_err(|e| format!("Failed to connect to AutoDev app: {e}"))?;

    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .ok();
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .ok();

    let request = format!(
        "POST /signal HTTP/1.1\r\n\
         Host: 127.0.0.1:{port}\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        body.len()
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|e| format!("Write failed: {e}"))?;

    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|e| format!("Read failed: {e}"))?;

    // Extract body from HTTP response
    let resp_body = response.split("\r\n\r\n").nth(1).unwrap_or("");

    // Parse response JSON to get the message
    if let Ok(resp_json) = serde_json::from_str::<Value>(resp_body) {
        if let Some(msg) = resp_json["message"].as_str() {
            return Ok(msg.to_string());
        }
        if let Some(err) = resp_json["error"].as_str() {
            return Err(err.to_string());
        }
    }

    // If response starts with HTTP/1.1 4xx or 5xx, it's an error
    if response.starts_with("HTTP/1.1 4") || response.starts_with("HTTP/1.1 5") {
        return Err(format!("AutoDev app returned error: {resp_body}"));
    }

    Ok("Signal sent successfully".to_string())
}
