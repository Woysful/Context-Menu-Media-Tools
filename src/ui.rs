//! The windows. One eframe window goes through the stages
//! options form -> progress -> result, or shows the Stats text, or the Settings.
//! Drawn by the program itself, so it looks the same on every system and needs no GUI toolkit.
//!
//! A window opens at its final size: before it is created the form is laid out once in an
//! invisible egui context (`measure_form`), so the width follows the information about the file
//! and the height follows the content. Nothing is resized afterwards, the user may resize freely.

use crate::action::Action;
use crate::config::{self, Codec, Config, Custom, Format, Settings};
use crate::effects;
use crate::engine::{self, ConvertKind, Job, LineKind, Progress, Request};
use crate::gpu;
use crate::i18n::{self, T};
use crate::i18n_settings as si;
use crate::media::{self, Info, Kind};
use crate::scale::{self, ScaleMode};
use crate::syspal::{self, Palette};
use crate::theme::{self, bold, c, CONTROL_H};
use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, FontId, Layout, Margin, Rect, RichText, Sense, Stroke, Vec2};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const MARGIN: f32 = 20.0;

/// What the window shows first
pub enum Start {
    /// The options form of the action (or straight to the job with `--yes`)
    Normal,
    /// The Settings window (`mmt settings`, or mmt started without arguments from a file manager)
    Settings,
    /// Text of a command that has no console to print to (Windows)
    Message { title: String, lines: Vec<String>, ok: bool },
}

pub struct Launch {
    pub start: Start,
    pub action: Action,
    /// An own menu item (`mmt run <id>`)
    pub custom_id: Option<String>,
    pub files: Vec<PathBuf>,
    pub cfg: Arc<Config>,
    pub ffmpeg: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
    /// Some = skip the options dialog (`--yes`)
    pub request: Option<Request>,
    pub first_kind: Option<Kind>,
}

pub fn run(l: Launch) -> Result<(), String> {
    let Launch { start, action, custom_id, files, cfg, ffmpeg, ffprobe, request, first_kind } = l;
    let t = i18n::t();
    let infos: Vec<Info> = match (&ffprobe, &start) {
        (Some(p), Start::Normal) => files.iter().take(3).filter_map(|f| media::probe(p, f)).collect(),
        _ => vec![],
    };
    let title = match (&start, &custom_id) {
        (Start::Settings, _) => si::s().title.to_string(),
        (Start::Message { title, .. }, _) => title.clone(),
        (_, Some(id)) => cfg.settings.custom.iter().find(|c| &c.id == id).map_or(id.clone(), |c| c.name.clone()),
        _ => action.title(t).to_string(),
    };
    let env = Env { t, cfg, action, title, custom_id, files, infos, first_kind, ffmpeg, ffprobe };
    let scale = env.cfg.settings.ui_scale.clamp(0.5, 3.0) as f32;
    let transparent = effects::transparency_enabled(&env.cfg.settings);
    let (w, h) = initial_size(&env, &start, request.is_some());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([w * scale, h * scale])
            .with_min_inner_size([360.0 * scale, 160.0 * scale])
            .with_title(format!("Media Tools — {}", env.title))
            .with_app_id("mmt")
            .with_transparent(transparent),
        centered: true,
        ..Default::default()
    };
    eframe::run_native(
        "mmt",
        options,
        Box::new(move |cc| {
            theme::install_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(env, start, request, cc.egui_ctx.clone(), transparent)))
        }),
    )
    .map_err(|e| e.to_string())
}

fn initial_size(env: &Env, start: &Start, direct: bool) -> (f32, f32) {
    match start {
        Start::Settings => (820.0, 620.0),
        Start::Message { .. } => (640.0, 300.0),
        Start::Normal => {
            if env.ffmpeg.is_none() || env.ffprobe.is_none() {
                (520.0, 200.0)
            } else if direct {
                (560.0, 340.0)
            } else if env.action == Action::Stats && env.custom_id.is_none() {
                (800.0, 540.0)
            } else {
                measure_form(env)
            }
        }
    }
}

// ------------------------------------------------------------------ widgets

fn muted() -> Color32 {
    c().muted
}

fn text_width(ctx: &egui::Context, text: &str, font: FontId) -> f32 {
    ctx.fonts(|f| f.layout_no_wrap(text.to_string(), font, Color32::WHITE).size().x)
}

/// A field: small upper-case caption above the control
fn field(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.label(RichText::new(label.to_uppercase()).size(11.0).color(muted()));
    add(ui);
    ui.add_space(6.0);
}

fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().color(muted()));
}

/// A row of right-aligned buttons with a fixed height (with_layout would take all the
/// remaining height of the panel)
fn button_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let size = Vec2::new(ui.available_width(), CONTROL_H + 2.0);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), add);
}

fn primary_button(text: &str) -> egui::Button<'static> {
    let k = c();
    egui::Button::new(bold(text, 13.5).color(k.on_accent)).fill(k.accent).stroke(Stroke::NONE).min_size(vec2(110.0, CONTROL_H))
}

fn secondary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(text.to_string()).min_size(vec2(96.0, CONTROL_H))
}

/// Tabs with an underline under the selected one
fn tabs(ui: &mut egui::Ui, labels: &[&str], current: &mut usize) {
    let k = c();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 22.0;
        for (i, label) in labels.iter().enumerate() {
            let selected = *current == i;
            let text = if selected { bold(*label, 13.5).color(k.text) } else { RichText::new(*label).color(k.muted) };
            let resp = ui.add(egui::Button::new(text).frame(false));
            if selected {
                let r = resp.rect;
                ui.painter().line_segment([pos2(r.left(), r.bottom() + 3.0), pos2(r.right(), r.bottom() + 3.0)], Stroke::new(2.0, k.accent));
            }
            if resp.clicked() {
                *current = i;
            }
        }
    });
    ui.add_space(2.0);
    ui.separator();
    ui.add_space(4.0);
}

/// A choice of one of several values, as flat buttons in a row
fn choice(ui: &mut egui::Ui, current: &mut String, options: &[(&str, &str)]) {
    let k = c();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
        for (value, label) in options {
            let selected = current == value;
            let text = if selected { bold(*label, 13.0).color(k.on_accent) } else { RichText::new(*label) };
            let mut b = egui::Button::new(text).min_size(vec2(0.0, CONTROL_H));
            if selected {
                b = b.fill(k.accent).stroke(Stroke::NONE);
            }
            if ui.add(b).clicked() {
                *current = value.to_string();
            }
        }
    });
}

/// A small flat button; the selected one is filled with the accent colour
fn chip(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let k = c();
    let text = if selected { bold(label, 12.5).color(k.on_accent) } else { RichText::new(label).size(12.5) };
    let mut b = egui::Button::new(text);
    if selected {
        b = b.fill(k.accent).stroke(Stroke::NONE);
    }
    ui.add(b).clicked()
}

fn short(name: &str, max: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max {
        name.to_string()
    } else {
        let head: String = chars[..max.saturating_sub(9)].iter().collect();
        let tail: String = chars[chars.len() - 6..].iter().collect();
        format!("{head}…{tail}")
    }
}

/// A text field for a number: left aligned like every other field, red when it is not valid
fn number_field(ui: &mut egui::Ui, text: &mut String, valid: bool, suffix: &str) {
    ui.horizontal(|ui| {
        let w = (ui.available_width() - if suffix.is_empty() { 0.0 } else { 36.0 }).max(40.0);
        let mut edit = egui::TextEdit::singleline(text).desired_width(w);
        if !valid {
            edit = edit.text_color(c().err);
        }
        ui.add(edit);
        if !suffix.is_empty() {
            ui.label(RichText::new(suffix).color(muted()));
        }
    });
}

// ------------------------------------------------------------------ drop-down lists

/// One column of a drop-down list: items are (key, label)
struct Col {
    title: Option<String>,
    items: Vec<(String, String)>,
}

const ITEM_H: f32 = 28.0;
const TITLE_H: f32 = 22.0;
const POP_PAD: f32 = 8.0;
const COL_GAP: f32 = 8.0;

