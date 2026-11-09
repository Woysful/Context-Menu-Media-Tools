//! The forms of Compress Image, Make Icon and Frames to Video. Compress Image of one picture
//! shows it at the right: turned and mirrored as it will be, with a frame to crop it.

use super::dropdown::{build_groups, codec_combo, Group};
use super::forms::{default_menu, original_check, result_row_with};
use super::scale_field::ScaleField;
use super::widgets::{card, form_row, hint, line_edit, muted, one_line, option_grid, secondary_button, segmented, slider_row};
use super::Env;
use crate::defaults::{self as dv, IMAGE_MODES, LEVELS, SQUARES};
use crate::engine::{self, ImageEdit, ImageTarget, Request};
use crate::i18n::T;
use crate::media;
use crate::theme::{c, Look, CONTROL_H};
use crate::units::{fmt_duration, fmt_float, fmt_size, parse_size};
use eframe::egui::{self, pos2, vec2, Align2, Color32, ColorImage, Pos2, Rect, RichText, Sense, Shape, Stroke, TextureHandle, TextureOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The size of the picture at the right of Compress Image
pub const PREVIEW_W: f32 = 360.0;
const PREVIEW_H: f32 = 330.0;
/// The longest side of the picture ffmpeg makes for the preview
const PREVIEW_PX: u32 = 900;
/// The ratios of the crop after Free and Source: width, height
const RATIOS: [(f64, f64); 4] = [(1.0, 1.0), (4.0, 3.0), (3.0, 2.0), (16.0, 9.0)];
const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];
const FPS: [f64; 4] = [24.0, 25.0, 30.0, 60.0];

/// A picture in memory, RGBA
#[derive(Clone)]
struct Picture {
    rgba: Arc<Vec<u8>>,
    w: usize,
    h: usize,
    /// the size of the file (turned by its EXIF orientation)
    full: (u32, u32),
}

impl Picture {
    /// The picture turned `turns` quarters clockwise, then mirrored left to right
    fn turned(&self, turns: u8, mirrored: bool) -> Picture {
        let (w, h) = (self.w, self.h);
        let k = turns % 4;
        let (ow, oh) = if k % 2 == 1 { (h, w) } else { (w, h) };
        let mut out = vec![0u8; ow * oh * 4];
        for y in 0..oh {
            for x in 0..ow {
                let x1 = if mirrored { ow - 1 - x } else { x };
                let (sx, sy) = match k {
                    1 => (y, h - 1 - x1),
                    2 => (w - 1 - x1, h - 1 - y),
                    3 => (w - 1 - y, x1),
                    _ => (x1, y),
                };
                let (s, d) = ((sy * w + sx) * 4, (y * ow + x) * 4);
                out[d..d + 4].copy_from_slice(&self.rgba[s..s + 4]);
            }
        }
        let full = if k % 2 == 1 { (self.full.1, self.full.0) } else { self.full };
        Picture { rgba: Arc::new(out), w: ow, h: oh, full }
    }
}

