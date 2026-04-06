use anyhow::{Context, Result};
use rumux_core::rpc::send_rpc;
use serde_json::{Map, Value, json};
use std::io::{self, BufRead, Write};

const CURRENT_PROTOCOL_VERSION: &str = "2025-06-18";
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2025-06-18", "2025-03-26", "2024-11-05"];

pub fn run() -> Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }

        let response = match serde_json::from_str::<Value>(&line) {
            Ok(Value::Array(messages)) => {
                let responses: Vec<Value> = messages
                    .into_iter()
                    .filter_map(|message| handle_message(message).transpose())
                    .collect::<Result<_>>()?;

                if responses.is_empty() {
                    None
                } else {
                    Some(Value::Array(responses))
                }
            }
            Ok(message) => handle_message(message)?,
            Err(error) => Some(error_response(
                Value::Null,
                -32700,
                format!("Invalid JSON: {error}"),
            )),
        };

        if let Some(response) = response {
            serde_json::to_writer(&mut stdout, &response)?;
            stdout.write_all(b"\n")?;
            stdout.flush()?;
        }
    }

    Ok(())
}

fn handle_message(message: Value) -> Result<Option<Value>> {
    let Value::Object(message) = message else {
        return Ok(Some(error_response(
            Value::Null,
            -32600,
            "Invalid request".to_string(),
        )));
    };

    let id = message.get("id").cloned();
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Ok(Some(error_response(
            id.unwrap_or(Value::Null),
            -32600,
            "Invalid request: missing method".to_string(),
        )));
    };
    let params = message
        .get("params")
        .cloned()
        .unwrap_or_else(|| Value::Object(Map::new()));

    let response = match method {
        "initialize" => Some(success_response(
            id.unwrap_or(Value::Null),
            initialize_result(&params),
        )),
        "ping" => id.map(|id| success_response(id, json!({}))),
        "tools/list" => id.map(|id| success_response(id, json!({ "tools": tool_definitions() }))),
        "tools/call" => {
            let Some(id) = id else {
                return Ok(None);
            };
            match handle_tool_call(&params) {
                Ok(result) => Some(success_response(id, result)),
                Err(error) => Some(error_response(id, -32602, error.to_string())),
            }
        }
        "notifications/initialized" | "$/cancelRequest" => None,
        _ if id.is_some() => Some(error_response(
            id.unwrap_or(Value::Null),
            -32601,
            format!("Method not found: {method}"),
        )),
        _ => None,
    };

    Ok(response)
}

fn initialize_result(params: &Value) -> Value {
    let requested_version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(CURRENT_PROTOCOL_VERSION);

    let negotiated_version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested_version) {
        requested_version
    } else {
        CURRENT_PROTOCOL_VERSION
    };

    json!({
        "protocolVersion": negotiated_version,
        "capabilities": {
            "tools": {
                "listChanged": false
            }
        },
        "serverInfo": {
            "name": "rumux",
            "title": "rumux MCP",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Use these tools to inspect and control a running rumux desktop app. Prefer agent-oriented tools like run_command, read_terminal, and wait_for_output. Use rpc_raw only as an escape hatch for unsupported actions."
    })
}

