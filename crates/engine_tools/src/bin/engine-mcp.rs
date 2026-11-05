//! The platform as an MCP server, on standard input and output.
//!
//! `engine-mcp <content>`. Each operation is a tool, and `open` chooses the
//! map to work on.

#![expect(clippy::print_stdout, reason = "standard output is the protocol")]

use std::io::BufRead;
use std::path::Path;

use engine_tools::{OPERATIONS, Session, run};
use serde_json::{Value, json};

/// The protocol version this speaks.
const PROTOCOL: &str = "2024-11-05";

/// The answer to a request.
fn answer(session: &mut Session, method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": PROTOCOL,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "engine-tools", "version": env!("CARGO_PKG_VERSION") },
        })),
        "tools/list" => {
            let tools: Vec<Value> = OPERATIONS
                .iter()
                .map(|operation| {
                    json!({
                        "name": operation.name,
                        "description": operation.help,
                        "inputSchema": { "type": "object" },
                    })
                })
                .collect();
            Ok(json!({ "tools": tools }))
        }
        "tools/call" => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .ok_or("a tool call needs a name")?;
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            Ok(match run(session, name, &args) {
                Ok(result) => json!({
                    "content": [{ "type": "text", "text": result.to_string() }],
                }),
                Err(error) => json!({
                    "content": [{ "type": "text", "text": error }],
                    "isError": true,
                }),
            })
        }
        "ping" => Ok(json!({})),
        other => Err(format!("unknown method {other}")),
    }
}

fn main() {
    let content = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "content".to_owned());
    let mut session = Session::new(Path::new(&content));
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else {
            break;
        };
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let Some(id) = request.get("id").cloned() else {
            continue;
        };
        let method = request.get("method").and_then(Value::as_str).unwrap_or("");
        let params = request.get("params").cloned().unwrap_or_else(|| json!({}));
        let reply = match answer(&mut session, method, &params) {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err(error) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": error },
            }),
        };
        println!("{reply}");
    }
}