/// The first frame of `file`, small enough for the preview
fn load_picture(tools: &super::Tools, file: &Path) -> Result<Picture, String> {
    let full = media::picture_size(&tools.ffprobe, file).ok_or_else(|| "ffprobe".to_string())?;
    let s = (PREVIEW_PX as f64 / full.0.max(full.1) as f64).min(1.0);
    let (w, h) = (((full.0 as f64 * s).round() as usize).max(1), ((full.1 as f64 * s).round() as usize).max(1));
    let out = crate::proc::command(&tools.ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-frames:v", "1", "-vf", &format!("scale={w}:{h}:flags=bicubic,format=rgba"), "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .output()
        .map_err(|e| e.to_string())?;
    if out.stdout.len() != w * h * 4 {
        let err = String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("ffmpeg").to_string();
        return Err(err);
    }
    Ok(Picture { rgba: Arc::new(out.stdout), w, h, full })
}

/// The texture of the preview and the size of the turned picture, or why there is none
type Shown = Result<(TextureHandle, (u32, u32)), String>;
/// An estimate: what it was made for and the size
type Estimated = Option<(String, Result<u64, String>)>;

#[derive(Default)]
struct Preview {
    slot: Arc<Mutex<Option<Result<Picture, String>>>>,
    started: bool,
    /// the texture of the picture as it is turned now
    shown: Option<(TextureHandle, u8, bool, (u32, u32))>,
}

impl Preview {
    fn start(&mut self, env: &Env, ctx: &egui::Context) {
        let (Some(tools), Some(file)) = (env.tools.clone(), env.files.first().cloned()) else { return };
        self.started = true;
        let (slot, ctx) = (self.slot.clone(), ctx.clone());
        std::thread::spawn(move || {
            let r = load_picture(&tools, &file);
            *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
            ctx.request_repaint();
        });
    }

    /// The texture for this turn (made again when the turn changed); Err: why there is none
    fn texture(&mut self, ctx: &egui::Context, turns: u8, mirrored: bool) -> Option<Shown> {
        let picture = self.slot.lock().unwrap_or_else(|e| e.into_inner()).clone()?;
        let picture = match picture {
            Ok(p) => p,
            Err(e) => return Some(Err(e)),
        };
        if !matches!(&self.shown, Some((_, t, m, _)) if *t == turns && *m == mirrored) {
            let p = picture.turned(turns, mirrored);
            let image = ColorImage::from_rgba_unmultiplied([p.w, p.h], &p.rgba);
            self.shown = Some((ctx.load_texture("cmmt_preview", image, TextureOptions::LINEAR), turns, mirrored, p.full));
        }
        self.shown.as_ref().map(|(tex, _, _, full)| Ok((tex.clone(), *full)))
    }
}

/// The size of the result before Start, made in the background a moment after the last change
#[derive(Default)]
struct Estimate {
    changed: Option<(String, Instant)>,
    asked: Option<String>,
    slot: Arc<Mutex<Estimated>>,
    busy: Arc<AtomicBool>,
}

impl Estimate {
    /// The line about the size for `req` (key: what it was made for)
    fn line(&mut self, ctx: &egui::Context, env: &Env, req: &Request, t: &T) -> (String, Color32) {
        let key = format!("{req:?}");
        if let Some((k, r)) = self.slot.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            if *k == key {
                let now = env.files.first().map_or(0, |f| std::fs::metadata(f).map_or(0, |m| m.len()));
                return match r {
                    Ok(size) => (t.img_estimate.replacen("{}", &fmt_size(*size), 1).replacen("{}", &fmt_size(now), 1), muted()),
                    Err(e) => (format!("{} {}", t.img_estimate_failed, e.lines().last().unwrap_or("")), c().warn),
                };
            }
        }
        if self.changed.as_ref().is_none_or(|(k, _)| *k != key) {
            self.changed = Some((key.clone(), Instant::now()));
        }
        let since = self.changed.as_ref().map_or(Duration::ZERO, |(_, at)| at.elapsed());
        let wait = Duration::from_millis(400);
        if self.busy.load(Ordering::SeqCst) || self.asked.as_deref() == Some(key.as_str()) {
            ctx.request_repaint_after(Duration::from_millis(150));
        } else if since < wait {
            ctx.request_repaint_after(wait - since);
        } else if let (Some(tools), Some(file)) = (env.tools.clone(), env.files.first().cloned()) {
            self.asked = Some(key.clone());
            self.busy.store(true, Ordering::SeqCst);
            let (slot, busy, ctx, cfg, req) = (self.slot.clone(), self.busy.clone(), ctx.clone(), env.cfg.clone(), req.clone());
            std::thread::spawn(move || {
                let r = engine::estimate_image(cfg, tools.ffmpeg, tools.ffprobe, &req, &file);
                *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, r));
                busy.store(false, Ordering::SeqCst);
                ctx.request_repaint();
            });
        }
        (t.img_estimating.to_string(), muted())
    }
}

/// What a drag on the picture does
#[derive(Clone, Copy)]
enum Drag {
    /// a corner (or a new frame) is moved, the other corner stays here
    Corner(Pos2),
    /// the frame is moved, the pointer holds it this far from its top left corner
    Move(egui::Vec2),
}

pub struct ImageForm {
    pub(super) tab: usize,
    format: String,
    quality: u32,
    limit_text: String,
    /// 0 = KB, 1 = MB
    limit_unit: usize,
    pub(super) scale: ScaleField,
    /// quarter turns clockwise, then mirrored left to right (every turn and mirror comes to this)
    turns: u8,
    mirrored: bool,
    /// x, y, width, height as parts of the turned picture
    crop: Option<[f64; 4]>,
    /// 0 = free, 1 = the picture's own, then RATIOS
    ratio: usize,
    /// the ratio stands upright (3:4 for 4:3)
    upright: bool,
    drag: Option<Drag>,
    preview: Preview,
    estimate: Estimate,
    /// a window on the screen, not one that is measured: it loads the picture and estimates
    pub(super) live: bool,
}

/// "500 KB" -> ("500", 0), "2 MB" -> ("2", 1)
fn split_size(text: &str) -> (String, usize) {
    let t = text.trim().to_lowercase();
    let unit = usize::from(t.ends_with('m') || t.ends_with("mb"));
    (text.trim().trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == ' ').to_string(), unit)
}

impl ImageForm {
    pub fn new(env: &Env) -> ImageForm {
        let cfg = &env.cfg;
        let d = |key: &str| dv::value(cfg, key);
        let (limit_text, limit_unit) = split_size(&d(dv::IMAGE_LIMIT));
        let format = d(dv::IMAGE_FORMAT);
        let format = if available(env).iter().any(|(k, _, _)| *k == format) { format } else { engine::SOURCE.to_string() };
        ImageForm {
            tab: usize::from(d(dv::IMAGE_MODE) == "limit"),
            format,
            quality: d(dv::IMAGE_QUALITY).parse().unwrap_or(80),
            limit_text,
            limit_unit,
            scale: ScaleField::from_code(&d(dv::IMAGE_RESOLUTION)),
            turns: 0,
            mirrored: false,
            crop: None,
            ratio: 0,
            upright: false,
            drag: None,
            preview: Preview::default(),
            estimate: Estimate::default(),
            live: false,
        }
    }