fn handle_tool_call(params: &Value) -> Result<Value> {
    let name = params
        .get("name")
        .and_then(Value::as_str)
        .context("Missing tool name")?;

    let arguments = params
        .get("arguments")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();

    let result = match name {
        "activate_app" => execute_rpc(name, "system.activate", Value::Object(arguments)),
        "get_app_identity" => execute_rpc(name, "system.identify", Value::Object(arguments)),
        "get_app_capabilities" => {
            execute_rpc(name, "system.capabilities", Value::Object(arguments))
        }
        "list_workspaces" => execute_rpc(name, "workspace.list", Value::Object(arguments)),
        "get_current_workspace" => execute_rpc(name, "workspace.current", Value::Object(arguments)),
        "select_workspace" => execute_rpc(name, "workspace.select", Value::Object(arguments)),
        "create_workspace" => execute_rpc(name, "workspace.create", Value::Object(arguments)),
        "rename_workspace" => execute_rpc(name, "workspace.rename", Value::Object(arguments)),
        "duplicate_workspace" => execute_rpc(name, "workspace.duplicate", Value::Object(arguments)),
        "next_workspace" => execute_rpc(name, "workspace.next", Value::Object(arguments)),
        "prev_workspace" => execute_rpc(name, "workspace.prev", Value::Object(arguments)),
        "close_workspace" => execute_rpc(name, "workspace.close", Value::Object(arguments)),
        "list_surfaces" => execute_rpc(name, "surface.list", Value::Object(arguments)),
        "focus_surface" => execute_rpc(name, "surface.focus", Value::Object(arguments)),
        "create_surface" => execute_rpc(name, "surface.create", Value::Object(arguments)),
        "split_surface" => execute_rpc(name, "surface.split", Value::Object(arguments)),
        "close_surface" => execute_rpc(name, "surface.close", Value::Object(arguments)),
        "rename_surface" => execute_rpc(name, "surface.rename", Value::Object(arguments)),
        "next_surface" => execute_rpc(name, "surface.next", Value::Object(arguments)),
        "prev_surface" => execute_rpc(name, "surface.prev", Value::Object(arguments)),
        "zoom_surface" => execute_rpc(name, "surface.zoom", Value::Object(arguments)),
        "send_text" => execute_rpc(name, "surface.send_text", Value::Object(arguments)),
        "send_key" => execute_rpc(name, "surface.send_key", Value::Object(arguments)),
        "read_terminal" => execute_rpc(name, "surface.read", Value::Object(arguments)),
        "wait_for_output" => execute_rpc(name, "surface.wait_for_output", Value::Object(arguments)),
        "run_command" => execute_rpc(name, "surface.run_command", Value::Object(arguments)),
        "interrupt_command" => execute_rpc(name, "surface.interrupt", Value::Object(arguments)),
        "select_all" => execute_rpc(name, "surface.select_all", Value::Object(arguments)),
        "copy_selection" => execute_rpc(name, "surface.copy_selection", Value::Object(arguments)),
        "copy_all" => execute_rpc(name, "surface.copy_all", Value::Object(arguments)),
        "paste" => execute_rpc(name, "surface.paste", Value::Object(arguments)),
        "create_notification" => execute_rpc(name, "notification.create", Value::Object(arguments)),
        "list_notifications" => execute_rpc(name, "notification.list", Value::Object(arguments)),
        "clear_notifications" => execute_rpc(name, "notification.clear", Value::Object(arguments)),
        "rpc_raw" => {
            let method = arguments
                .get("method")
                .and_then(Value::as_str)
                .context("rpc_raw requires a method string")?;
            let params = arguments
                .get("params")
                .cloned()
                .unwrap_or_else(|| Value::Object(Map::new()));
            execute_rpc(name, method, params)
        }
        _ => anyhow::bail!("Unknown tool: {name}"),
    };

    Ok(match result {
        Ok(value) => tool_success(value),
        Err(error) => tool_error(error.to_string()),
    })
}

fn execute_rpc(tool_name: &str, method: &str, params: Value) -> Result<Value> {
    let response = send_rpc(method, params)?;
    if response.get("ok") == Some(&Value::Bool(true)) {
        let result = response.get("result").cloned().unwrap_or_else(|| json!({}));
        Ok(normalize_tool_result(tool_name, result))
    } else {
        let error = response
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Unknown RPC error");
        anyhow::bail!("{error}");
    }
}

fn normalize_tool_result(tool_name: &str, value: Value) -> Value {
    match (tool_name, value) {
        ("list_workspaces", Value::Array(workspaces)) => json!({ "workspaces": workspaces }),
        ("list_notifications", Value::Array(notifications)) => {
            json!({ "notifications": notifications })
        }
        ("rpc_raw", response) => json!({ "response": response }),
        (_, value) => value,
    }
}

fn tool_success(value: Value) -> Value {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string());
    let mut result = json!({
        "content": [
            {
                "type": "text",
                "text": text,
            }
        ],
        "isError": false,
    });

    if value.is_object() {
        result["structuredContent"] = value;
    }

    result
}

fn tool_error(message: String) -> Value {
    json!({
        "content": [
            {
                "type": "text",
                "text": message,
            }
        ],
        "isError": true,
    })
}

fn success_response(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result,
    })
}

fn error_response(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message,
        }
    })
}

