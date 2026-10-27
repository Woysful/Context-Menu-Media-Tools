//! The options forms of the actions, and the size of their windows.

use super::dropdown::{build_groups, codec_combo, need_id, Group};
use super::scale_field::ScaleField;
use super::widgets::{
    button_row, card, chars_fitting, field, file_name, form_row, hint, line_edit, muted, number_field, one_line, option_grid, primary_button, secondary_button,
    segmented, short, slider_row, text_width, title, window, MARGIN,
};
use super::Env;
use crate::action::Action;
use crate::config::{Codec, ConvertKind};
use crate::engine::{self, Request};
use crate::i18n::T;
use crate::media;
use crate::scale::ScaleMode;
use crate::theme::{self, bold, c, CONTROL_H};
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
}

pub struct FormatForm {
    what: ConvertKind,
    selected: String,
}

pub struct WebpForm {
    size_text: String,
    fps_text: String,
    quality: u32,
    compression: u32,
}

pub enum Form {
    Compress(CompressForm),
    Format(FormatForm),
    Webp(WebpForm),
    /// Nothing to choose (Delete sound, own items)
    Confirm(Request),
}

fn parse_in(text: &str, lo: u32, hi: u32) -> Option<u32> {
    text.trim().parse::<u32>().ok().filter(|v| (lo..=hi).contains(v))
}

impl Form {
    pub fn new(env: &Env) -> Form {
        let cfg = &env.cfg;
        if let Some(id) = &env.custom_id {
            return Form::Confirm(Request::Custom { id: id.clone() });
        }
        if let Some(what) = env.action.convert_kind(env.first_kind) {
            return Form::Format(FormatForm { what, selected: cfg.default_format(what).unwrap_or_default().to_string() });
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
            Action::Webp => Form::Webp(WebpForm {
                size_text: cfg.webp.size.to_string(),
                fps_text: cfg.webp.fps.to_string(),
                quality: cfg.webp.quality,
                compression: cfg.webp.compression,
            }),
            _ => Form::Confirm(Request::DeleteSound),
        }
    }

    /// Draws the fields; returns the request when the form is valid (None = Start is disabled)
    fn ui(&mut self, ui: &mut egui::Ui, env: &Env) -> Option<Request> {
        let t = env.t;
        match self {
            Form::Compress(f) => f.ui(ui, env),
            Form::Format(f) => {
                let options: Vec<(&str, &str, &str)> = env.cfg.formats(f.what).iter().map(|(k, fmt)| (k.as_str(), fmt.label(k), fmt.hint.as_str())).collect();
                field(ui, t.format, |ui| option_grid(ui, &mut f.selected, &options));
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
                ui.add_space(10.0);
                field(ui, t.webp_quality, |ui| slider_row(ui, &mut f.quality, 0..=100));
                field(ui, t.webp_compression, |ui| {
                    slider_row(ui, &mut f.compression, 0..=6);
                    hint(ui, t.webp_compression_hint);
                });
                Some(Request::Webp { size: size?, fps: fps?, quality: f.quality, compression: f.compression })
            }
            Form::Confirm(req) => {
                match &*req {
                    Request::Custom { id } => {
                        if let Some(item) = env.cfg.settings.custom(id) {
                            field(ui, t.command, |ui| card(ui, |ui| ui.add(egui::Label::new(RichText::new(&item.command).monospace()).wrap())));
                        }
                    }
                    _ => {
                        ui.label(t.delete_sound_text);
                        ui.add_space(8.0);
                    }
                }
                Some(req.clone())
            }
        }
    }
}

impl CompressForm {
    /// Both modes have the same rows (a control with a line under it, the codec, the resolution,
    /// the result), so the window fits either of them without empty space
    fn ui(&mut self, ui: &mut egui::Ui, env: &Env) -> Option<Request> {
        let t = env.t;
        segmented(ui, &[t.tab_presets, t.tab_limit], &mut self.tab);
        ui.add_space(2.0);
        one_line(ui, if self.tab == 0 { t.tab_presets_text } else { t.tab_limit_text }, muted());
        ui.add_space(14.0);
        let req = if self.tab == 0 { self.quality_rows(ui, env) } else { self.limit_rows(ui, env) };
        result_row(ui, env, req.as_ref());
        req
    }

