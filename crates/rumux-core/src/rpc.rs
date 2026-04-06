use anyhow::{Context, Result};
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
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

    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    let response: Value = serde_json::from_str(&response_line)?;
    Ok(response)
}

pub fn send_rpc(method: &str, params: Value) -> Result<Value> {
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