    fn limit_bytes(&self) -> Option<u64> {
        parse_size(&format!("{}{}", self.limit_text.trim(), ["KB", "MB"][self.limit_unit.min(1)]))
    }

    fn limit_value(&self) -> Option<String> {
        self.limit_bytes().map(|_| format!("{} {}", self.limit_text.trim(), ["KB", "MB"][self.limit_unit.min(1)]))
    }

    fn one_file(env: &Env) -> bool {
        env.files.len() == 1
    }

    fn edit(&self, env: &Env) -> ImageEdit {
        let crop = self.crop.filter(|r| Self::one_file(env) && (r[2] < 0.999 || r[3] < 0.999));
        ImageEdit { turns: self.turns, flip_h: self.mirrored, flip_v: false, crop }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &Env, delete_original: &mut bool) -> Option<Request> {
        if !Self::one_file(env) {
            return self.rows(ui, env, delete_original);
        }
        let gap = 20.0;
        let left = (ui.available_width() - PREVIEW_W - gap).max(300.0);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let req = ui
                .vertical(|ui| {
                    ui.set_width(left);
                    ui.spacing_mut().item_spacing.x = 8.0;
                    self.rows(ui, env, delete_original)
                })
                .inner;
            ui.add_space(gap);
            ui.vertical(|ui| {
                ui.set_width(PREVIEW_W);
                ui.spacing_mut().item_spacing.x = 8.0;
                self.crop_panel(ui, env);
            });
            req
        })
        .inner
    }

    fn rows(&mut self, ui: &mut egui::Ui, env: &Env, delete_original: &mut bool) -> Option<Request> {
        let t = env.t;
        let r = segmented(ui, &[t.img_tab_quality, t.img_tab_limit], &mut self.tab);
        if let Some(v) = default_menu(&r, env, dv::IMAGE_MODE, Some(IMAGE_MODES[self.tab.min(1)])) {
            self.tab = usize::from(v == "limit");
        }
        ui.add_space(2.0);
        one_line(ui, if self.tab == 0 { t.img_tab_quality_text } else { t.img_tab_limit_text }, muted());
        ui.add_space(14.0);

        let options = available(env);
        let r = form_row(ui, t.img_format, |ui| {
            let names: Vec<(&str, &str, &str)> = options.iter().map(|(k, n, _)| (k.as_str(), n.as_str(), "")).collect();
            let r = option_grid(ui, &mut self.format, &names);
            let animated = env.infos.iter().any(|i| i.frames.unwrap_or(1) > 1);
            if animated {
                one_line(ui, t.img_first_frame, c().warn);
            } else {
                one_line(ui, &self.format_note(env), muted());
            }
            r
        });
        if let Some(v) = default_menu(&r, env, dv::IMAGE_FORMAT, Some(&self.format)) {
            self.format = v;
        }

        let png = self.format == "png"
            || (self.format == engine::SOURCE && env.files.first().and_then(|f| env.cfg.image_source_format(f)).is_some_and(|(k, _)| k == "png"));
        let target = if self.tab == 0 {
            let r = form_row(ui, t.img_quality, |ui| {
                let r = slider_row(ui, &mut self.quality, 1..=100);
                one_line(ui, if png { t.img_png_hint } else { t.img_quality_hint }, muted());
                r
            });
            if let Some(v) = default_menu(&r, env, dv::IMAGE_QUALITY, Some(&self.quality.to_string())) {
                self.quality = v.parse().unwrap_or(self.quality);
            }
            Some(ImageTarget::Quality(self.quality))
        } else {
            let bytes = self.limit_bytes();
            let r = form_row(ui, t.img_limit, |ui| {
                let r = ui
                    .horizontal(|ui| {
                        let r = ui.add(line_edit(&mut self.limit_text, 72.0, bytes.is_some()));
                        ui.add_space(4.0);
                        ui.allocate_ui(vec2(110.0, CONTROL_H), |ui| segmented(ui, &["KB", "MB"], &mut self.limit_unit)).inner.union(r)
                    })
                    .inner;
                one_line(ui, if bytes.is_some() { t.img_limit_hint } else { t.img_limit_invalid }, if bytes.is_some() { muted() } else { c().err });
                r
            });
            if let Some(v) = default_menu(&r, env, dv::IMAGE_LIMIT, self.limit_value().as_deref()) {
                (self.limit_text, self.limit_unit) = split_size(&v);
            }
            bytes.map(ImageTarget::Bytes)
        };

        let r = form_row(ui, t.resolution, |ui| self.scale.ui(ui, "img_scale", t, false, &env.infos));
        if let Some(v) = default_menu(&r, env, dv::IMAGE_RESOLUTION, self.scale.code().as_deref()) {
            self.scale = ScaleField::from_code(&v);
        }
        form_row(ui, t.img_turn, |ui| self.turn_row(ui, t));
        if !Self::one_file(env) {
            form_row(ui, t.img_crop, |ui| {
                ui.add_enabled_ui(false, |ui| segmented(ui, &ratio_labels(t), &mut 0));
                one_line(ui, t.img_crop_many, muted());
            });
        }
        form_row(ui, t.original, |ui| original_check(ui, env, delete_original, t.delete_after));

        let req = match (target, self.scale.invalid) {
            (Some(target), false) => Some(Request::CompressImage { format: self.format.clone(), target, scale: self.scale.mode, edit: self.edit(env) }),
            _ => None,
        };
        let note = match (&req, self.tab) {
            (Some(r), 0) if self.live => Some(self.estimate.line(ui.ctx(), env, r, t)),
            (_, 0) => Some((t.img_estimating.to_string(), muted())),
            _ => {
                let now = env.files.first().map_or(0, |f| std::fs::metadata(f).map_or(0, |m| m.len()));
                let limit = self.limit_bytes().map_or_else(|| "—".to_string(), fmt_size);
                Some((t.img_limit_note.replacen("{}", &limit, 1).replacen("{}", &fmt_size(now), 1), muted()))
            }
        };
        result_row_with(ui, env, req.as_ref(), note);
        req
    }

    /// The line under the formats: about the chosen one
    fn format_note(&self, env: &Env) -> String {
        let t = env.t;
        if self.format != engine::SOURCE {
            return env.cfg.image_format(&self.format).map(|f| f.hint.clone()).unwrap_or_default();
        }
        match (env.files.len(), env.files.first().and_then(|f| env.cfg.image_source_format(f))) {
            (1, Some((key, f))) => format!("{} {}", t.img_source_one, f.label(key)),
            _ => t.img_source_hint.to_string(),
        }
    }

    fn turn_row(&mut self, ui: &mut egui::Ui, t: &T) {
        ui.horizontal(|ui| {
            for (sym, tip) in [(Sym::Left, t.img_rotate_left), (Sym::Right, t.img_rotate_right), (Sym::MirrorH, t.img_flip_h), (Sym::MirrorV, t.img_flip_v)] {
                if symbol_button(ui, sym).on_hover_text(tip).clicked() {
                    self.apply(sym);
                }
            }
        });
        let mut text = match self.turns {
            0 => String::new(),
            3 => t.img_turned_left.to_string(),
            n => t.img_turned.replace("{}", &(n as u32 * 90).to_string()),
        };
        if self.mirrored {
            text = if text.is_empty() { capitalize(t.img_mirrored) } else { format!("{text}, {}", t.img_mirrored) };
        }
        one_line(ui, if text.is_empty() { t.img_turn_none } else { &text }, muted());
    }

    /// A turn or a mirror: the crop stays on the same part of the picture
    fn apply(&mut self, sym: Sym) {
        let m = self.mirrored;
        match sym {
            Sym::Right => self.turns = (self.turns + if m { 3 } else { 1 }) % 4,
            Sym::Left => self.turns = (self.turns + if m { 1 } else { 3 }) % 4,
            Sym::MirrorH => self.mirrored = !m,
            Sym::MirrorV => {
                self.mirrored = !m;
                self.turns = (self.turns + 2) % 4;
            }
        }
        if let Some([x, y, w, h]) = self.crop {
            self.crop = Some(match sym {
                Sym::Right => [1.0 - y - h, x, h, w],
                Sym::Left => [y, 1.0 - x - w, h, w],
                Sym::MirrorH => [1.0 - x - w, y, w, h],
                Sym::MirrorV => [x, 1.0 - y - h, w, h],
            });
        }
        if matches!(sym, Sym::Left | Sym::Right) {
            self.upright = !self.upright;
        }
    }

    /// The width / height of the crop in parts of the picture; None = free
    fn part_ratio(&self, full: (u32, u32)) -> Option<f64> {
        let (w, h) = match self.ratio {
            0 => return None,
            1 => (full.0 as f64, full.1 as f64),
            n => RATIOS[(n - 2).min(RATIOS.len() - 1)],
        };
        let (w, h) = if self.upright { (h, w) } else { (w, h) };
        Some(w / h * full.1.max(1) as f64 / full.0.max(1) as f64)
    }

    /// The largest frame of the ratio in the middle of the picture
    fn fit_ratio(&mut self, full: (u32, u32)) {
        self.crop = self.part_ratio(full).map(|a| if a <= 1.0 { [(1.0 - a) / 2.0, 0.0, a, 1.0] } else { [0.0, (1.0 - 1.0 / a) / 2.0, 1.0, 1.0 / a] });
    }

    fn crop_panel(&mut self, ui: &mut egui::Ui, env: &Env) {
        let t = env.t;
        if self.live && !self.preview.started {
            self.preview.start(env, ui.ctx());
        }
        let k = c();
        let (area, resp) = ui.allocate_exact_size(vec2(PREVIEW_W, PREVIEW_H), Sense::click_and_drag());
        let painter = ui.painter().clone();
        let frame_stroke = Stroke::new(1.0_f32, k.border);
        match k.look {
            Look::Y2k => {
                super::paint::cut_rect(&painter, area, 10.0, super::paint::DIAGONAL, k.surface, frame_stroke);
            }
            Look::Brutal => {
                painter.rect(area, 0.0, k.surface, frame_stroke);
            }
            Look::Classic => {
                painter.rect(area, 4.0, k.surface, frame_stroke);
            }
        }
        let mut full = None;
        match self.preview.texture(ui.ctx(), self.turns, self.mirrored) {
            None => {
                painter.text(area.center(), Align2::CENTER_CENTER, t.img_preview_loading, egui::FontId::proportional(12.0), muted());
            }
            Some(Err(e)) => {
                let text = format!("{} {e}", t.img_preview_failed);
                let galley = painter.layout(text, egui::FontId::proportional(12.0), k.warn, area.width() - 24.0);
                painter.galley(area.center() - galley.size() / 2.0, galley, k.warn);
            }
            Some(Ok((tex, size))) => {
                full = Some(size);
                let [tw, th] = tex.size().map(|v| v as f32);
                let room = area.shrink(10.0);
                let s = (room.width() / tw).min(room.height() / th);
                let pic = Rect::from_center_size(area.center(), vec2(tw * s, th * s));
                painter.image(tex.id(), pic, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                self.drag_crop(&resp, pic, size);
                if let Some(r) = self.crop {
                    draw_crop(&painter, pic, r);
                }
            }
        }
        if full.is_some() {
            resp.on_hover_cursor(egui::CursorIcon::Crosshair);
        }
        ui.add_space(8.0);
        let labels = ratio_labels(t);
        let before = self.ratio;
        let r = ui.add_enabled_ui(full.is_some(), |ui| segmented(ui, &labels, &mut self.ratio)).inner.on_hover_text(t.img_crop_upright);
        // a ratio clicked again stands up (or lies down)
        let again = r.clicked() && self.ratio == before && self.ratio > 1;
        if again {
            self.upright = !self.upright;
        }
        if let (Some(full), true) = (full, r.changed() || again) {
            self.fit_ratio(full);
        }
        ui.add_space(4.0);
        let note = match (self.crop, full) {
            (Some([_, _, w, h]), Some((fw, fh))) => {
                let (pw, ph) = ((w * fw as f64).round() as u32, (h * fh as f64).round() as u32);
                t.img_crop_size.replace("{}", &format!("{pw}×{ph}"))
            }
            _ => t.img_crop_hint.to_string(),
        };
        ui.allocate_ui_with_layout(vec2(ui.available_width(), CONTROL_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let reset = ui.add_enabled(self.crop.is_some(), secondary_button(t.img_crop_reset));
            if reset.clicked() {
                self.crop = None;
                self.ratio = 0;
            }
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(note).small().color(muted())).truncate());
            });
        });
    }

    /// Drag on the picture: a new frame, a corner of the frame, or the whole frame
    fn drag_crop(&mut self, resp: &egui::Response, pic: Rect, full: (u32, u32)) {
        let Some(p) = resp.interact_pointer_pos() else {
            self.drag = None;
            return;
        };
        let to_part = |p: Pos2| pos2(((p.x - pic.left()) / pic.width()).clamp(0.0, 1.0), ((p.y - pic.top()) / pic.height()).clamp(0.0, 1.0));
        if resp.drag_started() {
            let at = to_part(p);
            self.drag = Some(match self.crop {
                Some(r) => {
                    let rect = crop_rect(pic, r);
                    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
                    match corners.iter().position(|c| c.distance(p) < 12.0) {
                        Some(i) => Drag::Corner(to_part(corners[(i + 2) % 4])),
                        None if rect.contains(p) => Drag::Move(at - pos2(r[0] as f32, r[1] as f32)),
                        None => Drag::Corner(at),
                    }
                }
                None => Drag::Corner(at),
            });
        }
        if !resp.dragged() {
            return;
        }
        let at = to_part(p);
        match self.drag {
            Some(Drag::Corner(anchor)) => {
                let (dx, dy) = ((at.x - anchor.x) as f64, (at.y - anchor.y) as f64);
                let (mut w, mut h) = (dx.abs(), dy.abs());
                let room_w = if dx < 0.0 { anchor.x as f64 } else { 1.0 - anchor.x as f64 };
                let room_h = if dy < 0.0 { anchor.y as f64 } else { 1.0 - anchor.y as f64 };
                if let Some(a) = self.part_ratio(full) {
                    if w / h.max(1e-9) > a {
                        h = w / a;
                    } else {
                        w = h * a;
                    }
                    if w > room_w {
                        w = room_w;
                        h = w / a;
                    }
                    if h > room_h {
                        h = room_h;
                        w = h * a;
                    }
                }
                let (w, h) = (w.clamp(0.01, room_w.max(0.01)), h.clamp(0.01, room_h.max(0.01)));
                let x = if dx < 0.0 { anchor.x as f64 - w } else { anchor.x as f64 };
                let y = if dy < 0.0 { anchor.y as f64 - h } else { anchor.y as f64 };
                self.crop = Some([x.max(0.0), y.max(0.0), w, h]);
            }
            Some(Drag::Move(off)) => {
                if let Some([_, _, w, h]) = self.crop {
                    let x = ((at.x - off.x) as f64).clamp(0.0, 1.0 - w);
                    let y = ((at.y - off.y) as f64).clamp(0.0, 1.0 - h);
                    self.crop = Some([x, y, w, h]);
                }
            }
            None => {}
        }
    }
}