    fn quality_rows(&mut self, ui: &mut egui::Ui, env: &Env) -> Option<Request> {
        let t = env.t;
        form_row(ui, t.level, |ui| {
            segmented(ui, &[t.level_low, t.level_medium, t.level_high], &mut self.level);
            one_line(ui, [t.level_low_hint, t.level_medium_hint, t.level_high_hint][self.level.min(2)], muted());
        });
        form_row(ui, t.codec, |ui| codec_combo(ui, "p_codec", &self.p_groups, &mut self.p_codec, t));
        form_row(ui, t.resolution, |ui| self.p_scale.ui(ui, "p_scale", t, false, &env.infos));
        (!self.p_scale.invalid).then(|| Request::CompressPreset { level: self.level, codec: self.p_codec.clone(), scale: self.p_scale.mode })
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
        form_row(ui, t.limit, |ui| {
            ui.horizontal(|ui| {
                ui.add_enabled_ui(fixed_limit.is_none(), |ui| {
                    ui.add(line_edit(&mut self.mb_text, 48.0, mb.is_some()));
                    ui.label(RichText::new("MB").color(muted()));
                });
                ui.add_space(18.0);
                if twopass_ok {
                    ui.checkbox(&mut self.twopass, t.two_pass);
                } else {
                    ui.add_enabled(false, egui::Checkbox::new(&mut false, t.two_pass));
                }
            });
            let note = if fixed_limit.is_some() {
                t.limit_fixed
            } else if twopass_ok {
                t.two_pass_hint
            } else {
                t.two_pass_na
            };
            one_line(ui, note, muted());
        });
        form_row(ui, t.codec, |ui| codec_combo(ui, "l_codec", &self.l_groups, &mut self.l_codec, t));
        form_row(ui, t.resolution, |ui| self.l_scale.ui(ui, "l_scale", t, true, &env.infos));
        if self.l_scale.invalid {
            return None;
        }
        Some(Request::CompressLimit { mb: mb?, codec: self.l_codec.clone(), scale: self.l_scale.mode, twopass: self.twopass && twopass_ok })
    }
}

/// The name of the result in one row of the form (of the first file; the others are counted)
fn result_row(ui: &mut egui::Ui, env: &Env, req: Option<&Request>) {
    let first = req.and_then(|r| env.files.first().and_then(|f| engine::result_name(&env.cfg, r, f)));
    let more = env.files.len().saturating_sub(1);
    form_row(ui, env.t.result, |ui| {
        ui.allocate_ui_with_layout(vec2(ui.available_width(), CONTROL_H), Layout::left_to_right(Align::Center), |ui| {
            let text = match first {
                Some(n) if more > 0 => format!("{n}  (+{more})"),
                Some(n) => n,
                None => "—".into(),
            };
            ui.add(egui::Label::new(RichText::new(text).color(c().text)).truncate());
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
    card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 3.0;
        let max = chars_fitting(ui);
        for (i, f) in env.files.iter().take(3).enumerate() {
            if i > 0 {
                ui.add_space(6.0);
            }
            let name = file_name(f);
            ui.label(bold(short(&name, max), 13.0));
            let about = env.infos.iter().find(|x| x.name == name).map(|x| media::describe(x, env.t.audio_word)).unwrap_or_default();
            if !about.is_empty() {
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
        let max = chars_fitting(ui).saturating_sub(2);
        for n in &names {
            ui.label(RichText::new(format!("→ {}", short(n, max))).color(muted()));
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
    let req = form.ui(ui, env);
    // Compress Video has the result as a row of its own
    if let (Some(r), false) = (&req, matches!(form, Form::Compress(_))) {
        results(ui, env, r);
    }
    req
}

/// (Start pressed, Cancel pressed)
fn buttons(ui: &mut egui::Ui, t: &T, valid: bool) -> (bool, bool) {
    let mut pressed = (false, false);
    button_row(ui, |ui| {
        pressed.0 = ui.add_enabled(valid, primary_button(t.start)).clicked();
        pressed.1 = ui.add(secondary_button(t.cancel)).clicked();
    });
    pressed
}

pub struct Shown {
    /// The request the form makes now (None = it is not valid)
    pub request: Option<Request>,
    pub start: bool,
    pub cancel: bool,
}

/// Draw a form window. `valid`: whether the form was valid in the frame before (the buttons
/// are drawn before the fields)
pub fn show(ctx: &egui::Context, env: &Env, form: &mut Form, valid: bool) -> Shown {
    let mut pressed = (false, false);
    let (request, _) = window(ctx, true, |ui| pressed = buttons(ui, env.t, valid), |ui| content(ui, env, form));
    Shown { request, start: pressed.0, cancel: pressed.1 }
}

/// The forms the window has to be tall enough for (both tabs of Compress Video, the longest hints)
fn measured_forms(env: &Env) -> Vec<Form> {
    let Form::Compress(_) = Form::new(env) else { return vec![Form::new(env)] };
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
}

/// Lay the form out once in an invisible context and return the size the window needs.
/// The width follows the names of the files, the height follows the content.
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
    let name_font = FontId::new(13.0, theme::bold_family());
    let names = env.files.iter().take(3).map(|f| text_width(&ctx, &file_name(f), name_font.clone()));
    let about = env.infos.iter().map(|i| text_width(&ctx, &media::describe(i, env.t.audio_word), FontId::proportional(11.5)));
    let text_w = names.chain(about).fold(0.0, f32::max);
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
