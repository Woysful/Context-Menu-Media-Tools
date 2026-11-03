//! The options forms of the actions, and the size of their windows.

use super::dropdown::{build_groups, codec_combo, dropdown, need_id, Col, Group};
use super::scale_field::ScaleField;
use super::widgets::{
    button_row, card, field, file_name, fit_name, fit_name_in, form_row, hint, line_edit, muted, one_line, option_grid, primary_button, secondary_button,
    segmented, slider_row, tagged_card, text_width, title, window, MARGIN,
};
use super::Env;
use crate::action::Action;
use crate::config::{Codec, Config, ConvertKind};
use crate::defaults::{self as dv, LEVELS};
use crate::engine::{self, Request, Tracks};
use crate::i18n::T;
use crate::media;
use crate::scale::ScaleMode;
use crate::settings;
use crate::theme::{self, bold, c, Look, CONTROL_H};
use crate::units;
use eframe::egui::{self, pos2, vec2, Align, FontId, Layout, Rect, RichText};

pub struct CompressForm {
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
    p_audio: String,
    l_audio: String,
    /// the audio tracks of the files (shown from two on) and whether each one is kept
    tracks: Vec<(String, bool)>,
    merge: bool,
}

pub struct FormatForm {
    what: ConvertKind,
    selected: String,
}

/// The audio tracks of the files (by number) and whether each one is kept
pub struct DeleteForm {
    tracks: Vec<(String, bool)>,
    /// mix the kept tracks into one
    merge: bool,
}

pub struct WebpForm {
    size_text: String,
    fps_text: String,
    quality: u32,
    compression: u32,
}

/// The fields of the action and "Delete the original"
pub struct Form {
    fields: Fields,
    delete_original: bool,
}

impl Form {
    pub fn new(env: &Env) -> Form {
        Form { fields: Fields::new(env), delete_original: dv::flag(&env.cfg, dv::DELETE_ORIGINAL) }
    }
}

pub enum Fields {
    Compress(Box<CompressForm>),
    Format(FormatForm),
    Webp(WebpForm),
    DeleteAudio(DeleteForm),
    /// Nothing to choose (own items)
    Confirm(Request),
}

fn parse_in(text: &str, lo: u32, hi: u32) -> Option<u32> {
    text.trim().parse::<u32>().ok().filter(|v| (lo..=hi).contains(v))
}

impl Fields {
    fn new(env: &Env) -> Fields {
        let cfg = &env.cfg;
        if let Some(id) = &env.custom_id {
            return Fields::Confirm(Request::Custom { id: id.clone() });
        }
        // every field starts with its own default value (settings.toml) or the standard one
        let d = |key: &str| dv::value(cfg, key);
        if let Some(what) = env.action.convert_kind(env.first_kind) {
            return Fields::Format(FormatForm { what, selected: d(&dv::format_key(what)) });
        }
        match env.action {
            Action::Compress => Fields::Compress(Box::new(CompressForm {
                tab: usize::from(d(dv::MODE) == "limit"),
                level: dv::level_index(&d(dv::LEVEL)),
                p_groups: build_groups(&cfg.compress, env.t, cfg),
                p_codec: d(dv::CODEC),
                p_scale: ScaleField::from_code(&d(dv::RESOLUTION)),
                l_groups: build_groups(&cfg.limit, env.t, cfg),
                l_codec: d(dv::LIMIT_CODEC),
                l_scale: ScaleField::from_code(&d(dv::LIMIT_RESOLUTION)),
                mb_text: d(dv::LIMIT_SIZE),
                twopass: dv::flag(cfg, dv::TWO_PASS),
                p_audio: d(dv::AUDIO),
                l_audio: d(dv::LIMIT_AUDIO),
                tracks: track_list(env),
                merge: dv::flag(cfg, dv::MERGE),
            })),
            Action::Webp => Fields::Webp(WebpForm {
                size_text: d(dv::WEBP_SIZE),
                fps_text: d(dv::WEBP_FPS),
                quality: d(dv::WEBP_QUALITY).parse().unwrap_or(cfg.webp.quality),
                compression: d(dv::WEBP_COMPRESSION).parse().unwrap_or(cfg.webp.compression),
            }),
            _ => Fields::DeleteAudio(DeleteForm::new(env)),
        }
    }