fn need_id() -> egui::Id {
    egui::Id::new("mmt_dropdown_need")
}

/// A drop-down list. The list is a set of columns (groups side by side), so it is short enough to
/// open completely inside the window: no scrolling, and it never covers its own button. It opens
/// below the button, or above it when there is no room below. Returns true when an item was chosen.
///
/// While a form is being measured, the lowest edge of the open list is remembered so the window
/// is made tall enough for it.
fn dropdown(ui: &mut egui::Ui, id: impl std::hash::Hash, shown: &str, cols: &[Col], selected: &mut String) -> bool {
    let k = c();
    let ctx = ui.ctx().clone();
    let id = ui.make_persistent_id(id);
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, CONTROL_H), Sense::click());
    let open = ui.memory(|m| m.is_popup_open(id));

    // the button
    let stroke = if open || resp.hovered() { Stroke::new(1.0, k.accent) } else { Stroke::new(1.0, k.border) };
    let painter = ui.painter();
    painter.rect(rect, 3.0, k.surface, stroke);
    let clip = Rect::from_min_max(rect.min, pos2(rect.right() - 28.0, rect.max.y));
    painter.with_clip_rect(clip).text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, shown, FontId::proportional(13.5), k.text);
    let m = pos2(rect.right() - 15.0, rect.center().y);
    let tri = if open {
        vec![pos2(m.x - 4.0, m.y + 2.0), pos2(m.x + 4.0, m.y + 2.0), pos2(m.x, m.y - 3.0)]
    } else {
        vec![pos2(m.x - 4.0, m.y - 2.0), pos2(m.x + 4.0, m.y - 2.0), pos2(m.x, m.y + 3.0)]
    };
    painter.add(egui::Shape::convex_polygon(tri, k.muted, Stroke::NONE));
    if resp.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }

    // the size of the list
    let font = FontId::proportional(13.5);
    let title_font = FontId::proportional(10.5);
    let screen = ctx.screen_rect();
    let max_w = (screen.width() - 16.0).max(160.0);
    let natural: Vec<f32> = cols
        .iter()
        .map(|col| {
            let mut w = col.title.as_ref().map_or(0.0, |t| text_width(&ctx, &t.to_uppercase(), title_font.clone()));
            for (_, label) in &col.items {
                w = w.max(text_width(&ctx, label, font.clone()));
            }
            w + 28.0
        })
        .collect();
    let col_h: Vec<f32> = cols.iter().map(|col| col.title.as_ref().map_or(0.0, |_| TITLE_H) + col.items.len() as f32 * ITEM_H).collect();
    // columns are packed into rows no wider than the window
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut cur_w = 0.0;
    for (i, w) in natural.iter().enumerate() {
        let joined = cur_w + COL_GAP + w;
        match rows.last_mut() {
            Some(row) if joined + 2.0 * POP_PAD <= max_w => {
                row.push(i);
                cur_w = joined;
            }
            _ => {
                rows.push(vec![i]);
                cur_w = *w;
            }
        }
    }
    let row_w = |row: &Vec<usize>| row.iter().map(|&i| natural[i]).sum::<f32>() + COL_GAP * (row.len().saturating_sub(1)) as f32;
    let inner_w = rows.iter().map(row_w).fold(0.0, f32::max).max(width - 2.0 * POP_PAD).min(max_w - 2.0 * POP_PAD);
    let popup_w = inner_w + 2.0 * POP_PAD;
    let popup_h = rows.iter().map(|r| r.iter().map(|&i| col_h[i]).fold(0.0, f32::max)).sum::<f32>() + COL_GAP * rows.len().saturating_sub(1) as f32 + 2.0 * POP_PAD + 2.0;
    ctx.data_mut(|d| {
        let cur = d.get_temp::<f32>(need_id()).unwrap_or(0.0);
        d.insert_temp(need_id(), cur.max(rect.bottom() + 4.0 + popup_h + 8.0));
    });

    let mut chosen = false;
    if open {
        let below = screen.bottom() - rect.bottom() - 4.0;
        let above = rect.top() - 4.0 - screen.top();
        let y = if popup_h <= below || popup_h > above { rect.bottom() + 4.0 } else { rect.top() - 4.0 - popup_h };
        let x = rect.left().min(screen.right() - popup_w - 8.0).max(8.0);
        let area = egui::Area::new(id.with("list")).order(egui::Order::Foreground).fixed_pos(pos2(x, y)).constrain(false).show(&ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(Margin::same(POP_PAD)).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(COL_GAP, COL_GAP);
                for row in &rows {
                    // a row of columns shares the free width
                    let extra = ((inner_w - row_w(row)) / row.len() as f32).max(0.0);
                    ui.horizontal_top(|ui| {
                        for &ci in row {
                            let col = &cols[ci];
                            let w = natural[ci] + extra;
                            ui.allocate_ui_with_layout(vec2(w, 0.0), Layout::top_down(Align::Min), |ui| {
                                ui.set_width(w);
                                ui.spacing_mut().item_spacing.y = 0.0;
                                if let Some(title) = &col.title {
                                    let (r, _) = ui.allocate_exact_size(vec2(w, TITLE_H), Sense::hover());
                                    ui.painter().text(pos2(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, title.to_uppercase(), title_font.clone(), k.muted);
                                }
                                for (key, label) in &col.items {
                                    let (r, resp) = ui.allocate_exact_size(vec2(w, ITEM_H), Sense::click());
                                    let is_selected = key == selected;
                                    if is_selected {
                                        ui.painter().rect_filled(r, 3.0, theme::mix(k.surface, k.accent, 0.45));
                                    } else if resp.hovered() {
                                        ui.painter().rect_filled(r, 3.0, k.hover);
                                    }
                                    ui.painter().text(pos2(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, label, font.clone(), k.text);
                                    if resp.clicked() {
                                        *selected = key.clone();
                                        chosen = true;
                                    }
                                }
                            });
                        }
                    });
                }
            });
        });
        let list_rect = area.response.rect;
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().map_or(false, |p| !list_rect.contains(p) && !rect.contains(p)));
        if chosen || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.memory_mut(|mem| mem.close_popup());
        }
    }
    chosen
}

// ------------------------------------------------------------------ codec lists

struct Group {
    label: String,
    items: Vec<(String, String)>, // (key, label)
}

fn group_label(t: &T, group: &str) -> String {
    match group {
        "cpu" => t.group_cpu.into(),
        "nvidia" => t.group_nvidia.into(),
        "amd" => t.group_amd.into(),
        "intel" => t.group_intel.into(),
        "pi" => t.group_pi.into(),
        "preset" => t.group_preset.into(),
        other => other.to_string(),
    }
}

/// Presets first (size limit tab), then the codecs grouped by hardware. Groups of GPUs that
/// are not in this computer are left out (unless `show_all_gpus` is set or nothing is known).
fn build_groups(list: &[(String, Codec)], t: &T, cfg: &Config) -> Vec<Group> {
    let show_all = cfg.settings.show_all_gpus;
    let present = if show_all { None } else { gpu::vendors_present() };
    let pi = show_all || gpu::raspberry_pi_encoder();
    let mut groups: Vec<Group> = Vec::new();
    let mut add = |group: &str, key: &str, label: String| {
        let name = group_label(t, group);
        match groups.iter_mut().find(|g| g.label == name) {
            Some(g) => g.items.push((key.to_string(), label)),
            None => groups.push(Group { label: name, items: vec![(key.to_string(), label)] }),
        }
    };
    for (key, c) in list.iter().filter(|(_, c)| c.preset) {
        add("preset", key, c.label.clone());
    }
    for (key, c) in list.iter().filter(|(_, c)| !c.preset) {
        if c.group == "pi" && !pi {
            continue;
        }
        if let Some(set) = &present {
            if matches!(c.group.as_str(), "nvidia" | "amd" | "intel") && !set.contains(c.group.as_str()) {
                continue;
            }
        }
        add(&c.group, key, c.label.clone());
    }
    groups
}

