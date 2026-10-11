//! Headless PTY owner. This module never creates a GPUI application/window.
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use gpui_terminal::event::{GpuiEventProxy, TerminalEvent};
use gpui_terminal::mirror::{MirrorCursor, MirrorPacket, MirrorUpdate, bounded_history};
use gpui_terminal::terminal::TerminalState;
use parking_lot::{Condvar, Mutex};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use rumux_core::status::SurfaceStatus;
use rumux_core::terminal_protocol::{TerminalProtocolParser, TerminalSignal};
use rumux_core::terminal_service::{Client, Endpoint, PROTOCOL, Request, Response};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

pub struct ServiceClient(pub Client);
impl gpui::Global for ServiceClient {}

fn directory() -> PathBuf {
    rumux_core::runtime::rumux_config_dir().join("terminals")
}
fn lock_file() -> Result<File> {
    std::fs::create_dir_all(directory())?;
    protect_windows_path(&directory(), true)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(directory(), std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory().join("service.lock"))?)
}

pub fn ensure_started() -> Result<Client> {
    let lock = lock_file()?;
    if lock.try_lock_exclusive().is_err() {
        let client = Client::discover()?;
        client.call("ping", json!({}))?;
        return Ok(client);
    }
    FileExt::unlock(&lock)?;
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(directory().join("service.log"))?;
    private_file(&log)?;
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command
        .arg("--terminal-service")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(log);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00000200); // CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP
    }
    let mut child = command
        .spawn()
        .context("Unable to start terminal service")?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        if let Ok(client) = Client::discover()
            && client.call("ping", json!({})).is_ok()
        {
            return Ok(client);
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    anyhow::bail!(
        "Terminal service did not start; see {}",
        directory().join("service.log").display()
    )
}

fn private_file(file: &File) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(not(unix))]
    let _ = file;
    Ok(())
}

fn save_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("Missing terminal state directory")?;
    std::fs::create_dir_all(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    private_file(file.as_file())?;
    protect_windows_path(file.path(), false)?;
    serde_json::to_writer(&mut file, value)?;
    file.as_file().sync_all()?;
    file.persist(path)?;
    Ok(())
}

pub fn protect_windows_path(path: &Path, directory: bool) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use std::os::windows::process::CommandExt;
        use windows_sys::Win32::Foundation::LocalFree;
        use windows_sys::Win32::Security::Authorization::{
            ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1, SE_FILE_OBJECT,
            SetNamedSecurityInfoW,
        };
        use windows_sys::Win32::Security::{
            DACL_SECURITY_INFORMATION, GetSecurityDescriptorDacl,
            PROTECTED_DACL_SECURITY_INFORMATION,
        };
        let output = std::process::Command::new("whoami")
            .args(["/user", "/fo", "csv", "/nh"])
            .creation_flags(0x08000000)
            .output()?;
        ensure!(
            output.status.success(),
            "Unable to identify the profile owner"
        );
        let text = String::from_utf8_lossy(&output.stdout);
        let sid = text
            .trim()
            .rsplit(',')
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches('"');
        ensure!(
            sid.starts_with("S-1-") && sid[4..].chars().all(|ch| ch.is_ascii_digit() || ch == '-'),
            "Invalid profile owner identity"
        );
        // Replace the complete DACL in one operation. icacls /grant:r only
        // replaces this user's grants and leaves other explicit grants intact.
        // Protecting inheritance alone cannot make an existing file private.
        let inheritance = if directory { "OICI" } else { "" };
        let sddl: Vec<u16> = format!("D:P(A;{inheritance};FA;;;{sid})")
            .encode_utf16()
            .chain([0])
            .collect();
        struct Descriptor(*mut std::ffi::c_void);
        impl Drop for Descriptor {
            fn drop(&mut self) {
                // SAFETY: ConvertStringSecurityDescriptor allocates with LocalAlloc.
                unsafe {
                    LocalFree(self.0);
                }
            }
        }
        let mut descriptor = Descriptor(std::ptr::null_mut());
        // SAFETY: NUL-terminated SDDL and writable descriptor output remain valid.
        let converted = unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                SDDL_REVISION_1,
                &mut descriptor.0,
                std::ptr::null_mut(),
            )
        };
        ensure!(
            converted != 0,
            "Unable to create private terminal permissions: {}",
            std::io::Error::last_os_error()
        );
        let mut present = 0;
        let mut defaulted = 0;
        let mut acl = std::ptr::null_mut();
        // SAFETY: The descriptor is valid until its RAII owner is dropped.
        let read = unsafe {
            GetSecurityDescriptorDacl(descriptor.0, &mut present, &mut acl, &mut defaulted)
        };
        ensure!(
            read != 0 && present != 0 && !acl.is_null(),
            "Missing private terminal access list"
        );
        let name: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
        // SAFETY: The path and ACL are valid for the call. Protected DACL flags
        // replace all grants without briefly restoring broader inherited access.
        let result = unsafe {
            SetNamedSecurityInfoW(
                name.as_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                acl,
                std::ptr::null_mut(),
            )
        };
        ensure!(
            result == 0,
            "Unable to protect terminal state at {}: {}",
            path.display(),
            std::io::Error::from_raw_os_error(result as i32)
        );
    }
    #[cfg(not(windows))]
    let _ = (path, directory);
    Ok(())
}

