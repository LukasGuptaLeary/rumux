use anyhow::Context;
use async_net::TcpListener;
use futures_lite::io::{AsyncBufReadExt, AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use gpui::{AppContext, AsyncApp, Context as ViewContext, Keystroke, WeakEntity, Window};
use rumux_core::runtime::{IpcEndpoint, ipc_endpoint};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::app_state::AppState;

#[cfg(unix)]
use smol::net::unix::UnixListener;

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

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WorkspaceTargetParams {
    index: Option<usize>,
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct WorkspaceCreateParams {
    name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct SurfaceTargetParams {
    workspace_index: Option<usize>,
    workspace_name: Option<String>,
    surface_index: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SurfaceSendTextParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    text: String,
    #[serde(default)]
    append_newline: bool,
}

#[derive(Debug, Deserialize)]
struct SurfaceSendKeyParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    keystroke: String,
}

#[derive(Debug, Deserialize)]
struct NotificationCreateParams {
    title: String,
    #[serde(default)]
    subtitle: Option<String>,
    body: String,
    #[serde(default)]
    workspace_index: Option<usize>,
    #[serde(default)]
    workspace_name: Option<String>,
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

pub async fn start_socket_server(
    app_state: WeakEntity<AppState>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    match ipc_endpoint() {
        #[cfg(unix)]
        IpcEndpoint::Unix(path) => start_unix_socket_server(path, app_state, cx).await,
        IpcEndpoint::Tcp(addr) => start_tcp_socket_server(addr, app_state, cx).await,
    }
}

#[cfg(unix)]
async fn start_unix_socket_server(
    path: std::path::PathBuf,
    app_state: WeakEntity<AppState>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    if path.exists() {
        std::fs::remove_file(&path).ok();
    }

    let listener = UnixListener::bind(&path)
        .with_context(|| format!("Failed to bind Unix socket at {}", path.display()))?;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                handle_stream(stream, app_state.clone(), cx).await;
            }
            Err(error) => {
                eprintln!("Socket accept error: {error}");
            }
        }
    }
}

async fn start_tcp_socket_server(
    addr: std::net::SocketAddr,
    app_state: WeakEntity<AppState>,
    cx: &mut AsyncApp,
) -> anyhow::Result<()> {
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind TCP socket at {addr}"))?;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                handle_stream(stream, app_state.clone(), cx).await;
            }
            Err(error) => {
                eprintln!("Socket accept error: {error}");
            }
        }
    }
}

async fn handle_stream<S>(stream: S, app_state: WeakEntity<AppState>, cx: &mut AsyncApp)
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = smol::io::split(stream);
    let mut buf_reader = BufReader::new(reader);
    let mut line = String::new();

    if buf_reader.read_line(&mut line).await.is_ok() && !line.is_empty() {
        let response = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(req) => handle_request(req, &app_state, cx),
            Err(error) => RpcResponse::error(String::new(), format!("Invalid JSON: {error}")),
        };

        if let Ok(json) = serde_json::to_string(&response) {
            let _ = writer.write_all(json.as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
            let _ = writer.flush().await;
        }
    }
}

