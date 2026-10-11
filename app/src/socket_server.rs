use anyhow::Context;
use async_net::TcpListener;
use futures_lite::io::{
    AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader,
};
use gpui::{AppContext, AsyncApp, Context as ViewContext, Keystroke, WeakEntity, Window};
use gpui_component::Placement;
use rumux_core::runtime::{IpcEndpoint, default_shell, ipc_endpoint, ipc_token_path};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::app_state::AppState;
use crate::workspace::{SurfaceReadScope, Workspace};

#[cfg(unix)]
use smol::net::unix::UnixListener;

#[derive(Debug, Deserialize)]
pub struct RpcRequest {
    #[serde(default)]
    auth_token: Option<String>,
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
    ssh: Option<rumux_core::ssh::SshConnection>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WorkspaceRenameParams {
    #[serde(flatten)]
    target: WorkspaceTargetParams,
    new_name: String,
}

#[derive(Debug, Default, Clone, Deserialize, Serialize)]
#[serde(default)]
struct SurfaceTargetParams {
    workspace_id: Option<String>,
    surface_id: Option<String>,
    workspace_index: Option<usize>,
    workspace_name: Option<String>,
    surface_index: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SurfaceRenameParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SurfaceStatusParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    status: rumux_core::status::SurfaceStatus,
    message: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrowserCreateParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    url: Option<String>,
    placement: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BrowserNavigateParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    url: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct BrowserEvaluateParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    expression: String,
    selector: Option<String>,
    text: Option<String>,
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct SurfaceSplitParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    placement: String,
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
struct SurfaceReadParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    max_chars: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SurfaceWaitForOutputParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    pattern: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    max_chars: Option<usize>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct SurfaceRunCommandParams {
    #[serde(flatten)]
    target: SurfaceTargetParams,
    command: String,
    #[serde(default)]
    max_chars: Option<usize>,
    #[serde(default)]
    timeout_ms: Option<u64>,
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
    #[serde(default)]
    surface_index: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct NotificationTargetParams {
    id: String,
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
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create Unix socket directory at {}",
                parent.display()
            )
        })?;
    }

    if path.exists() {
        std::fs::remove_file(&path).ok();
    }

    let listener = UnixListener::bind(&path)
        .with_context(|| format!("Failed to bind Unix socket at {}", path.display()))?;
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let state = app_state.clone();
                cx.spawn(async move |cx| {
                    handle_stream(stream, state, None, cx).await;
                })
                .detach();
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
    anyhow::ensure!(
        addr.ip().is_loopback(),
        "Rumux RPC must bind to a loopback address"
    );
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("Failed to bind TCP socket at {addr}"))?;
    let token = create_auth_token(addr)?;

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                let state = app_state.clone();
                let token = token.clone();
                cx.spawn(async move |cx| {
                    handle_stream(stream, state, Some(&token), cx).await;
                })
                .detach();
            }
            Err(error) => {
                eprintln!("Socket accept error: {error}");
            }
        }
    }
}

fn create_auth_token(addr: std::net::SocketAddr) -> anyhow::Result<String> {
    create_auth_token_at(&ipc_token_path(addr))
}

fn create_auth_token_at(path: &std::path::Path) -> anyhow::Result<String> {
    use std::io::Write;
    let parent = path.parent().context("RPC token path has no parent")?;
    std::fs::create_dir_all(parent)?;
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(token.as_bytes())?;
    file.as_file().sync_all()?;
    file.persist(path)
        .context("Failed to save TCP authentication token")?;
    Ok(token)
}

fn token_matches(expected: &str, provided: Option<&str>) -> bool {
    let Some(provided) = provided else {
        return false;
    };
    if provided.len() != expected.len() {
        return false;
    }
    expected
        .bytes()
        .zip(provided.bytes())
        .fold(0u8, |diff, (a, b)| diff | (a ^ b))
        == 0
}

async fn handle_stream<S>(
    stream: S,
    app_state: WeakEntity<AppState>,
    auth_token: Option<&str>,
    cx: &mut AsyncApp,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = smol::io::split(stream);
    let mut buf_reader = BufReader::new(reader.take(1024 * 1024));
    let mut line = String::new();

    let read = smol::future::or(async { buf_reader.read_line(&mut line).await }, async {
        smol::Timer::after(std::time::Duration::from_secs(5)).await;
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "RPC request timed out",
        ))
    })
    .await;
    if read.is_ok() && line.ends_with('\n') {
        let response = match serde_json::from_str::<RpcRequest>(&line) {
            Ok(mut req) => match if auth_token
                .is_some_and(|token| !token_matches(token, req.auth_token.as_deref()))
            {
                Err("TCP RPC authentication failed".into())
            } else {
                normalize_targets(&mut req, &app_state, cx)
            } {
                Err(error) => RpcResponse::error(req.id, error),
                Ok(()) => match req.method.as_str() {
                    "browser.eval" | "browser.snapshot" | "browser.click" | "browser.fill" => {
                        handle_browser_evaluation(req, &app_state, cx).await
                    }
                    "surface.wait_for_output" => {
                        handle_surface_wait_for_output(req, &app_state, cx).await
                    }
                    "surface.run_command" => handle_surface_run_command(req, &app_state, cx).await,
                    _ => handle_request(req, &app_state, cx),
                },
            },
            Err(error) => RpcResponse::error(String::new(), format!("Invalid JSON: {error}")),
        };

        if let Ok(json) = serde_json::to_string(&response) {
            let _ = writer.write_all(json.as_bytes()).await;
            let _ = writer.write_all(b"\n").await;
            let _ = writer.flush().await;
        }
    }
}

