#![warn(clippy::all)]

mod app_state;
mod command_palette;
mod config;
mod custom_commands;
mod dropdown_menu;
mod find_bar;
mod notification_panel;
mod notifications;
mod pane;
mod root_view;
mod session;
mod sidebar;
mod socket_server;
mod terminal_surface;
mod text_input;
mod theme;
mod workspace;

use gpui::*;

use app_state::AppState;
use root_view::*;

fn main() {
    let app = Application::new().with_assets(gpui_component_assets::Assets);

    app.run(move |cx| {
        gpui_component::init(cx);
        gpui_component::Theme::change(gpui_component::ThemeMode::Dark, None, cx);

        cx.bind_keys([
            KeyBinding::new("ctrl-shift-n", NewWorkspace, None),
            KeyBinding::new("ctrl-shift-w", CloseWorkspace, None),
            KeyBinding::new("ctrl-shift-d", SplitRight, None),
            KeyBinding::new("ctrl-alt-d", SplitDown, None),
            KeyBinding::new("ctrl-shift-t", NewTerminal, None),
            KeyBinding::new("ctrl-shift-x", CloseTerminal, None),
            KeyBinding::new("ctrl-tab", NextWorkspace, None),
            KeyBinding::new("ctrl-shift-tab", PrevWorkspace, None),
            KeyBinding::new("ctrl-pagedown", NextTerminal, None),
            KeyBinding::new("ctrl-pageup", PrevTerminal, None),
            KeyBinding::new("ctrl-shift-p", ToggleCommandPalette, None),
            KeyBinding::new("ctrl-shift-i", ToggleNotificationPanel, None),
            KeyBinding::new("ctrl-b", ToggleSidebar, None),
            KeyBinding::new("ctrl-shift-enter", TogglePaneZoom, None),
            KeyBinding::new("ctrl-shift-u", JumpToUnread, None),
            KeyBinding::new("ctrl-shift-c", DuplicateWorkspace, None),
            KeyBinding::new("ctrl-f", ToggleFindBar, None),
            KeyBinding::new("ctrl-q", QuitApp, None),
        ]);

        // Start socket server in background
        cx.spawn(async |_cx: &mut AsyncApp| {
            if let Err(e) = socket_server::start_socket_server().await {
                eprintln!("Socket server error: {e}");
            }
            Ok::<_, anyhow::Error>(())
        })
        .detach();

        cx.spawn(async move |cx: &mut AsyncApp| {
            let app_state = cx.new(|cx| AppState::new(&mut *cx))?;

            cx.open_window(
                WindowOptions {
                    titlebar: Some(TitlebarOptions {
                        title: Some("rumux".into()),
                        ..Default::default()
                    }),
                    window_min_size: Some(size(px(600.0), px(400.0))),
                    ..Default::default()
                },
                |window, cx| {
                    let root_view = cx.new(|cx| RootView::new(app_state.clone(), cx));
                    root_view.read(cx).focus_handle.focus(window);
                    // Wrap in gpui-component's Root for Input, Dialog, Notification support
                    cx.new(|cx| gpui_component::Root::new(root_view, window, cx))
                },
            )?;

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
            // Try to detect git branch from CWD
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
