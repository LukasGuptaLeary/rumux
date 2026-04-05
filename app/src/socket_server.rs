use anyhow::{Context, Result};
use async_net::TcpListener;
use futures_lite::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use rumux_core::runtime::{IpcEndpoint, ipc_endpoint};
use serde::{Deserialize, Serialize};

#[cfg(unix)]
use smol::net::unix::UnixListener;

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub id: String,
    pub method: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub params: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct RpcResponse {
    pub id: String,
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl RpcResponse {
    pub fn success(id: String, result: serde_json::Value) -> Self {
        Self {
            id,
            ok: true,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: String, error: String) -> Self {
        Self {
            id,
            ok: false,
            result: None,
            error: Some(error),
        }
    }
}

pub async fn start_socket_server() -> Result<()> {
    match ipc_endpoint() {
        #[cfg(unix)]
        IpcEndpoint::Unix(path) => start_unix_socket_server(path).await,
        IpcEndpoint::Tcp(addr) => start_tcp_socket_server(addr).await,
    }
}

#[cfg(unix)]
async fn start_unix_socket_server(path: std::path::PathBuf) -> Result<()> {
    if path.exists() {
        std::fs::remove_file(&path).ok();
    }

    let listener = UnixListener::bind(&path)
        .with_context(|| format!("Failed to bind Unix socket at {}", path.display()))?;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                smol::spawn(handle_stream(stream)).detach();
            }
            Err(error) => {
                eprintln!("Socket accept error: {error}");
            }
        }
    }
}

async fn start_tcp_socket_server(addr: std::net::SocketAddr) -> Result<()> {
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind TCP socket at {addr}"))?;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                smol::spawn(handle_stream(stream)).detach();
            }
            Err(error) => {
                eprintln!("Socket accept error: {error}");
            }
        }
    }
}

async fn handle_stream<S>(stream: S)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (reader, mut writer) = smol::io::split(stream);
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    if buf_reader.read_line(&mut line).await.is_ok() && !line.is_empty() {
        let response = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(req) => handle_request(req),
            Err(error) => RpcResponse::error(String::new(), format!("Invalid JSON: {error}")),
        };

        if let Ok(json) = serde_json::to_string(&response) {
            let _ = writer.write_all(json.as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
            let _ = writer.flush().await;
        }
    }
}

fn handle_request(req: RpcRequest) -> RpcResponse {
    match req.method.as_str() {
        "system.ping" => RpcResponse::success(req.id, serde_json::json!({ "pong": true })),

        "system.capabilities" => RpcResponse::success(
            req.id,
            serde_json::json!({
                "methods": [
                    "system.ping",
                    "system.capabilities",
                    "system.identify",
                    "workspace.list",
                    "workspace.create",
                    "workspace.select",
                    "workspace.current",
                    "workspace.close",
                    "surface.list",
                    "surface.split",
                    "surface.focus",
                    "surface.send_text",
                    "surface.send_key",
                    "notification.create",
                    "notification.list",
                    "notification.clear",
                    "sidebar.set_status",
                    "sidebar.clear_status",
                    "sidebar.set_progress",
                    "sidebar.clear_progress",
                    "sidebar.log",
                    "sidebar.clear_log",
                    "sidebar.state"
                ],
                "access_mode": "rumuxOnly",
                "transport": format!("{:?}", ipc_endpoint())
            }),
        ),

        "system.identify" => RpcResponse::success(
            req.id,
            serde_json::json!({
                "app": "rumux",
                "version": env!("CARGO_PKG_VERSION")
            }),
        ),

        _ => RpcResponse::error(req.id, format!("Unknown method: {}", req.method)),
    }
}