fn codec_combo(ui: &mut egui::Ui, id: &str, groups: &[Group], selected: &mut String, t: &T) {
    let preset_label = group_label(t, "preset");
    let shown = groups
        .iter()
        .find_map(|g| {
            g.items.iter().find(|(k, _)| k == selected).map(|(_, label)| if g.label == preset_label { label.clone() } else { format!("{label} — {}", g.label) })
        })
        .unwrap_or_else(|| selected.clone());
    let cols: Vec<Col> = groups.iter().map(|g| Col { title: Some(g.label.clone()), items: g.items.clone() }).collect();
    dropdown(ui, id, &shown, &cols, selected);
}

fn format_combo(ui: &mut egui::Ui, id: &str, list: &[(String, Format)], selected: &mut String) {
    let shown = list.iter().find(|(k, _)| k == selected).map(|(_, f)| f.text.clone()).unwrap_or_else(|| selected.clone());
    // up to four items in a column
    let items: Vec<(String, String)> = list.iter().map(|(k, f)| (k.clone(), if f.text.is_empty() { k.clone() } else { f.text.clone() })).collect();
    let cols: Vec<Col> = items.chunks(4).map(|c| Col { title: None, items: c.to_vec() }).collect();
    dropdown(ui, id, &shown, &cols, selected);
}

// ------------------------------------------------------------------ resolution field

#[derive(Clone)]
struct ScaleField {
    mode: ScaleMode,
    custom: String,
    invalid: bool,
}

impl ScaleField {
    fn new() -> Self {
        ScaleField { mode: ScaleMode::Orig, custom: String::new(), invalid: false }
    }

    fn pick(&mut self, mode: ScaleMode) {
        self.mode = mode;
        self.custom.clear();
        self.invalid = false;
    }

    /// Three groups: original (+ auto) and an own value; sizes by the shorter side; scale in percent.
    /// A group that does not fit into a line continues in the next one, under its own chips.
    fn ui(&mut self, ui: &mut egui::Ui, t: &T, with_auto: bool, infos: &[Info]) {
        let k = c();
        ui.spacing_mut().item_spacing = vec2(5.0, 5.0);
        ui.spacing_mut().button_padding = vec2(9.0, 3.0);
        ui.spacing_mut().interact_size.y = 24.0;

        ui.horizontal_wrapped(|ui| {
            if chip(ui, t.res_original, !self.invalid && self.mode == ScaleMode::Orig) {
                self.pick(ScaleMode::Orig);
            }
            if with_auto && chip(ui, t.res_auto, !self.invalid && self.mode == ScaleMode::Auto) {
                self.pick(ScaleMode::Auto);
            }
            ui.add_space(8.0);
            let edit = egui::TextEdit::singleline(&mut self.custom).desired_width(118.0).hint_text(t.res_custom);
            if ui.add(edit).changed() {
                if self.custom.trim().is_empty() {
                    self.mode = ScaleMode::Orig;
                    self.invalid = false;
                } else {
                    match scale::parse_typed(&self.custom) {
                        Some(m) => {
                            self.mode = m;
                            self.invalid = false;
                        }
                        None => self.invalid = true,
                    }
                }
            }
        });

        let mut group = |ui: &mut egui::Ui, caption: &str, chips: Vec<(String, ScaleMode)>| {
            ui.horizontal_top(|ui| {
                let (r, _) = ui.allocate_exact_size(vec2(126.0, 24.0), Sense::hover());
                ui.painter().text(r.left_center(), Align2::LEFT_CENTER, caption.to_uppercase(), FontId::proportional(10.5), k.muted);
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (label, mode) in chips {
                            if chip(ui, &label, !self.invalid && self.mode == mode) {
                                self.pick(mode);
                            }
                        }
                    });
                });
            });
        };
        group(ui, t.res_group_size, [2160u32, 1440, 1080, 720, 540, 480, 360].iter().map(|h| (format!("{h}p"), ScaleMode::Height(*h))).collect());
        group(ui, t.res_group_scale, [75u32, 50, 33, 25].iter().map(|p| (format!("{p}%"), ScaleMode::Factor(*p as f64 / 100.0))).collect());

        // what comes out of it
        if self.invalid {
            ui.label(RichText::new(t.res_invalid).small().color(k.err));
        } else if self.mode == ScaleMode::Auto {
            hint(ui, t.res_auto_text);
        } else if matches!(self.mode, ScaleMode::Height(_) | ScaleMode::Factor(_)) {
            for i in infos.iter().filter(|i| i.width > 0) {
                let result = match scale::target_size(i.width, i.height, self.mode) {
                    Some((w, h)) => format!("{w}×{h}"),
                    None => t.res_unchanged.to_string(),
                };
                hint(ui, &format!("{}  →  {}", short(&i.name, 30), result));
            }
        } else {
            hint(ui, t.res_hint);
        }
    }
}

// ------------------------------------------------------------------ forms

struct Env {
    t: &'static T,
    cfg: Arc<Config>,
    action: Action,
    title: String,
    custom_id: Option<String>,
    files: Vec<PathBuf>,
    infos: Vec<Info>,
    first_kind: Option<Kind>,
    ffmpeg: Option<PathBuf>,
    ffprobe: Option<PathBuf>,
}

struct CompressForm {
    tab: usize,
    level: usize,
    p_groups: Vec<Group>,
    p_codec: String,
    p_scale: ScaleField,
    l_groups: Vec<Group>,
    l_codec: String,
    l_scale: ScaleField,
    mb_text: String,
    twopass: bool,
}

struct FormatForm {
    what: ConvertKind,
    selected: String,
}

struct WebpForm {
    size_text: String,
    fps_text: String,
    quality: u32,
    compression: u32,
}

enum Form {
    Compress(CompressForm),
    Format(FormatForm),
    Webp(WebpForm),
    /// Nothing to ask (Delete sound, own items)
    Confirm(Request),
}

fn default_key(list: &[(String, Format)]) -> String {
    list.iter().find(|(_, f)| f.default).or_else(|| list.first()).map(|(k, _)| k.clone()).unwrap_or_default()
}

fn parse_in(text: &str, lo: u32, hi: u32) -> Option<u32> {
    text.trim().parse::<u32>().ok().filter(|v| (lo..=hi).contains(v))
}

impl Form {
    fn new(env: &Env) -> Form {
        let cfg = &env.cfg;
        if let Some(id) = &env.custom_id {
            return Form::Confirm(Request::Custom { id: id.clone() });
        }
        match env.action {
            Action::Compress => Form::Compress(CompressForm {
                tab: 0,
                level: 1,
                p_groups: build_groups(&cfg.compress, env.t, cfg),
                p_codec: "h264_cpu".into(),
                p_scale: ScaleField::new(),
                l_groups: build_groups(&cfg.limit, env.t, cfg),
                l_codec: "h264_cpu".into(),
                l_scale: ScaleField::new(),
                mb_text: "20".into(),
                twopass: false,
            }),
            Action::ConvertVideo => Form::Format(FormatForm { what: ConvertKind::Video, selected: default_key(&cfg.video_convert) }),
            Action::ExtractAudio => Form::Format(FormatForm { what: ConvertKind::Extract, selected: default_key(&cfg.extract) }),
            Action::Frames => Form::Format(FormatForm { what: ConvertKind::Frames, selected: default_key(&cfg.frames) }),
            Action::ConvertImage => Form::Format(FormatForm { what: ConvertKind::Image, selected: default_key(&cfg.image) }),
            Action::ConvertAudio => {
                if env.first_kind == Some(Kind::Video) {
                    Form::Format(FormatForm { what: ConvertKind::VideoAudio, selected: default_key(&cfg.video_audio) })
                } else {
                    Form::Format(FormatForm { what: ConvertKind::AudioFile, selected: default_key(&cfg.audio_file) })
                }
            }
            Action::Webp => Form::Webp(WebpForm {
                size_text: cfg.webp.size.to_string(),
                fps_text: cfg.webp.fps.to_string(),
                quality: cfg.webp.quality,
                compression: cfg.webp.compression,
            }),
            Action::DeleteSound | Action::Stats => Form::Confirm(Request::DeleteSound),
        }
    }