    /// Draws the fields; returns the request when the form is valid (None = Start is disabled).
    /// The forms made of rows draw "Delete the original" and the result as rows of their own.
    fn ui(&mut self, ui: &mut egui::Ui, env: &Env, delete_original: &mut bool) -> Option<Request> {
        let t = env.t;
        match self {
            Fields::Compress(f) => f.ui(ui, env, delete_original),
            Fields::Format(f) => {
                let options: Vec<(&str, &str, &str)> = env.cfg.formats(f.what).iter().map(|(k, fmt)| (k.as_str(), fmt.label(k), fmt.hint.as_str())).collect();
                let r = field(ui, t.format, |ui| option_grid(ui, &mut f.selected, &options));
                if let Some(v) = default_menu(&r, env, &dv::format_key(f.what), Some(&f.selected)) {
                    f.selected = v;
                }
                Some(Request::Convert { what: f.what, format: f.selected.clone() })
            }
            Fields::Webp(f) => f.ui(ui, env, delete_original),
            Fields::DeleteAudio(f) => f.ui(ui, env),
            Fields::Confirm(req) => {
                if let Request::Custom { id } = &*req {
                    if let Some(item) = env.cfg.settings.custom(id) {
                        field(ui, t.command, |ui| card(ui, |ui| ui.add(egui::Label::new(RichText::new(&item.command).monospace()).wrap())));
                    }
                }
                Some(req.clone())
            }
        }
    }
}

/// The audio tracks of the files, all ticked. Track N is described by the first file that has it.
fn track_list(env: &Env) -> Vec<(String, bool)> {
    let count = env.infos.iter().map(|i| i.audio.len()).chain(env.track_counts.iter().copied()).max().unwrap_or(0);
    (0..count)
        .map(|n| {
            let about = env.infos.iter().find_map(|i| i.audio.get(n)).map(|a| a.describe()).unwrap_or_default();
            let label = if about.is_empty() { format!("{} {}", env.t.del_track, n + 1) } else { format!("{} {} — {about}", env.t.del_track, n + 1) };
            (label, true)
        })
        .collect()
}

/// A warning when the files have different numbers of audio tracks
fn tracks_differ(ui: &mut egui::Ui, env: &Env) {
    let (Some(lo), Some(hi)) = (env.track_counts.iter().min(), env.track_counts.iter().max()) else { return };
    if lo != hi {
        let text = env.t.tracks_differ.replace("{}", &format!("{lo}–{hi}"));
        ui.add(egui::Label::new(RichText::new(format!("⚠ {text}")).small().color(c().warn)).wrap());
    }
}

impl DeleteForm {
    fn new(env: &Env) -> DeleteForm {
        DeleteForm { tracks: track_list(env), merge: dv::flag(&env.cfg, dv::DELETE_MERGE) }
    }

