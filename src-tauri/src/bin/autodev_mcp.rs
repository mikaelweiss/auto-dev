use std::io::{self, BufRead, Write};

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
    // Read headers until empty line
    let mut content_length: Option<usize> = None;
    loop {
        let mut line = String::new();
        let bytes = reader.read_line(&mut line)?;
        if bytes == 0 {
            return Ok(None); // EOF
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            break; // End of headers
        }
        if let Some(len_str) = trimmed.strip_prefix("Content-Length: ") {
            content_length = len_str.trim().parse().ok();
        }
    }

    let length = match content_length {
        Some(len) => len,
        None => return Ok(None),
    };

    let mut body = vec![0u8; length];
    reader.read_exact(&mut body)?;

    serde_json::from_slice(&body)
        .map(Some)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
}

fn write_message(writer: &mut impl Write, msg: &Value) -> io::Result<()> {
    let body = serde_json::to_string(msg)?;
    write!(writer, "Content-Length: {}\r\n\r\n{}", body.len(), body)?;
    writer.flush()
}

fn handle_request(request: &Value) -> Option<Value> {
    let id = request.get("id").cloned();
    let method = request.get("method").and_then(|m| m.as_str()).unwrap_or("");

    match method {
        "initialize" => Some(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
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
                            "properties": {
                                "reason": {
                                    "type": "string",
                                    "description": "The specific questions or blockers that need human input"
                                }
                            },
                            "required": ["reason"]
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

            let message = match tool_name {
                "advance_to_blocked" => {
                    "State updated to blocked. AutoDev has been notified and will present your questions to the user. Stop working now and wait for a response."
                }
                "advance_to_in_progress" => {
                    "State updated to in_progress. AutoDev will automatically start the implementation phase in a new session. Stop working now."
                }
                "advance_to_review" => {
                    "State updated to review. The code is now marked as ready for human review. Stop working now."
                }
                _ => "Unknown tool",
            };

            Some(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{
                        "type": "text",
                        "text": message
                    }]
                }
            }))
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
