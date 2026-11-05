//! The platform's command line. Every command answers in JSON.
//!
//! `engine-tool <content> <map file or -> <operation> [json]`, or
//! `engine-tool <content> <map file or -> --batch <json file>` to run a list of
//! `{"op": ..., "args": ...}`. A map that changed is saved afterwards.

#![expect(clippy::print_stdout, reason = "this tool's output is its answer")]
#![expect(clippy::print_stderr, reason = "this tool reports its failures")]

use std::path::Path;
use std::process::ExitCode;

use engine_tools::{OPERATIONS, Session, run};
use serde_json::{Value, json};

/// Runs one step, printing its answer. Returns whether it worked.
fn step(session: &mut Session, name: &str, args: &Value) -> bool {
    match run(session, name, args) {
        Ok(answer) => {
            println!("{answer}");
            true
        }
        Err(error) => {
            println!("{}", json!({ "error": error }));
            false
        }
    }
}

/// The steps of a batch file.
fn batch_steps(path: &str) -> Result<Vec<(String, Value)>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
    let parsed: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
    let steps = parsed.as_array().ok_or("a batch is a list")?;
    steps
        .iter()
        .map(|item| {
            let name = item
                .get("op")
                .and_then(Value::as_str)
                .ok_or("each step needs an op")?;
            let args = item.get("args").cloned().unwrap_or_else(|| json!({}));
            Ok((name.to_owned(), args))
        })
        .collect()
}

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [content, map, rest @ ..] = arguments.as_slice() else {
        eprintln!("usage: engine-tool <content> <map file or -> <operation> [json]");
        eprintln!("       engine-tool <content> <map file or -> --batch <json file>");
        for operation in OPERATIONS {
            eprintln!("  {}: {}", operation.name, operation.help);
        }
        return ExitCode::FAILURE;
    };
    let content = Path::new(content);
    let mut session = if map == "-" || !Path::new(map).exists() {
        let mut session = Session::new(content);
        if map != "-" {
            let _saved = session.save(Some(Path::new(map)));
        }
        session
    } else {
        match Session::open(Path::new(map), content) {
            Ok(session) => session,
            Err(error) => {
                println!("{}", json!({ "error": error }));
                return ExitCode::FAILURE;
            }
        }
    };
    let steps = match rest {
        [flag, path] if flag == "--batch" => match batch_steps(path) {
            Ok(steps) => steps,
            Err(error) => {
                println!("{}", json!({ "error": error }));
                return ExitCode::FAILURE;
            }
        },
        [name] => vec![(name.clone(), json!({}))],
        [name, args] => match serde_json::from_str(args) {
            Ok(args) => vec![(name.clone(), args)],
            Err(error) => {
                println!("{}", json!({ "error": error.to_string() }));
                return ExitCode::FAILURE;
            }
        },
        _ => {
            eprintln!("expected an operation");
            return ExitCode::FAILURE;
        }
    };
    let mut worked = true;
    for (name, args) in &steps {
        worked &= step(&mut session, name, args);
    }
    if map != "-" && worked {
        let _saved = session.save(None);
    }
    if worked {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