    /// Draws the fields; returns the request when the form is valid (None = Start is disabled)
    fn ui(&mut self, ui: &mut egui::Ui, env: &Env) -> Option<Request> {
        let t = env.t;
        match self {
            Form::Compress(f) => f.ui(ui, env),
            Form::Format(f) => {
                let list = match f.what {
                    ConvertKind::Video => &env.cfg.video_convert,
                    ConvertKind::VideoAudio => &env.cfg.video_audio,
                    ConvertKind::Extract => &env.cfg.extract,
                    ConvertKind::Frames => &env.cfg.frames,
                    ConvertKind::AudioFile => &env.cfg.audio_file,
                    ConvertKind::Image => &env.cfg.image,
                };
                field(ui, t.format, |ui| format_combo(ui, "format", list, &mut f.selected));
                Some(Request::Convert { what: f.what, format: f.selected.clone() })
            }
            Form::Webp(f) => {
                let size = parse_in(&f.size_text, 64, 4320);
                let fps = parse_in(&f.fps_text, 1, 120);
                ui.columns(2, |cols| {
                    field(&mut cols[0], t.webp_size, |ui| number_field(ui, &mut f.size_text, size.is_some(), "px"));
                    field(&mut cols[1], t.webp_fps, |ui| number_field(ui, &mut f.fps_text, fps.is_some(), ""));
                });
                hint(ui, t.webp_size_hint);
                ui.add_space(8.0);
                field(ui, t.webp_quality, |ui| slider_row(ui, &mut f.quality, 0..=100));
                field(ui, t.webp_compression, |ui| {
                    slider_row(ui, &mut f.compression, 0..=6);
                    hint(ui, t.webp_compression_hint);
                });
                Some(Request::Webp { size: size?, fps: fps?, quality: f.quality, compression: f.compression })
            }
            Form::Confirm(req) => Some(req.clone()),
        }
    }
}

/// A slider with its value written at the right, left aligned like the other fields
fn slider_row(ui: &mut egui::Ui, value: &mut u32, range: std::ops::RangeInclusive<u32>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 48.0).max(80.0);
        ui.add(egui::Slider::new(value, range).show_value(false));
        ui.label(RichText::new(value.to_string()).monospace());
    });
}

impl CompressForm {
    fn ui(&mut self, ui: &mut egui::Ui, env: &Env) -> Option<Request> {
        let t = env.t;
        tabs(ui, &[t.tab_presets, t.tab_limit], &mut self.tab);
        if self.tab == 0 {
            hint(ui, t.tab_presets_text);
            ui.add_space(6.0);
            ui.columns(2, |cols| {
                field(&mut cols[0], t.level, |ui| {
                    let levels = [t.level_low, t.level_medium, t.level_high];
                    let list: Vec<Col> = vec![Col { title: None, items: levels.iter().enumerate().map(|(i, l)| (i.to_string(), l.to_string())).collect() }];
                    let mut sel = self.level.to_string();
                    dropdown(ui, "level", levels[self.level], &list, &mut sel);
                    self.level = sel.parse().unwrap_or(1);
                });
                field(&mut cols[1], t.codec, |ui| codec_combo(ui, "p_codec", &self.p_groups, &mut self.p_codec, t));
            });
            field(ui, t.resolution, |ui| self.p_scale.ui(ui, t, false, &env.infos));
            if self.p_scale.invalid {
                return None;
            }
            Some(Request::CompressPreset { level: self.level, codec: self.p_codec.clone(), scale: self.p_scale.mode })
        } else {
            hint(ui, t.tab_limit_text);
            ui.add_space(6.0);
            let entry = env.cfg.limit_codec(&self.l_codec);
            let fixed_limit = entry.filter(|c| c.preset && c.limit > 0.0).map(|c| c.limit);
            let supports_twopass = entry.map_or(false, |c| !c.twopass.is_empty() && !c.chunked);
            if let Some(l) = fixed_limit {
                self.mb_text = engine::fmt_float(l);
            }
            let valid = self.mb_text.trim().replace(',', ".").parse::<f64>().map_or(false, |v| v > 0.0);
            ui.columns(2, |cols| {
                field(&mut cols[0], t.codec, |ui| codec_combo(ui, "l_codec", &self.l_groups, &mut self.l_codec, t));
                field(&mut cols[1], t.limit, |ui| {
                    ui.add_enabled_ui(fixed_limit.is_none(), |ui| number_field(ui, &mut self.mb_text, valid, "MB"));
                });
            });
            field(ui, t.resolution, |ui| self.l_scale.ui(ui, t, true, &env.infos));
            if supports_twopass {
                ui.checkbox(&mut self.twopass, t.two_pass);
            } else {
                ui.add_enabled(false, egui::Checkbox::new(&mut false, t.two_pass));
            }
            hint(ui, t.two_pass_hint);
            if !valid || self.l_scale.invalid {
                return None;
            }
            let mb: f64 = self.mb_text.trim().replace(',', ".").parse().ok()?;
            Some(Request::CompressLimit { mb, codec: self.l_codec.clone(), scale: self.l_scale.mode, twopass: self.twopass && supports_twopass })
        }
    }
}

fn header(env: &Env, ui: &mut egui::Ui) {
    let t = env.t;
    let max_chars = ((ui.available_width() / 8.4) as usize).clamp(20, 120);
    ui.label(bold(env.title.as_str(), 18.0));
    for info in &env.infos {
        ui.label(bold(short(&info.name, max_chars), 13.0));
        let d = media::describe(info, t.audio_word);
        if !d.is_empty() {
            ui.label(RichText::new(d).small().color(muted()));
        }
    }
    if env.infos.is_empty() {
        for f in env.files.iter().take(3) {
            let n = f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            ui.label(bold(short(&n, max_chars), 13.0));
        }
    }
    if env.files.len() > 3 {
        ui.label(RichText::new(format!("+{} {}", env.files.len() - 3, t.more_word)).small().color(muted()));
    }
}

/// The content of a form window: what the file is, problems of the config, the fields
fn content_ui(ui: &mut egui::Ui, env: &Env, form: &mut Form) -> Option<Request> {
    header(env, ui);
    for w in &env.cfg.warnings {
        ui.label(RichText::new(format!("⚠ {} {w}", env.t.config_warning)).small().color(c().warn));
    }
    ui.add_space(10.0);
    form.ui(ui, env)
}

fn buttons_frame(fill: Color32) -> egui::Frame {
    egui::Frame::none().fill(fill).inner_margin(Margin { left: MARGIN, right: MARGIN, top: 8.0, bottom: MARGIN })
}

fn content_frame(fill: Color32) -> egui::Frame {
    egui::Frame::none().fill(fill).inner_margin(Margin::same(MARGIN))
}

fn form_buttons(ui: &mut egui::Ui, t: &T, valid: Option<&Request>, start: &mut Option<Request>, close: &mut bool) {
    button_row(ui, |ui| {
        let go = ui.add_enabled(valid.is_some(), primary_button(t.start));
        if ui.add(secondary_button(t.cancel)).clicked() {
            *close = true;
        }
        if go.clicked() {
            *start = valid.cloned();
        }
    });
}

// ------------------------------------------------------------------ the size of a window

/// The forms the window has to be tall enough for (both tabs of Compress Video, the longest hints)
fn measured_forms(env: &Env) -> Vec<Form> {
    if let Form::Compress(_) = Form::new(env) {
        let mut out = Vec::new();
        for tab in 0..2usize {
            for mode in [ScaleMode::Orig, ScaleMode::Height(720), ScaleMode::Auto] {
                if tab == 0 && mode == ScaleMode::Auto {
                    continue;
                }
                if let Form::Compress(mut f) = Form::new(env) {
                    f.tab = tab;
                    f.p_scale.mode = mode;
                    f.l_scale.mode = mode;
                    out.push(Form::Compress(f));
                }
            }
        }
        out
    } else {
        vec![Form::new(env)]
    }
}

