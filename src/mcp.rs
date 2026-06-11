//! Minimal stdio MCP (Model Context Protocol) server that wraps the CLI contract.
//!
//! Speaks newline-delimited JSON-RPC 2.0 over stdin/stdout. Each tool call shells
//! out to this same binary so the JSON contract stays defined in exactly one place.

use std::io::BufRead;
use std::io::Write;
use std::process::ExitCode;

use clap::Args;
use serde_json::Value;
use serde_json::json;

const PROTOCOL_VERSION: &str = "2025-06-18";
const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Args)]
pub struct McpArgs {}

/// The result of executing one tool.
pub struct ToolRun {
    pub text: String,
    pub is_error: bool,
}

pub fn run(_args: McpArgs) -> Result<ExitCode, Box<dyn std::error::Error>> {
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let line = line?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(trimmed) {
            Ok(request) => dispatch(&request, &mut run_tool_via_subprocess),
            Err(_) => Some(error_response(&Value::Null, -32700, "parse error")),
        };
        if let Some(response) = response {
            writeln!(out, "{}", serde_json::to_string(&response)?)?;
            out.flush()?;
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Handle a single JSON-RPC request. Returns `None` for notifications (no `id`).
///
/// `run_tool` is injected so the dispatch logic can be tested without spawning
/// subprocesses.
pub fn dispatch(
    request: &Value,
    run_tool: &mut dyn FnMut(&str, &Value) -> ToolRun,
) -> Option<Value> {
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");

    // Notifications carry no id and expect no response.
    let id = request.get("id")?;

    let response = match method {
        "initialize" => result_response(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "cloche", "version": VERSION }
            }),
        ),
        "ping" => result_response(id, json!({})),
        "tools/list" => result_response(id, json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let params = request.get("params").cloned().unwrap_or(Value::Null);
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            if !is_known_tool(name) {
                error_response(id, -32602, &format!("unknown tool: {name}"))
            } else {
                let run = run_tool(name, &arguments);
                result_response(
                    id,
                    json!({
                        "content": [{ "type": "text", "text": run.text }],
                        "isError": run.is_error
                    }),
                )
            }
        }
        _ => error_response(id, -32601, &format!("method not found: {method}")),
    };
    Some(response)
}

fn result_response(id: &Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn error_response(id: &Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn is_known_tool(name: &str) -> bool {
    matches!(
        name,
        "capture" | "polish" | "list_windows" | "doctor" | "latest" | "gallery"
    )
}

fn tool_definitions() -> Value {
    json!([
        {
            "name": "capture",
            "description": "Capture an app/window screenshot and return the Cloche JSON result.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "target": { "type": "string", "enum": ["active", "screen", "window"], "default": "active" },
                    "title": { "type": "string", "description": "Window title to match when target=window." },
                    "app": { "type": "string", "description": "App name to match when target=window." },
                    "windowId": { "type": "string" },
                    "outDir": { "type": "string", "description": "Output directory for the capture." },
                    "presentation": { "type": "string", "enum": ["raw", "card", "both"], "default": "both" },
                    "detail": { "type": "string", "enum": ["auto", "low", "high", "original"], "default": "high" },
                    "styleSeed": { "type": "integer" }
                }
            }
        },
        {
            "name": "polish",
            "description": "Style an existing image into a Cloche presentation card (rounded window, shadows, gradient backdrop) and return the JSON result.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Path to the image to style (PNG, JPEG, or WebP)." },
                    "out": { "type": "string", "description": "Output card path; defaults to <input>-card.png next to the input." },
                    "palette": { "type": "string", "enum": crate::polish::palette_names(), "description": "Gradient palette; random when omitted." },
                    "styleSeed": { "type": "integer", "description": "Seed for deterministic styling." }
                },
                "required": ["input"]
            }
        },
        {
            "name": "list_windows",
            "description": "List capturable windows as JSON.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "doctor",
            "description": "Report capture backend health and capabilities as JSON.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "latest",
            "description": "Return the most recent capture summary as JSON.",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "gallery",
            "description": "List recent captures as JSON.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "limit": { "type": "integer", "default": 20 }
                }
            }
        }
    ])
}

