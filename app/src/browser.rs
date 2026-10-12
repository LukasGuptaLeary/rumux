use anyhow::{Context as _, Result};
use gpui::*;
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::{IconName, Sizable};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use serde_json::{Value, json};
use wry::{WebContext, WebView, WebViewBuilder};

struct BrowserRuntime(Option<WebContext>);
impl Global for BrowserRuntime {}

pub fn init(cx: &mut App) {
    cx.set_global(BrowserRuntime(None));
    #[cfg(target_os = "linux")]
    cx.spawn(async move |cx| {
        loop {
            if cx
                .update(|_| {
                    if !gtk::is_initialized_main_thread() {
                        return;
                    }
                    // Wry's X11 host uses GTK on GPUI's foreground thread.
                    // Bound work per tick so a busy page cannot starve terminal I/O.
                    for _ in 0..32 {
                        if !gtk::events_pending() {
                            break;
                        }
                        gtk::main_iteration_do(false);
                    }
                })
                .is_err()
            {
                break;
            }
            cx.background_executor()
                .timer(std::time::Duration::from_millis(8))
                .await;
        }
    })
    .detach();
}

pub fn normalize_url(input: &str) -> Result<String> {
    let input = input.trim();
    anyhow::ensure!(
        !input.is_empty() && input.len() <= 16_384,
        "Enter a URL of at most 16384 bytes"
    );
    let url = if input == "about:blank" || input.contains("://") {
        input.to_string()
    } else if input.starts_with("localhost")
        || input.starts_with("127.0.0.1")
        || input.starts_with("[::1]")
    {
        format!("http://{input}")
    } else {
        format!("https://{input}")
    };
    let parsed = url::Url::parse(&url).context("Invalid browser URL")?;
    anyhow::ensure!(
        matches!(parsed.scheme(), "http" | "https") || url == "about:blank",
        "Browser navigation supports http, https and about:blank"
    );
    Ok(parsed.to_string())
}

pub enum BrowserEvent {
    Changed,
}
impl EventEmitter<BrowserEvent> for BrowserView {}

enum NativeEvent {
    Title(String),
    Page(wry::PageLoadEvent, String),
    NewWindow(String),
    Focus,
}

pub struct BrowserView {
    native: Option<WebView>,
    address: Entity<InputState>,
    focus: FocusHandle,
    pub url: String,
    pub title: String,
    pub loading: bool,
    pub error: Option<String>,
    selected: bool,
    allowed: bool,
    #[cfg(target_os = "windows")]
    native_bounds: Option<Bounds<Pixels>>,
    #[cfg(target_os = "windows")]
    native_visible: bool,
    _input_subscription: Subscription,
    _native_task: Task<()>,
    _creation_task: Option<Task<()>>,
}