// Resolve public IDs at the boundary and pin the implicit target of asynchronous
// calls. Indices stay in responses for existing clients; UUIDs own identity.
fn normalize_targets(
    request: &mut RpcRequest,
    app_state: &WeakEntity<AppState>,
    cx: &mut AsyncApp,
) -> Result<(), String> {
    let surface_method = request.method.starts_with("surface.");
    let browser_method = request.method.starts_with("browser.");
    let workspace_method = request.method.starts_with("workspace.");
    let notification_method = request.method == "notification.create";
    if !(surface_method || browser_method || workspace_method || notification_method) {
        return Ok(());
    }
    if !request.params.is_object() {
        return Err("Params must be an object".into());
    }
    with_state(app_state, cx, |state, cx| {
        if workspace_method {
            if let Some(id) = request.params.get("workspace_id") {
                let id = id.as_str().ok_or("workspace_id must be a UUID string")?;
                if request.params.get("index").is_some_and(|v| !v.is_null())
                    || request.params.get("name").is_some_and(|v| !v.is_null())
                {
                    return Err("Use one workspace selector: workspace_id, index or name".into());
                }
                let index = workspace_index_by_id(state, id, cx)?;
                request.params["index"] = serde_json::json!(index);
            }
            return Ok(());
        }
        let mut target: SurfaceTargetParams = parse_params(request.params.clone())?;
        if [
            target.workspace_id.is_some(),
            target.workspace_index.is_some(),
            target.workspace_name.is_some(),
        ]
        .into_iter()
        .filter(|provided| *provided)
        .count()
            > 1
        {
            return Err(
                "Use one workspace selector: workspace_id, workspace_index or workspace_name"
                    .into(),
            );
        }
        if target.surface_id.is_some() && target.surface_index.is_some() {
            return Err("Use one surface selector: surface_id or surface_index".into());
        }
        let (workspace_index, surface_index) = resolve_surface_params(state, &target, cx)?;
        let workspace = state.workspaces[workspace_index].read(cx);
        target.workspace_id = Some(workspace.id.to_string());
        request.params["workspace_index"] = serde_json::json!(workspace_index);
        request.params["workspace_id"] = serde_json::json!(target.workspace_id);
        if surface_method
            || (browser_method && request.method != "browser.create")
            || target.surface_id.is_some()
            || target.surface_index.is_some()
        {
            let surface = workspace
                .list_surfaces(cx)
                .into_iter()
                .find(|surface| {
                    surface_index.map_or(surface.target, |index| surface.index == index)
                })
                .ok_or_else(|| surface_not_found_message(surface_index))?;
            request.params["surface_index"] = serde_json::json!(surface.index);
            request.params["surface_id"] = serde_json::json!(surface.id);
        }
        Ok(())
    })
}

fn workspace_index_by_id(state: &AppState, id: &str, cx: &gpui::App) -> Result<usize, String> {
    let id = Uuid::parse_str(id).map_err(|_| "workspace_id must be a UUID string")?;
    state
        .workspaces
        .iter()
        .position(|workspace| workspace.read(cx).id == id)
        .ok_or_else(|| format!("Workspace {id} was not found"))
}

fn resolve_surface_params(
    state: &AppState,
    target: &SurfaceTargetParams,
    cx: &gpui::App,
) -> Result<(usize, Option<usize>), String> {
    let workspace_index = if let Some(id) = &target.workspace_id {
        workspace_index_by_id(state, id, cx)?
    } else if target.workspace_index.is_none()
        && target.workspace_name.is_none()
        && target.surface_id.is_some()
    {
        let id = Uuid::parse_str(target.surface_id.as_deref().unwrap_or_default())
            .map_err(|_| "surface_id must be a UUID string")?
            .to_string();
        state
            .workspaces
            .iter()
            .position(|workspace| {
                workspace
                    .read(cx)
                    .list_surfaces(cx)
                    .iter()
                    .any(|surface| surface.id == id)
            })
            .ok_or_else(|| format!("Surface {id} was not found"))?
    } else {
        resolve_workspace_index(
            state,
            target.workspace_index,
            target.workspace_name.as_deref(),
            cx,
        )?
    };
    let surface_index = if let Some(id) = &target.surface_id {
        let id = Uuid::parse_str(id)
            .map_err(|_| "surface_id must be a UUID string")?
            .to_string();
        Some(
            state.workspaces[workspace_index]
                .read(cx)
                .list_surfaces(cx)
                .iter()
                .find(|surface| surface.id == id)
                .ok_or_else(|| format!("Surface {id} was not found"))?
                .index,
        )
    } else {
        target.surface_index
    };
    Ok((workspace_index, surface_index))
}