    fn ui(&mut self, ui: &mut egui::Ui, env: &Env) -> Option<Request> {
        let t = env.t;
        ui.label(t.delete_audio_text);
        ui.add_space(10.0);
        if self.tracks.is_empty() {
            hint(ui, t.del_no_tracks);
            ui.add_space(4.0);
            return Some(Request::DeleteAudio { remove: None, merge: false });
        }
        let kept = self.tracks.iter().filter(|(_, on)| *on).count();
        field(ui, t.del_tracks, |ui| {
            card(ui, |ui| {
                for (label, on) in &mut self.tracks {
                    ui.checkbox(on, label.as_str());
                }
            });
            tracks_differ(ui, env);
            if env.files.len() > 1 {
                hint(ui, t.del_by_number);
            }
            if self.tracks.len() > 1 {
                ui.add_space(4.0);
                let r = ui.add_enabled(kept > 1, egui::Checkbox::new(&mut self.merge, t.merge_kept));
                if let Some(v) = default_menu(&r, env, dv::DELETE_MERGE, Some(bool_text(self.merge))) {
                    self.merge = v == "true";
                }
                if self.merge && kept > 1 {
                    hint(ui, t.del_merge_hint);
                }
            }
        });
        let merge = self.merge && kept > 1;
        let remove: Vec<usize> = self.tracks.iter().enumerate().filter(|(_, (_, keep))| !*keep).map(|(i, _)| i).collect();
        match remove.len() {
            0 if merge => Some(Request::DeleteAudio { remove: Some(remove), merge }),
            0 => {
                ui.add_space(4.0);
                hint(ui, t.del_untick);
                None
            }
            n if n == self.tracks.len() => Some(Request::DeleteAudio { remove: None, merge: false }),
            _ => Some(Request::DeleteAudio { remove: Some(remove), merge }),
        }
    }
}

/// A number field as high as the other controls, like "Size limit … MB" of Compress Video
fn small_number(ui: &mut egui::Ui, text: &mut String, valid: bool, suffix: &str) -> egui::Response {
    ui.horizontal(|ui| {
        let r = ui.add(line_edit(text, 64.0, valid));
        if !suffix.is_empty() {
            ui.label(RichText::new(suffix).color(muted()));
        }
        r
    })
    .inner
}

fn bool_text(on: bool) -> &'static str {
    if on {
        "true"
    } else {
        "false"
    }
}

/// "Delete the original when done": to the trash or for good, as the Settings say
fn original_check(ui: &mut egui::Ui, env: &Env, on: &mut bool, text: &str) {
    let t = env.t;
    let r = ui
        .horizontal(|ui| {
            let r = ui.checkbox(on, text);
            let (note, color) = if env.cfg.settings.delete_mode == "permanent" { (t.for_good, c().warn) } else { (t.to_trash, muted()) };
            ui.label(RichText::new(note).small().color(color));
            r
        })
        .inner;
    if let Some(v) = default_menu(&r, env, dv::DELETE_ORIGINAL, Some(bool_text(*on))) {
        *on = v == "true";
    }
}

impl WebpForm {
    /// Rows like those of Compress Video: the name at the left, the control and a line about it
    fn ui(&mut self, ui: &mut egui::Ui, env: &Env, delete_original: &mut bool) -> Option<Request> {
        let t = env.t;
        let size = parse_in(&self.size_text, 64, 4320);
        let fps = parse_in(&self.fps_text, 1, 120);
        let r = form_row(ui, t.webp_size, |ui| {
            let r = small_number(ui, &mut self.size_text, size.is_some(), "px");
            one_line(ui, t.webp_size_hint, muted());
            r
        });
        if let Some(v) = default_menu(&r, env, dv::WEBP_SIZE, size.map(|v| v.to_string()).as_deref()) {
            self.size_text = v;
        }
        let r = form_row(ui, t.webp_fps, |ui| {
            let r = small_number(ui, &mut self.fps_text, fps.is_some(), "fps");
            one_line(ui, t.webp_fps_hint, muted());
            r
        });
        if let Some(v) = default_menu(&r, env, dv::WEBP_FPS, fps.map(|v| v.to_string()).as_deref()) {
            self.fps_text = v;
        }
        let r = form_row(ui, t.webp_quality, |ui| {
            let r = slider_row(ui, &mut self.quality, 0..=100);
            one_line(ui, t.webp_quality_hint, muted());
            r
        });
        if let Some(v) = default_menu(&r, env, dv::WEBP_QUALITY, Some(&self.quality.to_string())) {
            self.quality = v.parse().unwrap_or(self.quality);
        }
        let r = form_row(ui, t.webp_compression, |ui| {
            let r = slider_row(ui, &mut self.compression, 0..=6);
            one_line(ui, t.webp_compression_hint, muted());
            r
        });
        if let Some(v) = default_menu(&r, env, dv::WEBP_COMPRESSION, Some(&self.compression.to_string())) {
            self.compression = v.parse().unwrap_or(self.compression);
        }
        form_row(ui, t.original, |ui| original_check(ui, env, delete_original, t.delete_after));
        let (size, fps) = (parse_in(&self.size_text, 64, 4320), parse_in(&self.fps_text, 1, 120));
        let req = match (size, fps) {
            (Some(size), Some(fps)) => Some(Request::Webp { size, fps, quality: self.quality, compression: self.compression }),
            _ => None,
        };
        result_row(ui, env, req.as_ref());
        req
    }
}

