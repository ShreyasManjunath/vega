use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use gtk4::gdk;
use gtk4::gio;
use gtk4::glib;
use gtk4::pango;
use gtk4::prelude::*;
use gtk4::{
    Application, ApplicationWindow, Box as GtkBox, CssProvider, Entry, Label, ListBox, ListBoxRow,
    Orientation, PolicyType, ScrolledWindow, SelectionMode,
};

use crate::candidate::Candidate;
use crate::fzf::{
    FzfBackend, FzfConfig, FzfError, QueryCancellation, QueryRequest, resolve_binary_path,
};
use crate::modes::Mode;
use crate::settings::{ResolvedSettings, SettingsManager};
use crate::template::TemplateSet;
use crate::theme::Theme;

const WIN_WIDTH: i32 = 760;
const WIN_HEIGHT: i32 = 460;

pub struct LauncherOptions {
    pub mode_name: String,
    pub mode: Box<dyn Mode>,
    pub fzf_config: FzfConfig,
    pub debug: bool,
    pub settings_manager: SettingsManager,
}

struct QueryResult {
    generation: u64,
    result: Result<crate::fzf::QueryResponse, FzfError>,
}

struct AppState {
    mode_name: String,
    mode: Box<dyn Mode>,
    settings_manager: SettingsManager,
    settings: Arc<ResolvedSettings>,
    fzf_config: FzfConfig,
    all_candidates: Vec<Candidate>,
    visible: Vec<Candidate>,
    query: String,
    selected: usize,
    limit: usize,
    generation: u64,
    active_query: Option<QueryCancellation>,
    tx: mpsc::Sender<QueryResult>,
    last_error: Option<String>,
    debug: bool,
    last_settings_poll: Instant,
}

impl AppState {
    fn cancel_active_query(&mut self) {
        if let Some(q) = self.active_query.take() {
            q.cancel();
        }
    }

    fn start_query(&mut self) {
        self.cancel_active_query();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let query = self.query.clone();
        let candidates = self.all_candidates.clone();
        let config = self.fzf_config.clone();
        let limit = self.limit;
        let cancellation = QueryCancellation::default();
        self.active_query = Some(cancellation.clone());
        let tx = self.tx.clone();

        thread::spawn(move || {
            let result = FzfBackend::start(config).and_then(|backend| {
                backend.query_with_cancellation(
                    QueryRequest {
                        query,
                        candidates,
                        limit,
                    },
                    Some(&cancellation),
                )
            });
            let _ = tx.send(QueryResult { generation, result });
        });
    }
}

pub fn run_launcher(options: LauncherOptions) -> Result<(), String> {
    let app = Application::builder()
        .application_id("io.github.vega.launcher")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();

    let options_cell = Rc::new(RefCell::new(Some(options)));
    app.connect_activate(move |app| {
        if let Some(opts) = options_cell.borrow_mut().take() {
            build_ui(app, opts);
        }
    });

    let status = app.run_with_args::<&str>(&[]);
    if status == glib::ExitCode::SUCCESS {
        Ok(())
    } else {
        Err("gtk application failed".to_string())
    }
}

