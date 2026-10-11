use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;

use crate::runtime::{IpcEndpoint, ipc_endpoint};

#[cfg(unix)]
use std::os::unix::net::UnixStream;

fn send_and_read_response<S>(mut stream: S, method: &str, params: Value) -> Result<Value>
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

pub fn send_rpc(method: &str, params: Value) -> Result<Value> {
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
            send_and_read_response(stream, method, params)
        }
        IpcEndpoint::Tcp(addr) => {
            let stream = TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(3))
                .with_context(|| {
                    format!(
                        "Failed to connect to rumux TCP socket at {addr}. Is rumux-app running?"
                    )
                })?;
            stream.set_read_timeout(timeout)?;
            stream.set_write_timeout(timeout)?;
            send_and_read_response(stream, method, params)
        }
    }
}