/// Lay the form out once in an invisible context and return the size the window needs.
/// The width follows the information about the file, the height follows the content.
fn measure_form(env: &Env) -> (f32, f32) {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    theme::apply(&ctx, theme::compute(&env.cfg.settings, None, None), 1.0);
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |_| {}); // the fonts take effect
    }
    let min_w = match env.action {
        Action::Compress if env.custom_id.is_none() => 640.0,
        Action::Webp if env.custom_id.is_none() => 480.0,
        _ => 400.0,
    };
    let mut info_w: f32 = 0.0;
    for info in &env.infos {
        info_w = info_w.max(text_width(&ctx, &info.name, FontId::new(13.0, theme::bold_family())));
        info_w = info_w.max(text_width(&ctx, &media::describe(info, env.t.audio_word), FontId::proportional(11.5)));
    }
    let width = (info_w + 2.0 * MARGIN + 6.0).clamp(min_w, 960.0_f32.max(min_w));

    let fill = ctx.style().visuals.panel_fill;
    let mut height: f32 = 0.0;
    for mut form in measured_forms(env) {
        let mut central = 0.0;
        let mut bottom = 0.0;
        let mut need = 0.0;
        for _ in 0..2 {
            ctx.data_mut(|d| d.insert_temp(need_id(), 0.0f32));
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 3000.0))), ..Default::default() };
            let _ = ctx.run(raw, |ctx| {
                let mut dummy_start = None;
                let mut dummy_close = false;
                let b = egui::TopBottomPanel::bottom("form_buttons").frame(buttons_frame(fill)).show_separator_line(false).show(ctx, |ui| {
                    form_buttons(ui, env.t, None, &mut dummy_start, &mut dummy_close);
                });
                bottom = b.response.rect.height();
                egui::CentralPanel::default().frame(content_frame(fill)).show(ctx, |ui| {
                    central = egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| ui.vertical(|ui| {
                            content_ui(ui, env, &mut form);
                        }).response.rect.height())
                        .inner;
                });
            });
            need = ctx.data(|d| d.get_temp::<f32>(need_id())).unwrap_or(0.0);
        }
        height = height.max(central + 2.0 * MARGIN + bottom).max(need);
    }
    (width.ceil(), height.ceil())
}

// ------------------------------------------------------------------ settings

struct SettingsState {
    s: Settings,
    saved: Settings,
    tab: usize,
    status: String,
    status_ok: bool,
    palette: Option<Palette>,
    ffmpeg_info: String,
    ffprobe_info: String,
    transfer_path: String,
    auto_close_text: String,
}

impl SettingsState {
    fn new(s: Settings) -> Self {
        let mut st = SettingsState {
            saved: s.clone(),
            auto_close_text: engine::fmt_float(s.auto_close),
            s,
            tab: 0,
            status: String::new(),
            status_ok: true,
            palette: syspal::detect(),
            ffmpeg_info: String::new(),
            ffprobe_info: String::new(),
            transfer_path: config::home_dir().join("mmt-commands.toml").display().to_string(),
        };
        st.refresh_tools();
        st
    }

    fn refresh_tools(&mut self) {
        let f = |name: &str, conf: &str| config::find_tool(name, conf).map(|p| p.display().to_string());
        self.ffmpeg_info = f("ffmpeg", &self.s.ffmpeg).unwrap_or_default();
        self.ffprobe_info = f("ffprobe", &self.s.ffprobe).unwrap_or_default();
    }
}

/// Open a file or folder with the program the system has for it
fn open_path(path: &Path) {
    let mut cmd = if cfg!(windows) {
        let mut c = std::process::Command::new("cmd");
        c.args(["/C", "start", ""]).arg(path);
        c
    } else if cfg!(target_os = "macos") {
        let mut c = std::process::Command::new("open");
        c.arg(path);
        c
    } else {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path);
        c
    };
    engine::hide_console(&mut cmd);
    let _ = cmd.spawn();
}

const STANDARD_ITEMS: &[Action] = &[
    Action::Compress,
    Action::ConvertVideo,
    Action::ConvertAudio,
    Action::ExtractAudio,
    Action::DeleteSound,
    Action::Frames,
    Action::Webp,
    Action::ConvertImage,
    Action::Stats,
];

const CONTROL_W: f32 = 270.0;

/// One setting: the name and a hint on the left, the control in a column of fixed width on the right
fn setting_row(ui: &mut egui::Ui, title: &str, hint_text: &str, add: impl FnOnce(&mut egui::Ui)) {
    // the width of the page, not of what the rows above have grown to
    let total = ui.max_rect().width();
    // the left block, the control, and the three gaps between and around them must fit exactly
    let gaps = ui.spacing().item_spacing.x * 2.0 + 24.0;
    let left_w = (total - CONTROL_W - gaps - 2.0).max(140.0);
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(vec2(left_w, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(left_w);
            ui.label(bold(title, 13.5));
            if !hint_text.is_empty() {
                ui.label(RichText::new(hint_text).small().color(muted()));
            }
        });
        ui.add_space(24.0);
        ui.allocate_ui_with_layout(vec2(CONTROL_W, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(CONTROL_W);
            add(ui);
        });
    });
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
}

/// A setting whose control is as wide as the page: the name and hint above it
fn wide_row(ui: &mut egui::Ui, title: &str, hint_text: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.label(bold(title, 13.5));
    if !hint_text.is_empty() {
        ui.label(RichText::new(hint_text).small().color(muted()));
    }
    ui.add_space(6.0);
    add(ui);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
}

/// An entry of the navigation at the left
fn nav_item(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let k = c();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, 3.0, k.hover);
        ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(3.0, rect.height())), 1.0, k.accent);
    } else if resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, mix_alpha(k.hover));
    }
    let text = if selected { bold(label, 13.5).color(k.text) } else { RichText::new(label).color(k.muted) };
    let galley = ui.painter().layout_no_wrap(text.text().to_string(), if selected { FontId::new(13.5, theme::bold_family()) } else { FontId::proportional(13.5) }, if selected { k.text } else { k.muted });
    ui.painter().galley(pos2(rect.left() + 14.0, rect.center().y - galley.size().y / 2.0), galley, k.text);
    resp.clicked()
}

fn mix_alpha(c: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 120)
}

/// The Settings window; returns the button that was pressed
fn settings_screen(ctx: &egui::Context, st: &mut SettingsState, t: &T) -> Option<&'static str> {
    let ts = si::s();
    let k = c();
    let fill = ctx.style().visuals.panel_fill;
    let mut pressed: Option<&'static str> = None;

    // top: the title and the menu buttons
    egui::TopBottomPanel::top("s_top").frame(egui::Frame::none().fill(fill).inner_margin(Margin { left: MARGIN, right: MARGIN, top: 16.0, bottom: 12.0 })).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(bold(ts.title, 18.0));
            let buttons = 110.0 + 96.0 + ui.spacing().item_spacing.x * 2.0 + 4.0;
            let free = ui.available_width() - buttons;
            if free > 0.0 {
                ui.add_space(free);
            }
            if ui.add(primary_button(t.w_install)).clicked() {
                pressed = Some("install");
            }
            if ui.add(secondary_button(t.w_uninstall)).clicked() {
                pressed = Some("uninstall");
            }
        });
    });

    // bottom: the state of the settings and the buttons
    let dirty = st.s != st.saved;
    egui::TopBottomPanel::bottom("s_foot").frame(egui::Frame::none().fill(fill).inner_margin(Margin { left: MARGIN, right: MARGIN, top: 12.0, bottom: 16.0 })).show(ctx, |ui| {
        button_row(ui, |ui| {
            if ui.add(secondary_button(t.close)).clicked() {
                pressed = Some("close");
            }
            if ui.add(primary_button(ts.save_menu).min_size(vec2(170.0, CONTROL_H))).clicked() {
                pressed = Some("save_menu");
            }
            if ui.add_enabled(dirty, secondary_button(ts.save)).clicked() {
                pressed = Some("save");
            }
            let (text, color) = if !st.status.is_empty() {
                (st.status.clone(), if st.status_ok { k.ok } else { k.err })
            } else if dirty {
                (ts.unsaved.to_string(), k.warn)
            } else {
                (String::new(), k.muted)
            };
            ui.label(RichText::new(text).small().color(color));
        });
    });

    // left: the pages
    egui::SidePanel::left("s_nav").exact_width(190.0).resizable(false).frame(egui::Frame::none().fill(k.surface).inner_margin(Margin::same(12.0))).show(ctx, |ui| {
        for (i, label) in [ts.tab_general, ts.tab_look, ts.tab_menu].iter().enumerate() {
            if nav_item(ui, label, st.tab == i) {
                st.tab = i;
            }
        }
        ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
            ui.label(RichText::new(format!("{} {}", ts.version, env!("CARGO_PKG_VERSION"))).small().color(k.muted));
        });
    });

    // the page
    egui::CentralPanel::default().frame(egui::Frame::none().fill(fill).inner_margin(Margin { left: 24.0, right: 16.0, top: 12.0, bottom: 0.0 })).show(ctx, |ui| {
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let page_w = ui.available_width() - 14.0;
            ui.set_max_width(page_w);
            ui.set_min_width(page_w);
            match st.tab {
                0 => general_page(ui, st, &mut pressed),
                1 => look_page(ui, st),
                _ => menu_page(ui, st, t),
            }
        });
    });
    pressed
}