/// Translate a tool call into Cloche CLI arguments. Pure and testable.
pub fn tool_command_args(name: &str, arguments: &Value) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    match name {
        "capture" => {
            args.push("capture".to_string());
            args.push("--format".to_string());
            args.push("json".to_string());
            if let Some(value) = string_arg(arguments, "target") {
                args.push("--target".to_string());
                args.push(value);
            }
            if let Some(value) = string_arg(arguments, "title") {
                args.push("--title".to_string());
                args.push(value);
            }
            if let Some(value) = string_arg(arguments, "app") {
                args.push("--app".to_string());
                args.push(value);
            }
            if let Some(value) = string_arg(arguments, "windowId") {
                args.push("--window-id".to_string());
                args.push(value);
            }
            if let Some(value) = string_arg(arguments, "outDir") {
                args.push("--out-dir".to_string());
                args.push(value);
            }
            if let Some(value) = string_arg(arguments, "presentation") {
                args.push("--presentation".to_string());
                args.push(value);
            }
            if let Some(value) = string_arg(arguments, "detail") {
                args.push("--detail".to_string());
                args.push(value);
            }
            if let Some(value) = arguments.get("styleSeed").and_then(Value::as_u64) {
                args.push("--style-seed".to_string());
                args.push(value.to_string());
            }
        }
        "polish" => {
            let input =
                string_arg(arguments, "input").ok_or("polish requires an input image path")?;
            args.push("polish".to_string());
            args.push("--format".to_string());
            args.push("json".to_string());
            args.push(input);
            if let Some(value) = string_arg(arguments, "out") {
                args.push("--out".to_string());
                args.push(value);
            }
            if let Some(value) = string_arg(arguments, "palette") {
                args.push("--palette".to_string());
                args.push(value);
            }
            if let Some(value) = arguments.get("styleSeed").and_then(Value::as_u64) {
                args.push("--style-seed".to_string());
                args.push(value.to_string());
            }
        }
        "list_windows" => {
            args.push("list-windows".to_string());
            args.push("--format".to_string());
            args.push("json".to_string());
        }
        "doctor" => {
            args.push("doctor".to_string());
            args.push("--format".to_string());
            args.push("json".to_string());
        }
        "latest" => {
            args.push("latest".to_string());
            args.push("--format".to_string());
            args.push("json".to_string());
        }
        "gallery" => {
            args.push("gallery".to_string());
            args.push("--format".to_string());
            args.push("json".to_string());
            if let Some(value) = arguments.get("limit").and_then(Value::as_u64) {
                args.push("--limit".to_string());
                args.push(value.to_string());
            }
        }
        other => return Err(format!("unknown tool: {other}")),
    }
    Ok(args)
}

fn string_arg(arguments: &Value, key: &str) -> Option<String> {
    arguments
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|value| !value.is_empty())
}

