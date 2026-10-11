use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use rumux_core::ssh::SshConnection;

use crate::{app_state::AppState, theme};

pub struct SshDialog {
    app_state: Entity<AppState>,
    destination: Entity<InputState>,
    directory: Entity<InputState>,
    port: Entity<InputState>,
    identity: Entity<InputState>,
    config: Entity<InputState>,
    known_hosts: Entity<InputState>,
    advanced: bool,
    error: Option<String>,
    pending_connect: bool,
    focus_handle: FocusHandle,
    _subscriptions: Vec<Subscription>,
}
pub struct DismissSshDialog;
impl EventEmitter<DismissSshDialog> for SshDialog {}

impl SshDialog {
    pub fn new(app_state: Entity<AppState>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut input = |placeholder: &'static str, cx: &mut Context<Self>| {
            cx.new(|cx| {
                let mut state = InputState::new(window, cx);
                state.set_placeholder(placeholder, window, cx);
                state
            })
        };
        let destination = input("user@host or SSH config alias", cx);
        let directory = input("Remote directory (optional, e.g. /home/me/project)", cx);
        let port = input("Port (optional)", cx);
        let identity = input("Identity file (optional)", cx);
        let config = input("SSH config file (optional)", cx);
        let known_hosts = input("Known-hosts file (optional, strict verification)", cx);
        destination.update(cx, |input, cx| input.focus(window, cx));
        let subscriptions = [
            &destination,
            &directory,
            &port,
            &identity,
            &config,
            &known_hosts,
        ]
        .into_iter()
        .map(|input| {
            cx.subscribe(input, |dialog: &mut Self, _, event, cx| {
                if matches!(event, InputEvent::Change) && dialog.error.take().is_some() {
                    cx.notify();
                }
                if matches!(event, InputEvent::PressEnter { .. }) {
                    dialog.pending_connect = true;
                    cx.notify();
                }
            })
        })
        .collect();
        Self {
            app_state,
            destination,
            directory,
            port,
            identity,
            config,
            known_hosts,
            advanced: false,
            error: None,
            pending_connect: false,
            focus_handle: cx.focus_handle(),
            _subscriptions: subscriptions,
        }
    }

    fn connection(&self, cx: &App) -> anyhow::Result<SshConnection> {
        let destination = self
            .destination
            .read(cx)
            .text()
            .to_string()
            .trim()
            .to_owned();
        let (user, host) = destination
            .split_once('@')
            .map_or((None, destination.as_str()), |(user, host)| {
                (Some(user.to_owned()), host)
            });
        let directory = self.directory.read(cx).text().to_string().trim().to_owned();
        let port = self.port.read(cx).text().to_string().trim().to_owned();
        let connection = SshConnection {
            host: host.to_owned(),
            user,
            port: if port.is_empty() {
                None
            } else {
                Some(
                    port.parse()
                        .map_err(|_| anyhow::anyhow!("Port must be between 1 and 65535"))?,
                )
            },
            directory: (!directory.is_empty()).then_some(directory),
            identity_file: optional_path(&self.identity, cx),
            known_hosts_file: optional_path(&self.known_hosts, cx),
            config_file: optional_path(&self.config, cx),
            forwards: Vec::new(),
        };
        connection.validate()?;
        Ok(connection)
    }

    fn connect(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.connection(cx) {
            Ok(connection) => {
                self.app_state.update(cx, |state, cx| {
                    state.add_ssh_workspace(None, connection, window, cx)
                });
                cx.emit(DismissSshDialog);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                cx.notify();
            }
        }
    }
}

impl Render for SshDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if std::mem::take(&mut self.pending_connect) {
            self.connect(window, cx);
        }
        div().absolute().inset_0().bg(gpui::black().opacity(0.5))
            .flex().items_center().justify_center().track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|_, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "escape" { cx.emit(DismissSshDialog); cx.stop_propagation(); }
            }))
            .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| cx.emit(DismissSshDialog)))
            .child(div().w(px(480.0)).p(px(20.0)).flex().flex_col().gap(px(12.0))
                .bg(rgb(theme::BG_SECONDARY)).rounded(px(8.0)).border_1().border_color(rgb(theme::BORDER))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(div().text_size(px(18.0)).child("Connect SSH"))
                .child(Input::new(&self.destination))
                .child(Input::new(&self.directory))
                .child(Input::new(&self.port))
                .child(Button::new("ssh-advanced").ghost().label(if self.advanced { "Hide advanced options" } else { "Advanced options" })
                    .on_click(cx.listener(|dialog, _, _, cx| { dialog.advanced = !dialog.advanced; cx.notify(); })))
                .when(self.advanced, |view| view
                    .child(Input::new(&self.identity))
                    .child(Input::new(&self.config))
                    .child(Input::new(&self.known_hosts)))
                .child(div().text_size(px(12.0)).text_color(rgb(theme::TEXT_DIM))
                    .child("Uses your OpenSSH config and keys. Authentication prompts appear in the terminal. New tabs inherit the remote directory."))
                .when_some(self.error.clone(), |view, error| view.child(div().text_color(rgb(theme::ACCENT_RED)).child(error)))
                .child(div().flex().justify_end().gap(px(8.0))
                    .child(Button::new("ssh-cancel").ghost().label("Cancel").on_click(cx.listener(|_, _, _, cx| cx.emit(DismissSshDialog))))
                    .child(Button::new("ssh-connect").label("Connect").on_click(cx.listener(|dialog, _, window, cx| dialog.connect(window, cx))))))
    }
}

fn optional_path(input: &Entity<InputState>, cx: &App) -> Option<std::path::PathBuf> {
    let text = input.read(cx).text().to_string();
    let value = text.trim();
    (!value.is_empty()).then(|| value.into())
}
