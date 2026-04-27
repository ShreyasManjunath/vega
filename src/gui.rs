use std::sync::Arc;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::candidate::Candidate;
use crate::config::KeybindingsConfig;
use crate::fzf::{
    FzfBackend, FzfConfig, FzfError, QueryCancellation, QueryRequest, resolve_binary_path,
};
use crate::modes::Mode;
use crate::settings::{ResolvedSettings, SettingsManager};
use crate::theme::Theme;

pub struct LauncherOptions {
    pub mode_name: String,
    pub mode: Box<dyn Mode>,
    pub fzf_config: FzfConfig,
    pub debug: bool,
    pub settings_manager: SettingsManager,
}

pub fn run_launcher(options: LauncherOptions) -> Result<(), String> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("vega")
            .with_inner_size([760.0, 460.0])
            .with_resizable(false)
            .with_decorations(false)
            .with_transparent(false)
            .with_window_level(egui::WindowLevel::AlwaysOnTop),
        ..Default::default()
    };

    eframe::run_native(
        "vega",
        native_options,
        Box::new(move |_cc| Ok(Box::new(LauncherApp::new(options)))),
    )
    .map_err(|error| error.to_string())
}

struct LauncherApp {
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
    pending: Option<Receiver<QueryResult>>,
    last_error: Option<String>,
    debug: bool,
    last_query_started: Option<Instant>,
    last_settings_poll: Instant,
    should_focus_input: bool,
    centered: bool,
    scroll_to_selected: bool,
    keybindings: ResolvedKeybindings,
}

impl LauncherApp {
    fn new(options: LauncherOptions) -> Self {
        let settings = options.settings_manager.current();
        let limit = settings.config.runtime.limit;
        let keybindings = resolve_keybindings(&settings.config.keybindings);
        if options.debug {
            let resolved = resolve_binary_path(&options.fzf_config.binary)
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "<not found on PATH>".to_string());
            eprintln!(
                "vega: gui fzf binary={} resolved={}",
                options.fzf_config.binary, resolved
            );
        }
        let (all_candidates, last_error) = match options.mode.load() {
            Ok(candidates) => (candidates, None),
            Err(error) => (Vec::new(), Some(error.to_string())),
        };
        let visible = all_candidates.iter().take(limit).cloned().collect();

        Self {
            mode_name: options.mode_name,
            mode: options.mode,
            settings_manager: options.settings_manager,
            settings,
            fzf_config: options.fzf_config,
            all_candidates,
            visible,
            query: String::new(),
            selected: 0,
            limit,
            generation: 0,
            active_query: None,
            pending: None,
            last_error,
            debug: options.debug,
            last_query_started: None,
            last_settings_poll: Instant::now(),
            should_focus_input: true,
            centered: false,
            scroll_to_selected: false,
            keybindings,
        }
    }

    fn apply_settings(&mut self, settings: Arc<ResolvedSettings>, ctx: &egui::Context) {
        self.settings = settings;
        self.limit = self.settings.config.runtime.limit;
        self.debug = self.debug || self.settings.config.runtime.debug;
        self.keybindings = resolve_keybindings(&self.settings.config.keybindings);
        self.fzf_config.binary = self.settings.config.runtime.fzf_binary.clone();
        self.fzf_config.timeout = Duration::from_millis(self.settings.config.runtime.timeout_ms);
        self.fzf_config.extra_flags = self.settings.config.runtime.fzf_flags.clone();

        if self.query.is_empty() {
            self.visible = self
                .all_candidates
                .iter()
                .take(self.limit)
                .cloned()
                .collect();
        } else {
            self.start_query(ctx);
        }
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }

    fn cancel_pending_query(&mut self) {
        if let Some(active_query) = self.active_query.take() {
            active_query.cancel();
        }
        self.pending = None;
        self.last_query_started = None;
    }

    fn start_query(&mut self, ctx: &egui::Context) {
        self.cancel_pending_query();
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        let query = self.query.clone();
        let candidates = self.all_candidates.clone();
        let config = self.fzf_config.clone();
        let limit = self.limit;
        let cancellation = QueryCancellation::default();
        let (tx, rx) = mpsc::channel();
        self.active_query = Some(cancellation.clone());
        self.pending = Some(rx);
        self.last_query_started = Some(Instant::now());

        let repaint = ctx.clone();
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
            repaint.request_repaint();
        });
    }

    fn apply_query_results(&mut self) {
        let Some(receiver) = &self.pending else {
            return;
        };
        let Ok(result) = receiver.try_recv() else {
            return;
        };
        if result.generation != self.generation {
            return;
        }

        match result.result {
            Ok(response) => {
                self.active_query = None;
                self.visible = response
                    .matches
                    .into_iter()
                    .map(|matched| matched.candidate)
                    .collect();
                self.selected = self.selected.min(self.visible.len().saturating_sub(1));
                self.last_error = None;
                if self.debug {
                    eprintln!(
                        "vega: gui mode={} candidates={} results={} elapsed_ms={}",
                        self.mode_name,
                        response.candidate_count,
                        self.visible.len(),
                        response.elapsed.as_millis()
                    );
                }
            }
            Err(FzfError::Cancelled) => {
                self.active_query = None;
            }
            Err(error) => {
                self.active_query = None;
                self.visible.clear();
                self.selected = 0;
                self.last_error = Some(error.to_string());
            }
        }
        self.pending = None;
    }

    fn execute_selected(&mut self, ctx: &egui::Context) {
        let Some(candidate) = self.visible.get(self.selected).cloned() else {
            return;
        };
        if let Err(error) = self.mode.execute(&candidate) {
            self.last_error = Some(error.to_string());
            return;
        }
        self.cancel_pending_query();
        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
    }

    fn handle_keys(&mut self, ctx: &egui::Context) {
        if ctx.input(|input| input.key_pressed(self.keybindings.cancel)) {
            self.cancel_pending_query();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|input| input.key_pressed(self.keybindings.select_next))
            && !self.visible.is_empty()
        {
            self.selected = (self.selected + 1).min(self.visible.len() - 1);
            self.scroll_to_selected = true;
        }
        if ctx.input(|input| input.key_pressed(self.keybindings.select_prev))
            && !self.visible.is_empty()
        {
            self.selected = self.selected.saturating_sub(1);
            self.scroll_to_selected = true;
        }
        if ctx.input(|input| input.key_pressed(self.keybindings.submit)) {
            self.execute_selected(ctx);
        }
    }

    fn poll_settings_reload(&mut self, ctx: &egui::Context) {
        if !self.settings.config.behavior.hot_reload {
            return;
        }

        let poll_interval =
            Duration::from_millis(self.settings.config.behavior.poll_interval_ms.max(50));
        if self.last_settings_poll.elapsed() < poll_interval {
            return;
        }
        self.last_settings_poll = Instant::now();

        match self.settings_manager.reload_if_changed() {
            Ok(Some(settings)) => {
                self.apply_settings(settings, ctx);
                self.last_error = None;
                ctx.request_repaint();
            }
            Ok(None) => {}
            Err(error) => {
                self.last_error = Some(error.to_string());
            }
        }
    }
}