fn build_ui(app: &Application, options: LauncherOptions) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("vega")
        .default_width(WIN_WIDTH)
        .default_height(WIN_HEIGHT)
        .decorated(false)
        .resizable(false)
        .build();

    setup_layer_shell(&window);
    setup_centering(&window);

    let settings = options.settings_manager.current();
    let debug = options.debug || settings.config.runtime.debug;
    let limit = settings.config.runtime.limit;

    if debug {
        let resolved = resolve_binary_path(&options.fzf_config.binary)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<not found on PATH>".to_string());
        eprintln!(
            "vega: gui fzf binary={} resolved={}",
            options.fzf_config.binary, resolved
        );
    }

    let (all_candidates, initial_error) = match options.mode.load() {
        Ok(candidates) => (candidates, None),
        Err(e) => (Vec::new(), Some(e.to_string())),
    };
    let visible: Vec<Candidate> = all_candidates.iter().take(limit).cloned().collect();

    let (tx, rx) = mpsc::channel::<QueryResult>();
    let rx = Rc::new(RefCell::new(rx));

    let mode_badge_text = settings.templates.render_mode_badge(&options.mode_name);

    let state = Rc::new(RefCell::new(AppState {
        mode_name: options.mode_name,
        mode: options.mode,
        settings_manager: options.settings_manager,
        settings: Arc::clone(&settings),
        fzf_config: options.fzf_config,
        all_candidates,
        visible,
        query: String::new(),
        selected: 0,
        limit,
        generation: 0,
        active_query: None,
        tx,
        last_error: initial_error,
        debug,
        last_settings_poll: Instant::now(),
    }));

    // CSS provider applied to all widgets on this display
    let css = CssProvider::new();
    refresh_css(&css, &settings.theme);
    gtk4::style_context_add_provider_for_display(
        &gtk4::prelude::WidgetExt::display(&window),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    // ── Layout ───────────────────────────────────────────────────────────────

    let root = GtkBox::new(Orientation::Vertical, 0);
    root.add_css_class("vega-root");

    let header = GtkBox::new(Orientation::Horizontal, 0);
    header.add_css_class("vega-header");

    let badge = Label::new(Some(&mode_badge_text));
    badge.add_css_class("vega-badge");
    header.append(&badge);

    let entry = Entry::new();
    entry.set_placeholder_text(Some("Search"));
    entry.add_css_class("vega-entry");
    entry.set_hexpand(true);
    header.append(&entry);

    root.append(&header);

    let error_label = Label::new(None);
    error_label.add_css_class("vega-error");
    error_label.set_visible(false);
    error_label.set_halign(gtk4::Align::Start);
    root.append(&error_label);

    let scroll = ScrolledWindow::new();
    scroll.set_vexpand(true);
    scroll.set_hscrollbar_policy(PolicyType::Never);
    scroll.set_vscrollbar_policy(PolicyType::Automatic);

    let list = ListBox::new();
    list.set_selection_mode(SelectionMode::Single);
    list.add_css_class("vega-list");
    scroll.set_child(Some(&list));
    root.append(&scroll);

    window.set_child(Some(&root));

    // Initial population
    {
        let s = state.borrow();
        repopulate(&list, &s.visible, s.selected, &settings.templates);
        if let Some(ref err) = s.last_error {
            error_label.set_text(err);
            error_label.set_visible(true);
        }
    }

    // ── Entry changed ────────────────────────────────────────────────────────

    {
        let state = Rc::clone(&state);
        let list = list.clone();
        let error_label = error_label.clone();
        entry.connect_changed(move |e| {
            let text = e.text().to_string();
            error_label.set_visible(false);

            if text.is_empty() {
                let visible_snap = {
                    let mut s = state.borrow_mut();
                    s.query = String::new();
                    s.selected = 0;
                    s.cancel_active_query();
                    s.visible = s.all_candidates.iter().take(s.limit).cloned().collect();
                    s.visible.clone()
                };
                let settings = Arc::clone(&state.borrow().settings);
                repopulate(&list, &visible_snap, 0, &settings.templates);
            } else {
                let mut s = state.borrow_mut();
                s.query = text;
                s.selected = 0;
                s.start_query();
            }
        });
    }

    // ── Key handling (Capture phase — before entry sees keys) ─────────────────

    {
        let state = Rc::clone(&state);
        let list = list.clone();
        let scroll = scroll.clone();
        let window_ref = window.clone();
        let error_label = error_label.clone();
        let ctrl = gtk4::EventControllerKey::new();
        ctrl.set_propagation_phase(gtk4::PropagationPhase::Capture);
        ctrl.connect_key_pressed(move |_, key, _, _| {
            let (cancel, submit, next, prev) = {
                let s = state.borrow();
                let kb = &s.settings.config.keybindings;
                (
                    parse_key(&kb.cancel),
                    parse_key(&kb.submit),
                    parse_key(&kb.select_next),
                    parse_key(&kb.select_prev),
                )
            };

            if key == cancel {
                state.borrow_mut().cancel_active_query();
                window_ref.close();
                return glib::Propagation::Stop;
            }

            if key == submit {
                let exec = {
                    let mut s = state.borrow_mut();
                    if let Some(candidate) = s.visible.get(s.selected).cloned() {
                        let result = s.mode.execute(&candidate);
                        if result.is_ok() {
                            s.cancel_active_query();
                        }
                        Some(result.map_err(|e| e.to_string()))
                    } else {
                        None
                    }
                };
                match exec {
                    Some(Ok(())) => window_ref.close(),
                    Some(Err(msg)) => {
                        error_label.set_text(&msg);
                        error_label.set_visible(true);
                    }
                    None => {}
                }
                return glib::Propagation::Stop;
            }

            if key == next {
                let new_sel = {
                    let mut s = state.borrow_mut();
                    if s.visible.is_empty() {
                        return glib::Propagation::Stop;
                    }
                    s.selected = (s.selected + 1).min(s.visible.len() - 1);
                    s.selected
                };
                if let Some(row) = list.row_at_index(new_sel as i32) {
                    list.select_row(Some(&row));
                    scroll_into_view(&scroll, &list, &row);
                }
                return glib::Propagation::Stop;
            }

            if key == prev {
                let new_sel = {
                    let mut s = state.borrow_mut();
                    if s.visible.is_empty() {
                        return glib::Propagation::Stop;
                    }
                    s.selected = s.selected.saturating_sub(1);
                    s.selected
                };
                if let Some(row) = list.row_at_index(new_sel as i32) {
                    list.select_row(Some(&row));
                    scroll_into_view(&scroll, &list, &row);
                }
                return glib::Propagation::Stop;
            }

            glib::Propagation::Proceed
        });
        window.add_controller(ctrl);
    }

    // ── Row selected → sync index ─────────────────────────────────────────────

    {
        let state = Rc::clone(&state);
        list.connect_row_selected(move |_, row_opt| {
            if let Some(row) = row_opt {
                state.borrow_mut().selected = row.index() as usize;
            }
        });
    }

    // ── Row activated (double-click / Enter on row) → execute ─────────────────

    {
        let state = Rc::clone(&state);
        let window_ref = window.clone();
        let error_label = error_label.clone();
        list.connect_row_activated(move |_, row| {
            let exec = {
                let mut s = state.borrow_mut();
                s.selected = row.index() as usize;
                if let Some(candidate) = s.visible.get(s.selected).cloned() {
                    let result = s.mode.execute(&candidate);
                    if result.is_ok() {
                        s.cancel_active_query();
                    }
                    Some(result.map_err(|e| e.to_string()))
                } else {
                    None
                }
            };
            match exec {
                Some(Ok(())) => window_ref.close(),
                Some(Err(msg)) => {
                    error_label.set_text(&msg);
                    error_label.set_visible(true);
                }
                None => {}
            }
        });
    }

    // ── Poll query results + settings hot-reload (combined timer) ─────────────

    {
        let state = Rc::clone(&state);
        let css = css.clone();
        let list = list.clone();
        let error_label = error_label.clone();
        let badge = badge.clone();
        glib::timeout_add_local(Duration::from_millis(16), move || {
            // — Query results —
            while let Ok(result) = rx.borrow().try_recv() {
                let mut s = state.borrow_mut();
                if result.generation != s.generation {
                    continue;
                }
                match result.result {
                    Ok(response) => {
                        s.active_query = None;
                        if s.debug {
                            eprintln!(
                                "vega: gui mode={} candidates={} results={} elapsed_ms={}",
                                s.mode_name,
                                response.candidate_count,
                                response.matches.len(),
                                response.elapsed.as_millis()
                            );
                        }
                        s.visible = response.matches.into_iter().map(|m| m.candidate).collect();
                        s.selected = s.selected.min(s.visible.len().saturating_sub(1));
                        s.last_error = None;
                        let visible_snap = s.visible.clone();
                        let selected = s.selected;
                        let settings = Arc::clone(&s.settings);
                        drop(s);
                        repopulate(&list, &visible_snap, selected, &settings.templates);
                        error_label.set_visible(false);
                    }
                    Err(FzfError::Cancelled) => {
                        s.active_query = None;
                    }
                    Err(e) => {
                        s.active_query = None;
                        s.visible.clear();
                        s.selected = 0;
                        let msg = e.to_string();
                        drop(s);
                        let settings = Arc::clone(&state.borrow().settings);
                        repopulate(&list, &[], 0, &settings.templates);
                        error_label.set_text(&msg);
                        error_label.set_visible(true);
                    }
                }
            }

            // — Settings hot-reload —
            let (hot_reload, poll_interval, elapsed) = {
                let s = state.borrow();
                (
                    s.settings.config.behavior.hot_reload,
                    Duration::from_millis(s.settings.config.behavior.poll_interval_ms.max(50)),
                    s.last_settings_poll.elapsed(),
                )
            };
            if hot_reload && elapsed >= poll_interval {
                state.borrow_mut().last_settings_poll = Instant::now();

                let reload = {
                    let mut s = state.borrow_mut();
                    s.settings_manager.reload_if_changed()
                };

                match reload {
                    Ok(None) => {}
                    Ok(Some(new_settings)) => {
                        let (visible_snap, selected) = {
                            let mut s = state.borrow_mut();
                            s.settings = Arc::clone(&new_settings);
                            s.limit = new_settings.config.runtime.limit;
                            s.debug = s.debug || new_settings.config.runtime.debug;
                            s.fzf_config.binary = new_settings.config.runtime.fzf_binary.clone();
                            s.fzf_config.timeout =
                                Duration::from_millis(new_settings.config.runtime.timeout_ms);
                            s.fzf_config.extra_flags =
                                new_settings.config.runtime.fzf_flags.clone();
                            refresh_css(&css, &new_settings.theme);
                            badge.set_text(&new_settings.templates.render_mode_badge(&s.mode_name));
                            if s.query.is_empty() {
                                s.visible =
                                    s.all_candidates.iter().take(s.limit).cloned().collect();
                                s.selected = s.selected.min(s.visible.len().saturating_sub(1));
                            } else {
                                s.start_query();
                            }
                            (s.visible.clone(), s.selected)
                        };
                        let settings = Arc::clone(&state.borrow().settings);
                        repopulate(&list, &visible_snap, selected, &settings.templates);
                        error_label.set_visible(false);
                    }
                    Err(e) => {
                        error_label.set_text(&e.to_string());
                        error_label.set_visible(true);
                    }
                }
            }

            glib::ControlFlow::Continue
        });
    }

    entry.grab_focus();
    window.present();
}