fn general_page(ui: &mut egui::Ui, st: &mut SettingsState, pressed: &mut Option<&'static str>) {
    let ts = si::s();
    let k = c();
    let close_ok = st.auto_close_text.trim().replace(',', ".").parse::<f64>().ok().filter(|v| *v >= 0.0 && *v <= 3600.0);
    setting_row(ui, ts.auto_close, ts.auto_close_hint, |ui| number_field(ui, &mut st.auto_close_text, close_ok.is_some(), "s"));
    if let Some(v) = close_ok {
        st.s.auto_close = v;
    }
    setting_row(ui, ts.gpu_codecs, ts.gpu_codecs_hint, |ui| {
        ui.checkbox(&mut st.s.show_all_gpus, ts.gpu_codecs_check);
    });
    let mut changed = false;
    for (title, path, info) in [(ts.ffmpeg, &mut st.s.ffmpeg, &st.ffmpeg_info), (ts.ffprobe, &mut st.s.ffprobe, &st.ffprobe_info)] {
        let status = if info.is_empty() { format!("{} — {}", ts.not_found, ts.tool_hint) } else { format!("{}: {info}", ts.found) };
        setting_row(ui, title, &status, |ui| {
            if ui.add(egui::TextEdit::singleline(path).desired_width(CONTROL_W).hint_text("auto")).changed() {
                changed = true;
            }
        });
    }
    if changed {
        st.refresh_tools();
    }
    setting_row(ui, ts.commands, &config::user_file().display().to_string(), |ui| {
        ui.horizontal(|ui| {
            if ui.add(secondary_button(ts.open_file).min_size(vec2(150.0, CONTROL_H))).clicked() {
                *pressed = Some("open_presets");
            }
            if ui.add(secondary_button(ts.open_folder).min_size(vec2(104.0, CONTROL_H))).clicked() {
                *pressed = Some("open_folder");
            }
        });
    });
    let _ = k;
}

fn look_page(ui: &mut egui::Ui, st: &mut SettingsState) {
    let ts = si::s();
    let k = c();
    setting_row(ui, ts.theme, ts.theme_hint, |ui| {
        choice(ui, &mut st.s.theme, &[("auto", ts.theme_auto), ("dark", ts.theme_dark), ("light", ts.theme_light)]);
    });
    let have_palette = st.palette.is_some();
    if !have_palette {
        st.s.system_palette = false;
    }
    setting_row(ui, ts.system_colours, if have_palette { ts.system_colours_hint } else { ts.system_colours_na }, |ui| {
        ui.add_enabled(have_palette, egui::Checkbox::new(&mut st.s.system_palette, "Use them"));
    });
    setting_row(ui, ts.accent, ts.accent_hint, |ui| {
        ui.add_enabled_ui(!st.s.system_palette, |ui| {
            ui.horizontal(|ui| {
                let current = theme::parse_hex(&st.s.accent).unwrap_or_else(|| theme::default_accent(k.dark));
                let mut rgb = [current.r(), current.g(), current.b()];
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    st.s.accent = theme::to_hex(Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                }
                ui.label(RichText::new(theme::to_hex(current)).monospace());
                if ui.add(secondary_button(ts.reset).min_size(vec2(70.0, CONTROL_H))).clicked() {
                    st.s.accent.clear();
                }
            });
        });
    });
    setting_row(ui, ts.transparency, ts.transparency_hint, |ui| {
        choice(ui, &mut st.s.transparency, &[("auto", ts.tr_auto), ("on", ts.tr_on), ("off", ts.tr_off)]);
    });
    setting_row(ui, ts.opacity, ts.opacity_hint, |ui| {
        ui.add_enabled_ui(st.s.transparency != "off", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().slider_width = (ui.available_width() - 52.0).max(80.0);
                ui.add(egui::Slider::new(&mut st.s.opacity, 0.5..=1.0).show_value(false));
                ui.label(RichText::new(format!("{:.2}", st.s.opacity)).monospace());
            });
        });
    });
    setting_row(ui, ts.scale, ts.scale_hint, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 52.0).max(80.0);
            ui.add(egui::Slider::new(&mut st.s.ui_scale, 0.8..=1.6).show_value(false));
            ui.label(RichText::new(format!("{:.2}", st.s.ui_scale)).monospace());
        });
    });
}

fn menu_page(ui: &mut egui::Ui, st: &mut SettingsState, t: &T) {
    let ts = si::s();
    let k = c();
    wide_row(ui, ts.standard, ts.standard_hint, |ui| {
        ui.columns(3, |cols| {
            for (i, a) in STANDARD_ITEMS.iter().enumerate() {
                let name = a.cli_name();
                let mut on = st.s.enabled(name);
                if cols[i % 3].checkbox(&mut on, a.title(t)).changed() {
                    st.s.menu.disabled.retain(|d| d != name);
                    if !on {
                        st.s.menu.disabled.push(name.to_string());
                    }
                }
            }
        });
    });
    setting_row(ui, ts.numbered, ts.numbered_hint, |ui| {
        ui.checkbox(&mut st.s.numbered_scripts, "Number them");
    });

    wide_row(ui, ts.custom, ts.custom_hint, |ui| {
        let mut remove: Option<usize> = None;
        for (i, item) in st.s.custom.iter_mut().enumerate() {
            egui::Frame::none().fill(k.surface).stroke(Stroke::new(1.0, k.border)).rounding(3.0).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                ui.columns(2, |cols| {
                    field(&mut cols[0], ts.name, |ui| {
                        ui.add(egui::TextEdit::singleline(&mut item.name).desired_width(f32::INFINITY));
                    });
                    field(&mut cols[1], ts.kind, |ui| {
                        choice(ui, &mut item.kind, &[("video", ts.kind_video), ("audio", ts.kind_audio), ("image", ts.kind_image)]);
                    });
                });
                field(ui, ts.command, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut item.command).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                });
                field(ui, ts.output, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut item.output).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                });
                button_row(ui, |ui| {
                    if ui.add(secondary_button(ts.remove)).clicked() {
                        remove = Some(i);
                    }
                });
            });
            ui.add_space(8.0);
        }
        if let Some(i) = remove {
            st.s.custom.remove(i);
        }
        if ui.add(secondary_button(&format!("+ {}", ts.add)).min_size(vec2(150.0, CONTROL_H))).clicked() {
            st.s.custom.push(Custom { name: "New item".into(), ..Default::default() });
        }
    });

    wide_row(ui, ts.transfer, ts.transfer_hint, |ui| {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 2.0 * 96.0 - 2.0 * ui.spacing().item_spacing.x).max(120.0);
            ui.add(egui::TextEdit::singleline(&mut st.transfer_path).desired_width(w));
            if ui.add(secondary_button(ts.export)).clicked() {
                if st.s.custom.is_empty() {
                    st.status = ts.nothing_to_export.to_string();
                    st.status_ok = false;
                } else {
                    match config::export_custom(&st.s.custom, Path::new(st.transfer_path.trim())) {
                        Ok(n) => {
                            st.status = format!("{} {}", ts.exported.replace("{}", &n.to_string()), st.transfer_path.trim());
                            st.status_ok = true;
                        }
                        Err(e) => {
                            st.status = e;
                            st.status_ok = false;
                        }
                    }
                }
            }
            if ui.add(secondary_button(ts.import)).clicked() {
                match config::import_custom(Path::new(st.transfer_path.trim())) {
                    Ok(items) => {
                        st.status = ts.imported.replace("{}", &items.len().to_string());
                        st.status_ok = true;
                        st.s.custom.extend(items);
                    }
                    Err(e) => {
                        st.status = format!("{} {e}", ts.import_failed);
                        st.status_ok = false;
                    }
                }
            }
        });
    });

    // every item needs a unique id (it is the name of the command `mmt run <id>`)
    let mut seen: Vec<String> = Vec::new();
    for item in st.s.custom.iter_mut() {
        let wanted = if item.id.is_empty() { config::slug(&item.name) } else { item.id.clone() };
        let mut id = wanted.clone();
        let mut n = 2;
        while seen.contains(&id) {
            id = format!("{wanted}_{n}");
            n += 1;
        }
        item.id = id.clone();
        seen.push(id);
    }
}