impl eframe::App for LauncherApp {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.centered {
            let info = ctx.input(|i| i.viewport().clone());
            if let (Some(monitor), Some(inner)) = (info.monitor_size, info.inner_rect) {
                let x = (monitor.x - inner.width()) / 2.0;
                let y = (monitor.y - inner.height()) / 2.0;
                ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
                self.centered = true;
            }
        }
        self.poll_settings_reload(ctx);
        self.apply_query_results();
        self.handle_keys(ctx);
        if self.pending.is_some()
            && self
                .last_query_started
                .is_some_and(|started| started.elapsed() > Duration::from_millis(80))
        {
            ctx.request_repaint_after(Duration::from_millis(16));
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let settings = Arc::clone(&self.settings);
        let theme = &settings.theme;
        apply_style(&ctx, theme);

        let panel_frame = egui::Frame::new()
            .fill(theme.window_background.to_egui())
            .inner_margin(egui::Margin::same(theme.panel_padding));

        egui::CentralPanel::default()
            .frame(panel_frame)
            .show_inside(ui, |ui| {
                let templates = &settings.templates;
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(theme.badge_width, theme.header_height),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            egui::Frame::new()
                                .fill(theme.badge_background.to_egui())
                                .corner_radius(egui::CornerRadius::same(theme.badge_radius))
                                .inner_margin(egui::Margin::symmetric(
                                    theme.badge_padding_x,
                                    theme.badge_padding_y,
                                ))
                                .show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(
                                        theme.badge_width,
                                        theme.header_height,
                                    ));
                                    ui.centered_and_justified(|ui| {
                                        ui.label(
                                            egui::RichText::new(
                                                templates.render_mode_badge(&self.mode_name),
                                            )
                                            .size(theme.badge_font_size)
                                            .color(theme.badge_foreground.to_egui())
                                            .strong(),
                                        );
                                    });
                                });
                        },
                    );
                    ui.add_space(theme.header_gap);
                    let input = ui.add_sized(
                        [ui.available_width(), theme.header_height],
                        egui::TextEdit::singleline(&mut self.query)
                            .font(egui::FontId::new(
                                theme.input_font_size,
                                egui::FontFamily::Proportional,
                            ))
                            .hint_text("Search")
                            .margin(egui::Margin::symmetric(
                                theme.input_padding_x,
                                theme.input_padding_y,
                            ))
                            .background_color(theme.input_background.to_egui())
                            .text_color(theme.input_foreground.to_egui())
                            .desired_width(f32::INFINITY),
                    );
                    if self.should_focus_input {
                        input.request_focus();
                        self.should_focus_input = false;
                    }
                    if input.changed() {
                        self.selected = 0;
                        if self.query.is_empty() {
                            self.visible = self
                                .all_candidates
                                .iter()
                                .take(self.limit)
                                .cloned()
                                .collect();
                            self.cancel_pending_query();
                        } else {
                            self.start_query(&ctx);
                        }
                    }
                });

                ui.add_space(theme.header_gap);
                if let Some(error) = &self.last_error {
                    ui.colored_label(theme.error_foreground.to_egui(), error);
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.visible.is_empty() {
                            ui.add_space(24.0);
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    egui::RichText::new(templates.render_empty_state(&self.query))
                                        .size(theme.empty_font_size)
                                        .color(theme.empty_foreground.to_egui()),
                                );
                            });
                            return;
                        }

                        for index in 0..self.limit {
                            let Some(candidate) = self.visible.get(index) else {
                                break;
                            };
                            let selected = index == self.selected;
                            let row_height = theme.row_height;
                            let (row_rect, row_response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), row_height),
                                egui::Sense::click(),
                            );
                            let background = if selected {
                                theme.row_selected_background.to_egui()
                            } else if row_response.hovered() {
                                theme.row_hover_background.to_egui()
                            } else {
                                theme.row_background.to_egui()
                            };
                            ui.painter().rect_filled(
                                row_rect,
                                egui::CornerRadius::same(0),
                                background,
                            );
                            let inner_rect = row_rect
                                .shrink2(egui::vec2(theme.row_padding_x, theme.row_padding_y));
                            ui.scope_builder(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
                                ui.horizontal(|ui| {
                                    let primary = templates.render_row_primary(candidate);
                                    ui.label(
                                        egui::RichText::new(primary)
                                            .size(theme.row_primary_font_size)
                                            .color(theme.row_foreground.to_egui()),
                                    );
                                    let secondary = templates.render_row_secondary(candidate);
                                    if !secondary.is_empty() {
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new(secondary)
                                                .size(theme.row_secondary_font_size)
                                                .color(theme.row_secondary_foreground.to_egui()),
                                        );
                                    }
                                });
                            });
                            if selected && self.scroll_to_selected {
                                row_response.scroll_to_me(None);
                                self.scroll_to_selected = false;
                            }
                            if row_response.clicked() {
                                self.selected = index;
                            }
                            if row_response.double_clicked() {
                                self.selected = index;
                                self.execute_selected(&ctx);
                            }
                            ui.add_space(theme.row_gap);
                        }
                    });
            });
    }
}

