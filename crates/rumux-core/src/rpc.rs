use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

use crate::runtime::{IpcEndpoint, ipc_endpoint, ipc_token_path};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

fn send_and_read_response<S>(
    mut stream: S,
    method: &str,
    params: Value,
    auth_token: Option<&str>,
) -> Result<Value>
where
    S: std::io::Read + Write,
{
    let mut request = serde_json::json!({
        "id": "1",
        "method": method,
        "params": params,
    });
    if let Some(token) = auth_token {
        request["auth_token"] = serde_json::json!(token);
    }

    let mut data = serde_json::to_string(&request)?;
    data.push('\n');
    stream.write_all(data.as_bytes())?;
    stream.flush()?;

    let mut reader = BufReader::new(stream).take(4 * 1024 * 1024);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;
    anyhow::ensure!(
        response_line.ends_with('\n'),
        "RPC response was incomplete or exceeded 4 MiB"
    );

    let response: Value = serde_json::from_str(&response_line)?;
    Ok(response)
}

pub fn send_rpc(method: &str, mut params: Value) -> Result<Value> {
    anyhow::ensure!(params.is_object(), "RPC params must be a JSON object");
    if (method.starts_with("surface.") || method == "notification.create")
        && ![
            "workspace_id",
            "workspace_index",
            "workspace_name",
            "surface_id",
            "surface_index",
        ]
        .iter()
        .any(|key| params.get(key).is_some_and(|value| !value.is_null()))
    {
        if let Ok(id) = std::env::var("RUMUX_SURFACE_ID") {
            if !id.is_empty() {
                params["surface_id"] = serde_json::json!(id);
            }
        }
    }
    let timeout_ms = params
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(if method == "surface.run_command" {
            30_000
        } else {
            5_000
        })
        .clamp(100, 300_000)
        .saturating_add(5_000);
    let timeout = Some(std::time::Duration::from_millis(timeout_ms));
    match ipc_endpoint() {
        #[cfg(unix)]
        IpcEndpoint::Unix(path) => {
            let stream = UnixStream::connect(&path).with_context(|| {
                format!(
                    "Failed to connect to rumux Unix socket at {}. Is rumux-app running?",
                    path.display()
                )
            })?;
            stream.set_read_timeout(timeout)?;
            stream.set_write_timeout(timeout)?;
            send_and_read_response(stream, method, params, None)
        }
        IpcEndpoint::Tcp(addr) => {
            anyhow::ensure!(
                addr.ip().is_loopback(),
                "Rumux RPC requires a loopback address"
            );
            let token = std::fs::read_to_string(ipc_token_path(addr))
                .context("Failed to read Rumux TCP authentication token. Is rumux-app running in this profile?")?;
            let stream = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3))
                .with_context(|| {
                    format!(
                        "Failed to connect to rumux TCP socket at {addr}. Is rumux-app running?"
                    )
                })?;
            stream.set_read_timeout(timeout)?;
            stream.set_write_timeout(timeout)?;
            send_and_read_response(stream, method, params, Some(token.trim()))
        }
    }
}