impl CompressForm {
    /// Both modes have the same rows (a control with a line under it, the codec, the resolution,
    /// the result), so the window fits either of them without empty space
    fn ui(&mut self, ui: &mut egui::Ui, env: &Env, delete_original: &mut bool) -> Option<Request> {
        let t = env.t;
        let r = segmented(ui, &[t.tab_presets, t.tab_limit], &mut self.tab);
        if let Some(v) = default_menu(&r, env, dv::MODE, Some(["presets", "limit"][self.tab.min(1)])) {
            self.tab = usize::from(v == "limit");
        }
        ui.add_space(2.0);
        one_line(ui, if self.tab == 0 { t.tab_presets_text } else { t.tab_limit_text }, muted());
        ui.add_space(14.0);
        let req = if self.tab == 0 { self.quality_rows(ui, env) } else { self.limit_rows(ui, env) };
        form_row(ui, t.original, |ui| original_check(ui, env, delete_original, t.delete_after));
        result_row(ui, env, req.as_ref());
        req
    }

    fn quality_rows(&mut self, ui: &mut egui::Ui, env: &Env) -> Option<Request> {
        let t = env.t;
        let r = form_row(ui, t.level, |ui| {
            let r = segmented(ui, &[t.level_low, t.level_medium, t.level_high], &mut self.level);
            one_line(ui, [t.level_low_hint, t.level_medium_hint, t.level_high_hint][self.level.min(2)], muted());
            r
        });
        if let Some(v) = default_menu(&r, env, dv::LEVEL, Some(LEVELS[self.level.min(2)])) {
            self.level = dv::level_index(&v);
        }
        let r = form_row(ui, t.codec, |ui| codec_combo(ui, "p_codec", &self.p_groups, &mut self.p_codec, t));
        if let Some(v) = default_menu(&r, env, dv::CODEC, Some(&self.p_codec)) {
            self.p_codec = v;
        }
        let r = form_row(ui, t.resolution, |ui| self.p_scale.ui(ui, "p_scale", t, false, &env.infos));
        if let Some(v) = default_menu(&r, env, dv::RESOLUTION, self.p_scale.code().as_deref()) {
            self.p_scale = ScaleField::from_code(&v);
        }
        let r = form_row(ui, t.audio, |ui| audio_combo(ui, "p_audio", t, &env.cfg, &mut self.p_audio));
        if let Some(v) = default_menu(&r, env, dv::AUDIO, Some(&self.p_audio)) {
            self.p_audio = v;
        }
        let tracks = self.tracks_row(ui, env, &self.p_audio.clone());
        if self.p_scale.invalid {
            return None;
        }
        Some(Request::CompressPreset { level: self.level, codec: self.p_codec.clone(), scale: self.p_scale.mode, audio: self.p_audio.clone(), tracks: tracks? })
    }