#[derive(Deserialize)]
pub struct Launch {
    pub id: String,
    pub cwd: Option<String>,
    pub ssh: Option<rumux_core::ssh::SshConnection>,
    pub history: usize,
}

#[derive(Default, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub title: String,
    pub cwd: Option<String>,
    pub status: SurfaceStatus,
    #[serde(default)]
    pub status_message: Option<String>,
    pub notifications: Vec<Notice>,
    pub notification_sequence: u64,
    pub pid: Option<u32>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Notice {
    pub sequence: u64,
    pub title: String,
    pub body: String,
}

struct State {
    terminal: TerminalState,
    protocol: TerminalProtocolParser,
    revision: u64,
    history: usize,
    history_requested: usize,
    metadata: Metadata,
    exited: bool,
    bell: bool,
    clipboard: VecDeque<String>,
    cell_width: u16,
    cell_height: u16,
}
struct Terminal {
    id: uuid::Uuid,
    state: Mutex<State>,
    changed: Condvar,
    writer: Mutex<Option<Box<dyn Write + Send>>>,
    master: Mutex<Option<Box<dyn MasterPty + Send>>>,
    killer: Mutex<Option<Box<dyn ChildKiller + Send + Sync>>>,
    closed: AtomicBool,
    reaped: AtomicBool,
    checkpoint_revision: AtomicU64,
    checkpoint_lock: Mutex<()>,
    persist_history: bool,
}

#[derive(Serialize, Deserialize)]
struct Checkpoint {
    version: u32,
    packet: MirrorPacket,
    metadata: Metadata,
}
fn history_path(id: uuid::Uuid) -> PathBuf {
    directory().join(format!("{id}.history"))
}
fn load_checkpoint(id: uuid::Uuid) -> Result<Option<Checkpoint>> {
    let path = history_path(id);
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    ensure!(
        file.metadata()?.len() <= rumux_core::terminal_service::MAX_COMPRESSED as u64 + 4,
        "History file exceeds its limit; preserved at {}",
        path.display()
    );
    let response: Response<Checkpoint> = rumux_core::terminal_service::read_response(&mut file)
        .with_context(|| format!("History could not be read; preserved at {}", path.display()))?;
    let checkpoint = response.result.context("Missing history snapshot")?;
    ensure!(
        checkpoint.version == 1,
        "History version is unsupported; file preserved at {}",
        path.display()
    );
    checkpoint.packet.validate()?;
    Ok(Some(checkpoint))
}

