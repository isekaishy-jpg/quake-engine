//! The MCP server itself, spoken to as an agent speaks to it.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use serde_json::{Value, json};

/// The server, and how to talk to it.
struct Server {
    /// The program.
    child: Child,
    /// What goes to it.
    input: ChildStdin,
    /// What comes back.
    output: BufReader<ChildStdout>,
}

impl Server {
    /// A server working on an empty content folder.
    fn start(name: &str) -> Self {
        let folder = std::env::temp_dir().join(format!("mcp_{name}_{}", std::process::id()));
        std::fs::create_dir_all(&folder).expect("a folder");
        let mut child = Command::new(env!("CARGO_BIN_EXE_engine-mcp"))
            .arg(&folder)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("starts");
        let input = child.stdin.take().expect("input");
        let output = BufReader::new(child.stdout.take().expect("output"));
        Self {
            child,
            input,
            output,
        }
    }

    /// Sends `line`, and reads the answer.
    fn say(&mut self, line: &str) -> Value {
        writeln!(self.input, "{line}").expect("sends");
        self.input.flush().expect("sends");
        let mut answer = String::new();
        self.output.read_line(&mut answer).expect("answers");
        serde_json::from_str(&answer).expect("JSON")
    }

    /// Asks for `method`.
    fn ask(&mut self, method: &str, params: &Value) -> Value {
        let request = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        self.say(&request.to_string())
    }

    /// Calls the tool `name`.
    fn call(&mut self, name: &str, arguments: &Value) -> Value {
        self.ask(
            "tools/call",
            &json!({ "name": name, "arguments": arguments }),
        )["result"]
            .clone()
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _killed = self.child.kill();
        let _waited = self.child.wait();
    }
}

#[test]
fn every_tool_says_what_it_takes() {
    let mut server = Server::start("schemas");
    let answer = server.ask("tools/list", &json!({}));
    let tools = answer["result"]["tools"].as_array().expect("tools");
    assert!(tools.len() > 40, "{}", tools.len());
    let add_brush = tools
        .iter()
        .find(|tool| tool["name"] == "add_brush")
        .expect("add_brush");
    let schema = &add_brush["inputSchema"];
    assert_eq!(schema["required"], json!(["cuboid"]));
    assert!(schema["properties"]["material"]["description"].is_string());
    for tool in tools {
        let schema = &tool["inputSchema"];
        assert_eq!(schema["type"], "object", "{}", tool["name"]);
        assert!(schema["properties"].is_object(), "{}", tool["name"]);
    }
}

#[test]
fn what_isnt_json_gets_an_answer() {
    let mut server = Server::start("broken");
    let answer = server.say("{not json");
    assert_eq!(answer["error"]["code"], -32700);
    assert_eq!(server.ask("ping", &json!({}))["id"], 1);
}

#[test]
fn a_mistake_comes_back_as_an_error_with_a_reason() {
    let mut server = Server::start("mistake");
    let answer = server.call("add_brush", &json!({ "cuboid": [10, 10, 10, 0, 0, 0] }));
    assert_eq!(answer["isError"], true);
    let answer = server.call("no_such_tool", &json!({}));
    assert_eq!(answer["isError"], true);
}

#[test]
fn a_path_cant_leave_the_content() {
    let mut server = Server::start("paths");
    let answer = server.call("save", &json!({ "path": "../outside.emap" }));
    assert_eq!(answer["isError"], true);
    let answer = server.call("save", &json!({ "path": "maps/inside.emap" }));
    assert_ne!(answer["isError"], true, "{answer}");
}