fn run_tool_via_subprocess(name: &str, arguments: &Value) -> ToolRun {
    let args = match tool_command_args(name, arguments) {
        Ok(args) => args,
        Err(message) => {
            return ToolRun {
                text: message,
                is_error: true,
            };
        }
    };
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(err) => {
            return ToolRun {
                text: format!("failed to locate Cloche binary: {err}"),
                is_error: true,
            };
        }
    };
    match std::process::Command::new(exe).args(&args).output() {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let text = if output.status.success() {
                stdout
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if stdout.is_empty() { stderr } else { stdout }
            };
            ToolRun {
                text,
                is_error: !output.status.success(),
            }
        }
        Err(err) => ToolRun {
            text: format!("failed to run capture: {err}"),
            is_error: true,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn never_called(_name: &str, _arguments: &Value) -> ToolRun {
        panic!("tool runner should not be called");
    }

    #[test]
    fn initialize_reports_protocol_and_server() {
        let request = json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" });
        let response = dispatch(&request, &mut never_called).expect("response");
        assert_eq!(response["result"]["protocolVersion"], PROTOCOL_VERSION);
        assert_eq!(response["result"]["serverInfo"]["name"], "cloche");
        assert_eq!(response["id"], 1);
    }

    #[test]
    fn tools_list_includes_capture() {
        let request = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" });
        let response = dispatch(&request, &mut never_called).expect("response");
        let tools = response["result"]["tools"].as_array().expect("tools array");
        assert!(tools.iter().any(|tool| tool["name"] == "capture"));
    }

    #[test]
    fn notification_yields_no_response() {
        let request = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
        assert!(dispatch(&request, &mut never_called).is_none());
    }

    #[test]
    fn unknown_method_returns_method_not_found() {
        let request = json!({ "jsonrpc": "2.0", "id": 3, "method": "frobnicate" });
        let response = dispatch(&request, &mut never_called).expect("response");
        assert_eq!(response["error"]["code"], -32601);
    }

    #[test]
    fn unknown_tool_returns_invalid_params() {
        let request = json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "bogus", "arguments": {} }
        });
        let response = dispatch(&request, &mut never_called).expect("response");
        assert_eq!(response["error"]["code"], -32602);
    }

    #[test]
    fn tools_call_wraps_runner_output() {
        let request = json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "doctor", "arguments": {} }
        });
        let mut runner = |name: &str, _args: &Value| {
            assert_eq!(name, "doctor");
            ToolRun {
                text: "{\"ok\":true}".to_string(),
                is_error: false,
            }
        };
        let response = dispatch(&request, &mut runner).expect("response");
        assert_eq!(response["result"]["isError"], false);
        assert_eq!(response["result"]["content"][0]["text"], "{\"ok\":true}");
    }

    #[test]
    fn capture_args_include_target_and_title() {
        let args = tool_command_args(
            "capture",
            &json!({ "target": "window", "title": "Firefox", "styleSeed": 7 }),
        )
        .expect("args");
        assert_eq!(args[0], "capture");
        assert!(args.windows(2).any(|w| w == ["--target", "window"]));
        assert!(args.windows(2).any(|w| w == ["--title", "Firefox"]));
        assert!(args.windows(2).any(|w| w == ["--style-seed", "7"]));
    }

    #[test]
    fn gallery_args_pass_limit() {
        let args = tool_command_args("gallery", &json!({ "limit": 5 })).expect("args");
        assert!(args.windows(2).any(|w| w == ["--limit", "5"]));
    }

    #[test]
    fn tools_list_includes_polish() {
        let request = json!({ "jsonrpc": "2.0", "id": 6, "method": "tools/list" });
        let response = dispatch(&request, &mut never_called).expect("response");
        let tools = response["result"]["tools"].as_array().expect("tools array");
        assert!(tools.iter().any(|tool| tool["name"] == "polish"));
    }

    #[test]
    fn polish_args_map_input_out_palette_and_seed() {
        let args = tool_command_args(
            "polish",
            &json!({
                "input": "/tmp/shot.png",
                "out": "/tmp/card.png",
                "palette": "violet-haze",
                "styleSeed": 12
            }),
        )
        .expect("args");
        assert_eq!(args[0], "polish");
        assert!(args.contains(&"/tmp/shot.png".to_string()));
        assert!(args.windows(2).any(|w| w == ["--out", "/tmp/card.png"]));
        assert!(args.windows(2).any(|w| w == ["--palette", "violet-haze"]));
        assert!(args.windows(2).any(|w| w == ["--style-seed", "12"]));
    }

    #[test]
    fn polish_args_require_input() {
        let error = tool_command_args("polish", &json!({})).expect_err("missing input");
        assert!(error.contains("input"));
    }
}