    /// The audio tracks to keep and "merge into one" (for files with several tracks). None: no
    /// track is ticked.
    fn tracks_row(&mut self, ui: &mut egui::Ui, env: &Env, audio: &str) -> Option<Tracks> {
        let t = env.t;
        if self.tracks.len() < 2 {
            return Some(Tracks::default());
        }
        let removes = env.cfg.audio_codec(audio).is_some_and(|a| a.removes());
        let kept: Vec<usize> = self.tracks.iter().enumerate().filter(|(_, (_, on))| *on).map(|(i, _)| i).collect();
        let merge = form_row(ui, t.tracks, |ui| {
            let merge = ui
                .add_enabled_ui(!removes, |ui| {
                    card(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for (label, on) in &mut self.tracks {
                            ui.checkbox(on, label.as_str());
                        }
                    });
                    tracks_differ(ui, env);
                    ui.add_space(2.0);
                    ui.add_enabled(kept.len() > 1, egui::Checkbox::new(&mut self.merge, t.merge_tracks))
                })
                .inner;
            let (note, color) = if removes {
                (t.tracks_hint, muted())
            } else if kept.is_empty() {
                (t.tracks_tick, c().err)
            } else if self.merge && kept.len() > 1 {
                (t.merge_hint, muted())
            } else if env.files.len() > 1 {
                (t.tracks_by_number, muted())
            } else {
                (t.tracks_hint, muted())
            };
            one_line(ui, note, color);
            merge
        });
        if let Some(v) = default_menu(&merge, env, dv::MERGE, Some(bool_text(self.merge))) {
            self.merge = v == "true";
        }
        if removes {
            return Some(Tracks::default());
        }
        if kept.is_empty() {
            return None;
        }
        let keep = (kept.len() < self.tracks.len()).then_some(kept.clone());
        Some(Tracks { keep, merge: self.merge && kept.len() > 1 })
    }

    fn limit_rows(&mut self, ui: &mut egui::Ui, env: &Env) -> Option<Request> {
        let t = env.t;
        let entry = env.cfg.limit_codec(&self.l_codec);
        let fixed_limit = entry.and_then(Codec::fixed_limit);
        let twopass_ok = entry.is_some_and(Codec::supports_twopass);
        if let Some(l) = fixed_limit {
            self.mb_text = units::fmt_float(l);
        }
        let mb = units::parse_decimal(&self.mb_text).filter(|v| *v > 0.0);
        let (r_mb, r_twopass) = form_row(ui, t.limit, |ui| {
            let rows = ui
                .horizontal(|ui| {
                    let r_mb = ui
                        .add_enabled_ui(fixed_limit.is_none(), |ui| {
                            let r = ui.add(line_edit(&mut self.mb_text, 48.0, mb.is_some()));
                            ui.label(RichText::new("MB").color(muted()));
                            r
                        })
                        .inner;
                    ui.add_space(18.0);
                    let r_twopass = if twopass_ok {
                        ui.checkbox(&mut self.twopass, t.two_pass)
                    } else {
                        ui.add_enabled(false, egui::Checkbox::new(&mut false, t.two_pass))
                    };
                    (r_mb, r_twopass)
                })
                .inner;
            let note = if fixed_limit.is_some() {
                t.limit_fixed
            } else if twopass_ok {
                t.two_pass_hint
            } else {
                t.two_pass_na
            };
            one_line(ui, note, muted());
            rows
        });
        // the size of a preset with its own limit is not a value of the field
        let own_mb = if fixed_limit.is_none() { mb.map(units::fmt_float) } else { None };
        if let Some(v) = default_menu(&r_mb, env, dv::LIMIT_SIZE, own_mb.as_deref()) {
            self.mb_text = v;
        }
        if let Some(v) = default_menu(&r_twopass, env, dv::TWO_PASS, Some(bool_text(self.twopass))) {
            self.twopass = v == "true";
        }
        let r = form_row(ui, t.codec, |ui| codec_combo(ui, "l_codec", &self.l_groups, &mut self.l_codec, t));
        if let Some(v) = default_menu(&r, env, dv::LIMIT_CODEC, Some(&self.l_codec)) {
            self.l_codec = v;
        }
        let r = form_row(ui, t.resolution, |ui| self.l_scale.ui(ui, "l_scale", t, true, &env.infos));
        if let Some(v) = default_menu(&r, env, dv::LIMIT_RESOLUTION, self.l_scale.code().as_deref()) {
            self.l_scale = ScaleField::from_code(&v);
        }
        let r = form_row(ui, t.audio, |ui| audio_combo(ui, "l_audio", t, &env.cfg, &mut self.l_audio));
        if let Some(v) = default_menu(&r, env, dv::LIMIT_AUDIO, Some(&self.l_audio)) {
            self.l_audio = v;
        }
        let tracks = self.tracks_row(ui, env, &self.l_audio.clone());
        if self.l_scale.invalid {
            return None;
        }
        Some(Request::CompressLimit {
            mb: mb?,
            codec: self.l_codec.clone(),
            scale: self.l_scale.mode,
            twopass: self.twopass && twopass_ok,
            audio: self.l_audio.clone(),
            tracks: tracks?,
        })
    }
}