impl Terminal {
    fn spawn(mut launch: Launch) -> Result<Arc<Self>> {
        let id = uuid::Uuid::parse_str(&launch.id).context("Invalid terminal identity")?;
        let persist_history = crate::config::RumuxConfig::load().persist_scrollback;
        let checkpoint = if persist_history {
            load_checkpoint(id)?
        } else {
            None
        };
        if let Some(checkpoint) = &checkpoint
            && let Some(cwd) = &checkpoint.metadata.cwd
        {
            if let Some(ssh) = &mut launch.ssh {
                if cwd.starts_with('/') && !cwd.chars().any(char::is_control) {
                    ssh.directory = Some(cwd.clone());
                }
            } else if Path::new(cwd).is_absolute() && Path::new(cwd).is_dir() {
                launch.cwd = Some(cwd.clone());
            }
        }
        let (columns, rows) = checkpoint.as_ref().map_or((80, 24), |checkpoint| {
            (
                checkpoint.packet.cursor.columns,
                checkpoint.packet.cursor.rows,
            )
        });
        if let Some(connection) = &launch.ssh {
            connection.validate()?;
        }
        if let Some(cwd) = &launch.cwd {
            ensure!(
                Path::new(cwd).is_absolute() && Path::new(cwd).is_dir(),
                "Launch directory does not exist"
            );
        }
        let pair = native_pty_system().openpty(PtySize {
            cols: columns as u16,
            rows: rows as u16,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let shell = if launch.ssh.is_some() {
            "ssh".into()
        } else {
            rumux_core::runtime::default_shell()
        };
        let mut command = CommandBuilder::new(&shell);
        if let Some(connection) = &launch.ssh {
            command.args(connection.arguments(&id.to_string())?);
        }
        #[cfg(target_os = "macos")]
        if launch.ssh.is_none()
            && matches!(
                Path::new(&shell)
                    .file_name()
                    .and_then(|value| value.to_str()),
                Some("zsh" | "bash" | "sh" | "fish")
            )
        {
            command.arg("-l");
        }
        for (name, value) in [
            ("TERM", "xterm-256color"),
            ("COLORTERM", "truecolor"),
            ("TERM_PROGRAM", "rumux"),
        ] {
            command.env(name, value);
        }
        command.env("RUMUX_SURFACE_ID", id.to_string());
        if let Some(cwd) = &launch.cwd {
            command.cwd(cwd);
        }
        let mut child = pair.slave.spawn_command(command)?;
        let mut killer = child.clone_killer();
        let streams = (|| -> Result<_> {
            Ok((pair.master.take_writer()?, pair.master.try_clone_reader()?))
        })();
        let (writer, mut reader) = match streams {
            Ok(streams) => streams,
            Err(error) => {
                let _ = killer.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        drop(pair.slave);
        let (events_tx, events_rx) = mpsc::channel();
        let history = bounded_history(columns, rows, launch.history);
        let mut terminal =
            TerminalState::with_scrollback(columns, rows, history, GpuiEventProxy::new(events_tx));
        let recovered_notices = checkpoint
            .as_ref()
            .map(|checkpoint| {
                (
                    checkpoint.metadata.notifications.clone(),
                    checkpoint.metadata.notification_sequence,
                )
            })
            .unwrap_or_default();
        if let Some(checkpoint) = checkpoint {
            checkpoint
                .packet
                .cold_history(history)?
                .apply(&mut terminal)?;
        }
        let metadata = Metadata {
            cwd: launch
                .ssh
                .as_ref()
                .and_then(|connection| connection.directory.clone())
                .or(launch.cwd),
            pid: child.process_id(),
            notifications: recovered_notices.0,
            notification_sequence: recovered_notices.1,
            ..Metadata::default()
        };
        let pane = Arc::new(Self {
            id,
            state: Mutex::new(State {
                terminal,
                protocol: TerminalProtocolParser::default().with_posix_paths(launch.ssh.is_some()),
                revision: 1,
                history,
                history_requested: launch.history.min(gpui_terminal::mirror::MAX_HISTORY),
                metadata,
                exited: false,
                bell: false,
                clipboard: VecDeque::new(),
                cell_width: 8,
                cell_height: 16,
            }),
            changed: Condvar::new(),
            writer: Mutex::new(Some(writer)),
            master: Mutex::new(Some(pair.master)),
            killer: Mutex::new(Some(killer)),
            closed: AtomicBool::new(false),
            reaped: AtomicBool::new(false),
            checkpoint_revision: AtomicU64::new(0),
            checkpoint_lock: Mutex::new(()),
            persist_history,
        });
        // Each reader owns the authoritative parser; render clients never parse these bytes.
        let pane_for_reader = pane.clone();
        std::thread::spawn(move || {
            let mut bytes = [0u8; 8192];
            loop {
                let length = match reader.read(&mut bytes) {
                    Ok(0) => break,
                    Ok(length) => length,
                    Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => break,
                };
                let mut state = pane_for_reader.state.lock();
                state.terminal.process_bytes(&bytes[..length]);
                for signal in state.protocol.feed(&bytes[..length]) {
                    match signal {
                        TerminalSignal::Title(title) => state.metadata.title = title,
                        TerminalSignal::WorkingDirectory(path) => state.metadata.cwd = Some(path),
                        TerminalSignal::Status(status) => {
                            state.metadata.status = status;
                            state.metadata.status_message = None;
                        }
                        TerminalSignal::Notification { title, body } => {
                            state.metadata.notification_sequence =
                                state.metadata.notification_sequence.saturating_add(1);
                            let sequence = state.metadata.notification_sequence;
                            state.metadata.notifications.push(Notice {
                                sequence,
                                title,
                                body,
                            });
                            if state.metadata.notifications.len() > 256 {
                                state.metadata.notifications.remove(0);
                            }
                        }
                    }
                }
                let mut replies = Vec::new();
                while let Ok(event) = events_rx.try_recv() {
                    match event {
                        TerminalEvent::PtyWrite(reply) => replies.push(reply),
                        TerminalEvent::Query(event) => {
                            if let Some(reply) = reply_to_query(&state, &event) {
                                replies.push(reply);
                            }
                        }
                        TerminalEvent::Bell => state.bell = true,
                        TerminalEvent::ClipboardStore(text) if text.len() <= 64 * 1024 => {
                            if state.clipboard.len() == 16 {
                                state.clipboard.pop_front();
                            }
                            state.clipboard.push_back(text);
                        }
                        _ => {}
                    }
                }
                state.revision = state.revision.saturating_add(1);
                drop(state);
                pane_for_reader.changed.notify_all();
                for reply in replies {
                    let _ = pane_for_reader.write(reply.as_bytes());
                }
            }
            pane_for_reader.exited();
        });
        let pane_for_waiter = pane.clone();
        std::thread::spawn(move || {
            let _ = child.wait();
            pane_for_waiter.reaped.store(true, Ordering::Release);
            pane_for_waiter.exited();
            pane_for_waiter.changed.notify_all();
        });
        Ok(pane)
    }

    fn exited(&self) {
        let mut state = self.state.lock();
        if !state.exited {
            state.exited = true;
            state.metadata.status = SurfaceStatus::Exited;
            state.revision = state.revision.saturating_add(1);
            self.changed.notify_all();
        }
    }
    fn write(&self, bytes: &[u8]) -> Result<()> {
        if std::env::var_os("RUMUX_TERMINAL_TRACE").is_some() {
            eprintln!(
                "service input {} bytes={} last={:?}",
                self.id,
                bytes.len(),
                bytes.last()
            );
        }
        ensure!(bytes.len() <= 256 * 1024, "Terminal input exceeds 256 KiB");
        let mut writer = self.writer.lock();
        let writer = writer.as_mut().context("Terminal is closed")?;
        writer.write_all(bytes)?;
        writer.flush()?;
        if std::env::var_os("RUMUX_TERMINAL_TRACE").is_some() {
            eprintln!("service input complete {}", self.id);
        }
        Ok(())
    }
    fn resize(&self, columns: usize, rows: usize, cell_width: u16, cell_height: u16) -> Result<()> {
        ensure!(
            (1..=1024).contains(&cell_width) && (1..=1024).contains(&cell_height),
            "Invalid terminal cell size"
        );
        ensure!(
            (2..=gpui_terminal::mirror::MAX_COLUMNS).contains(&columns)
                && (1..=gpui_terminal::mirror::MAX_ROWS).contains(&rows),
            "Invalid terminal dimensions"
        );
        let mut state = self.state.lock();
        if state.terminal.cols() == columns
            && state.terminal.rows() == rows
            && state.cell_width == cell_width
            && state.cell_height == cell_height
        {
            return Ok(());
        }
        self.master
            .lock()
            .as_ref()
            .context("Terminal is closed")?
            .resize(PtySize {
                cols: columns as u16,
                rows: rows as u16,
                pixel_width: (columns * usize::from(cell_width)).min(u16::MAX as usize) as u16,
                pixel_height: (rows * usize::from(cell_height)).min(u16::MAX as usize) as u16,
            })?;
        state.history = bounded_history(columns, rows, state.history_requested);
        state.cell_width = cell_width;
        state.cell_height = cell_height;
        let history = state.history;
        state.terminal.with_term_mut(|term| {
            term.set_options(alacritty_terminal::term::Config {
                scrolling_history: history,
                ..Default::default()
            })
        });
        state.terminal.resize(columns, rows);
        state.revision = state.revision.saturating_add(1);
        self.changed.notify_all();
        Ok(())
    }
    fn poll(&self, after: Option<MirrorCursor>) -> Result<Option<MirrorUpdate>> {
        let mut state = self.state.lock();
        if after
            .as_ref()
            .is_some_and(|cursor| cursor.revision == state.revision)
        {
            self.changed
                .wait_for(&mut state, Duration::from_millis(1000));
        }
        if after
            .as_ref()
            .is_some_and(|cursor| cursor.revision == state.revision)
        {
            return Ok(None);
        }
        let packet = state.terminal.with_term(|term| {
            MirrorPacket::capture(term, after.as_ref(), state.revision, state.history)
        })?;
        let metadata = serde_json::to_value(&state.metadata)?;
        let bell = std::mem::take(&mut state.bell);
        let clipboard = state.clipboard.drain(..).collect();
        Ok(Some(MirrorUpdate {
            packet,
            metadata,
            exited: state.exited,
            bell,
            clipboard,
        }))
    }
    fn close(&self) {
        if self.closed.swap(true, Ordering::SeqCst) {
            return;
        }
        if let Some(mut killer) = self.killer.lock().take() {
            let _ = killer.kill();
        }
        self.writer.lock().take();
        self.master.lock().take();
        self.exited();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut state = self.state.lock();
        while !self.reaped.load(Ordering::Acquire) && Instant::now() < deadline {
            self.changed.wait_for(&mut state, Duration::from_millis(20));
        }
    }

    fn checkpoint(&self) -> Result<()> {
        let _guard = self.checkpoint_lock.lock();
        if self.closed.load(Ordering::Acquire) || !self.persist_history {
            return Ok(());
        }
        let state = self.state.lock();
        if self.checkpoint_revision.load(Ordering::Relaxed) == state.revision {
            return Ok(());
        }
        let packet = state
            .terminal
            .with_term(|term| MirrorPacket::capture_primary(term, state.revision, state.history))?;
        let revision = state.revision;
        let checkpoint = Checkpoint {
            version: 1,
            packet,
            metadata: state.metadata.clone(),
        };
        drop(state);
        let mut file = tempfile::NamedTempFile::new_in(directory())?;
        private_file(file.as_file())?;
        rumux_core::terminal_service::write_response(
            &mut file,
            &Response {
                ok: true,
                result: Some(checkpoint),
                error: None,
            },
        )?;
        file.as_file().sync_all()?;
        file.persist(history_path(self.id))?;
        self.checkpoint_revision.store(revision, Ordering::Relaxed);
        Ok(())
    }

    fn remove_history(&self) {
        let _guard = self.checkpoint_lock.lock();
        std::fs::remove_file(history_path(self.id)).ok();
    }
}

fn reply_to_query(state: &State, event: &alacritty_terminal::event::Event) -> Option<String> {
    use alacritty_terminal::event::Event;
    match event {
        Event::ClipboardLoad(_, format) => Some(format("")),
        Event::ColorRequest(index, format) => {
            use alacritty_terminal::vte::ansi::{Color, NamedColor, Rgb};
            let color = match *index {
                0..=255 => Color::Indexed(*index as u8),
                257 => Color::Named(NamedColor::Background),
                258 => Color::Named(NamedColor::Cursor),
                _ => Color::Named(NamedColor::Foreground),
            };
            let rgb = state.terminal.with_term(|term| {
                crate::theme::catppuccin_mocha()
                    .resolve(color, term.colors())
                    .to_rgb()
            });
            Some(format(Rgb {
                r: (rgb.r * 255.0).round() as u8,
                g: (rgb.g * 255.0).round() as u8,
                b: (rgb.b * 255.0).round() as u8,
            }))
        }
        Event::TextAreaSizeRequest(format) => Some(format(alacritty_terminal::event::WindowSize {
            num_cols: state.terminal.cols() as u16,
            num_lines: state.terminal.rows() as u16,
            cell_width: state.cell_width,
            cell_height: state.cell_height,
        })),
        _ => None,
    }
}

struct Service {
    terminals: Mutex<HashMap<uuid::Uuid, Arc<Terminal>>>,
    stopping: AtomicBool,
}
#[derive(Serialize)]
#[serde(untagged)]
enum Reply {
    Mirror(Box<Option<MirrorUpdate>>),
    Json(Value),
}
impl Service {
    fn handle(&self, request: Request) -> Result<Reply> {
        if request.method == "poll" {
            let id = request
                .params
                .get("id")
                .and_then(Value::as_str)
                .context("Missing terminal identity")?;
            let id = uuid::Uuid::parse_str(id)?;
            let terminal = self
                .terminals
                .lock()
                .get(&id)
                .cloned()
                .context("Terminal not found")?;
            let after = request
                .params
                .get("after")
                .filter(|value| !value.is_null())
                .map(|value| serde_json::from_value(value.clone()))
                .transpose()?;
            return terminal
                .poll(after)
                .map(|update| Reply::Mirror(Box::new(update)));
        }
        self.handle_other(request).map(Reply::Json)
    }
    fn handle_other(&self, request: Request) -> Result<Value> {
        ensure!(
            request.params.is_object(),
            "Terminal service params must be an object"
        );
        match request.method.as_str() {
            "ping" => Ok(json!({"protocol":PROTOCOL,"pid":std::process::id()})),
            "list" => Ok(Value::Array(
                self.terminals
                    .lock()
                    .values()
                    .map(|terminal| {
                        let state = terminal.state.lock();
                        json!({"id":terminal.id,"pid":state.metadata.pid,"exited":state.exited})
                    })
                    .collect(),
            )),
            "open" => {
                let launch: Launch = serde_json::from_value(request.params)?;
                let id = uuid::Uuid::parse_str(&launch.id)?;
                let mut terminals = self.terminals.lock();
                if let Some(terminal) = terminals.get(&id) {
                    return Ok(
                        json!({"id":id,"pid":terminal.state.lock().metadata.pid,"reattached":true}),
                    );
                }
                ensure!(
                    terminals.len() < 64,
                    "Terminal service supports up to 64 live panes"
                );
                let terminal = Terminal::spawn(launch)?;
                let pid = terminal.state.lock().metadata.pid;
                terminals.insert(id, terminal);
                Ok(json!({"id":id,"pid":pid,"reattached":false}))
            }
            "stop" => Ok(json!({"stopping":true})),
            method => {
                let id = request
                    .params
                    .get("id")
                    .and_then(Value::as_str)
                    .context("Missing terminal identity")?;
                let id = uuid::Uuid::parse_str(id)?;
                let terminal = self
                    .terminals
                    .lock()
                    .get(&id)
                    .cloned()
                    .context("Terminal not found")?;
                match method {
                    "poll" => {
                        let after = request
                            .params
                            .get("after")
                            .filter(|value| !value.is_null())
                            .map(|value| serde_json::from_value(value.clone()))
                            .transpose()?;
                        Ok(serde_json::to_value(terminal.poll(after)?)?)
                    }
                    "write" => {
                        let bytes: Vec<u8> = serde_json::from_value(
                            request
                                .params
                                .get("bytes")
                                .cloned()
                                .context("Missing terminal input")?,
                        )?;
                        terminal.write(&bytes)?;
                        Ok(json!({"written":bytes.len()}))
                    }
                    "ack" => {
                        let sequence = request
                            .params
                            .get("sequence")
                            .and_then(Value::as_u64)
                            .context("Missing notification sequence")?;
                        let mut state = terminal.state.lock();
                        ensure!(
                            sequence <= state.metadata.notification_sequence,
                            "Invalid notification sequence"
                        );
                        state
                            .metadata
                            .notifications
                            .retain(|notice| notice.sequence > sequence);
                        state.revision = state.revision.saturating_add(1);
                        terminal.changed.notify_all();
                        Ok(json!({}))
                    }
                    "status" => {
                        let status: SurfaceStatus = serde_json::from_value(
                            request
                                .params
                                .get("status")
                                .cloned()
                                .context("Missing terminal status")?,
                        )?;
                        let message: Option<String> = serde_json::from_value(
                            request
                                .params
                                .get("message")
                                .cloned()
                                .unwrap_or(Value::Null),
                        )?;
                        ensure!(
                            message.as_ref().is_none_or(|message| message.len() <= 4096),
                            "Status message exceeds 4096 bytes"
                        );
                        let mut state = terminal.state.lock();
                        state.metadata.status = if state.exited {
                            SurfaceStatus::Exited
                        } else {
                            status
                        };
                        state.metadata.status_message = message;
                        state.revision = state.revision.saturating_add(1);
                        terminal.changed.notify_all();
                        Ok(json!({}))
                    }
                    "resize" => {
                        let columns = request
                            .params
                            .get("columns")
                            .and_then(Value::as_u64)
                            .context("Missing columns")?;
                        let rows = request
                            .params
                            .get("rows")
                            .and_then(Value::as_u64)
                            .context("Missing rows")?;
                        let cell_width = request
                            .params
                            .get("cell_width")
                            .and_then(Value::as_u64)
                            .unwrap_or(8);
                        let cell_height = request
                            .params
                            .get("cell_height")
                            .and_then(Value::as_u64)
                            .unwrap_or(16);
                        terminal.resize(
                            usize::try_from(columns)?,
                            usize::try_from(rows)?,
                            u16::try_from(cell_width)?,
                            u16::try_from(cell_height)?,
                        )?;
                        Ok(json!({}))
                    }
                    "close" => {
                        terminal.close();
                        terminal.remove_history();
                        self.terminals.lock().remove(&id);
                        Ok(json!({"closed":true}))
                    }
                    _ => anyhow::bail!("Unknown terminal service method"),
                }
            }
        }
    }
}

pub fn run() -> Result<()> {
    let _lock = lock_file()?;
    _lock
        .try_lock_exclusive()
        .context("Another terminal service owns this profile")?;
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let endpoint = Endpoint {
        protocol: PROTOCOL,
        address: listener.local_addr()?,
        pid: std::process::id(),
        token: token.clone(),
    };
    save_json(&rumux_core::terminal_service::endpoint_path(), &endpoint)?;
    let service = Arc::new(Service {
        terminals: Mutex::new(HashMap::new()),
        stopping: AtomicBool::new(false),
    });
    let workers = Arc::new(AtomicUsize::new(0));
    let checkpoint_service = service.clone();
    std::thread::spawn(move || {
        while !checkpoint_service.stopping.load(Ordering::Acquire) {
            let terminals: Vec<_> = checkpoint_service
                .terminals
                .lock()
                .values()
                .cloned()
                .collect();
            for terminal in terminals {
                if let Err(error) = terminal.checkpoint() {
                    eprintln!("History checkpoint failed for {}: {error:#}", terminal.id);
                }
            }
            for _ in 0..50 {
                if checkpoint_service.stopping.load(Ordering::Acquire) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    });
    while !service.stopping.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => {
                if workers.fetch_add(1, Ordering::SeqCst) >= 96 {
                    workers.fetch_sub(1, Ordering::SeqCst);
                    drop(stream);
                    continue;
                }
                let service = service.clone();
                let token = token.clone();
                let workers = workers.clone();
                std::thread::spawn(move || {
                    let _ = serve(stream, &service, &token);
                    workers.fetch_sub(1, Ordering::SeqCst);
                });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(5))
            }
            Err(error) => return Err(error.into()),
        }
    }
    for terminal in service.terminals.lock().values() {
        if let Err(error) = terminal.checkpoint() {
            eprintln!("History checkpoint failed for {}: {error:#}", terminal.id);
        }
        terminal.close();
    }
    std::fs::remove_file(rumux_core::terminal_service::endpoint_path()).ok();
    Ok(())
}

fn serve(mut stream: TcpStream, service: &Service, token: &str) -> Result<()> {
    use rumux_core::terminal_service::{read_request, token_matches, write_response};
    // macOS accepts inherit O_NONBLOCK from the listener. These workers use
    // blocking framed reads with deadlines; a partial request must not be lost.
    stream.set_nonblocking(false)?;
    stream.set_nodelay(true)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    let mut stop_requested = false;
    let result = (|| -> Result<_> {
        let request = read_request(&mut stream)?;
        ensure!(
            request.protocol == PROTOCOL,
            "Terminal service protocol mismatch"
        );
        ensure!(
            token_matches(token, &request.token),
            "Terminal service authentication failed"
        );
        stop_requested = request.method == "stop";
        if std::env::var_os("RUMUX_TERMINAL_TRACE").is_some() {
            eprintln!(
                "service request {} {}",
                request.method,
                request.params.get("id").unwrap_or(&Value::Null)
            );
        }
        service.handle(request)
    })();
    let response = match result {
        Ok(result) => Response {
            ok: true,
            result: Some(result),
            error: None,
        },
        Err(error) => Response {
            ok: false,
            result: None,
            error: Some(error.to_string()),
        },
    };
    if std::env::var_os("RUMUX_TERMINAL_TRACE").is_some()
        && let Some(error) = &response.error
    {
        eprintln!("service request error {error}");
    }
    let written = write_response(&mut stream, &response);
    if written.is_ok() && response.ok && stop_requested {
        service.stopping.store(true, Ordering::Release);
    }
    written
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;

    #[test]
    fn private_permissions_replace_existing_explicit_grants() {
        let directory = tempfile::tempdir().unwrap();
        let file = directory.path().join("private-state");
        std::fs::write(&file, b"owned test fixture").unwrap();
        // A pre-existing explicit Everyone grant survives /grant:r for a
        // different SID. The production operation must remove it completely.
        let grant = std::process::Command::new("icacls")
            .arg(&file)
            .args(["/grant", "*S-1-1-0:R"])
            .output()
            .unwrap();
        assert!(grant.status.success());
        protect_windows_path(&file, false).unwrap();
        let inspected = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-Command",
                "$ErrorActionPreference='Stop'; $sid=[Security.Principal.WindowsIdentity]::GetCurrent().User; $acl=Get-Acl -LiteralPath $env:RUMUX_TEST_PRIVATE_PATH; $rules=$acl.GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier]); if (@($rules).Count -ne 1 -or $rules[0].IdentityReference -ne $sid -or $rules[0].AccessControlType -ne 'Allow' -or -not $acl.AreAccessRulesProtected) { throw 'Private permissions were not replaced' }"])
            .env("RUMUX_TEST_PRIVATE_PATH", &file)
            .env_remove("PSModulePath")
            .output()
            .unwrap();
        assert!(
            inspected.status.success(),
            "{}",
            String::from_utf8_lossy(&inspected.stderr)
        );
    }
}
