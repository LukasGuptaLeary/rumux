#![warn(clippy::all)]

mod app_state;
mod pane;
mod root_view;
mod sidebar;
mod terminal_surface;
mod theme;
mod workspace;

use gpui::*;

use app_state::AppState;
use root_view::*;

fn main() {
    let app = Application::new();

    app.run(move |cx| {
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
        ]);

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
                    let root = cx.new(|cx| RootView::new(app_state, cx));
                    root.read(cx).focus_handle.focus(window);
                    root
                },
            )?;

            Ok::<_, anyhow::Error>(())
        })
        .detach();
    });
}
