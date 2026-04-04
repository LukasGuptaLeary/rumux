use anyhow::{Context, Result};
use futures_lite::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use serde::{Deserialize, Serialize};
use smol::net::unix::UnixListener;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    pub id: String,
    pub method: String,
    #[serde(default)]
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

pub fn socket_path() -> PathBuf {
    std::env::var("RUMUX_SOCKET_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/rumux.sock"))
}

pub async fn start_socket_server() -> Result<()> {
    let path = socket_path();

    if path.exists() {
        std::fs::remove_file(&path).ok();
    }

    let listener = UnixListener::bind(&path).context("Failed to bind Unix socket")?;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                smol::spawn(async move {
                    let (reader, mut writer) = smol::io::split(stream);
                    let mut buf_reader = BufReader::new(reader);
                    let mut line = String::new();

                    if buf_reader.read_line(&mut line).await.is_ok() && !line.is_empty() {
                        let response = match serde_json::from_str::<RpcRequest>(&line) {
                            Ok(req) => handle_request(req),
                            Err(e) => {
                                RpcResponse::error(String::new(), format!("Invalid JSON: {e}"))
                            }
                        };

                        if let Ok(json) = serde_json::to_string(&response) {
                            let _ = writer.write_all(json.as_bytes()).await;
                            let _ = writer.write_all(b"\n").await;
                            let _ = writer.flush().await;
                        }
                    }
                })
                .detach();
            }
            Err(e) => {
                eprintln!("Socket accept error: {e}");
            }
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
                "access_mode": "rumuxOnly"
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