/// Right click on a field: "Save as default" (`value`; None = the field is not valid now) or
/// "Reset to standard". Returns the standard value when that was chosen.
fn default_menu(resp: &egui::Response, env: &Env, key: &str, value: Option<&str>) -> Option<String> {
    let t = env.t;
    let mut reset = None;
    resp.context_menu(|ui| {
        if ui.add_enabled(value.is_some(), egui::Button::new(t.save_default)).clicked() {
            if let Some(v) = value {
                set_note(ui.ctx(), t, settings::store_default(key, Some(v)), t.default_saved);
            }
            ui.close_menu();
        }
        if ui.button(t.reset_default).clicked() {
            set_note(ui.ctx(), t, settings::store_default(key, None), t.default_reset);
            reset = Some(dv::standard(&env.cfg, key));
            ui.close_menu();
        }
    });
    reset
}

/// What became of the last "Save as default" / "Reset to standard": (text, ok)
fn note_id() -> egui::Id {
    egui::Id::new("cmmt_default_note")
}

fn set_note(ctx: &egui::Context, t: &T, result: Result<(), String>, done: &str) {
    let note = match result {
        Ok(()) => (done.to_string(), true),
        Err(e) => (format!("{} {e}", t.default_failed), false),
    };
    ctx.data_mut(|d| d.insert_temp(note_id(), note));
}

/// The Audio list of Compress Video ([compress_audio] of presets.toml), a column per group
pub fn audio_combo(ui: &mut egui::Ui, id: &str, t: &T, cfg: &Config, selected: &mut String) -> egui::Response {
    let mut cols: Vec<(String, Col)> = Vec::new();
    for (key, a) in &cfg.audio {
        let label = if a.label.is_empty() { key.clone() } else { a.label.clone() };
        match cols.iter_mut().find(|(g, _)| *g == a.group) {
            Some((_, col)) => col.items.push((key.clone(), label)),
            None => {
                let title = match a.group.as_str() {
                    "source" => t.audio_source,
                    "lossy" => t.audio_lossy,
                    "lossless" => t.audio_lossless,
                    other => other,
                };
                let title = (!title.is_empty()).then(|| title.to_string());
                cols.push((a.group.clone(), Col { title, items: vec![(key.clone(), label)] }));
            }
        }
    }
    let cols: Vec<Col> = cols.into_iter().map(|(_, c)| c).collect();
    let shown = cols.iter().flat_map(|c| &c.items).find(|(k, _)| k == selected).map_or_else(|| selected.clone(), |(_, l)| l.clone());
    dropdown(ui, id, &shown, &cols, selected)
}