fn handle_request(
    req: RpcRequest,
    app_state: &WeakEntity<AppState>,
    cx: &mut AsyncApp,
) -> RpcResponse {
    let RpcRequest {
        id, method, params, ..
    } = req;

    let result = match method.as_str() {
        "browser.create" => {
            let params = match parse_params::<BrowserCreateParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };
            let url =
                match crate::browser::normalize_url(params.url.as_deref().unwrap_or("about:blank"))
                {
                    Ok(url) => url,
                    Err(error) => return RpcResponse::error(id, error.to_string()),
                };
            let placement = match params
                .placement
                .as_deref()
                .map(parse_surface_split_placement)
                .transpose()
            {
                Ok(placement) => placement,
                Err(error) => return RpcResponse::error(id, error),
            };
            with_window_state(app_state, cx, |window, state, cx| {
                let (workspace_index, surface_index) =
                    resolve_surface_params(state, &params.target, cx)?;
                state.set_active_workspace(workspace_index, window, cx);
                let workspace = state.workspaces[workspace_index].clone();
                let index = workspace.update(cx, |workspace, cx| {
                    if surface_index.is_some() {
                        workspace.focus_surface(surface_index, window, cx);
                    }
                    workspace.add_browser(url, placement, window, cx)
                });
                surface_operation_response(
                    "created",
                    workspace_index,
                    workspace.read(cx),
                    Some(index),
                    cx,
                )
            })
        }
        "browser.navigate" => {
            let params = match parse_params::<BrowserNavigateParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };
            with_window_state(app_state, cx, |window, state, cx| {
                let browser = browser_for_target(state, &params.target, cx)?;
                browser
                    .update(cx, |browser, cx| browser.navigate(&params.url, window, cx))
                    .map_err(|error| error.to_string())?;
                Ok(browser.read(cx).summary())
            })
        }
        "browser.state" | "browser.back" | "browser.forward" | "browser.reload"
        | "browser.stop" => {
            let target = match parse_params::<SurfaceTargetParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };
            with_state(app_state, cx, |state, cx| {
                let browser = browser_for_target(state, &target, cx)?;
                if method != "browser.state" {
                    browser
                        .read(cx)
                        .action(method.strip_prefix("browser.").unwrap_or_default())
                        .map_err(|error| error.to_string())?;
                }
                Ok(browser.read(cx).summary())
            })
        }
        "surface.set_status" => {
            let params = match parse_params::<SurfaceStatusParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };
            if params.status == rumux_core::status::SurfaceStatus::Exited {
                return RpcResponse::error(
                    id,
                    "Exited status is set by the terminal process".into(),
                );
            }
            if params
                .message
                .as_ref()
                .is_some_and(|message| message.len() > 4096)
            {
                return RpcResponse::error(id, "Status message exceeds 4096 bytes".into());
            }
            with_state(app_state, cx, |state, cx| {
                let (workspace_index, surface_index) =
                    resolve_surface_params(state, &params.target, cx)?;
                let workspace = state.workspaces[workspace_index].clone();
                workspace.update(cx, |workspace, cx| {
                    workspace.set_surface_status(surface_index, params.status, params.message, cx);
                });
                surface_operation_response(
                    "surface",
                    workspace_index,
                    &workspace.read(cx),
                    surface_index,
                    cx,
                )
            })
        }
        "system.ping" => Ok(serde_json::json!({ "pong": true })),
        "system.capabilities" => Ok(serde_json::json!({
            "stable_ids": true,
            "browser": {"engine": "system-webview", "linux_display": "X11 or XWayland", "evaluation": "synchronous JavaScript"},
            "methods": supported_methods(),
            "access_mode": "rumuxOnly",
            "transport": format!("{:?}", ipc_endpoint()),
        })),
        "system.identify" => Ok(serde_json::json!({
            "app": "rumux",
            "version": env!("CARGO_PKG_VERSION"),
        })),
        "system.activate" => with_window_state(app_state, cx, |window, state, cx| {
            cx.activate(true);
            state.focus_active_workspace(window, cx);
            Ok(serde_json::json!({ "activated": true }))
        }),

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
                if let Some(connection) = params.ssh {
                    connection.validate().map_err(|error| error.to_string())?;
                    state.add_ssh_workspace(params.name.clone(), connection, window, cx);
                } else {
                    state.add_workspace_named(params.name.clone(), window, cx);
                }
                to_value(
                    state.workspaces[state.active_workspace_idx]
                        .read(cx)
                        .summary(state.active_workspace_idx, true, &**cx),
                )
            })
        }
        "workspace.rename" => {
            let params = match parse_params::<WorkspaceRenameParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            let name = params.new_name.trim().to_string();
            if name.is_empty() {
                return RpcResponse::error(id, "Workspace name cannot be empty".to_string());
            }

            with_state(app_state, cx, move |state, cx| {
                let workspace_index = resolve_workspace_index(
                    state,
                    params.target.index,
                    params.target.name.as_deref(),
                    &**cx,
                )?;
                state.rename_workspace(workspace_index, name.clone(), cx);
                to_value(state.workspaces[workspace_index].read(cx).summary(
                    workspace_index,
                    workspace_index == state.active_workspace_idx,
                    &**cx,
                ))
            })
        }
        "workspace.duplicate" => {
            let params = match parse_params::<WorkspaceTargetParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            with_window_state(app_state, cx, move |window, state, cx| {
                let workspace_index =
                    resolve_workspace_index(state, params.index, params.name.as_deref(), &**cx)?;
                let duplicated_index = state
                    .duplicate_workspace_at(workspace_index, window, cx)
                    .ok_or_else(|| {
                        format!("Workspace {workspace_index} could not be duplicated")
                    })?;
                to_value(state.workspaces[duplicated_index].read(cx).summary(
                    duplicated_index,
                    true,
                    &**cx,
                ))
            })
        }
        "workspace.next" => with_window_state(app_state, cx, move |window, state, cx| {
            let next = (state.active_workspace_idx + 1) % state.workspaces.len();
            state.set_active_workspace(next, window, cx);
            to_value(state.workspaces[next].read(cx).summary(next, true, &**cx))
        }),
        "workspace.prev" => with_window_state(app_state, cx, move |window, state, cx| {
            let prev = if state.active_workspace_idx == 0 {
                state.workspaces.len() - 1
            } else {
                state.active_workspace_idx - 1
            };
            state.set_active_workspace(prev, window, cx);
            to_value(state.workspaces[prev].read(cx).summary(prev, true, &**cx))
        }),
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
        "surface.create" => {
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
                let created_surface_index = workspace
                    .update(cx, |workspace, cx| {
                        workspace.add_terminal_to_surface(params.surface_index, window, cx)
                    })
                    .ok_or_else(|| surface_not_found_message(params.surface_index))?;

                let workspace = workspace.read(cx);
                let mut result = surface_operation_response(
                    "created",
                    workspace_index,
                    &workspace,
                    Some(created_surface_index),
                    &**cx,
                )?;
                if result.get("created").is_none() {
                    result["created"] =
                        to_value(surface_placeholder(created_surface_index, &workspace))?;
                }
                Ok(result)
            })
        }
        "surface.split" => {
            let params = match parse_params::<SurfaceSplitParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            let placement = match parse_surface_split_placement(&params.placement) {
                Ok(placement) => placement,
                Err(error) => return RpcResponse::error(id, error),
            };

            with_window_state(app_state, cx, move |window, state, cx| {
                let workspace_index = resolve_workspace_index(
                    state,
                    params.target.workspace_index,
                    params.target.workspace_name.as_deref(),
                    &**cx,
                )?;

                if workspace_index != state.active_workspace_idx {
                    state.set_active_workspace(workspace_index, window, cx);
                }

                let workspace = state.workspaces[workspace_index].clone();
                let created_surface_index = workspace
                    .update(cx, |workspace, cx| {
                        workspace.split_surface(params.target.surface_index, placement, window, cx)
                    })
                    .ok_or_else(|| surface_not_found_message(params.target.surface_index))?;

                let workspace = workspace.read(cx);
                let mut result = surface_operation_response(
                    "created",
                    workspace_index,
                    &workspace,
                    Some(created_surface_index),
                    &**cx,
                )?;
                if result.get("created").is_none() {
                    result["created"] =
                        to_value(surface_placeholder(created_surface_index, &workspace))?;
                }
                Ok(result)
            })
        }
        "surface.close" => {
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
                let closed = workspace.update(cx, |workspace, cx| {
                    workspace.close_surface(params.surface_index, window, cx)
                });

                if !closed {
                    return Err(surface_not_found_message(params.surface_index));
                }

                let workspace = workspace.read(cx);
                let mut result =
                    surface_operation_response("active", workspace_index, &workspace, None, &**cx)?;
                result["closed_surface_index"] = serde_json::json!(params.surface_index);
                Ok(result)
            })
        }
        "surface.rename" => {
            let params = match parse_params::<SurfaceRenameParams>(params) {
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
                let renamed = workspace.update(cx, |workspace, cx| {
                    workspace.rename_surface(params.target.surface_index, params.name.clone(), cx)
                });

                if !renamed {
                    return Err(surface_not_found_message(params.target.surface_index));
                }

                state.save_session(cx);
                let workspace = workspace.read(cx);
                surface_operation_response(
                    "surface",
                    workspace_index,
                    &workspace,
                    params.target.surface_index,
                    &**cx,
                )
            })
        }
        "surface.next" => {
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
                let moved = workspace.update(cx, |workspace, cx| {
                    workspace.next_surface(params.surface_index, window, cx)
                });

                if !moved {
                    return Err(surface_not_found_message(params.surface_index));
                }

                let workspace = workspace.read(cx);
                surface_operation_response("active", workspace_index, &workspace, None, &**cx)
            })
        }
        "surface.prev" => {
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
                let moved = workspace.update(cx, |workspace, cx| {
                    workspace.prev_surface(params.surface_index, window, cx)
                });

                if !moved {
                    return Err(surface_not_found_message(params.surface_index));
                }

                let workspace = workspace.read(cx);
                surface_operation_response("active", workspace_index, &workspace, None, &**cx)
            })
        }
        "surface.zoom" => {
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
                let zoomed = workspace
                    .update(cx, |workspace, cx| {
                        workspace.toggle_zoom_surface(params.surface_index, window, cx)
                    })
                    .ok_or_else(|| surface_not_found_message(params.surface_index))?;

                let workspace = workspace.read(cx);
                let mut result =
                    surface_operation_response("active", workspace_index, &workspace, None, &**cx)?;
                result["zoomed"] = serde_json::json!(zoomed);
                Ok(result)
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
                    // Match the Enter key; LF does not submit a cmd.exe input line.
                    text.push('\r');
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
        "surface.read" => {
            let params = match parse_params::<SurfaceReadParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            let scope = match parse_surface_read_scope(params.scope.as_deref()) {
                Ok(scope) => scope,
                Err(error) => return RpcResponse::error(id, error),
            };

            with_state(app_state, cx, move |state, cx| {
                let workspace_index = resolve_workspace_index(
                    state,
                    params.target.workspace_index,
                    params.target.workspace_name.as_deref(),
                    &**cx,
                )?;
                let workspace = state.workspaces[workspace_index].read(cx);
                let text = workspace
                    .read_surface_text(params.target.surface_index, scope, &**cx)
                    .ok_or_else(|| surface_not_found_message(params.target.surface_index))?;
                let (text, truncated) = trim_text_tail(text, params.max_chars);
                Ok(serde_json::json!({
                    "workspace_index": workspace_index,
                    "workspace_name": workspace.name.clone(),
                    "surface_index": resolved_surface_index(&workspace, params.target.surface_index, &**cx),
                    "scope": surface_read_scope_name(scope),
                    "text": text,
                    "truncated": truncated,
                }))
            })
        }
        "surface.wait_for_output" => {
            let params = match parse_params::<SurfaceWaitForOutputParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            if params.pattern.is_empty() {
                return RpcResponse::error(id, "Pattern cannot be empty".to_string());
            }

            let scope = match parse_surface_read_scope(params.scope.as_deref()) {
                Ok(scope) => scope,
                Err(error) => return RpcResponse::error(id, error),
            };

            let timeout_ms = params.timeout_ms.unwrap_or(5_000).clamp(100, 120_000);
            let started = std::time::Instant::now();

            let initial_text =
                match read_surface_text_snapshot(app_state, cx, &params.target, scope) {
                    Ok(snapshot) => snapshot,
                    Err(error) => return RpcResponse::error(id, error),
                };

            loop {
                let snapshot =
                    match read_surface_text_snapshot(app_state, cx, &params.target, scope) {
                        Ok(snapshot) => snapshot,
                        Err(error) => return RpcResponse::error(id, error),
                    };

                let observed = text_since_snapshot(&initial_text.text, &snapshot.text);

                if observed.contains(&params.pattern) {
                    let (text, truncated) = trim_text_tail(observed, params.max_chars);
                    let elapsed_ms = started.elapsed().as_millis() as u64;
                    return RpcResponse::success(
                        id,
                        serde_json::json!({
                            "workspace_index": snapshot.workspace_index,
                            "workspace_name": snapshot.workspace_name,
                            "surface_index": snapshot.surface_index,
                            "scope": surface_read_scope_name(scope),
                            "pattern": params.pattern,
                            "matched": true,
                            "timed_out": false,
                            "elapsed_ms": elapsed_ms,
                            "text": text,
                            "truncated": truncated,
                        }),
                    );
                }

                if started.elapsed().as_millis() as u64 >= timeout_ms {
                    let (text, truncated) = trim_text_tail(observed, params.max_chars);
                    let elapsed_ms = started.elapsed().as_millis() as u64;
                    return RpcResponse::success(
                        id,
                        serde_json::json!({
                            "workspace_index": snapshot.workspace_index,
                            "workspace_name": snapshot.workspace_name,
                            "surface_index": snapshot.surface_index,
                            "scope": surface_read_scope_name(scope),
                            "pattern": params.pattern,
                            "matched": false,
                            "timed_out": true,
                            "elapsed_ms": elapsed_ms,
                            "text": text,
                            "truncated": truncated,
                        }),
                    );
                }

                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        "surface.run_command" => {
            let params = match parse_params::<SurfaceRunCommandParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            if params.command.trim().is_empty() {
                return RpcResponse::error(id, "Command cannot be empty".to_string());
            }

            let timeout_ms = params.timeout_ms.unwrap_or(30_000).clamp(100, 300_000);
            let token = Uuid::new_v4().simple().to_string();
            let start_marker = format!("__RUMUX_START_{token}__");
            let exit_marker_prefix = format!("__RUMUX_EXIT_{token}__:");
            let wrapped =
                wrap_command_for_agent(&params.command, &start_marker, &exit_marker_prefix);

            let initial_snapshot = match read_surface_text_snapshot(
                app_state,
                cx,
                &params.target,
                SurfaceReadScope::Buffer,
            ) {
                Ok(snapshot) => snapshot,
                Err(error) => return RpcResponse::error(id, error),
            };

            let sent = with_state(app_state, cx, |state, cx| {
                let workspace_index = resolve_workspace_index(
                    state,
                    params.target.workspace_index,
                    params.target.workspace_name.as_deref(),
                    &**cx,
                )?;
                let workspace = state.workspaces[workspace_index].clone();
                Ok(workspace.update(cx, |workspace, cx| {
                    workspace.send_text_to_surface(params.target.surface_index, &wrapped, cx)
                }))
            });

            let sent = match sent {
                Ok(sent) => sent,
                Err(error) => return RpcResponse::error(id, error),
            };

            if !sent {
                return RpcResponse::error(
                    id,
                    surface_not_found_message(params.target.surface_index),
                );
            }

            let started = std::time::Instant::now();

            loop {
                let snapshot = match read_surface_text_snapshot(
                    app_state,
                    cx,
                    &params.target,
                    SurfaceReadScope::Buffer,
                ) {
                    Ok(snapshot) => snapshot,
                    Err(error) => return RpcResponse::error(id, error),
                };

                let search_text = text_since_snapshot(&initial_snapshot.text, &snapshot.text);

                if let Some((output, exit_code)) =
                    parse_agent_command_output(&search_text, &start_marker, &exit_marker_prefix)
                {
                    let (text, truncated) = trim_text_tail(output, params.max_chars);
                    return RpcResponse::success(
                        id,
                        serde_json::json!({
                            "workspace_index": snapshot.workspace_index,
                            "workspace_name": snapshot.workspace_name,
                            "surface_index": snapshot.surface_index,
                            "command": params.command,
                            "completed": true,
                            "timed_out": false,
                            "elapsed_ms": started.elapsed().as_millis() as u64,
                            "exit_code": exit_code,
                            "output": text,
                            "truncated": truncated,
                        }),
                    );
                }

                if started.elapsed().as_millis() as u64 >= timeout_ms {
                    let observed = after_agent_start_marker(&search_text, &start_marker)
                        .unwrap_or(search_text);
                    let (text, truncated) = trim_text_tail(observed, params.max_chars);
                    return RpcResponse::success(
                        id,
                        serde_json::json!({
                            "workspace_index": snapshot.workspace_index,
                            "workspace_name": snapshot.workspace_name,
                            "surface_index": snapshot.surface_index,
                            "command": params.command,
                            "completed": false,
                            "timed_out": true,
                            "elapsed_ms": started.elapsed().as_millis() as u64,
                            "exit_code": serde_json::Value::Null,
                            "output": text,
                            "truncated": truncated,
                        }),
                    );
                }

                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
        "surface.interrupt" => {
            let params = match parse_params::<SurfaceTargetParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };

            let keystroke = match Keystroke::parse("ctrl-c") {
                Ok(keystroke) => keystroke,
                Err(error) => {
                    return RpcResponse::error(id, format!("Failed to parse ctrl-c: {error}"));
                }
            };

            with_state(app_state, cx, move |state, cx| {
                let workspace_index = resolve_workspace_index(
                    state,
                    params.workspace_index,
                    params.workspace_name.as_deref(),
                    &**cx,
                )?;

                let workspace = state.workspaces[workspace_index].clone();
                let sent = workspace.update(cx, |workspace, cx| {
                    workspace.send_keystroke_to_surface(params.surface_index, &keystroke, cx)
                });

                if sent {
                    Ok(serde_json::json!({
                        "interrupted": true,
                        "workspace_index": workspace_index,
                        "surface_index": params.surface_index,
                    }))
                } else {
                    Err(surface_not_found_message(params.surface_index))
                }
            })
        }
        "surface.select_all" => {
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

                let workspace = state.workspaces[workspace_index].clone();
                let selected = workspace.update(cx, |workspace, cx| {
                    workspace.select_all_in_surface(params.surface_index, cx)
                });

                if selected {
                    Ok(serde_json::json!({
                        "selected": true,
                        "workspace_index": workspace_index,
                        "surface_index": params.surface_index,
                    }))
                } else {
                    Err(surface_not_found_message(params.surface_index))
                }
            })
        }
        "surface.copy_selection" => {
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

                let workspace = state.workspaces[workspace_index].clone();
                let copied = workspace.update(cx, |workspace, cx| {
                    workspace.copy_selection_from_surface(params.surface_index, cx)
                });

                if copied {
                    Ok(serde_json::json!({
                        "copied": true,
                        "workspace_index": workspace_index,
                        "surface_index": params.surface_index,
                    }))
                } else {
                    Err(surface_not_found_message(params.surface_index))
                }
            })
        }
        "surface.copy_all" => {
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

                let workspace = state.workspaces[workspace_index].clone();
                let copied = workspace.update(cx, |workspace, cx| {
                    workspace.copy_all_from_surface(params.surface_index, cx)
                });

                if copied {
                    Ok(serde_json::json!({
                        "copied": true,
                        "workspace_index": workspace_index,
                        "surface_index": params.surface_index,
                    }))
                } else {
                    Err(surface_not_found_message(params.surface_index))
                }
            })
        }
        "surface.paste" => {
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

                let workspace = state.workspaces[workspace_index].clone();
                let pasted = workspace.update(cx, |workspace, cx| {
                    workspace.paste_into_surface(params.surface_index, cx)
                });

                if pasted {
                    Ok(serde_json::json!({
                        "pasted": true,
                        "workspace_index": workspace_index,
                        "surface_index": params.surface_index,
                    }))
                } else {
                    Err(surface_not_found_message(params.surface_index))
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
                if let Some(surface_index) = params.surface_index {
                    if !state.workspaces[workspace_index]
                        .read(cx)
                        .list_surfaces(cx)
                        .iter()
                        .any(|surface| surface.index == surface_index)
                    {
                        return Err(surface_not_found_message(Some(surface_index)));
                    }
                }
                let notification = state.create_notification_at(
                    params.title.clone(),
                    params.subtitle.clone(),
                    params.body.clone(),
                    workspace_index,
                    params.surface_index,
                    cx,
                );
                to_value(notification)
            })
        }
        "notification.list" => with_state(app_state, cx, |state, _cx| {
            to_value(state.notifications.clone())
        }),
        "notification.focus" => {
            let params = match parse_params::<NotificationTargetParams>(params) {
                Ok(params) => params,
                Err(error) => return RpcResponse::error(id, error),
            };
            with_window_state(app_state, cx, move |window, state, cx| {
                cx.activate(true);
                if state.open_notification(&params.id, window, cx) {
                    Ok(serde_json::json!({ "focused": true }))
                } else {
                    Err("Notification was not found".into())
                }
            })
        }
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
        "system.activate",
        "workspace.list",
        "workspace.create",
        "workspace.rename",
        "workspace.duplicate",
        "workspace.next",
        "workspace.prev",
        "workspace.select",
        "workspace.current",
        "workspace.close",
        "surface.list",
        "surface.focus",
        "surface.create",
        "surface.split",
        "surface.close",
        "surface.rename",
        "surface.set_status",
        "surface.next",
        "surface.prev",
        "surface.zoom",
        "surface.send_text",
        "surface.send_key",
        "surface.read",
        "surface.wait_for_output",
        "surface.run_command",
        "surface.interrupt",
        "surface.select_all",
        "surface.copy_selection",
        "surface.copy_all",
        "surface.paste",
        "browser.create",
        "browser.navigate",
        "browser.state",
        "browser.back",
        "browser.forward",
        "browser.reload",
        "browser.stop",
        "browser.eval",
        "browser.snapshot",
        "browser.click",
        "browser.fill",
        "notification.create",
        "notification.list",
        "notification.focus",
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
    let window = match app_state.update(cx, |state, cx| {
        state.primary_window().or_else(|| cx.active_window())
    }) {
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

struct SurfaceTextSnapshot {
    workspace_index: usize,
    workspace_name: String,
    surface_index: Option<usize>,
    remote_posix: bool,
    text: String,
}

fn parse_surface_split_placement(raw: &str) -> Result<Placement, String> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "right" => Ok(Placement::Right),
        "down" | "bottom" => Ok(Placement::Bottom),
        other => Err(format!(
            "Invalid split placement '{other}'. Expected 'right' or 'down'"
        )),
    }
}

