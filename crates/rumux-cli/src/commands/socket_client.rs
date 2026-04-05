use anyhow::{Context, Result};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;

use rumux_core::runtime::{IpcEndpoint, ipc_endpoint};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

fn send_and_read_response<S>(
    mut stream: S,
    method: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value>
where
    S: std::io::Read + Write,
{
    let request = serde_json::json!({
        "id": "1",
        "method": method,
        "params": params,
    });

    let mut data = serde_json::to_string(&request)?;
    data.push('\n');
    stream.write_all(data.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    let response: serde_json::Value = serde_json::from_str(&response_line)?;
    Ok(response)
}

fn send_rpc(method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
    match ipc_endpoint() {
        #[cfg(unix)]
        IpcEndpoint::Unix(path) => {
            let stream = UnixStream::connect(&path).with_context(|| {
                format!(
                    "Failed to connect to rumux Unix socket at {}. Is rumux-app running?",
                    path.display()
                )
            })?;
            send_and_read_response(stream, method, params)
        }
        IpcEndpoint::Tcp(addr) => {
            let stream = TcpStream::connect(addr).with_context(|| {
                format!("Failed to connect to rumux TCP socket at {addr}. Is rumux-app running?")
            })?;
            send_and_read_response(stream, method, params)
        }
    }
}

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
    Ok(())
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
    Ok(())
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
    Ok(())
}

pub fn run_notify(title: &str, body: &str, json: bool) -> Result<()> {
    let response = send_rpc(
        "notification.create",
        serde_json::json!({ "title": title, "body": body }),
    )?;
    if json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        println!("Notification sent");
    }
    Ok(())
}

pub fn run_raw(method: &str, params: &str, _json: bool) -> Result<()> {
    let params: serde_json::Value = if params.is_empty() {
        serde_json::json!({})
    } else {
        serde_json::from_str(params).context("Invalid JSON params")?
    };
    let response = send_rpc(method, params)?;
    println!("{}", serde_json::to_string_pretty(&response)?);
    Ok(())
}