/// "2.5 s" for a short video, "1:05" for a longer one
fn length(sec: f64) -> String {
    if sec < 60.0 {
        format!("{} s", fmt_float((sec * 10.0).round() / 10.0))
    } else {
        fmt_duration(sec)
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

fn ratio_labels(t: &T) -> Vec<&str> {
    vec![t.img_crop_free, t.img_crop_source, "1:1", "4:3", "3:2", "16:9"]
}

/// The formats this ffmpeg can make: (key, name, hint); "As source" first
fn available(env: &Env) -> Vec<(String, String, String)> {
    let t = env.t;
    let mut out = vec![(engine::SOURCE.to_string(), t.img_source.to_string(), t.img_source_hint.to_string())];
    for (key, f) in &env.cfg.image_compress {
        if env.encoders.is_empty() || f.encoder.is_empty() || env.encoders.contains(&f.encoder) {
            out.push((key.clone(), f.label(key).to_string(), f.hint.clone()));
        }
    }
    out
}

fn crop_rect(pic: Rect, r: [f64; 4]) -> Rect {
    let [x, y, w, h] = r.map(|v| v as f32);
    Rect::from_min_size(pos2(pic.left() + x * pic.width(), pic.top() + y * pic.height()), vec2(w * pic.width(), h * pic.height()))
}

/// The frame of the crop: the rest of the picture darker, thirds and corners
fn draw_crop(p: &egui::Painter, pic: Rect, r: [f64; 4]) {
    let k = c();
    let rect = crop_rect(pic, r);
    let shade = Color32::from_black_alpha(150);
    // one mesh: rectangles of their own would leave light seams where their soft edges meet
    let mut mesh = egui::epaint::Mesh::default();
    for part in [
        Rect::from_min_max(pic.left_top(), pos2(pic.right(), rect.top())),
        Rect::from_min_max(pos2(pic.left(), rect.bottom()), pic.right_bottom()),
        Rect::from_min_max(pos2(pic.left(), rect.top()), pos2(rect.left(), rect.bottom())),
        Rect::from_min_max(pos2(rect.right(), rect.top()), pos2(pic.right(), rect.bottom())),
    ] {
        if part.is_positive() {
            mesh.add_colored_rect(part, shade);
        }
    }
    p.add(Shape::mesh(mesh));
    let thin = Stroke::new(1.0_f32, Color32::from_white_alpha(70));
    for i in 1..3 {
        let f = i as f32 / 3.0;
        let x = rect.left() + rect.width() * f;
        let y = rect.top() + rect.height() * f;
        p.line_segment([pos2(x, rect.top()), pos2(x, rect.bottom())], thin);
        p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], thin);
    }
    p.rect_stroke(rect, 0.0, Stroke::new(1.5_f32, k.accent));
    for corner in [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()] {
        p.rect_filled(Rect::from_center_size(corner, vec2(8.0, 8.0)), 0.0, k.accent);
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Sym {
    Left,
    Right,
    MirrorH,
    MirrorV,
}

/// A small button with a drawn symbol (the fonts have no arrows that turn)
fn symbol_button(ui: &mut egui::Ui, sym: Sym) -> egui::Response {
    let resp = ui.add(secondary_button("").min_size(vec2(46.0, CONTROL_H)));
    let k = c();
    let color = if ui.is_enabled() { k.text } else { k.muted };
    let p = ui.painter();
    let m = resp.rect.center();
    let stroke = Stroke::new(1.6_f32, color);
    match sym {
        Sym::Left | Sym::Right => {
            let dir = if sym == Sym::Right { 1.0 } else { -1.0 };
            let r = 6.5;
            // an arc from the lower left over the top to the right, with the arrow at its end
            let pts: Vec<Pos2> = (0..=16)
                .map(|i| {
                    let a = (-200.0 + 220.0 * i as f32 / 16.0).to_radians();
                    pos2(m.x + dir * r * a.cos(), m.y + r * a.sin())
                })
                .collect();
            let end = *pts.last().unwrap_or(&m);
            p.add(Shape::line(pts, stroke));
            let a = 20.0_f32.to_radians();
            // the direction of travel at the end of the arc (clockwise for Right)
            let tangent = vec2(-dir * a.sin(), a.cos());
            let side = vec2(-tangent.y, tangent.x);
            let tip = end + tangent * 3.5;
            p.add(Shape::convex_polygon(vec![tip, end - tangent * 1.5 + side * 3.5, end - tangent * 1.5 - side * 3.5], color, Stroke::NONE));
        }
        Sym::MirrorH | Sym::MirrorV => {
            let flip = |v: egui::Vec2| if sym == Sym::MirrorV { vec2(v.y, v.x) } else { v };
            let pt = |x: f32, y: f32| m + flip(vec2(x, y));
            p.add(Shape::convex_polygon(vec![pt(-2.5, -6.0), pt(-2.5, 6.0), pt(-9.0, 6.0)], color, Stroke::NONE));
            p.add(Shape::closed_line(vec![pt(2.5, -6.0), pt(2.5, 6.0), pt(9.0, 6.0)], Stroke::new(1.3_f32, color)));
            p.add(Shape::dashed_line(&[pt(0.0, -9.0), pt(0.0, 9.0)], Stroke::new(1.0_f32, color), 2.0, 2.0));
        }
    }
    resp
}

// ------------------------------------------------------------------ Make Icon

pub struct IconForm {
    sizes: Vec<(u32, bool)>,
    /// 0 = transparent borders, 1 = the middle
    square: usize,
}

impl IconForm {
    pub fn new(env: &Env) -> IconForm {
        let on = dv::parse_sizes(&dv::value(&env.cfg, dv::ICON_SIZES)).unwrap_or_else(|| env.cfg.icon.sizes.clone());
        IconForm { sizes: SIZES.iter().map(|s| (*s, on.contains(s))).collect(), square: usize::from(dv::value(&env.cfg, dv::ICON_SQUARE) == "crop") }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &Env, delete_original: &mut bool) -> Option<Request> {
        let t = env.t;
        let ticked: Vec<u32> = self.sizes.iter().filter(|(_, on)| *on).map(|(s, _)| *s).collect();
        let r = form_row(ui, t.icon_sizes, |ui| {
            let r = ui
                .horizontal(|ui| {
                    ui.set_min_height(CONTROL_H);
                    let mut all: Option<egui::Response> = None;
                    for (size, on) in &mut self.sizes {
                        let r = ui.checkbox(on, size.to_string());
                        all = Some(match all {
                            Some(a) => a.union(r),
                            None => r,
                        });
                    }
                    all.unwrap_or_else(|| ui.label(""))
                })
                .inner;
            if ticked.is_empty() {
                one_line(ui, t.icon_tick, c().err);
            } else {
                one_line(ui, t.icon_sizes_hint, muted());
            }
            r
        });
        let value = (!ticked.is_empty()).then(|| dv::sizes_text(&ticked));
        if let Some(v) = default_menu(&r, env, dv::ICON_SIZES, value.as_deref()) {
            let on = dv::parse_sizes(&v).unwrap_or_default();
            for (s, tick) in &mut self.sizes {
                *tick = on.contains(s);
            }
        }
        let r = form_row(ui, t.icon_square, |ui| {
            let r = segmented(ui, &[t.icon_pad, t.icon_crop], &mut self.square);
            one_line(ui, if self.square == 0 { t.icon_pad_hint } else { t.icon_crop_hint }, muted());
            r
        });
        if let Some(v) = default_menu(&r, env, dv::ICON_SQUARE, Some(SQUARES[self.square.min(1)])) {
            self.square = usize::from(v == "crop");
        }
        form_row(ui, t.original, |ui| original_check(ui, env, delete_original, t.delete_after));
        let ticked: Vec<u32> = self.sizes.iter().filter(|(_, on)| *on).map(|(s, _)| *s).collect();
        let crop = self.square == 1;
        let req = (!ticked.is_empty()).then_some(Request::Icon { sizes: ticked, crop });
        result_row_with(ui, env, req.as_ref(), None);
        req
    }
}

// ------------------------------------------------------------------ Frames to Video

/// A video of Frames to Video: its pictures and the size of the first one
pub struct SetInfo {
    pub set: engine::FrameSet,
    pub size: Option<(u32, u32)>,
}

pub struct FramesForm {
    /// FPS, then "own"
    fps: usize,
    own: String,
    groups: Vec<Group>,
    codec: String,
    level: usize,
}

impl FramesForm {
    pub fn new(env: &Env) -> FramesForm {
        let cfg = &env.cfg;
        let fps_text = dv::value(cfg, dv::FRAMES_FPS);
        let fps_value = dv::parse_fps(&fps_text).unwrap_or(30.0);
        let fps = FPS.iter().position(|f| (f - fps_value).abs() < 1e-6).unwrap_or(FPS.len());
        FramesForm {
            fps,
            own: if fps == FPS.len() { fps_text } else { String::new() },
            groups: build_groups(&cfg.compress, env.t, cfg),
            codec: dv::value(cfg, dv::FRAMES_CODEC),
            level: dv::level_index(&dv::value(cfg, dv::FRAMES_LEVEL)),
        }
    }

    fn fps_value(&self) -> Option<f64> {
        FPS.get(self.fps).copied().or_else(|| dv::parse_fps(&self.own))
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &Env) -> Option<Request> {
        let t = env.t;
        let sets = &env.sets;
        let usable = sets.iter().filter(|s| s.set.frames.len() >= 2).count();
        form_row(ui, t.fv_frames, |ui| {
            card(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                if sets.is_empty() {
                    ui.label(RichText::new(t.fv_no_frames).color(c().err));
                }
                for s in sets.iter().take(3) {
                    let mut about = format!("{} {}", s.set.frames.len(), t.fv_pictures_word);
                    if let Some((w, h)) = s.size {
                        about.push_str(&format!(" · {w}×{h}"));
                    }
                    let name = engine::set_name(&s.set);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(name).strong());
                        ui.label(RichText::new(about).small().color(if s.set.frames.len() < 2 { c().err } else { muted() }));
                    });
                }
                if sets.len() > 3 {
                    hint(ui, &format!("+{} {}", sets.len() - 3, t.more_word));
                }
            });
            let note = if sets.len() > 1 {
                format!("{} {}", t.fv_videos.replace("{}", &sets.len().to_string()), t.fv_sizes_hint)
            } else {
                t.fv_sizes_hint.to_string()
            };
            one_line(ui, &note, muted());
        });

        let fps = self.fps_value();
        let r = form_row(ui, t.fv_fps, |ui| {
            let r = ui
                .horizontal(|ui| {
                    let labels = ["24", "25", "30", "60", t.fv_own];
                    let w = ui.available_width() - 72.0;
                    let r = ui.allocate_ui(vec2(w, CONTROL_H), |ui| segmented(ui, &labels, &mut self.fps)).inner;
                    let own = ui.add_enabled(self.fps == FPS.len(), line_edit(&mut self.own, 64.0, self.fps != FPS.len() || fps.is_some()));
                    r.union(own)
                })
                .inner;
            match (fps, sets.first()) {
                (Some(f), Some(s)) => {
                    let n = s.set.frames.len();
                    one_line(
                        ui,
                        &t.fv_fps_hint.replacen("{}", &n.to_string(), 1).replacen("{}", &fmt_float(f), 1).replacen("{}", &length(n as f64 / f), 1),
                        muted(),
                    )
                }
                (Some(_), None) => one_line(ui, "", muted()),
                (None, _) => one_line(ui, t.fv_fps_invalid, c().err),
            }
            r
        });
        if let Some(v) = default_menu(&r, env, dv::FRAMES_FPS, fps.map(fmt_float).as_deref()) {
            let f = dv::parse_fps(&v).unwrap_or(30.0);
            self.fps = FPS.iter().position(|x| (x - f).abs() < 1e-6).unwrap_or(FPS.len());
            self.own = if self.fps == FPS.len() { v } else { String::new() };
        }
        let r = form_row(ui, t.codec, |ui| codec_combo(ui, "fv_codec", &self.groups, &mut self.codec, t));
        if let Some(v) = default_menu(&r, env, dv::FRAMES_CODEC, Some(&self.codec)) {
            self.codec = v;
        }
        let r = form_row(ui, t.level, |ui| {
            let r = segmented(ui, &[t.level_low, t.level_medium, t.level_high], &mut self.level);
            one_line(ui, [t.level_low_hint, t.level_medium_hint, t.level_high_hint][self.level.min(2)], muted());
            r
        });
        if let Some(v) = default_menu(&r, env, dv::FRAMES_LEVEL, Some(LEVELS[self.level.min(2)])) {
            self.level = dv::level_index(&v);
        }
        let req = match (fps, usable) {
            (Some(fps), n) if n > 0 => Some(Request::FramesVideo { fps, codec: self.codec.clone(), level: self.level }),
            _ => None,
        };
        self.result(ui, env, req.as_ref());
        req
    }

    /// The names of the videos (of the first one, the others are counted)
    fn result(&self, ui: &mut egui::Ui, env: &Env, req: Option<&Request>) {
        let names: Vec<PathBuf> =
            env.sets.iter().filter(|s| s.set.frames.len() >= 2).map(|s| if s.set.chosen { s.set.folder.clone() } else { s.set.frames[0].clone() }).collect();
        super::forms::result_names(ui, env, req, &names, None);
    }
}