fn surface_operation_response(
    surface_key: &str,
    workspace_index: usize,
    workspace: &crate::workspace::Workspace,
    surface_index: Option<usize>,
    cx: &gpui::App,
) -> Result<serde_json::Value, String> {
    let surfaces = workspace.list_surfaces(cx);
    let surface = match surface_index {
        Some(surface_index) => surfaces
            .iter()
            .find(|surface| surface.index == surface_index)
            .cloned(),
        None => surfaces
            .iter()
            .find(|surface| surface.target)
            .cloned()
            .or_else(|| surfaces.iter().find(|surface| surface.active_tab).cloned()),
    };

    let mut result = serde_json::Map::new();
    result.insert(
        "workspace_index".to_string(),
        serde_json::json!(workspace_index),
    );
    result.insert(
        "workspace_name".to_string(),
        serde_json::json!(workspace.name.clone()),
    );
    result.insert("surfaces".to_string(), to_value(surfaces)?);
    if let Some(surface) = surface {
        result.insert(surface_key.to_string(), to_value(surface)?);
    }

    Ok(serde_json::Value::Object(result))
}

fn surface_placeholder(
    index: usize,
    workspace: &crate::workspace::Workspace,
) -> crate::workspace::SurfaceSummary {
    crate::workspace::SurfaceSummary {
        kind: "terminal",
        ssh: workspace
            .ssh
            .as_ref()
            .map(|connection| connection.for_new_pane(None)),
        url: None,
        id: String::new(),
        status: rumux_core::status::SurfaceStatus::Idle,
        status_message: None,
        exited: false,
        index,
        title: format!("Terminal {}", index + 1),
        cwd: workspace.cwd.clone(),
        active_tab: true,
        target: true,
    }
}