// ── Layer-shell ───────────────────────────────────────────────────────────────

#[cfg(feature = "layer-shell")]
fn setup_layer_shell(window: &ApplicationWindow) {
    use gtk4_layer_shell::{KeyboardMode, Layer, LayerShell};
    // is_supported() internally asserts a Wayland display and prints a CRITICAL on X11.
    // Guard with a GDK type name check first so the assertion is never reached.
    let display = gtk4::prelude::WidgetExt::display(window);
    if display.type_().name() != "GdkWaylandDisplay" {
        return;
    }
    if gtk4_layer_shell::is_supported() {
        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(KeyboardMode::Exclusive);
        // No edge anchors → compositor centers the window
    }
}

#[cfg(not(feature = "layer-shell"))]
fn setup_layer_shell(_window: &ApplicationWindow) {}

// ── X11 centering ─────────────────────────────────────────────────────────────
//
// GTK4 removed set_position(). On Wayland the compositor handles placement.
// On X11 we call XMoveWindow after the window is mapped.
// libX11 is linked transitively through GTK4's X11 backend so no extra dep.

#[cfg(feature = "x11")]
fn setup_centering(window: &ApplicationWindow) {
    use gdk4_x11::X11Surface;

    // Symbols live in libgdk-4.so (X11 backend) and libX11.so,
    // both linked transitively through the gdk4-x11 crate.
    unsafe extern "C" {
        fn gdk_x11_display_get_xdisplay(display: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
        fn gdk_x11_surface_get_xid(surface: *mut std::ffi::c_void) -> u64;
        fn XMoveWindow(
            display: *mut std::ffi::c_void,
            w: u64,
            x: std::ffi::c_int,
            y: std::ffi::c_int,
        ) -> std::ffi::c_int;
        fn XFlush(display: *mut std::ffi::c_void) -> std::ffi::c_int;
    }

    let win_clone = window.clone();
    window.connect_map(move |_| {
        let win = win_clone.clone();
        // Defer to idle so the WM has processed MapRequest and placed the window first.
        glib::idle_add_local_once(move || {
            let Some(surface) = win.surface() else { return };

            // downcast_ref succeeds only under X11; silently skip on Wayland
            if surface.downcast_ref::<X11Surface>().is_none() {
                return;
            }

            let display = gtk4::prelude::WidgetExt::display(&win);
            let Some(monitor) = display.monitor_at_surface(&surface) else {
                return;
            };
            let geom = monitor.geometry();
            let x = geom.x() + (geom.width() - WIN_WIDTH) / 2;
            let y = geom.y() + (geom.height() - WIN_HEIGHT) / 2;

            let disp_ptr = display.as_ptr() as *mut std::ffi::c_void;
            let surf_ptr = surface.as_ptr() as *mut std::ffi::c_void;

            unsafe {
                let xdisplay = gdk_x11_display_get_xdisplay(disp_ptr);
                let xwindow = gdk_x11_surface_get_xid(surf_ptr);
                XMoveWindow(xdisplay, xwindow, x, y);
                XFlush(xdisplay);
            }
        });
    });
}

#[cfg(not(feature = "x11"))]
fn setup_centering(_window: &ApplicationWindow) {}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn scroll_into_view(scroll: &ScrolledWindow, list: &ListBox, row: &ListBoxRow) {
    let Some(bounds) = row.compute_bounds(list) else {
        return;
    };
    let adj = scroll.vadjustment();
    let row_top = bounds.y() as f64;
    let row_bot = row_top + bounds.height() as f64;
    let view_top = adj.value();
    let view_bot = view_top + adj.page_size();
    if row_top < view_top {
        adj.set_value(row_top);
    } else if row_bot > view_bot {
        adj.set_value((row_bot - adj.page_size()).max(0.0));
    }
}

fn repopulate(list: &ListBox, candidates: &[Candidate], selected: usize, templates: &TemplateSet) {
    list.remove_all();

    for (i, candidate) in candidates.iter().enumerate() {
        let row_box = GtkBox::new(Orientation::Horizontal, 0);

        let primary_text = templates.render_row_primary(candidate);
        let primary = Label::new(Some(&primary_text));
        primary.add_css_class("vega-primary");
        primary.set_halign(gtk4::Align::Start);
        primary.set_ellipsize(pango::EllipsizeMode::End);
        primary.set_hexpand(true);
        row_box.append(&primary);

        let sec_text = templates.render_row_secondary(candidate);
        if !sec_text.is_empty() {
            let secondary = Label::new(Some(&sec_text));
            secondary.add_css_class("vega-secondary");
            secondary.set_halign(gtk4::Align::End);
            secondary.set_ellipsize(pango::EllipsizeMode::End);
            row_box.append(&secondary);
        }

        let row = ListBoxRow::new();
        row.set_child(Some(&row_box));
        list.append(&row);

        if i == selected {
            list.select_row(Some(&row));
        }
    }
}

// ── CSS ───────────────────────────────────────────────────────────────────────

fn refresh_css(provider: &CssProvider, theme: &Theme) {
    provider.load_from_string(&build_css(theme));
}

fn build_css(t: &Theme) -> String {
    format!(
        r#"
.vega-root {{
    background-color: {win_bg};
    padding: {pad}px;
}}
.vega-header {{
    margin-bottom: {hgap}px;
}}
.vega-badge {{
    background-color: {badge_bg};
    color: {badge_fg};
    border-radius: {badge_r}px;
    padding: {badge_py}px {badge_px}px;
    font-size: {badge_fs}px;
    font-weight: bold;
    min-width: {badge_w}px;
    min-height: {header_h}px;
    margin-right: {hgap}px;
}}
.vega-entry,
.vega-entry text {{
    background-color: {input_bg};
    color: {input_fg};
    font-size: {input_fs}px;
    padding: {input_py}px {input_px}px;
    border: none;
    box-shadow: none;
    outline: none;
    min-height: {header_h}px;
    border-radius: 0;
}}
.vega-entry:focus,
.vega-entry:focus text {{
    background-color: {input_bg};
    box-shadow: none;
    outline: none;
    border: none;
}}
.vega-list {{
    background-color: {row_bg};
    border: none;
    outline: none;
}}
.vega-list row {{
    background-color: {row_bg};
    min-height: {row_h}px;
    padding: {row_py}px {row_px}px;
    border: none;
    outline: none;
    margin-bottom: {row_gap}px;
}}
.vega-list row:hover {{
    background-color: {row_hover};
}}
.vega-list row:selected,
.vega-list row:selected:focus {{
    background-color: {row_sel};
    outline: none;
    box-shadow: none;
}}
.vega-primary {{
    color: {row_fg};
    font-size: {row_pfs}px;
}}
.vega-secondary {{
    color: {row_sfg};
    font-size: {row_sfs}px;
}}
.vega-error {{
    color: {err_fg};
    font-size: 14px;
    padding: 2px 0;
}}
"#,
        win_bg = t.window_background.to_css_color(),
        pad = t.panel_padding,
        hgap = t.header_gap as i32,
        badge_bg = t.badge_background.to_css_color(),
        badge_fg = t.badge_foreground.to_css_color(),
        badge_r = t.badge_radius,
        badge_py = t.badge_padding_y,
        badge_px = t.badge_padding_x,
        badge_fs = t.badge_font_size as u32,
        badge_w = t.badge_width as u32,
        header_h = t.header_height as u32,
        input_bg = t.input_background.to_css_color(),
        input_fg = t.input_foreground.to_css_color(),
        input_fs = t.input_font_size as u32,
        input_py = t.input_padding_y,
        input_px = t.input_padding_x,
        row_bg = t.row_background.to_css_color(),
        row_h = t.row_height as u32,
        row_py = t.row_padding_y as u32,
        row_px = t.row_padding_x as u32,
        row_gap = t.row_gap as u32,
        row_hover = t.row_hover_background.to_css_color(),
        row_sel = t.row_selected_background.to_css_color(),
        row_fg = t.row_foreground.to_css_color(),
        row_pfs = t.row_primary_font_size as u32,
        row_sfg = t.row_secondary_foreground.to_css_color(),
        row_sfs = t.row_secondary_font_size as u32,
        err_fg = t.error_foreground.to_css_color(),
    )
}

fn parse_key(name: &str) -> gdk::Key {
    match name.trim().to_ascii_lowercase().as_str() {
        "arrowdown" | "down" | "j" => gdk::Key::Down,
        "arrowup" | "up" | "k" => gdk::Key::Up,
        "escape" | "esc" => gdk::Key::Escape,
        "enter" | "return" => gdk::Key::Return,
        _ => gdk::Key::VoidSymbol,
    }
}