fn handle_request(
    req: RpcRequest,
    app_state: &WeakEntity<AppState>,
    cx: &mut AsyncApp,
) -> RpcResponse {
    let RpcRequest { id, method, params } = req;

    let result = match method.as_str() {
        "system.ping" => Ok(serde_json::json!({ "pong": true })),
        "system.capabilities" => Ok(serde_json::json!({
            "methods": supported_methods(),
            "access_mode": "rumuxOnly",
            "transport": format!("{:?}", ipc_endpoint()),
        })),
        "system.identify" => Ok(serde_json::json!({
            "app": "rumux",
            "version": env!("CARGO_PKG_VERSION"),
        })),

        "workspace.list" => with_state(app_state, cx, |state, cx| {
            to_value(state.workspace_summaries(&**cx))
        }),
        "workspace.current" => with_state(app_state, cx, |state, cx| {
            let summary = state
                .workspace_summaries(&**cx)
                .into_iter()
                .find(|workspace| workspace.active)
                .ok_or_else(|| "No active workspace".to_string())?;
            to_value(summary)
        }),
        "workspace.select" => {
            let params = match parse_params::<WorkspaceTargetParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            with_window_state(app_state, cx, move |window, state, cx| {
                let workspace_index =
                    resolve_workspace_index(state, params.index, params.name.as_deref(), &**cx)?;
                state.set_active_workspace(workspace_index, window, cx);
                to_value(state.workspaces[workspace_index].read(cx).summary(
                    workspace_index,
                    true,
                    &**cx,
                ))
            })
        }
        "workspace.create" => {
            let params = match parse_params::<WorkspaceCreateParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            with_window_state(app_state, cx, move |window, state, cx| {
                state.add_workspace_named(params.name.clone(), window, cx);
                to_value(
                    state.workspaces[state.active_workspace_idx]
                        .read(cx)
                        .summary(state.active_workspace_idx, true, &**cx),
                )
            })
        }
        "workspace.close" => {
            let params = match parse_params::<WorkspaceTargetParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            with_window_state(app_state, cx, move |window, state, cx| {
                let workspace_index =
                    resolve_workspace_index(state, params.index, params.name.as_deref(), &**cx)?;
                if state.workspaces.len() <= 1 {
                    return Err("Cannot close the last workspace".to_string());
                }
                state.close_workspace(workspace_index, window, cx);
                Ok(serde_json::json!({
                    "closed": workspace_index,
                    "active_workspace_index": state.active_workspace_idx
                }))
            })
        }

        "surface.list" => {
            let params = match parse_params::<SurfaceTargetParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            with_state(app_state, cx, move |state, cx| {
                let workspace_index = resolve_workspace_index(
                    state,
                    params.workspace_index,
                    params.workspace_name.as_deref(),
                    &**cx,
                )?;
                let workspace = state.workspaces[workspace_index].read(cx);
                Ok(serde_json::json!({
                    "workspace_index": workspace_index,
                    "workspace_name": workspace.name.clone(),
                    "surfaces": workspace.list_surfaces(&**cx),
                }))
            })
        }
        "surface.focus" => {
            let params = match parse_params::<SurfaceTargetParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            with_window_state(app_state, cx, move |window, state, cx| {
                let workspace_index = resolve_workspace_index(
                    state,
                    params.workspace_index,
                    params.workspace_name.as_deref(),
                    &**cx,
                )?;

                if workspace_index != state.active_workspace_idx {
                    state.set_active_workspace(workspace_index, window, cx);
                }

                let workspace = state.workspaces[workspace_index].clone();
                let focused = workspace.update(cx, |workspace, cx| {
                    workspace.focus_surface(params.surface_index, window, cx)
                });

                if focused {
                    Ok(serde_json::json!({
                        "focused": true,
                        "workspace_index": workspace_index,
                        "surface_index": params.surface_index,
                    }))
                } else {
                    Err(surface_not_found_message(params.surface_index))
                }
            })
        }
        "surface.send_text" => {
            let params = match parse_params::<SurfaceSendTextParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            with_state(app_state, cx, move |state, cx| {
                let workspace_index = resolve_workspace_index(
                    state,
                    params.target.workspace_index,
                    params.target.workspace_name.as_deref(),
                    &**cx,
                )?;

                let workspace = state.workspaces[workspace_index].clone();
                let mut text = params.text.clone();
                if params.append_newline {
                    text.push('\n');
                }

                let sent = workspace.update(cx, |workspace, cx| {
                    workspace.send_text_to_surface(params.target.surface_index, &text, cx)
                });

                if sent {
                    Ok(serde_json::json!({
                        "sent": true,
                        "workspace_index": workspace_index,
                        "surface_index": params.target.surface_index,
                    }))
                } else {
                    Err(surface_not_found_message(params.target.surface_index))
                }
            })
        }
        "surface.send_key" => {
            let params = match parse_params::<SurfaceSendKeyParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            let keystroke = match Keystroke::parse(&params.keystroke) {
                Ok(keystroke) => keystroke,
                Err(error) => {
                    return RpcResponse::error(
                        id,
                        format!("Invalid keystroke '{}': {error}", params.keystroke),
                    );
                }
            };

            with_state(app_state, cx, move |state, cx| {
                let workspace_index = resolve_workspace_index(
                    state,
                    params.target.workspace_index,
                    params.target.workspace_name.as_deref(),
                    &**cx,
                )?;

                let workspace = state.workspaces[workspace_index].clone();
                let sent = workspace.update(cx, |workspace, cx| {
                    workspace.send_keystroke_to_surface(params.target.surface_index, &keystroke, cx)
                });

                if sent {
                    Ok(serde_json::json!({
                        "sent": true,
                        "workspace_index": workspace_index,
                        "surface_index": params.target.surface_index,
                        "keystroke": params.keystroke,
                    }))
                } else {
                    Err(surface_not_found_message(params.target.surface_index))
                }
            })
        }

        "notification.create" => {
            let params = match parse_params::<NotificationCreateParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            with_state(app_state, cx, move |state, cx| {
                let workspace_index = resolve_workspace_index(
                    state,
                    params.workspace_index,
                    params.workspace_name.as_deref(),
                    &**cx,
                )?;
                let notification = state.create_notification(
                    params.title.clone(),
                    params.subtitle.clone(),
                    params.body.clone(),
                    workspace_index,
                    cx,
                );
                to_value(notification)
            })
        }
        "notification.list" => with_state(app_state, cx, |state, _cx| {
            to_value(state.notifications.clone())
        }),
        "notification.clear" => with_state(app_state, cx, |state, cx| {
            let cleared = state.notifications.len();
            state.clear_notifications(cx);
            Ok(serde_json::json!({ "cleared": cleared }))
        }),

        _ => Err(format!("Unknown method: {method}")),
    };

    match result {
        Ok(result) => RpcResponse::success(id, result),
        Err(error) => RpcResponse::error(id, error),
    }
}