fn apply_style(ctx: &egui::Context, theme: &Theme) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(theme.item_spacing_x, theme.item_spacing_y);
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(theme.heading_font_size, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(theme.body_font_size, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(theme.button_font_size, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(theme.small_font_size, egui::FontFamily::Proportional),
    );
    style.visuals.override_text_color = Some(theme.row_foreground.to_egui());
    style.visuals.widgets.inactive.bg_fill = theme.input_background.to_egui();
    style.visuals.widgets.active.bg_fill = theme.row_selected_background.to_egui();
    style.visuals.widgets.hovered.bg_fill = theme.row_hover_background.to_egui();
    style.visuals.selection.bg_fill = theme.row_selected_background.to_egui();
    ctx.set_global_style(style);
}

#[derive(Clone, Copy)]
struct ResolvedKeybindings {
    submit: egui::Key,
    cancel: egui::Key,
    select_next: egui::Key,
    select_prev: egui::Key,
}

fn resolve_keybindings(config: &KeybindingsConfig) -> ResolvedKeybindings {
    let defaults = KeybindingsConfig::default();
    ResolvedKeybindings {
        submit: parse_key(&config.submit).unwrap_or(parse_key(&defaults.submit).expect("Enter")),
        cancel: parse_key(&config.cancel).unwrap_or(parse_key(&defaults.cancel).expect("Escape")),
        select_next: parse_key(&config.select_next)
            .unwrap_or(parse_key(&defaults.select_next).expect("ArrowDown")),
        select_prev: parse_key(&config.select_prev)
            .unwrap_or(parse_key(&defaults.select_prev).expect("ArrowUp")),
    }
}

fn parse_key(name: &str) -> Option<egui::Key> {
    match name.trim().to_ascii_lowercase().as_str() {
        "arrowdown" | "down" | "j" => Some(egui::Key::ArrowDown),
        "arrowup" | "up" | "k" => Some(egui::Key::ArrowUp),
        "escape" | "esc" => Some(egui::Key::Escape),
        "enter" | "return" => Some(egui::Key::Enter),
        _ => None,
    }
}

struct QueryResult {
    generation: u64,
    result: Result<crate::fzf::QueryResponse, crate::fzf::FzfError>,
}