impl BrowserView {
    pub fn new(url: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let address = cx.new(|cx| {
            let mut input = InputState::new(window, cx);
            input.set_value(&url, window, cx);
            input
        });
        let subscription = cx.subscribe_in(
            &address,
            window,
            |view: &mut Self, address, event: &InputEvent, window, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    let url = address.read(cx).text().to_string();
                    if let Err(error) = view.navigate(&url, window, cx) {
                        view.error = Some(error.to_string());
                        cx.notify();
                    }
                }
            },
        );
        let (sender, receiver) = flume::bounded(64);
        #[cfg(not(target_os = "windows"))]
        let native = create_native(&url, sender, window, cx);
        #[cfg(target_os = "windows")]
        let native: Result<WebView> = Err(anyhow::anyhow!("Browser is starting"));
        #[cfg(not(target_os = "windows"))]
        let error = native
            .as_ref()
            .err()
            .map(|error| format!("Unable to open browser: {error:#}"));
        #[cfg(target_os = "windows")]
        let error = None;
        let window_handle = Window::window_handle(window);
        // WebView2 construction pumps Windows messages. Run it outside every
        // GPUI App/Entity borrow so pending PTY tasks can update their views.
        #[cfg(target_os = "windows")]
        let creation_task = {
            let parent = window.native_handle_lease();
            let initial_url = url.clone();
            Some(cx.spawn(async move |view, cx| {
                let native = match parent {
                    Some(parent) => {
                        let mut context = WebContext::new(Some(
                            rumux_core::runtime::rumux_config_dir().join("browser"),
                        ));
                        trace_browser("native creation started");
                        let native = build_native(&initial_url, sender, &parent, &mut context);
                        trace_browser("native creation finished");
                        native
                    }
                    None => Err(anyhow::anyhow!("Native browser parent is unavailable")),
                };
                let _ = view.update(cx, |view, cx| {
                    match native {
                        Ok(native) => view.native = Some(native),
                        Err(error) => {
                            view.error = Some(format!("Unable to open browser: {error:#}"));
                            view.loading = false;
                        }
                    }
                    cx.emit(BrowserEvent::Changed);
                    cx.notify();
                });
            }))
        };
        #[cfg(not(target_os = "windows"))]
        let creation_task = None;
        let task = cx.spawn(async move |view, cx| {
            while let Ok(event) = receiver.recv_async().await {
                if cx
                    .update_window(window_handle, |_, window, cx| {
                        view.update(cx, |view, cx| {
                            match event {
                                NativeEvent::Title(title) => {
                                    view.title = title
                                        .chars()
                                        .filter(|ch| !ch.is_control())
                                        .take(1024)
                                        .collect();
                                }
                                NativeEvent::Page(event, url) => {
                                    view.loading = matches!(event, wry::PageLoadEvent::Started);
                                    trace_browser(if view.loading {
                                        "page loading"
                                    } else {
                                        "page loaded"
                                    });
                                    view.address
                                        .update(cx, |input, cx| input.set_value(&url, window, cx));
                                    view.url = url;
                                    view.error = None;
                                }
                                NativeEvent::Focus => {
                                    if view.selected && view.allowed {
                                        view.focus.focus(window);
                                    }
                                }
                                NativeEvent::NewWindow(url) => {
                                    if let Some(native) = &view.native {
                                        if normalize_url(&url).is_ok() {
                                            let _ = native.load_url(&url);
                                        }
                                    }
                                }
                            }
                            cx.emit(BrowserEvent::Changed);
                            cx.notify();
                        })
                    })
                    .is_err()
                {
                    break;
                }
            }
        });
        Self {
            native: native.ok(),
            address,
            focus: cx.focus_handle(),
            url,
            title: "Browser".into(),
            loading: true,
            error,
            selected: true,
            allowed: true,
            #[cfg(target_os = "windows")]
            native_bounds: None,
            #[cfg(target_os = "windows")]
            native_visible: false,
            _input_subscription: subscription,
            _native_task: task,
            _creation_task: creation_task,
        }
    }

    pub fn navigate(
        &mut self,
        url: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<()> {
        let url = normalize_url(url)?;
        self.native
            .as_ref()
            .context("Browser engine is unavailable")?
            .load_url(&url)?;
        self.url = url.clone();
        self.loading = true;
        self.error = None;
        self.address
            .update(cx, |input, cx| input.set_value(&url, window, cx));
        cx.emit(BrowserEvent::Changed);
        cx.notify();
        Ok(())
    }

    pub fn evaluate(&self, expression: &str) -> Result<flume::Receiver<String>> {
        anyhow::ensure!(!self.loading, "Browser page is loading");
        anyhow::ensure!(
            expression.len() <= 64 * 1024,
            "Browser expression exceeds 64 KiB"
        );
        let expression = serde_json::to_string(expression)?;
        let script = format!(
            "(() => {{ try {{ const value = (0, eval)({expression}); if (value && typeof value.then === 'function') throw new Error('Use a synchronous JavaScript expression'); return JSON.stringify({{ok:true,value:value === undefined ? null : value}}); }} catch (e) {{ return JSON.stringify({{ok:false,error:String(e)}}); }} }})()"
        );
        let (sender, receiver) = flume::bounded(1);
        self.native
            .as_ref()
            .context("Browser engine is unavailable")?
            .evaluate_script_with_callback(&script, move |result| {
                trace_browser("evaluation callback");
                let _ = sender.try_send(result);
            })?;
        trace_browser("evaluation dispatched");
        Ok(receiver)
    }

    pub fn action(&self, action: &str) -> Result<()> {
        let script = match action {
            "back" => "history.back()",
            "forward" => "history.forward()",
            "reload" => "location.reload()",
            "stop" => "window.stop()",
            _ => anyhow::bail!("Unknown browser action"),
        };
        self.native
            .as_ref()
            .context("Browser engine is unavailable")?
            .evaluate_script(script)?;
        Ok(())
    }

    pub fn set_visibility(&mut self, selected: bool, allowed: bool) {
        self.selected = selected;
        self.allowed = allowed;
        #[cfg(target_os = "windows")]
        let visible = selected
            && allowed
            && self
                .native_bounds
                .is_some_and(|bounds| bounds.size.width > px(0.0) && bounds.size.height > px(0.0));
        #[cfg(not(target_os = "windows"))]
        let visible = selected && allowed;
        self.sync_native_visibility(visible);
    }

    fn sync_native_visibility(&mut self, visible: bool) {
        #[cfg(target_os = "windows")]
        if self.native_visible == visible {
            return;
        }
        let Some(native) = &self.native else {
            return;
        };
        // Record the transition before calling a native API that may deliver
        // synchronous window messages, so a re-entrant layout cannot repeat it.
        #[cfg(target_os = "windows")]
        {
            self.native_visible = visible;
        }
        let _ = native.set_visible(visible);
    }

    fn layout_native(&mut self, bounds: Bounds<Pixels>) {
        #[cfg(target_os = "windows")]
        if self.native_bounds == Some(bounds) {
            return;
        }
        if let Some(native) = &self.native {
            #[cfg(target_os = "windows")]
            {
                self.native_bounds = Some(bounds);
            }
            let _ = native.set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(
                    f64::from(bounds.origin.x),
                    f64::from(bounds.origin.y),
                )
                .into(),
                size: wry::dpi::LogicalSize::new(
                    f64::from(bounds.size.width),
                    f64::from(bounds.size.height),
                )
                .into(),
            });
            self.sync_native_visibility(
                self.selected
                    && self.allowed
                    && bounds.size.width > px(0.0)
                    && bounds.size.height > px(0.0),
            );
        }
    }

    pub fn summary(&self) -> Value {
        json!({"url":self.url,"title":self.title,"loading":self.loading,"error":self.error,"visible":self.selected && self.allowed})
    }
}