fn parse_surface_read_scope(raw: Option<&str>) -> Result<SurfaceReadScope, String> {
    match raw.unwrap_or("buffer").trim().to_ascii_lowercase().as_str() {
        "buffer" | "all" | "scrollback" => Ok(SurfaceReadScope::Buffer),
        "visible" | "screen" => Ok(SurfaceReadScope::Visible),
        other => Err(format!(
            "Invalid read scope '{other}'. Expected 'buffer' or 'visible'"
        )),
    }
}

fn surface_read_scope_name(scope: SurfaceReadScope) -> &'static str {
    match scope {
        SurfaceReadScope::Buffer => "buffer",
        SurfaceReadScope::Visible => "visible",
    }
}

fn trim_text_tail(text: String, max_chars: Option<usize>) -> (String, bool) {
    let Some(max_chars) = max_chars else {
        return (text, false);
    };
    if text.chars().count() <= max_chars {
        return (text, false);
    }

    let kept: String = text
        .chars()
        .rev()
        .take(max_chars)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    (kept, true)
}

fn text_since_snapshot(initial_text: &str, current_text: &str) -> String {
    if let Some(stripped) = current_text.strip_prefix(initial_text) {
        return stripped.to_string();
    }

    let initial_bytes = initial_text.as_bytes();
    let current_bytes = current_text.as_bytes();
    let max_overlap = initial_bytes.len().min(current_bytes.len());
    let mut overlap = 0;
    let mut boundaries = current_text
        .char_indices()
        .map(|(idx, _)| idx)
        .collect::<Vec<_>>();
    boundaries.push(current_text.len());

    for boundary in boundaries.into_iter().rev() {
        if boundary > max_overlap {
            continue;
        }
        if initial_bytes[initial_bytes.len() - boundary..] == current_bytes[..boundary] {
            overlap = boundary;
            break;
        }
    }

    current_text[overlap..].to_string()
}