// ------------------------------------------------------------------ the app

enum Stage {
    Settings(Box<SettingsState>),
    Message { title: String, lines: Vec<String>, ok: bool },
    Form(Form),
    Running,
    Stats(Arc<Mutex<Option<Vec<(String, String)>>>>),
    Error(String),
}

struct App {
    env: Env,
    ctx: egui::Context,
    stage: Stage,
    prog: Option<Arc<Progress>>,
    worker: Option<std::thread::JoinHandle<()>>,
    /// The request the form makes right now (None = the form is not valid)
    valid: Option<Request>,
    close_at: Option<Instant>,
    keep_open: bool,
    request_title: String,
    // look
    transparent: bool,
    palette: Option<Palette>,
    applied: Option<Settings>,
    blur_state: Option<bool>,
}

impl App {
    fn new(env: Env, start: Start, request: Option<Request>, ctx: egui::Context, transparent: bool) -> App {
        let t = env.t;
        let mut app = App {
            stage: Stage::Error(String::new()),
            ctx,
            prog: None,
            worker: None,
            valid: None,
            close_at: None,
            keep_open: false,
            request_title: String::new(),
            transparent,
            palette: syspal::detect(),
            applied: None,
            blur_state: None,
            env,
        };
        app.stage = match start {
            Start::Settings => Stage::Settings(Box::new(SettingsState::new(app.env.cfg.settings.clone()))),
            Start::Message { title, lines, ok } => Stage::Message { title, lines, ok },
            Start::Normal => {
                if app.env.ffmpeg.is_none() || app.env.ffprobe.is_none() {
                    Stage::Error(t.ffmpeg_missing.to_string())
                } else if app.env.action == Action::Stats && app.env.custom_id.is_none() {
                    Stage::Stats(app.spawn_stats())
                } else if let Some(req) = request {
                    app.start(req);
                    Stage::Running
                } else {
                    Stage::Form(Form::new(&app.env))
                }
            }
        };
        app
    }

    /// Put the look of `settings` into egui (only when something changed)
    fn restyle(&mut self, ctx: &egui::Context, settings: &Settings) {
        if self.applied.as_ref() == Some(settings) {
            return;
        }
        let system_dark = ctx.input(|i| i.raw.system_theme).map(|t| t == egui::Theme::Dark);
        let colors = theme::compute(settings, system_dark, self.palette);
        let alpha = if self.transparent && settings.transparency != "off" { settings.opacity as f32 } else { 1.0 };
        theme::apply(ctx, colors, alpha);
        ctx.set_zoom_factor(settings.ui_scale.clamp(0.5, 3.0) as f32);
        self.applied = Some(settings.clone());
    }

    fn spawn_stats(&self) -> Arc<Mutex<Option<Vec<(String, String)>>>> {
        let slot = Arc::new(Mutex::new(None));
        let (slot2, files, probe, ctx) = (slot.clone(), self.env.files.clone(), self.env.ffprobe.clone().unwrap(), self.ctx.clone());
        std::thread::spawn(move || {
            let out: Vec<(String, String)> = files
                .iter()
                .map(|f| (f.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(), engine::stats_text(&probe, f)))
                .collect();
            *slot2.lock().unwrap() = Some(out);
            ctx.request_repaint();
        });
        slot
    }

    fn start(&mut self, req: Request) {
        let ctx = self.ctx.clone();
        let prog = Progress::new(Box::new(move || ctx.request_repaint()), false);
        let job = Job {
            cfg: self.env.cfg.clone(),
            ffmpeg: self.env.ffmpeg.clone().unwrap(),
            ffprobe: self.env.ffprobe.clone().unwrap(),
            prog: prog.clone(),
        };
        let files = self.env.files.clone();
        self.request_title = self.env.title.clone();
        self.worker = Some(std::thread::spawn(move || engine::run(&job, &req, &files)));
        self.prog = Some(prog);
        self.stage = Stage::Running;
    }

    /// The window changes its content (form -> progress, settings -> result): one resize, at the user's action
    fn resize_to(&self, ctx: &egui::Context, min_w: f32, h: f32) {
        let z = ctx.zoom_factor();
        let w = (ctx.screen_rect().width() * z).max(min_w * z);
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(w, h * z)));
    }
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.transparent {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            let b = c().bg;
            [b.r() as f32 / 255.0, b.g() as f32 / 255.0, b.b() as f32 / 255.0, 1.0]
        }
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // look: the Settings window previews its changes live
        let look = match &self.stage {
            Stage::Settings(st) => st.s.clone(),
            _ => self.env.cfg.settings.clone(),
        };
        self.restyle(ctx, &look);
        // the blur behind a transparent window is always asked for where the system can do it
        let want_blur = self.transparent && look.transparency != "off" && effects::blur_supported();
        if effects::blur_supported() && self.blur_state != Some(want_blur) {
            if let Err(e) = effects::set_blur(frame, want_blur, c().dark) {
                eprintln!("mmt: blur not changed: {e}");
            }
            self.blur_state = Some(want_blur);
        }

        let k = c();
        let fill = ctx.style().visuals.panel_fill;
        let t = self.env.t;
        let mut close = false;
        let mut start: Option<Request> = None;
        let mut settings_action: Option<&'static str> = None;

        match &mut self.stage {
            Stage::Settings(st) => {
                settings_action = settings_screen(ctx, st, t);
            }
            Stage::Form(form) => {
                // the buttons stay at the bottom, the fields scroll when the window is made smaller
                let valid = self.valid.clone();
                egui::TopBottomPanel::bottom("form_buttons").frame(buttons_frame(fill)).show_separator_line(false).show(ctx, |ui| {
                    form_buttons(ui, t, valid.as_ref(), &mut start, &mut close);
                });
                let mut now: Option<Request> = None;
                egui::CentralPanel::default().frame(content_frame(fill)).show(ctx, |ui| {
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                        now = ui.vertical(|ui| content_ui(ui, &self.env, form)).inner;
                    });
                });
                if now.is_some() != self.valid.is_some() {
                    ctx.request_repaint();
                }
                self.valid = now;
                if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && start.is_none() {
                    start = self.valid.clone();
                }
                if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !ctx.memory(|m| m.any_popup_open()) {
                    close = true;
                }
            }
            stage => {
                egui::CentralPanel::default().frame(content_frame(fill)).show(ctx, |ui| match stage {
                    Stage::Message { title, lines, ok } => {
                        ui.label(bold(title.as_str(), 18.0).color(if *ok { k.ok } else { k.err }));
                        ui.add_space(8.0);
                        let h = (ui.available_height() - 56.0).max(60.0);
                        egui::Frame::none().fill(k.surface).stroke(Stroke::new(1.0, k.border)).rounding(3.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
                            egui::ScrollArea::both().max_height(h).auto_shrink([false, false]).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                for l in lines.iter() {
                                    ui.add(egui::Label::new(RichText::new(l.as_str()).monospace().size(12.0)).wrap());
                                }
                            });
                        });
                        ui.add_space(10.0);
                        button_row(ui, |ui| {
                            if ui.add(primary_button(t.close)).clicked() {
                                close = true;
                            }
                        });
                        if ui.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter)) {
                            close = true;
                        }
                    }
                    Stage::Running => {
                        if let Some(prog) = self.prog.clone() {
                            running_ui(ui, ctx, &prog, &self.env, &self.request_title, &mut self.close_at, &mut self.keep_open, &mut close);
                        }
                    }
                    Stage::Stats(slot) => {
                        let data = slot.lock().unwrap().clone();
                        ui.label(bold(t.t_stats, 18.0));
                        ui.add_space(6.0);
                        match data {
                            None => {
                                ui.label(t.stats_loading);
                            }
                            Some(list) => {
                                let all: String = list.iter().map(|(n, s)| format!("== {n}\n{s}\n")).collect::<Vec<_>>().join("\n");
                                let avail = ui.available_height() - 52.0;
                                egui::ScrollArea::both().max_height(avail.max(120.0)).auto_shrink([false, false]).show(ui, |ui| {
                                    for (name, text) in &list {
                                        ui.label(bold(name.as_str(), 13.0));
                                        ui.add(egui::Label::new(RichText::new(text).monospace().size(12.0)).wrap_mode(egui::TextWrapMode::Extend));
                                        ui.add_space(10.0);
                                    }
                                });
                                ui.add_space(6.0);
                                button_row(ui, |ui| {
                                    if ui.add(primary_button(t.close)).clicked() {
                                        close = true;
                                    }
                                    if ui.add(secondary_button(t.copy_all)).clicked() {
                                        ui.output_mut(|o| o.copied_text = all.clone());
                                    }
                                });
                            }
                        }
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            close = true;
                        }
                    }
                    Stage::Error(msg) => {
                        ui.label(bold(self.env.title.as_str(), 18.0));
                        ui.add_space(8.0);
                        ui.label(RichText::new(msg.as_str()).color(k.err));
                        ui.add_space(10.0);
                        button_row(ui, |ui| {
                            if ui.add(primary_button(t.close)).clicked() {
                                close = true;
                            }
                        });
                    }
                    Stage::Settings(_) | Stage::Form(_) => {}
                });
            }
        }

        if let Some(req) = start {
            self.start(req);
            self.resize_to(ctx, 560.0, 340.0);
        }
        if let Some(what) = settings_action {
            self.settings_action(ctx, what, &mut close);
        }
        if close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Some(p) = &self.prog {
            p.request_cancel();
        }
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }
}