fn trace_browser(event: &str) {
    if std::env::var_os("RUMUX_TERMINAL_TRACE").is_some() {
        eprintln!("browser {event}");
    }
}

// Wry's GTK host accepts an Xlib XID, while GPUI owns that same X11 window
// through XCB. The window identifier is shared by both X11 client libraries.
struct BrowserParent<'a>(&'a Window);
impl HasWindowHandle for BrowserParent<'_> {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let raw = HasWindowHandle::window_handle(self.0)?.as_raw();
        #[cfg(target_os = "linux")]
        let raw = match raw {
            RawWindowHandle::Xcb(handle) => {
                let mut xlib = raw_window_handle::XlibWindowHandle::new(handle.window.get().into());
                xlib.visual_id = handle.visual_id.map_or(0, |id| id.get().into());
                RawWindowHandle::Xlib(xlib)
            }
            raw => raw,
        };
        // The borrowed GPUI window retains the native parent for this lifetime.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

#[cfg(not(target_os = "windows"))]
fn create_native(
    url: &str,
    sender: flume::Sender<NativeEvent>,
    window: &Window,
    cx: &mut App,
) -> Result<WebView> {
    let handle = HasWindowHandle::window_handle(window)?;
    anyhow::ensure!(
        matches!(
            handle.as_raw(),
            RawWindowHandle::AppKit(_)
                | RawWindowHandle::Win32(_)
                | RawWindowHandle::Xlib(_)
                | RawWindowHandle::Xcb(_)
        ),
        "Embedded browsers require Linux X11/XWayland, macOS or Windows"
    );
    #[cfg(target_os = "linux")]
    {
        if !gtk::is_initialized_main_thread() {
            gtk::gdk::set_allowed_backends("x11");
            gtk::init().context("Failed to initialize GTK for the embedded browser")?;
        }
    }
    let parent = BrowserParent(window);
    cx.update_global::<BrowserRuntime, _>(|runtime, _| {
        let context = runtime.0.get_or_insert_with(|| {
            WebContext::new(Some(
                rumux_core::runtime::rumux_config_dir().join("browser"),
            ))
        });
        build_native(url, sender, &parent, context)
    })
}

