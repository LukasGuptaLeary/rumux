use crate::{config::RumuxConfig, terminal_service::ServiceClient, theme};
use anyhow::Result;
use gpui::*;
use gpui_terminal::mirror::{MirrorCursor, MirrorUpdate};
use gpui_terminal::{TerminalConfig, TerminalView};
use rumux_core::terminal_service::Client;
use std::io::Write;
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

pub fn spawn_terminal_view_for_surface(
    cx: &mut App,
    cwd: Option<&Path>,
    id: uuid::Uuid,
) -> Result<Entity<TerminalView>> {
    attach(cx, cwd, id, None)
}
pub fn spawn_ssh_terminal_view(
    cx: &mut App,
    connection: &rumux_core::ssh::SshConnection,
    id: uuid::Uuid,
) -> Result<Entity<TerminalView>> {
    attach(cx, None, id, Some(connection))
}

fn attach(
    cx: &mut App,
    cwd: Option<&Path>,
    id: uuid::Uuid,
    ssh: Option<&rumux_core::ssh::SshConnection>,
) -> Result<Entity<TerminalView>> {
    let client = cx.global::<ServiceClient>().0.clone();
    let settings = RumuxConfig::load();
    client.call("open", serde_json::json!({"id":id.to_string(),"cwd":cwd.map(|path|path.to_string_lossy()),"ssh":ssh,"history":settings.scrollback}))?;
    let config = TerminalConfig {
        font_family: settings.font_family,
        font_size: px(settings.font_size),
        cols: 80,
        rows: 24,
        scrollback: settings.scrollback,
        line_height_multiplier: 1.0,
        padding: Edges::all(px(4.0)),
        colors: theme::catppuccin_mocha(),
    };
    let alive = Arc::new(AtomicBool::new(true));
    let (updates_tx, updates_rx) = mpsc::sync_channel(2);
    let polling = client.clone();
    let polling_alive = alive.clone();
    std::thread::spawn(move || {
        let mut after: Option<MirrorCursor> = None;
        while polling_alive.load(Ordering::Relaxed) {
            let response: Result<Option<MirrorUpdate>> = polling.call_typed(
                "poll",
                serde_json::json!({"id":id.to_string(),"after":after}),
            );
            match response {
                Ok(Some(update)) => {
                    after = Some(update.packet.cursor.clone());
                    if updates_tx.send(update).is_err() {
                        break;
                    }
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("Terminal connection lost for {id}: {error:#}");
                    break;
                }
            }
        }
    });
    let pending_resize = Arc::new(parking_lot::Mutex::new(None));
    let writer = RemoteInput::new(client, id, pending_resize.clone(), alive.clone());
    Ok(cx.new(|cx| {
        TerminalView::new_mirror(writer, updates_rx, config, cx)
            .with_pixel_resize_callback(move |columns, rows, cell_width, cell_height| {
                *pending_resize.lock() = Some((columns, rows, cell_width, cell_height));
            })
            .with_close_callback(move || {
                if std::env::var_os("RUMUX_TERMINAL_TRACE").is_some() {
                    eprintln!("detach view {id}");
                }
                alive.store(false, Ordering::Relaxed);
            })
    }))
}

struct RemoteInput {
    sender: mpsc::SyncSender<Vec<u8>>,
}
type PendingResize = Arc<parking_lot::Mutex<Option<(usize, usize, u16, u16)>>>;
impl RemoteInput {
    fn new(client: Client, id: uuid::Uuid, resize: PendingResize, alive: Arc<AtomicBool>) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Vec<u8>>(64);
        std::thread::spawn(move || {
            while alive.load(Ordering::Relaxed) {
                let next_resize = { resize.lock().take() };
                if let Some((columns, rows, cell_width, cell_height)) = next_resize
                    && let Err(error) = client.call(
                        "resize",
                        serde_json::json!({"id":id.to_string(),"columns":columns,"rows":rows,"cell_width":cell_width,"cell_height":cell_height}),
                    )
                {
                    eprintln!("Terminal resize failed: {error:#}");
                }
                match receiver.recv_timeout(std::time::Duration::from_millis(20)) {
                    Ok(bytes) => {
                        if std::env::var_os("RUMUX_TERMINAL_TRACE").is_some() {
                            eprintln!("client input {id} bytes={}", bytes.len());
                        }
                        if let Err(error) = client.call(
                            "write",
                            serde_json::json!({"id":id.to_string(),"bytes":bytes}),
                        ) {
                            eprintln!("Terminal input failed: {error:#}");
                        }
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        });
        Self { sender }
    }
}
impl Write for RemoteInput {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.len() > 256 * 1024 {
            return Err(std::io::Error::other("Terminal input exceeds 256 KiB"));
        }
        self.sender
            .try_send(bytes.to_vec())
            .map_err(|error| std::io::Error::other(error.to_string()))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn close_owned(id: uuid::Uuid, cx: &App) {
    let client = cx.global::<ServiceClient>().0.clone();
    std::thread::spawn(move || {
        if let Err(error) = client.call("close", serde_json::json!({"id":id.to_string()})) {
            eprintln!("Unable to close terminal {id}: {error:#}");
        }
    });
}

pub fn terminal_error_view(cx: &mut App, error: &anyhow::Error) -> Entity<TerminalView> {
    let message = format!(
        "\x1b[31mUnable to start terminal: {error}\x1b[0m\r\nCheck your shell and working directory, then open a new terminal.\r\n"
    );
    cx.new(|cx| {
        TerminalView::new(
            std::io::sink(),
            std::io::Cursor::new(message.into_bytes()),
            TerminalConfig {
                colors: theme::catppuccin_mocha(),
                ..TerminalConfig::default()
            },
            cx,
        )
    })
}