fn find_matching_line<F>(text: &str, mut predicate: F) -> Option<(usize, usize)>
where
    F: FnMut(&str) -> bool,
{
    let mut offset = 0;
    for segment in text.split_inclusive('\n') {
        let end = offset + segment.len();
        let line = segment.trim_end_matches(['\r', '\n']);
        if predicate(line) {
            return Some((offset, end));
        }
        offset = end;
    }

    if offset < text.len() {
        let line = text[offset..].trim_end_matches('\r');
        if predicate(line) {
            return Some((offset, text.len()));
        }
    }

    None
}

fn after_agent_start_marker(search_text: &str, start_marker: &str) -> Option<String> {
    let (_, marker_end) = find_matching_line(search_text, |line| line == start_marker)?;
    Some(
        search_text[marker_end..]
            .trim_start_matches(['\r', '\n'])
            .to_string(),
    )
}

fn parse_agent_command_output(
    search_text: &str,
    start_marker: &str,
    exit_marker_prefix: &str,
) -> Option<(String, Option<i32>)> {
    let (_, marker_end) = find_matching_line(search_text, |line| line == start_marker)?;
    let after_start = &search_text[marker_end..];
    // A command such as `printf hello` leaves the exit marker on the same
    // line as its last output. Accept only a complete numeric marker suffix;
    // echoed wrapper source contains a quoted marker and is not completion.
    let mut offset = 0;
    for segment in after_start.split_inclusive('\n') {
        let line = segment.trim_end_matches(['\r', '\n']);
        if let Some(marker) = line.rfind(exit_marker_prefix) {
            if let Ok(code) = line[marker + exit_marker_prefix.len()..]
                .trim()
                .parse::<i32>()
            {
                let output = after_start[..offset + marker]
                    .trim_start_matches(['\r', '\n'])
                    .to_string();
                return Some((output, Some(code)));
            }
        }
        offset += segment.len();
    }
    None
}

fn read_surface_text_snapshot(
    app_state: &WeakEntity<AppState>,
    cx: &mut AsyncApp,
    target: &SurfaceTargetParams,
    scope: SurfaceReadScope,
) -> Result<SurfaceTextSnapshot, String> {
    with_state(app_state, cx, |state, cx| {
        let (workspace_index, surface_index) = resolve_surface_params(state, target, cx)?;
        let workspace = state.workspaces[workspace_index].read(cx);
        let text = workspace
            .read_surface_text(surface_index, scope, &**cx)
            .ok_or_else(|| surface_not_found_message(surface_index))?;
        Ok(SurfaceTextSnapshot {
            workspace_index,
            workspace_name: workspace.name.clone(),
            surface_index: resolved_surface_index(&workspace, surface_index, &**cx),
            remote_posix: workspace.ssh.is_some(),
            text,
        })
    })
}