fn build_native(
    url: &str,
    sender: flume::Sender<NativeEvent>,
    parent: &impl HasWindowHandle,
    context: &mut WebContext,
) -> Result<WebView> {
    let titles = sender.clone();
    let pages = sender.clone();
    let focus = sender.clone();
    WebViewBuilder::new_with_web_context(context)
            .with_url(url)
            .with_visible(false)
            .with_initialization_script("document.addEventListener('pointerdown', () => window.ipc.postMessage('rumux:focus'), {capture:true});")
            .with_ipc_handler(move |request| {
                if request.body() == "rumux:focus" { let _ = focus.try_send(NativeEvent::Focus); }
            })
            .with_document_title_changed_handler(move |title| {
                let _ = titles.try_send(NativeEvent::Title(title));
            })
            .with_on_page_load_handler(move |event, url| {
                let _ = pages.try_send(NativeEvent::Page(event, url));
            })
            .with_navigation_handler(|url| normalize_url(&url).is_ok())
            .with_new_window_req_handler(move |url, _| {
                let _ = sender.try_send(NativeEvent::NewWindow(url));
                wry::NewWindowResponse::Deny
            })
            .build_as_child(parent)
            .context("Failed to create native browser child view")
}

impl Drop for BrowserView {
    fn drop(&mut self) {
        if let Some(native) = &self.native {
            let _ = native.set_visible(false);
        }
    }
}

impl Focusable for BrowserView {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for BrowserView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let view = cx.entity();
        let mut toolbar = div().flex().items_center().gap(px(4.0)).p(px(6.0));
        for (id, icon, tooltip, action) in [
            ("browser-back", IconName::ArrowLeft, "Back", "back"),
            (
                "browser-forward",
                IconName::ArrowRight,
                "Forward",
                "forward",
            ),
            ("browser-reload", IconName::Redo, "Reload", "reload"),
        ] {
            toolbar = toolbar.child(
                Button::new(id)
                    .ghost()
                    .compact()
                    .icon(icon)
                    .tooltip(tooltip)
                    .on_click(cx.listener(move |view, _, _, cx| {
                        if let Err(error) = view.action(action) {
                            view.error = Some(error.to_string());
                            cx.notify();
                        }
                    })),
            );
        }
        toolbar = toolbar.child(div().flex_1().child(Input::new(&self.address).small()));
        let content = if let Some(error) = &self.error {
            let error = error.clone();
            self.sync_native_visibility(false);
            div()
                .flex_1()
                .p(px(20.0))
                .text_color(rgb(crate::theme::TEXT_SECONDARY))
                .child(error)
                .into_any_element()
        } else {
            canvas(
                move |bounds, _, cx| view.update(cx, |view, _| view.layout_native(bounds)),
                |_, _, _, _| {},
            )
            .size_full()
            .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .track_focus(&self.focus)
            .bg(rgb(crate::theme::BG_PRIMARY))
            .child(toolbar)
            .child(div().flex_1().min_h(px(0.0)).child(content))
    }
}

pub fn decode_evaluation(result: &str) -> Result<Value> {
    anyhow::ensure!(
        result.len() <= 4 * 1024 * 1024,
        "Browser result exceeded 4 MiB"
    );
    let value: Value = serde_json::from_str(result).context("Browser returned invalid JSON")?;
    let value = if let Value::String(encoded) = value {
        serde_json::from_str(&encoded)?
    } else {
        value
    };
    anyhow::ensure!(
        value.get("ok") == Some(&Value::Bool(true)),
        "{}",
        value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("Browser evaluation failed")
    );
    Ok(value.get("value").cloned().unwrap_or(Value::Null))
}

#[cfg(test)]
mod tests {
    use super::{decode_evaluation, normalize_url};
    use serde_json::json;
    #[test]
    fn browser_urls_accept_local_development_and_reject_host_commands() {
        assert_eq!(
            normalize_url("localhost:3000/test").unwrap(),
            "http://localhost:3000/test"
        );
        assert_eq!(
            normalize_url("example.com").unwrap(),
            "https://example.com/"
        );
        assert_eq!(normalize_url("about:blank").unwrap(), "about:blank");
        assert!(normalize_url("file:///etc/passwd").is_err());
        assert!(normalize_url("javascript://alert(1)").is_err());
    }
    #[test]
    fn evaluation_handles_serialized_values_and_exceptions() {
        assert_eq!(
            decode_evaluation(r#""{\"ok\":true,\"value\":42}""#).unwrap(),
            json!(42)
        );
        assert!(decode_evaluation(r#"{"ok":false,"error":"ReferenceError"}"#).is_err());
    }
}
