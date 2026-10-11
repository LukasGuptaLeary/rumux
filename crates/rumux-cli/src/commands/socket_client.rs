use anyhow::{Context, Result};
use rumux_core::rpc::send_rpc;

pub fn run_ping(json: bool) -> Result<()> {
    let response = send_rpc("system.ping", serde_json::json!({}))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if response.get("ok") == Some(&serde_json::Value::Bool(true)) {
        println!("pong");
    } else {
        let err = response
            .get("error")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown error");
        anyhow::bail!("{err}");
    }
    ensure_success(&response)
}

pub fn run_capabilities(json: bool) -> Result<()> {
    let response = send_rpc("system.capabilities", serde_json::json!({}))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if let Some(result) = response.get("result") {
        if let Some(methods) = result.get("methods").and_then(|m| m.as_array()) {
            for m in methods {
                if let Some(s) = m.as_str() {
                    println!("  {s}");
                }
            }
        }
    }
    ensure_success(&response)
}

pub fn run_identify(json: bool) -> Result<()> {
    let response = send_rpc("system.identify", serde_json::json!({}))?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else if let Some(result) = response.get("result") {
        let app = result.get("app").and_then(|a| a.as_str()).unwrap_or("?");
        let version = result
            .get("version")
            .and_then(|v| v.as_str())
            .unwrap_or("?");
        println!("{app} {version}");
    }
    ensure_success(&response)
}

pub fn run_notify(
    title: &str,
    body: &str,
    workspace_index: Option<usize>,
    surface_index: Option<usize>,
    json: bool,
) -> Result<()> {
    let response = send_rpc(
        "notification.create",
        serde_json::json!({ "title": title, "body": body, "workspace_index": workspace_index, "surface_index": surface_index }),
    )?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        ensure_success(&response)?;
        println!("Notification sent");
    }
    ensure_success(&response)
}

pub fn run_raw(method: &str, params: &str, _json: bool) -> Result<()> {
    let params: serde_json::Value = if params.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(params).context("Invalid JSON params")?
    };
    let response = send_rpc(method, params)?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    ensure_success(&response)
}

fn ensure_success(response: &serde_json::Value) -> Result<()> {
    if response.get("ok") == Some(&serde_json::Value::Bool(true)) {
        return Ok(());
    }
    anyhow::bail!(
        "{}",
        response
            .get("error")
            .and_then(|value| value.as_str())
            .unwrap_or("Invalid RPC response")
    );
}