async fn handle_surface_wait_for_output(
    req: RpcRequest,
    app_state: &WeakEntity<AppState>,
    cx: &mut AsyncApp,
) -> RpcResponse {
    let RpcRequest { id, params, .. } = req;

    let params = match parse_params::<SurfaceWaitForOutputParams>(params) {
        Ok(params) => params,
        Err(error) => return RpcResponse::error(id, error),
    };

    if params.pattern.is_empty() {
        return RpcResponse::error(id, "Pattern cannot be empty".to_string());
    }

    let scope = match parse_surface_read_scope(params.scope.as_deref()) {
        Ok(scope) => scope,
        Err(error) => return RpcResponse::error(id, error),
    };

    let timeout_ms = params.timeout_ms.unwrap_or(5_000).clamp(100, 120_000);
    let started = std::time::Instant::now();

    let initial_text = match read_surface_text_snapshot(app_state, cx, &params.target, scope) {
        Ok(snapshot) => snapshot,
        Err(error) => return RpcResponse::error(id, error),
    };

    loop {
        let snapshot = match read_surface_text_snapshot(app_state, cx, &params.target, scope) {
            Ok(snapshot) => snapshot,
            Err(error) => return RpcResponse::error(id, error),
        };

        let observed = text_since_snapshot(&initial_text.text, &snapshot.text);

        if observed.contains(&params.pattern) {
            let (text, truncated) = trim_text_tail(observed, params.max_chars);
            let elapsed_ms = started.elapsed().as_millis() as u64;
            return RpcResponse::success(
                id,
                serde_json::json!({
                    "workspace_index": snapshot.workspace_index,
                    "workspace_name": snapshot.workspace_name,
                    "surface_index": snapshot.surface_index,
                    "scope": surface_read_scope_name(scope),
                    "pattern": params.pattern,
                    "matched": true,
                    "timed_out": false,
                    "elapsed_ms": elapsed_ms,
                    "text": text,
                    "truncated": truncated,
                }),
            );
        }

        if started.elapsed().as_millis() as u64 >= timeout_ms {
            let (text, truncated) = trim_text_tail(observed, params.max_chars);
            let elapsed_ms = started.elapsed().as_millis() as u64;
            return RpcResponse::success(
                id,
                serde_json::json!({
                    "workspace_index": snapshot.workspace_index,
                    "workspace_name": snapshot.workspace_name,
                    "surface_index": snapshot.surface_index,
                    "scope": surface_read_scope_name(scope),
                    "pattern": params.pattern,
                    "matched": false,
                    "timed_out": true,
                    "elapsed_ms": elapsed_ms,
                    "text": text,
                    "truncated": truncated,
                }),
            );
        }

        cx.background_executor()
            .timer(std::time::Duration::from_millis(100))
            .await;
    }
}

async fn handle_surface_run_command(
    req: RpcRequest,
    app_state: &WeakEntity<AppState>,
    cx: &mut AsyncApp,
) -> RpcResponse {
    let RpcRequest { id, params, .. } = req;

    let params = match parse_params::<SurfaceRunCommandParams>(params) {
        Ok(params) => params,
        Err(error) => return RpcResponse::error(id, error),
    };

    if params.command.trim().is_empty() {
        return RpcResponse::error(id, "Command cannot be empty".to_string());
    }

    let timeout_ms = params.timeout_ms.unwrap_or(30_000).clamp(100, 300_000);
    let token = Uuid::new_v4().simple().to_string();
    let start_marker = format!("__RUMUX_START_{token}__");
    let exit_marker_prefix = format!("__RUMUX_EXIT_{token}__:");

    let initial_snapshot =
        match read_surface_text_snapshot(app_state, cx, &params.target, SurfaceReadScope::Buffer) {
            Ok(snapshot) => snapshot,
            Err(error) => return RpcResponse::error(id, error),
        };

    let wrapped = if initial_snapshot.remote_posix {
        wrap_posix_command(
            &params.command,
            "/bin/sh",
            &start_marker,
            &exit_marker_prefix,
        )
    } else {
        wrap_command_for_agent(&params.command, &start_marker, &exit_marker_prefix)
    };

    let sent = with_state(app_state, cx, |state, cx| {
        let workspace_index = resolve_workspace_index(
            state,
            params.target.workspace_index,
            params.target.workspace_name.as_deref(),
            &**cx,
        )?;
        let workspace = state.workspaces[workspace_index].clone();
        Ok(workspace.update(cx, |workspace, cx| {
            workspace.send_text_to_surface(params.target.surface_index, &wrapped, cx)
        }))
    });

    let sent = match sent {
        Ok(sent) => sent,
        Err(error) => return RpcResponse::error(id, error),
    };

    if !sent {
        return RpcResponse::error(id, surface_not_found_message(params.target.surface_index));
    }

    let started = std::time::Instant::now();

    loop {
        let snapshot = match read_surface_text_snapshot(
            app_state,
            cx,
            &params.target,
            SurfaceReadScope::Buffer,
        ) {
            Ok(snapshot) => snapshot,
            Err(error) => return RpcResponse::error(id, error),
        };

        let search_text = text_since_snapshot(&initial_snapshot.text, &snapshot.text);

        if let Some((output, exit_code)) =
            parse_agent_command_output(&search_text, &start_marker, &exit_marker_prefix)
        {
            let (text, truncated) = trim_text_tail(output, params.max_chars);
            return RpcResponse::success(
                id,
                serde_json::json!({
                    "workspace_index": snapshot.workspace_index,
                    "workspace_name": snapshot.workspace_name,
                    "surface_index": snapshot.surface_index,
                    "command": params.command,
                    "completed": true,
                    "timed_out": false,
                    "elapsed_ms": started.elapsed().as_millis() as u64,
                    "exit_code": exit_code,
                    "output": text,
                    "truncated": truncated,
                }),
            );
        }

        if started.elapsed().as_millis() as u64 >= timeout_ms {
            let observed =
                after_agent_start_marker(&search_text, &start_marker).unwrap_or(search_text);
            let (text, truncated) = trim_text_tail(observed, params.max_chars);
            return RpcResponse::success(
                id,
                serde_json::json!({
                    "workspace_index": snapshot.workspace_index,
                    "workspace_name": snapshot.workspace_name,
                    "surface_index": snapshot.surface_index,
                    "command": params.command,
                    "completed": false,
                    "timed_out": true,
                    "elapsed_ms": started.elapsed().as_millis() as u64,
                    "exit_code": serde_json::Value::Null,
                    "output": text,
                    "truncated": truncated,
                }),
            );
        }

        cx.background_executor()
            .timer(std::time::Duration::from_millis(100))
            .await;
    }
}

fn resolved_surface_index(
    workspace: &Workspace,
    requested_surface_index: Option<usize>,
    cx: &gpui::App,
) -> Option<usize> {
    match requested_surface_index {
        Some(index) => Some(index),
        None => workspace
            .list_surfaces(cx)
            .into_iter()
            .find(|surface| surface.target)
            .or_else(|| {
                workspace
                    .list_surfaces(cx)
                    .into_iter()
                    .find(|surface| surface.active_tab)
            })
            .map(|surface| surface.index),
    }
}