/// The name of the result in one row of the form (of the first file; the others are counted)
fn result_row(ui: &mut egui::Ui, env: &Env, req: Option<&Request>) {
    let first = req.and_then(|r| env.files.first().and_then(|f| engine::result_name(&env.cfg, r, f)));
    let more = env.files.len().saturating_sub(1);
    form_row(ui, env.t.result, |ui| {
        ui.allocate_ui_with_layout(vec2(ui.available_width(), CONTROL_H), Layout::left_to_right(Align::Center), |ui| {
            let font = egui::TextStyle::Body.resolve(ui.style());
            let text = match first {
                Some(n) if more > 0 => {
                    let suffix = format!("  (+{more})");
                    let room = ui.available_width() - 2.0 - text_width(ui.ctx(), &suffix, font.clone());
                    format!("{}{suffix}", fit_name_in(ui.ctx(), &n, font, room))
                }
                Some(n) => fit_name(ui, &n, font),
                None => "—".into(),
            };
            let text = if c().look == Look::Classic { RichText::new(text) } else { bold(text, 13.0) };
            ui.add(egui::Label::new(text.color(c().text)).truncate());
        });
    });
}
/// The title of the window and the files it works on
fn header(ui: &mut egui::Ui, env: &Env) {
    title(ui, &env.title, c().text);
    ui.add_space(10.0);
    if env.files.is_empty() {
        return;
    }
    let count = env.files.len();
    let look = c().look;
    let note = if count > 1 { format!("{count:02} files") } else { String::new() };
    tagged_card(ui, "input", &note, |ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        let font = FontId::new(13.0, theme::bold_family());
        for (i, f) in env.files.iter().take(3).enumerate() {
            if i > 0 && look == Look::Classic {
                ui.add_space(6.0);
            } else if i > 0 {
                ui.add_space(4.0);
                ui.add(egui::Separator::default().spacing(6.0));
            }
            let name = file_name(f);
            ui.label(bold(fit_name(ui, &name, font.clone()), 13.0));
            let about = env.infos.iter().find(|x| x.name == name).map(|x| media::describe(x, env.t.audio_word)).unwrap_or_default();
            if !about.is_empty() {
                let about = if look == Look::Brutal { about.replace(" · ", " / ") } else { about };
                ui.label(RichText::new(about).small().color(muted()));
            }
        }
        if env.files.len() > 3 {
            ui.add_space(4.0);
            hint(ui, &format!("+{} {}", env.files.len() - 3, env.t.more_word));
        }
    });
}

/// What comes out of it: the names of the results
fn results(ui: &mut egui::Ui, env: &Env, req: &Request) {
    let names: Vec<String> = env.files.iter().take(3).filter_map(|f| engine::result_name(&env.cfg, req, f)).collect();
    if names.is_empty() {
        return;
    }
    ui.add_space(4.0);
    field(ui, env.t.result, |ui| {
        ui.spacing_mut().item_spacing.y = 2.0;
        let font = egui::TextStyle::Body.resolve(ui.style());
        let room = ui.available_width() - 2.0 - text_width(ui.ctx(), "→ ", font.clone());
        for n in &names {
            ui.label(RichText::new(format!("→ {}", fit_name_in(ui.ctx(), n, font.clone(), room))).color(muted()));
        }
    });
}

/// The content of a form window: what the files are, problems of the config, the fields
fn content(ui: &mut egui::Ui, env: &Env, form: &mut Form) -> Option<Request> {
    header(ui, env);
    for w in &env.cfg.warnings {
        ui.label(RichText::new(format!("⚠ {} {w}", env.t.config_warning)).small().color(c().warn));
    }
    ui.add_space(14.0);
    let req = form.fields.ui(ui, env, &mut form.delete_original);
    if !matches!(form.fields, Fields::Compress(_) | Fields::Webp(_)) {
        ui.add_space(2.0);
        original_check(ui, env, &mut form.delete_original, env.t.delete_original);
        if let Some(r) = &req {
            results(ui, env, r);
        }
    }
    req
}

/// (Start pressed, Cancel pressed)
fn buttons(ui: &mut egui::Ui, t: &T, valid: bool) -> (bool, bool) {
    let mut pressed = (false, false);
    let note = ui.ctx().data(|d| d.get_temp::<(String, bool)>(note_id()));
    button_row(ui, |ui| {
        pressed.0 = ui.add_enabled(valid, primary_button(t.start)).clicked();
        pressed.1 = ui.add(secondary_button(t.cancel)).clicked();
        if let Some((text, ok)) = note {
            ui.add_space(6.0);
            ui.add(egui::Label::new(RichText::new(text).small().color(if ok { c().ok } else { c().err })).truncate());
        }
    });
    pressed
}