impl App {
    fn settings_action(&mut self, ctx: &egui::Context, what: &'static str, close: &mut bool) {
        let Stage::Settings(st) = &mut self.stage else { return };
        let ts = si::s();
        match what {
            "close" => *close = true,
            "save" | "save_menu" => {
                match st.s.save() {
                    Ok(()) => {
                        st.saved = st.s.clone();
                        st.status = ts.saved.to_string();
                        st.status_ok = true;
                    }
                    Err(e) => {
                        st.status = e;
                        st.status_ok = false;
                    }
                }
                let ok = st.status_ok;
                if what == "save_menu" && ok {
                    let (code, lines) = crate::install::run("install", &[]);
                    self.stage = Stage::Message { title: self.env.t.w_install.to_string(), lines, ok: code == 0 };
                    self.resize_to(ctx, 640.0, 280.0);
                }
            }
            "install" | "uninstall" => {
                let (code, lines) = crate::install::run(what, &[]);
                let title = if what == "install" { self.env.t.w_install } else { self.env.t.w_uninstall };
                self.stage = Stage::Message { title: title.to_string(), lines, ok: code == 0 };
                self.resize_to(ctx, 640.0, 280.0);
            }
            "open_presets" => {
                let path = config::user_file();
                if !path.exists() {
                    let _ = config::init_presets();
                }
                open_path(&path);
            }
            "open_folder" => {
                let dir = config::config_dir();
                let _ = std::fs::create_dir_all(&dir);
                open_path(&dir);
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn running_ui(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    prog: &Arc<Progress>,
    env: &Env,
    request_title: &str,
    close_at: &mut Option<Instant>,
    keep_open: &mut bool,
    close: &mut bool,
) {
    let t = env.t;
    let k = c();
    let (finished, cancelled, errors, warnings, idx, count, name, step, frac, speed, started, lines) = {
        let s = prog.state.lock().unwrap_or_else(|e| e.into_inner());
        (
            s.finished,
            s.cancelled,
            s.errors,
            s.warnings,
            s.file_index,
            s.file_count.max(1),
            s.file_name.clone(),
            s.step.clone(),
            s.fraction,
            s.speed,
            s.file_started,
            s.lines.clone(),
        )
    };
    let (title, color) = if !finished {
        (request_title.to_string(), k.text)
    } else if cancelled {
        (t.cancelled.to_string(), k.text)
    } else if errors > 0 {
        (t.finished_with_errors.to_string(), k.err)
    } else {
        (t.all_done.to_string(), k.ok)
    };
    ui.label(bold(title, 18.0).color(color));

    if !finished {
        let overall = (idx as f32 + frac as f32) / count as f32;
        if count > 1 {
            ui.label(RichText::new(t.file_of.replacen("{}", &(idx + 1).to_string(), 1).replacen("{}", &count.to_string(), 1)).small().color(k.muted));
        }
        ui.label(bold(short(&name, ((ui.available_width() / 8.4) as usize).clamp(20, 120)), 13.0));
        ui.add_space(4.0);
        ui.add(egui::ProgressBar::new(overall.clamp(0.0, 1.0)).desired_height(6.0).rounding(2.0).fill(k.accent));
        let mut info = vec![format!("{:.0}%", overall.clamp(0.0, 1.0) * 100.0)];
        if !step.is_empty() {
            info.push(step);
        }
        if let Some(s) = speed {
            info.push(format!("{} {s:.2}×", t.speed));
        }
        if let (Some(st), true) = (started, frac > 0.03) {
            let left = st.elapsed().as_secs_f64() * (1.0 - frac) / frac;
            info.push(format!("{} {}", media::fmt_duration(left), t.remaining));
        }
        ui.label(RichText::new(info.join("  ·  ")).small().color(k.muted));
    }

    if !lines.is_empty() {
        ui.add_space(8.0);
        let log_h = (ui.available_height() - 60.0).max(60.0);
        egui::Frame::none().fill(k.surface).stroke(Stroke::new(1.0, k.border)).rounding(3.0).inner_margin(Margin::same(10.0)).show(ui, |ui| {
            egui::ScrollArea::vertical().max_height(log_h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
                ui.set_width(ui.available_width());
                for l in &lines {
                    let (prefix, col) = match l.kind {
                        LineKind::Info => ("", k.muted),
                        LineKind::Warn => ("⚠ ", k.warn),
                        LineKind::Error => ("✖ ", k.err),
                        LineKind::Result => ("✔ ", k.ok),
                    };
                    ui.label(RichText::new(format!("{prefix}{}", l.text)).size(12.0).color(col));
                }
            });
        });
    }

    ui.add_space(10.0);
    button_row(ui, |ui| {
        if !finished {
            if ui.add(secondary_button(t.cancel)).clicked() {
                prog.request_cancel();
            }
            return;
        }
        if ui.add(primary_button(t.close)).clicked() {
            *close = true;
        }
        // a clean run closes itself, the rest stays for the user to read
        let auto = env.cfg.settings.auto_close;
        if errors == 0 && warnings == 0 && !cancelled && auto > 0.0 && !*keep_open {
            let deadline = *close_at.get_or_insert_with(|| Instant::now() + Duration::from_secs_f64(auto));
            let left = deadline.saturating_duration_since(Instant::now());
            if left.is_zero() {
                *close = true;
            }
            ui.label(RichText::new(t.closing_in.replace("{}", &(left.as_secs() + 1).to_string())).small().color(k.muted));
            if ui.add(secondary_button(t.keep_open)).clicked() {
                *keep_open = true;
            }
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    });
    if ui.input(|i| i.key_pressed(egui::Key::Escape)) && finished {
        *close = true;
    }
}