fn wrap_posix_command(
    command: &str,
    shell: &str,
    start_marker: &str,
    exit_marker_prefix: &str,
) -> String {
    let shell = shell_single_quote(shell);
    let command = shell_single_quote(command);
    let start_marker = shell_single_quote(start_marker);
    let exit_marker_prefix = shell_single_quote(exit_marker_prefix);
    format!(
        "__rumux_restore_tty=$(stty -g 2>/dev/null || true); __rumux_restore() {{ if [ -n \"$__rumux_restore_tty\" ]; then stty \"$__rumux_restore_tty\" 2>/dev/null || stty echo 2>/dev/null || true; else stty echo 2>/dev/null || true; fi; }}; trap '__rumux_restore' EXIT INT TERM; stty -echo 2>/dev/null || true; printf '%s\\n' {start_marker}; {shell} -lc {command}; rumux_status=$?; printf '%s%s\\n' {exit_marker_prefix} \"$rumux_status\"; __rumux_restore; trap - EXIT INT TERM\n"
    )
}

fn wrap_command_for_agent(command: &str, start_marker: &str, exit_marker_prefix: &str) -> String {
    #[cfg(windows)]
    {
        let shell = default_shell().to_ascii_lowercase();
        if shell.contains("pwsh") || shell.contains("powershell") {
            format!(
                "Write-Output '{start_marker}'\n{command}\n$rumuxStatus = if ($LASTEXITCODE -ne $null) {{ $LASTEXITCODE }} elseif ($?) {{ 0 }} else {{ 1 }}\nWrite-Output '{exit_marker_prefix}'$rumuxStatus\n"
            )
        } else {
            format!(
                "echo {start_marker}\r\n{command}\r\nset RUMUX_EXIT=%ERRORLEVEL%\r\necho {exit_marker_prefix}%RUMUX_EXIT%\r\n"
            )
        }
    }

    #[cfg(not(windows))]
    {
        wrap_posix_command(command, &default_shell(), start_marker, exit_marker_prefix)
    }
}

fn shell_single_quote(raw: &str) -> String {
    format!("'{}'", raw.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::token_matches;

    #[test]
    fn command_completion_handles_output_without_a_trailing_newline() {
        let text = "echo '__start__'; printf '__exit__:' \"$status\"\n__start__\nhello__exit__:7\n";
        assert_eq!(
            super::parse_agent_command_output(text, "__start__", "__exit__:"),
            Some(("hello".to_owned(), Some(7)))
        );
        assert_eq!(
            super::parse_agent_command_output(
                "__start__\n__exit__:' \"$status\"\n",
                "__start__",
                "__exit__:"
            ),
            None
        );
    }

    #[test]
    fn tcp_authentication_requires_the_complete_token() {
        assert!(!token_matches("private-rpc-token", None));
        assert!(!token_matches("private-rpc-token", Some("")));
        assert!(!token_matches(
            "private-rpc-token",
            Some("private-rpc-tokeN")
        ));
        assert!(!token_matches(
            "private-rpc-token",
            Some("private-rpc-token-extra")
        ));
        assert!(token_matches(
            "private-rpc-token",
            Some("private-rpc-token")
        ));
    }

    #[test]
    fn tcp_token_is_rotated_and_private_on_unix() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("token");
        let first = super::create_auth_token_at(&path).unwrap();
        let second = super::create_auth_token_at(&path).unwrap();
        assert_ne!(first, second);
        assert_eq!(second.len(), 64);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), second);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}

fn browser_for_target(
    state: &AppState,
    target: &SurfaceTargetParams,
    cx: &gpui::App,
) -> Result<gpui::Entity<crate::browser::BrowserView>, String> {
    let (workspace_index, surface_index) = resolve_surface_params(state, target, cx)?;
    state.workspaces[workspace_index]
        .read(cx)
        .browser_for_surface(surface_index, cx)
        .ok_or_else(|| "The target surface is not a browser".into())
}

async fn handle_browser_evaluation(
    req: RpcRequest,
    state: &WeakEntity<AppState>,
    cx: &mut AsyncApp,
) -> RpcResponse {
    let RpcRequest {
        id, method, params, ..
    } = req;
    let params: BrowserEvaluateParams = match parse_params(params) {
        Ok(params) => params,
        Err(error) => return RpcResponse::error(id, error),
    };
    let expression = match method.as_str() {
        "browser.snapshot" => "JSON.stringify({url:location.href,title:document.title,text:(document.body?.innerText || '').slice(0,100000),elements:Array.from(document.querySelectorAll('a,button,input,textarea,select,[role]')).slice(0,1000).map(e=>({tag:e.tagName.toLowerCase(),id:e.id,role:e.getAttribute('role'),text:(e.innerText || e.getAttribute('aria-label') || '').slice(0,200),value:e.type === 'password' ? undefined : e.value,href:e.href}))})".to_string(),
        "browser.click" | "browser.fill" => {
            let Some(selector) = &params.selector else { return RpcResponse::error(id, "A CSS selector is required".into()); };
            let selector = match serde_json::to_string(selector) { Ok(selector) => selector, Err(error) => return RpcResponse::error(id, error.to_string()) };
            if method == "browser.click" {
                format!("(() => {{ const e = document.querySelector({selector}); if (!e) throw new Error('Element was not found'); e.click(); return true; }})()")
            } else {
                let text = match serde_json::to_string(&params.text.unwrap_or_default()) { Ok(text) => text, Err(error) => return RpcResponse::error(id, error.to_string()) };
                format!("(() => {{ const e = document.querySelector({selector}); if (!e) throw new Error('Element was not found'); const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(e),'value')?.set; if (setter) setter.call(e,{text}); else e.value={text}; e.dispatchEvent(new Event('input',{{bubbles:true}})); e.dispatchEvent(new Event('change',{{bubbles:true}})); return true; }})()")
            }
        }
        _ => params.expression,
    };
    let receiver = match with_state(state, cx, |state, cx| {
        browser_for_target(state, &params.target, cx)?
            .read(cx)
            .evaluate(&expression)
            .map_err(|error| error.to_string())
    }) {
        Ok(receiver) => receiver,
        Err(error) => return RpcResponse::error(id, error),
    };
    let timeout =
        std::time::Duration::from_millis(params.timeout_ms.unwrap_or(10_000).clamp(100, 120_000));
    let result = smol::future::or(
        async {
            receiver
                .recv_async()
                .await
                .map_err(|_| "Browser was closed".to_string())
        },
        async {
            smol::Timer::after(timeout).await;
            Err("Browser evaluation timed out".to_string())
        },
    )
    .await;
    let result = result.and_then(|value| {
        crate::browser::decode_evaluation(&value).map_err(|error| error.to_string())
    });
    match result {
        Ok(value) => {
            let value = if method == "browser.snapshot" {
                value
                    .as_str()
                    .and_then(|text| serde_json::from_str(text).ok())
                    .unwrap_or(value)
            } else {
                value
            };
            RpcResponse::success(id, serde_json::json!({"value": value}))
        }
        Err(error) => RpcResponse::error(id, error),
    }
}