pub struct Shown {
    /// The request the form makes now (None = it is not valid)
    pub request: Option<Request>,
    pub delete_original: bool,
    pub start: bool,
    pub cancel: bool,
}

/// Draw a form window. `valid`: whether the form was valid in the frame before (the buttons
/// are drawn before the fields)
pub fn show(ctx: &egui::Context, env: &Env, form: &mut Form, valid: bool) -> Shown {
    let mut pressed = (false, false);
    let (request, _) = window(ctx, true, |ui| pressed = buttons(ui, env.t, valid), |ui| content(ui, env, form));
    Shown { request, delete_original: form.delete_original, start: pressed.0, cancel: pressed.1 }
}

/// The forms the window has to be tall enough for (both tabs of Compress Video, the longest hints)
fn measured_forms(env: &Env) -> Vec<Form> {
    let with = |fields: Fields| Form { fields, delete_original: false };
    if let Fields::DeleteAudio(_) = Fields::new(env) {
        // the names of the results appear once a track is unticked, the line about merging
        // once it is ticked
        let mut out = vec![Form::new(env)];
        for merge in [false, true] {
            if let Fields::DeleteAudio(mut f) = Fields::new(env) {
                if let (Some(last), false) = (f.tracks.last_mut(), merge) {
                    last.1 = false;
                }
                f.merge = merge;
                out.push(with(Fields::DeleteAudio(f)));
            }
        }
        return out;
    }
    let Fields::Compress(_) = Fields::new(env) else { return vec![Form::new(env)] };
    let mut out = Vec::new();
    for tab in 0..2usize {
        for mode in [ScaleMode::Orig, ScaleMode::Height(720), ScaleMode::Auto] {
            if tab == 0 && mode == ScaleMode::Auto {
                continue;
            }
            if let Fields::Compress(mut f) = Fields::new(env) {
                f.tab = tab;
                f.p_scale.mode = mode;
                f.l_scale.mode = mode;
                out.push(with(Fields::Compress(f)));
            }
        }
    }
    out
}

/// Lay the form out once in an invisible context and return the size the window needs.
/// The height follows the content.
pub fn measure(env: &Env) -> (f32, f32) {
    let ctx = egui::Context::default();
    theme::install_fonts(&ctx);
    theme::apply(&ctx, theme::compute(&env.cfg.settings, None, None), 1.0);
    for _ in 0..2 {
        let _ = ctx.run(egui::RawInput::default(), |_| {}); // the fonts take effect
    }
    let min_w = match (env.action, env.custom_id.is_some()) {
        (Action::Compress, false) => 480.0,
        (Action::Webp, false) => 480.0,
        _ => 440.0,
    };
    // long names are cut to the width, only the line about the file widens the window
    let text_w = env.infos.iter().map(|i| text_width(&ctx, &media::describe(i, env.t.audio_word), FontId::proportional(11.5))).fold(0.0, f32::max);
    // the card adds its margins and border
    let width = (text_w + 2.0 * MARGIN + 2.0 * 13.0 + 6.0).clamp(min_w, 960.0_f32.max(min_w));

    let mut height: f32 = 0.0;
    for mut form in measured_forms(env) {
        let mut h = 0.0;
        let mut need = 0.0;
        for _ in 0..2 {
            ctx.data_mut(|d| d.insert_temp(need_id(), 0.0f32));
            let raw = egui::RawInput { screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(width, 3000.0))), ..Default::default() };
            let _ = ctx.run(raw, |ctx| {
                h = window(
                    ctx,
                    true,
                    |ui| {
                        buttons(ui, env.t, true);
                    },
                    |ui| {
                        content(ui, env, &mut form);
                    },
                )
                .1;
            });
            need = ctx.data(|d| d.get_temp::<f32>(need_id())).unwrap_or(0.0);
        }
        height = height.max(h).max(need);
    }
    (width.ceil(), height.ceil())
}
