use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant};

use eframe::egui;

use crate::candidate::Candidate;
use crate::fzf::{
    FzfBackend, FzfConfig, FzfError, QueryCancellation, QueryRequest, resolve_binary_path,
};
use crate::modes::Mode;

const HEADER_HEIGHT: f32 = 54.0;
const MODE_BADGE_WIDTH: f32 = 96.0;

pub struct LauncherOptions {
    pub mode_name: String,
    pub mode: Box<dyn Mode>,
    pub fzf_config: FzfConfig,
    pub limit: usize,
    pub debug: bool,
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
    should_focus_input: bool,
    centered: bool,
    scroll_to_selected: bool,
}

impl LauncherApp {
    fn new(options: LauncherOptions) -> Self {
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
        let visible = all_candidates.iter().take(options.limit).cloned().collect();

        Self {
            mode_name: options.mode_name,
            mode: options.mode,
            fzf_config: options.fzf_config,
            all_candidates,
            visible,
            query: String::new(),
            selected: 0,
            limit: options.limit,
            generation: 0,
            active_query: None,
            pending: None,
            last_error,
            debug: options.debug,
            last_query_started: None,
            should_focus_input: true,
            centered: false,
            scroll_to_selected: false,
        }
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
        if ctx.input(|input| input.key_pressed(egui::Key::Escape)) {
            self.cancel_pending_query();
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctx.input(|input| input.key_pressed(egui::Key::ArrowDown)) && !self.visible.is_empty() {
            self.selected = (self.selected + 1).min(self.visible.len() - 1);
            self.scroll_to_selected = true;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::ArrowUp)) && !self.visible.is_empty() {
            self.selected = self.selected.saturating_sub(1);
            self.scroll_to_selected = true;
        }
        if ctx.input(|input| input.key_pressed(egui::Key::Enter)) {
            self.execute_selected(ctx);
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
        apply_style(&ctx);

        let panel_frame = egui::Frame::new()
            .fill(egui::Color32::from_rgb(22, 24, 28))
            .inner_margin(egui::Margin::same(16));

        egui::CentralPanel::default()
            .frame(panel_frame)
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(MODE_BADGE_WIDTH, HEADER_HEIGHT),
                        egui::Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            egui::Frame::new()
                                .fill(egui::Color32::from_rgb(28, 33, 42))
                                .corner_radius(egui::CornerRadius::same(8))
                                .inner_margin(egui::Margin::symmetric(12, 8))
                                .show(ui, |ui| {
                                    ui.set_min_size(egui::vec2(MODE_BADGE_WIDTH, HEADER_HEIGHT));
                                    ui.centered_and_justified(|ui| {
                                        ui.label(
                                            egui::RichText::new(&self.mode_name)
                                                .size(21.0)
                                                .color(egui::Color32::from_rgb(138, 180, 248))
                                                .strong(),
                                        );
                                    });
                                });
                        },
                    );
                    ui.add_space(12.0);
                    let input = ui.add_sized(
                        [ui.available_width(), HEADER_HEIGHT],
                        egui::TextEdit::singleline(&mut self.query)
                            .font(egui::FontId::new(20.0, egui::FontFamily::Proportional))
                            .hint_text("Search")
                            .margin(egui::Margin::symmetric(14, 8))
                            .background_color(egui::Color32::from_rgb(16, 18, 22))
                            .text_color(egui::Color32::from_rgb(239, 241, 245))
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

                ui.add_space(12.0);
                if let Some(error) = &self.last_error {
                    ui.colored_label(egui::Color32::from_rgb(255, 138, 128), error);
                    return;
                }

                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if self.visible.is_empty() {
                            ui.add_space(24.0);
                            ui.centered_and_justified(|ui| {
                                ui.label(
                                    egui::RichText::new("No matches")
                                        .size(16.0)
                                        .color(egui::Color32::from_rgb(166, 173, 186)),
                                );
                            });
                            return;
                        }

                        for index in 0..self.limit {
                            let Some(candidate) = self.visible.get(index) else {
                                break;
                            };
                            let selected = index == self.selected;
                            let row_height = 40.0;
                            let (row_rect, row_response) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), row_height),
                                egui::Sense::click(),
                            );
                            let background = if selected {
                                egui::Color32::from_rgb(47, 75, 118)
                            } else if row_response.hovered() {
                                egui::Color32::from_rgb(38, 44, 54)
                            } else {
                                egui::Color32::from_rgb(22, 24, 28)
                            };
                            ui.painter().rect_filled(
                                row_rect,
                                egui::CornerRadius::same(0),
                                background,
                            );
                            let inner_rect = row_rect.shrink2(egui::vec2(14.0, 8.0));
                            ui.scope_builder(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        egui::RichText::new(&candidate.primary)
                                            .size(19.0)
                                            .color(egui::Color32::from_rgb(239, 241, 245)),
                                    );
                                    if let Some(secondary) = &candidate.secondary
                                        && !secondary.is_empty()
                                    {
                                        ui.add_space(8.0);
                                        ui.label(
                                            egui::RichText::new(secondary)
                                                .size(14.0)
                                                .color(egui::Color32::from_rgb(166, 173, 186)),
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
                            ui.add_space(2.0);
                        }
                    });
            });
    }
}

fn apply_style(ctx: &egui::Context) {
    let mut style = (*ctx.global_style()).clone();
    style.spacing.item_spacing = egui::vec2(8.0, 6.0);
    style.text_styles.insert(
        egui::TextStyle::Heading,
        egui::FontId::new(22.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Body,
        egui::FontId::new(16.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Button,
        egui::FontId::new(16.0, egui::FontFamily::Proportional),
    );
    style.text_styles.insert(
        egui::TextStyle::Small,
        egui::FontId::new(12.0, egui::FontFamily::Proportional),
    );
    style.visuals.override_text_color = Some(egui::Color32::from_rgb(239, 241, 245));
    style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(35, 38, 46);
    style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(47, 75, 118);
    style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(38, 44, 54);
    style.visuals.selection.bg_fill = egui::Color32::from_rgb(47, 75, 118);
    ctx.set_global_style(style);
}

struct QueryResult {
    generation: u64,
    result: Result<crate::fzf::QueryResponse, crate::fzf::FzfError>,
}
