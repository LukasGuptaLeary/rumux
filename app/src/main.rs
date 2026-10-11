#![warn(clippy::all)]

mod app_state;
mod browser;
mod browser_panel;
mod command_palette;
mod config;
mod custom_commands;
mod find_bar;
mod instance;
mod notification_panel;
mod notifications;
mod root_view;
mod session;
mod sidebar;
mod socket_server;
mod ssh_dialog;
mod terminal_panel;
mod terminal_service;
mod terminal_surface;
mod theme;
mod workspace;

use gpui::*;

use app_state::AppState;
use root_view::*;

fn main() {
    if std::env::args().any(|argument| argument == "--terminal-service") {
        if let Err(error) = terminal_service::run() {
            eprintln!("Terminal service failed: {error:#}");
            std::process::exit(1);
        }
        return;
    }
    #[cfg(target_os = "windows")]
    // This runs before any application threads are created. Native child
    // WebView2 windows need GPUI's HWND composition path.
    unsafe {
        std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "true");
    }
    const APP_ID: &str = "io.github.lukasguptaleary.rumux";

    let _instance_guard = match instance::acquire_or_activate_existing() {
        Ok(Some(guard)) => guard,
        Ok(None) => return,
        Err(error) => {
            eprintln!("Failed to start rumux: {error}");
            std::process::exit(1);
        }
    };
    let terminal_client = match terminal_service::ensure_started() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("Failed to start rumux: {error:#}");
            std::process::exit(1);
        }
    };

    // GTK child browsers can share an X11/XWayland parent; keep the original
    // session environment intact for terminals and programs launched inside.
    #[cfg(target_os = "linux")]
    let application = if std::env::var_os("DISPLAY").is_some_and(|value| !value.is_empty())
        && std::env::var("RUMUX_LINUX_BACKEND").as_deref() != Ok("wayland")
    {
        match Application::x11() {
            Ok(app) => app,
            Err(error) => {
                eprintln!("Failed to open X11/XWayland display: {error:#}");
                std::process::exit(1);
            }
        }
    } else {
        Application::new()
    };
    #[cfg(not(target_os = "linux"))]
    let application = Application::new();
    let app = application.with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        cx.set_global(terminal_service::ServiceClient(terminal_client));
        gpui_component::init(cx);
        gpui_component::Theme::change(gpui_component::ThemeMode::Dark, None, cx);

        // Register panel types for DockArea serialization
        terminal_panel::register(cx);
        browser::init(cx);
        browser_panel::register(cx);

        cx.bind_keys([
            KeyBinding::new("ctrl-shift-n", NewWorkspace, None),
            KeyBinding::new("cmd-shift-n", NewWorkspace, None),
            KeyBinding::new("ctrl-shift-w", CloseWorkspace, None),
            KeyBinding::new("cmd-shift-w", CloseWorkspace, None),
            KeyBinding::new("ctrl-shift-d", SplitRight, None),
            KeyBinding::new("cmd-d", SplitRight, None),
            KeyBinding::new("ctrl-alt-d", SplitDown, None),
            KeyBinding::new("cmd-shift-d", SplitDown, None),
            KeyBinding::new("ctrl-shift-t", NewTerminal, None),
            KeyBinding::new("cmd-t", NewTerminal, None),
            KeyBinding::new("ctrl-shift-o", NewBrowser, None),
            KeyBinding::new("cmd-shift-o", NewBrowser, None),
            KeyBinding::new("ctrl-shift-x", CloseTerminal, None),
            KeyBinding::new("cmd-w", CloseTerminal, None),
            KeyBinding::new("ctrl-tab", NextWorkspace, None),
            KeyBinding::new("ctrl-shift-tab", PrevWorkspace, None),
            KeyBinding::new("ctrl-pagedown", NextTerminal, None),
            KeyBinding::new("ctrl-pageup", PrevTerminal, None),
            KeyBinding::new("ctrl-shift-p", ToggleCommandPalette, None),
            KeyBinding::new("cmd-shift-p", ToggleCommandPalette, None),
            KeyBinding::new("ctrl-shift-i", ToggleNotificationPanel, None),
            KeyBinding::new("cmd-shift-i", ToggleNotificationPanel, None),
            KeyBinding::new("ctrl-b", ToggleSidebar, None),
            KeyBinding::new("cmd-b", ToggleSidebar, None),
            KeyBinding::new("ctrl-shift-enter", TogglePaneZoom, None),
            KeyBinding::new("cmd-shift-enter", TogglePaneZoom, None),
            KeyBinding::new("ctrl-shift-u", JumpToUnread, None),
            KeyBinding::new("cmd-shift-u", JumpToUnread, None),
            KeyBinding::new("ctrl-alt-c", DuplicateWorkspace, None),
            KeyBinding::new("cmd-shift-c", DuplicateWorkspace, None),
            KeyBinding::new("ctrl-f", ToggleFindBar, None),
            KeyBinding::new("cmd-f", ToggleFindBar, None),
            KeyBinding::new("ctrl-q", QuitApp, None),
            KeyBinding::new("cmd-q", QuitApp, None),
        ]);

        cx.spawn(async move |cx: &mut AsyncApp| {
            let app_state = cx.new(|cx| AppState::new(&mut *cx))?;
            let state_for_quit = app_state.downgrade();
            cx.update(|cx| {
                cx.on_app_quit(move |cx| {
                    if let Some(state) = state_for_quit.upgrade() {
                        state.read(cx).save_session(cx);
                    }
                    async {}
                })
                .detach();
            })?;

            let main_window = cx.open_window(
                WindowOptions {
                    app_id: Some(APP_ID.into()),
                    titlebar: Some(TitlebarOptions {
                        title: Some("rumux".into()),
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(600.0), px(400.0))),
                    ..Default::default()
                },
                |window, cx| {
                    let root_view = cx.new(|cx| RootView::new(app_state.clone(), window, cx));
                    root_view.read(cx).focus_handle.focus(window);
                    cx.new(|cx| gpui_component::Root::new(root_view, window, cx))
                },
            )?;

            app_state.update(cx, |state, _cx| {
                state.set_primary_window(main_window.into());
            })?;

            let socket_state = app_state.downgrade();
            cx.spawn(async move |cx: &mut AsyncApp| {
                if let Err(error) =
                    socket_server::start_socket_server(socket_state.clone(), cx).await
                {
                    eprintln!("Socket server error: {error}");
                }
                Ok::<_, anyhow::Error>(())
            })
            .detach();

            // Git branch polling task (every 3 seconds)
            let state_for_git = app_state.downgrade();
            cx.spawn(async move |cx: &mut AsyncApp| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_secs(3))
                        .await;

                    let result = state_for_git.update(cx, |state, cx| {
                        detect_git_branches(state, cx);
                    });
                    if result.is_err() {
                        break;
                    }
                }
                Ok::<_, anyhow::Error>(())
            })
            .detach();

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}

fn detect_git_branches(state: &mut AppState, cx: &mut gpui::Context<AppState>) {
    for ws in &state.workspaces {
        ws.update(cx, |ws, cx| {
            let cwd = ws.cwd.as_deref().unwrap_or(".");
            let branch = detect_branch(cwd);
            if ws.git_branch != branch {
                ws.git_branch = branch;
                cx.notify();
            }
        });
    }
}

fn detect_branch(path: &str) -> Option<String> {
    let repo = git2::Repository::discover(path).ok()?;
    let head = repo.head().ok()?;
    head.shorthand().map(|s| s.to_string())
}