fn tool_definitions() -> Vec<Value> {
    vec![
        tool(
            "activate_app",
            "Raise and focus the running rumux desktop app.",
            json_schema_object([]),
        ),
        tool(
            "list_workspaces",
            "List all workspaces in the running rumux app.",
            json_schema_object([]),
        ),
        tool(
            "get_current_workspace",
            "Return the currently active workspace.",
            json_schema_object([]),
        ),
        tool(
            "select_workspace",
            "Activate a workspace by index or exact name.",
            json_schema_object([
                ("index", integer_schema("Workspace index")),
                ("name", string_schema("Exact workspace name")),
            ]),
        ),
        tool(
            "create_workspace",
            "Create a new workspace, optionally with a custom name.",
            json_schema_object([("name", string_schema("Optional workspace name"))]),
        ),
        tool(
            "rename_workspace",
            "Rename a workspace by index or exact name.",
            json_schema_object([
                ("index", integer_schema("Workspace index")),
                ("name", string_schema("Exact workspace name")),
                ("new_name", string_schema("New workspace name")),
            ]),
        ),
        tool(
            "duplicate_workspace",
            "Duplicate a workspace by index or exact name.",
            json_schema_object([
                ("index", integer_schema("Workspace index")),
                ("name", string_schema("Exact workspace name")),
            ]),
        ),
        tool(
            "close_workspace",
            "Close a workspace by index or exact name.",
            json_schema_object([
                ("index", integer_schema("Workspace index")),
                ("name", string_schema("Exact workspace name")),
            ]),
        ),
        tool(
            "list_surfaces",
            "List terminal surfaces for a workspace.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
            ]),
        ),
        tool(
            "focus_surface",
            "Focus a terminal surface by workspace and optional surface index.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
                ("surface_index", integer_schema("Surface index")),
            ]),
        ),
        tool(
            "create_surface",
            "Create a new terminal tab in the targeted pane.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
                ("surface_index", integer_schema("Anchor surface index")),
            ]),
        ),
        tool(
            "split_surface",
            "Split the targeted surface to the right or down.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
                ("surface_index", integer_schema("Anchor surface index")),
                (
                    "placement",
                    json!({
                        "type": "string",
                        "description": "Split placement: right or down",
                        "enum": ["right", "down"],
                    }),
                ),
            ]),
        ),
        tool(
            "close_surface",
            "Close a terminal surface by workspace and optional surface index.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
                ("surface_index", integer_schema("Surface index")),
            ]),
        ),
        tool(
            "rename_surface",
            "Rename a terminal tab by workspace and optional surface index. Pass an empty string to clear the custom title.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
                ("surface_index", integer_schema("Surface index")),
                ("name", string_schema("New tab title")),
            ]),
        ),
        tool(
            "zoom_surface",
            "Toggle zoom for the targeted surface pane.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
                ("surface_index", integer_schema("Anchor surface index")),
            ]),
        ),
        tool(
            "read_terminal",
            "Read terminal text from the targeted surface.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
                ("surface_index", integer_schema("Surface index")),
                (
                    "scope",
                    json!({
                        "type": "string",
                        "description": "Read scope: buffer or visible",
                        "enum": ["buffer", "visible"],
                    }),
                ),
                (
                    "max_chars",
                    integer_schema("Maximum number of trailing characters to return"),
                ),
            ]),
        ),
        tool(
            "wait_for_output",
            "Wait for new terminal output containing a substring.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
                ("surface_index", integer_schema("Surface index")),
                (
                    "pattern",
                    string_schema("Substring to wait for in new output"),
                ),
                (
                    "scope",
                    json!({
                        "type": "string",
                        "description": "Read scope: buffer or visible",
                        "enum": ["buffer", "visible"],
                    }),
                ),
                (
                    "timeout_ms",
                    integer_schema("Maximum time to wait in milliseconds"),
                ),
                (
                    "max_chars",
                    integer_schema("Maximum number of trailing characters to return"),
                ),
            ]),
        ),
        tool(
            "run_command",
            "Run a shell command in the targeted terminal and wait for completion using rumux markers.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
                ("surface_index", integer_schema("Surface index")),
                ("command", string_schema("Shell command text to execute")),
                (
                    "timeout_ms",
                    integer_schema("Maximum time to wait in milliseconds"),
                ),
                (
                    "max_chars",
                    integer_schema("Maximum number of trailing characters to return"),
                ),
            ]),
        ),
        tool(
            "interrupt_command",
            "Interrupt the targeted terminal with ctrl-c.",
            json_schema_object([
                ("workspace_index", integer_schema("Workspace index")),
                ("workspace_name", string_schema("Exact workspace name")),
                ("surface_index", integer_schema("Surface index")),
            ]),
        ),
        tool(
            "create_notification",
            "Create a notification in the running rumux app.",
            json_schema_object([
                ("title", string_schema("Notification title")),
                ("subtitle", string_schema("Optional notification subtitle")),
                ("body", string_schema("Notification body")),
                ("workspace_index", integer_schema("Target workspace index")),
                (
                    "workspace_name",
                    string_schema("Exact target workspace name"),
                ),
            ]),
        ),
        tool(
            "rpc_raw",
            "Advanced escape hatch for calling a raw rumux desktop RPC method with arbitrary JSON params.",
            json_schema_object([
                ("method", string_schema("Raw RPC method name")),
                (
                    "params",
                    json!({
                        "type": "object",
                        "description": "Raw JSON params object",
                        "additionalProperties": true,
                    }),
                ),
            ]),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema,
    })
}

fn json_schema_object<const N: usize>(properties: [(&str, Value); N]) -> Value {
    let mut map = Map::new();
    for (name, value) in properties {
        map.insert(name.to_string(), value);
    }

    Value::Object(
        [
            ("type".to_string(), Value::String("object".to_string())),
            ("properties".to_string(), Value::Object(map)),
            ("additionalProperties".to_string(), Value::Bool(false)),
        ]
        .into_iter()
        .collect(),
    )
}

fn string_schema(description: &str) -> Value {
    json!({
        "type": "string",
        "description": description,
    })
}

fn integer_schema(description: &str) -> Value {
    json!({
        "type": "integer",
        "description": description,
        "minimum": 0,
    })
}