fn supported_methods() -> &'static [&'static str] {
    &[
        "system.ping",
        "system.capabilities",
        "system.identify",
        "workspace.list",
        "workspace.create",
        "workspace.select",
        "workspace.current",
        "workspace.close",
        "surface.list",
        "surface.focus",
        "surface.send_text",
        "surface.send_key",
        "notification.create",
        "notification.list",
        "notification.clear",
    ]
}

fn parse_params<T: DeserializeOwned>(params: serde_json::Value) -> Result<T, String> {
    serde_json::from_value(params).map_err(|error| format!("Invalid params: {error}"))
}

fn to_value<T: Serialize>(value: T) -> Result<serde_json::Value, String> {
    serde_json::to_value(value).map_err(|error| format!("Failed to serialize response: {error}"))
}

fn with_state<T, F>(app_state: &WeakEntity<AppState>, cx: &mut AsyncApp, f: F) -> Result<T, String>
where
    F: FnOnce(&mut AppState, &mut ViewContext<AppState>) -> Result<T, String>,
{
    match app_state.update(cx, f) {
        Ok(result) => result,
        Err(_) => Err("rumux app state is not available".to_string()),
    }
}

fn with_window_state<T, F>(
    app_state: &WeakEntity<AppState>,
    cx: &mut AsyncApp,
    f: F,
) -> Result<T, String>
where
    F: FnOnce(&mut Window, &mut AppState, &mut ViewContext<AppState>) -> Result<T, String>,
{
    let window = match cx.update(|cx| cx.active_window()) {
        Ok(Some(window)) => window,
        Ok(None) => return Err("rumux window is not available".to_string()),
        Err(_) => return Err("rumux window is not available".to_string()),
    };

    match cx.update_window(window, |_, window, cx| {
        app_state.update(cx, |state, cx| f(window, state, cx))
    }) {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err("rumux app state is not available".to_string()),
        Err(_) => Err("rumux window is not available".to_string()),
    }
}

fn resolve_workspace_index(
    state: &AppState,
    index: Option<usize>,
    name: Option<&str>,
    cx: &gpui::App,
) -> Result<usize, String> {
    if let Some(index) = index {
        if index < state.workspaces.len() {
            return Ok(index);
        }
        return Err(format!("Workspace index {index} is out of range"));
    }

    if let Some(name) = name {
        return state
            .workspace_index_by_name(name, cx)
            .ok_or_else(|| format!("Workspace '{name}' was not found"));
    }

    Ok(state.active_workspace_idx)
}

fn surface_not_found_message(surface_index: Option<usize>) -> String {
    match surface_index {
        Some(surface_index) => format!("Surface {surface_index} was not found"),
        None => "No target surface is available".to_string(),
    }
}
