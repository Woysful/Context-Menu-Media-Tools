//! The forms of Compress Image, Make Icon and Frames to Video. Compress Image of one picture
//! shows it at the right: turned and mirrored as it will be, with a frame to crop it.

use super::dropdown::{build_groups, codec_combo, Group};
use super::forms::{default_menu, original_check, result_row_with};
use super::scale_field::ScaleField;
use super::widgets::{
    button_row, card, form_row, hint, line_edit, muted, one_line, option_grid, primary_button, secondary_button, segmented, slider_row, title, window,
};
use super::Env;
use crate::defaults::{self as dv, IMAGE_MODES, LEVELS, SQUARES};
use crate::engine::{self, ImageEdit, ImageTarget, Request};
use crate::i18n::T;
use crate::media;
use crate::theme::{bold_family, c, tech_family, Look, CONTROL_H};
use crate::units::{fmt_duration, fmt_float, fmt_size, parse_size};
use eframe::egui::{
    self, pos2, vec2, Align2, Color32, ColorImage, Pos2, Rect, RichText, Sense, Shape, Stroke, TextureFilter, TextureHandle, TextureOptions, Vec2,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The width of the fields of Compress Image of one picture; the picture at their right takes
/// the rest of the window
pub const FIELDS_W: f32 = 460.0;
pub const GAP: f32 = 20.0;
/// The picture at the right: the narrowest and widest it starts with, the height it starts with
const PREVIEW_MIN_W: f32 = 340.0;
const PREVIEW_START_W: f32 = 560.0;
const PREVIEW_START_H: f32 = 470.0;
const PREVIEW_MIN_H: f32 = 240.0;
/// The room between the picture and the edge of its box
const PREVIEW_PAD: f32 = 10.0;
/// The longest side of the picture ffmpeg makes for the preview (sharp on the whole window too)
const PREVIEW_PX: u32 = 1600;
/// The magnifier: points to a pixel and the side of its square, the least and the most
const LOUPE_ZOOM: (f32, f32) = (2.0, 40.0);
const LOUPE_SIDE: (f32, f32) = (120.0, 480.0);
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

/// The first frame of `file`, its longest side at most `max_px`; `turn`: turned (quarters
/// clockwise) and mirrored by ffmpeg
fn load_picture(tools: &super::Tools, file: &Path, max_px: u32, turn: Option<(u8, bool)>) -> Result<Picture, String> {
    let full = media::picture_size(&tools.ffprobe, file).ok_or_else(|| "ffprobe".to_string())?;
    let s = (max_px as f64 / full.0.max(full.1) as f64).min(1.0);
    let (w, h) = (((full.0 as f64 * s).round() as usize).max(1), ((full.1 as f64 * s).round() as usize).max(1));
    let mut vf = format!("scale={w}:{h}:flags=bicubic");
    let (turns, mirrored) = turn.unwrap_or((0, false));
    vf.push_str(match turns % 4 {
        1 => ",transpose=1",
        2 => ",hflip,vflip",
        3 => ",transpose=2",
        _ => "",
    });
    if mirrored {
        vf.push_str(",hflip");
    }
    vf.push_str(",format=rgba");
    let out = crate::proc::command(&tools.ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-frames:v", "1", "-vf", &vf, "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .output()
        .map_err(|e| e.to_string())?;
    if out.stdout.len() != w * h * 4 {
        let err = String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or("ffmpeg").to_string();
        return Err(err);
    }
    let (w, h, full) = if turns % 2 == 1 { (h, w, (full.1, full.0)) } else { (w, h, full) };
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
            let r = load_picture(&tools, &file, PREVIEW_PX, None);
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
            // mipmaps: the picture is mostly drawn much smaller than it is
            let options = TextureOptions { mipmap_mode: Some(TextureFilter::Linear), ..TextureOptions::LINEAR };
            self.shown = Some((ctx.load_texture("cmmt_preview", image, options), turns, mirrored, p.full));
        }
        self.shown.as_ref().map(|(tex, _, _, full)| Ok((tex.clone(), *full)))
    }
}

/// A picture made for a turn and a mirror
type Turned = ((u8, bool), ColorImage);

/// The picture at its own size (as far as the graphics card takes it) for the magnifier,
/// turned and mirrored by ffmpeg
#[derive(Default)]
struct Loupe {
    asked: Option<(u8, bool)>,
    slot: Arc<Mutex<Option<Turned>>>,
    tex: Option<TextureHandle>,
}

impl Loupe {
    /// The texture for this turn; None until it is loaded
    fn texture(&mut self, env: &Env, ctx: &egui::Context, turn: (u8, bool)) -> Option<TextureHandle> {
        if let Some((key, image)) = self.slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
            // a picture of a turn asked for before comes too late
            if Some(key) == self.asked {
                self.tex = Some(ctx.load_texture("cmmt_loupe", image, TextureOptions::NEAREST));
            }
        }
        if self.asked != Some(turn) {
            self.tex = None;
            let (Some(tools), Some(file)) = (env.tools.clone(), env.files.first().cloned()) else { return None };
            self.asked = Some(turn);
            let max = ctx.input(|i| i.max_texture_side).min(8192) as u32;
            let (slot, ctx) = (self.slot.clone(), ctx.clone());
            std::thread::spawn(move || {
                if let Ok(p) = load_picture(&tools, &file, max, Some(turn)) {
                    let image = ColorImage::from_rgba_unmultiplied([p.w, p.h], &p.rgba);
                    *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some((turn, image));
                    ctx.request_repaint();
                }
            });
        }
        self.tex.clone()
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
    /// an edge (left, top, right, bottom) is moved; the frame as it was when the drag started
    Edge(usize, [f64; 4]),
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
    /// the lowest edge of the window's content that is seen without scrolling (the picture
    /// grows down to it)
    pub(super) view_bottom: Option<f32>,
    /// the height of what is under the picture, as it was drawn last
    below: f32,
    /// the crop takes the whole window
    pub(super) big: bool,
    /// the corner or the point of the edge that is dragged (parts of the picture)
    focus: Option<Pos2>,
    loupe: Loupe,
    loupe_on: bool,
    loupe_zoom: f32,
    loupe_side: f32,
}

/// The width the picture at the right of Compress Image starts with: about as wide as the
/// picture is at the height it starts with
pub fn preview_width(env: &Env) -> f32 {
    let aspect = env.infos.first().filter(|i| i.width > 0 && i.height > 0).map_or(1.0, |i| i.width as f32 / i.height as f32);
    ((PREVIEW_START_H - 2.0 * PREVIEW_PAD) * aspect + 2.0 * PREVIEW_PAD).clamp(PREVIEW_MIN_W, PREVIEW_START_W)
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
            view_bottom: None,
            below: 8.0 + 4.0 + 2.0 * CONTROL_H,
            big: false,
            focus: None,
            loupe: Loupe::default(),
            loupe_on: false,
            loupe_zoom: 8.0,
            loupe_side: 200.0,
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
        // the fields keep their width, a wider window widens the picture
        let avail = ui.available_width();
        let left = FIELDS_W.min(avail - GAP - PREVIEW_MIN_W).max(300.0);
        let right = (avail - left - GAP).max(PREVIEW_MIN_W);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let fields = ui.vertical(|ui| {
                ui.set_width(left);
                ui.spacing_mut().item_spacing.x = 8.0;
                self.rows(ui, env, delete_original)
            });
            ui.add_space(GAP);
            ui.vertical(|ui| {
                ui.set_width(right);
                ui.spacing_mut().item_spacing.x = 8.0;
                self.crop_panel(ui, env, fields.response.rect.height());
            });
            fields.inner
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

    /// The width / height of the picture as it is turned now
    fn aspect(&self, env: &Env) -> Option<f32> {
        let i = env.infos.first().filter(|i| i.width > 0 && i.height > 0)?;
        let a = i.width as f32 / i.height as f32;
        Some(if self.turns % 2 == 1 { 1.0 / a } else { a })
    }

    /// The picture at the right: as tall as the fields, taller when the window has room for
    /// it (but not taller than the picture needs at this width)
    fn crop_panel(&mut self, ui: &mut egui::Ui, env: &Env, fields_h: f32) {
        let t = env.t;
        let w = ui.available_width();
        let top = ui.cursor().top();
        let mut h = fields_h - self.below;
        if let (true, Some(bottom)) = (self.live, self.view_bottom) {
            let room = bottom - top - self.below;
            let need = self.aspect(env).map_or(room, |a| (w - 2.0 * PREVIEW_PAD) / a + 2.0 * PREVIEW_PAD);
            h = h.max(room.min(need));
        }
        let full = self.picture(ui, env, vec2(w, h.max(PREVIEW_MIN_H)), false);
        let box_bottom = ui.min_rect().bottom();
        ui.add_space(8.0);
        self.ratio_row(ui, t, full);
        ui.add_space(4.0);
        let note = self.crop_note(t, full);
        ui.allocate_ui_with_layout(vec2(ui.available_width(), CONTROL_H), egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(self.crop.is_some(), secondary_button(t.img_crop_reset)).clicked() {
                self.reset_crop();
            }
            ui.add_space(4.0);
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.add(egui::Label::new(RichText::new(note).small().color(muted())).truncate());
            });
        });
        self.below = ui.min_rect().bottom() - box_bottom;
    }

    /// The picture with the frame of the crop in a box of `size`; a drag on it crops. `big`: on
    /// the whole window, with the magnifier when it is on; otherwise a button in the corner opens
    /// the whole window. Returns the size of the turned picture once it is loaded.
    fn picture(&mut self, ui: &mut egui::Ui, env: &Env, size: Vec2, big: bool) -> Option<(u32, u32)> {
        let t = env.t;
        if self.live && !self.preview.started {
            self.preview.start(env, ui.ctx());
        }
        let k = c();
        let (area, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
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
                let room = area.shrink(PREVIEW_PAD);
                let s = (room.width() / tw).min(room.height() / th);
                let pic = Rect::from_center_size(area.center(), vec2(tw * s, th * s));
                painter.image(tex.id(), pic, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                self.drag_crop(&resp, pic, size);
                if let Some(r) = self.crop {
                    draw_crop(&painter, pic, r, Some(size));
                }
                self.set_cursor(ui.ctx(), &resp, pic);
                if big && self.loupe_on {
                    let fine = self.loupe.texture(env, ui.ctx(), (self.turns, self.mirrored));
                    // only while a corner or an edge is dragged
                    if let (true, Some(focus), Some(pointer)) = (resp.dragged(), self.focus, resp.interact_pointer_pos()) {
                        self.wheel(ui.ctx());
                        draw_loupe(ui.ctx(), fine.as_ref().unwrap_or(&tex), size, focus, pointer, self.crop, self.loupe_zoom, self.loupe_side);
                    }
                }
            }
        }
        if full.is_some() && !big && expand_button(ui, area, t.img_crop_expand) {
            self.big = true;
        }
        full
    }

    /// The ratios of the crop; a ratio clicked again stands up (or lies down)
    fn ratio_row(&mut self, ui: &mut egui::Ui, t: &T, full: Option<(u32, u32)>) {
        let labels = ratio_labels(t);
        let before = self.ratio;
        let r = ui.add_enabled_ui(full.is_some(), |ui| segmented(ui, &labels, &mut self.ratio)).inner.on_hover_text(t.img_crop_upright);
        let again = r.clicked() && self.ratio == before && self.ratio > 1;
        if again {
            self.upright = !self.upright;
        }
        if let (Some(full), true) = (full, r.changed() || again) {
            self.fit_ratio(full);
        }
    }

    /// The size of the crop in pixels, or how to crop
    fn crop_note(&self, t: &T, full: Option<(u32, u32)>) -> String {
        match (self.crop, full) {
            (Some([_, _, w, h]), Some((fw, fh))) => {
                let (pw, ph) = ((w * fw as f64).round() as u32, (h * fh as f64).round() as u32);
                t.img_crop_size.replace("{}", &format!("{pw}×{ph}"))
            }
            _ => t.img_crop_hint.to_string(),
        }
    }

    fn reset_crop(&mut self) {
        self.crop = None;
        self.ratio = 0;
    }

    /// The crop on the whole window: the picture, the ratios under it, Done at the bottom.
    /// Done or Escape go back to the form.
    pub fn crop_window(&mut self, ctx: &egui::Context, env: &Env) {
        let t = env.t;
        let note = self.crop_note(t, self.preview.shown.as_ref().map(|s| s.3));
        let (mut done, mut reset, cropped) = (false, false, self.crop.is_some());
        window(
            ctx,
            false,
            |ui| {
                button_row(ui, |ui| {
                    done = ui.add(primary_button(t.img_crop_done)).clicked();
                    reset = ui.add_enabled(cropped, secondary_button(t.img_crop_reset)).clicked();
                    ui.add_space(6.0);
                    ui.add(egui::Label::new(RichText::new(note).small().color(muted())).truncate());
                });
            },
            |ui| {
                title(ui, t.img_crop, c().text);
                ui.add_space(12.0);
                let size = vec2(ui.available_width(), (ui.available_height() - 8.0 - CONTROL_H - 14.0).max(120.0));
                let full = self.picture(ui, env, size, true);
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    let toggle_w = 150.0;
                    let w = (ui.available_width() - toggle_w - 12.0).min(560.0);
                    ui.allocate_ui(vec2(w, CONTROL_H), |ui| self.ratio_row(ui, t, full));
                    // one part that is chosen while the magnifier is on
                    let mut part = usize::from(!self.loupe_on);
                    let r = ui
                        .allocate_ui(vec2(toggle_w, CONTROL_H), |ui| ui.add_enabled_ui(full.is_some(), |ui| segmented(ui, &[t.img_loupe], &mut part)).inner)
                        .inner;
                    if r.on_hover_text(t.img_loupe_hint).clicked() {
                        self.loupe_on = !self.loupe_on;
                    }
                });
            },
        );
        if reset {
            self.reset_crop();
        }
        if done || ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !ctx.memory(|m| m.any_popup_open()) {
            self.big = false;
            ctx.request_repaint();
        }
    }
    /// Drag on the picture: a new frame, a corner or an edge of the frame, or the whole frame.
    /// The frame stays on whole pixels; `focus` is the corner or the point of the edge that moves.
    fn drag_crop(&mut self, resp: &egui::Response, pic: Rect, full: (u32, u32)) {
        self.focus = None;
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
                    match grip(rect, p) {
                        Grip::Corner(i) => Drag::Corner(to_part(corners[(i + 2) % 4])),
                        Grip::Edge(i) => Drag::Edge(i, r),
                        Grip::Inside => Drag::Move(at - pos2(r[0] as f32, r[1] as f32)),
                        Grip::Outside => Drag::Corner(at),
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
                let r = snap([x.max(0.0), y.max(0.0), w, h], full);
                let fx = if dx < 0.0 { r[0] } else { r[0] + r[2] };
                let fy = if dy < 0.0 { r[1] } else { r[1] + r[3] };
                self.focus = Some(pos2(fx as f32, fy as f32));
                self.crop = Some(r);
            }
            Some(Drag::Edge(side, start)) => {
                let r = snap(edge_drag(side, start, at, self.part_ratio(full)), full);
                let along_x = (at.x as f64).clamp(r[0], r[0] + r[2]) as f32;
                let along_y = (at.y as f64).clamp(r[1], r[1] + r[3]) as f32;
                self.focus = Some(match side {
                    0 => pos2(r[0] as f32, along_y),
                    1 => pos2(along_x, r[1] as f32),
                    2 => pos2((r[0] + r[2]) as f32, along_y),
                    _ => pos2(along_x, (r[1] + r[3]) as f32),
                });
                self.crop = Some(r);
            }
            Some(Drag::Move(off)) => {
                if let Some([_, _, w, h]) = self.crop {
                    let (fw, fh) = (full.0.max(1) as f64, full.1.max(1) as f64);
                    let x = (((at.x - off.x) as f64).clamp(0.0, 1.0 - w) * fw).round() / fw;
                    let y = (((at.y - off.y) as f64).clamp(0.0, 1.0 - h) * fh).round() / fh;
                    self.crop = Some([x.min(1.0 - w).max(0.0), y.min(1.0 - h).max(0.0), w, h]);
                }
            }
            None => {}
        }
    }

    /// The cursor tells what a drag would do
    fn set_cursor(&self, ctx: &egui::Context, resp: &egui::Response, pic: Rect) {
        if !(resp.hovered() || resp.dragged()) {
            return;
        }
        let icon = match (resp.dragged(), self.drag, self.crop, resp.hover_pos()) {
            (true, Some(Drag::Edge(i, _)), _, _) => grip_cursor(Grip::Edge(i)),
            (true, Some(Drag::Move(_)), _, _) => egui::CursorIcon::Grabbing,
            (true, _, _, _) => egui::CursorIcon::Crosshair,
            (false, _, Some(r), Some(p)) => grip_cursor(grip(crop_rect(pic, r), p)),
            _ => egui::CursorIcon::Crosshair,
        };
        ctx.set_cursor_icon(icon);
    }

    /// The wheel zooms the magnifier, Ctrl + wheel makes it larger or smaller
    fn wheel(&mut self, ctx: &egui::Context) {
        let (mut zoom, mut side) = (0.0_f32, 0.0_f32);
        ctx.input(|i| {
            for e in &i.events {
                if let egui::Event::MouseWheel { unit, delta, modifiers } = e {
                    let notches = match unit {
                        egui::MouseWheelUnit::Line => delta.y,
                        egui::MouseWheelUnit::Page => delta.y * 3.0,
                        egui::MouseWheelUnit::Point => delta.y / 50.0,
                    };
                    if modifiers.ctrl || modifiers.command {
                        side += notches;
                    } else {
                        zoom += notches;
                    }
                }
            }
        });
        if zoom != 0.0 {
            self.loupe_zoom = (self.loupe_zoom * 1.25_f32.powf(zoom)).clamp(LOUPE_ZOOM.0, LOUPE_ZOOM.1);
        }
        if side != 0.0 {
            self.loupe_side = (self.loupe_side + 30.0 * side).clamp(LOUPE_SIDE.0, LOUPE_SIDE.1);
        }
    }
}

/// What is under the pointer on the picture: a corner or an edge of the frame (left, top,
/// right, bottom), the inside of the frame or the rest
#[derive(Clone, Copy, PartialEq, Debug)]
enum Grip {
    Corner(usize),
    Edge(usize),
    Inside,
    Outside,
}

fn grip(rect: Rect, p: Pos2) -> Grip {
    let corners = [rect.left_top(), rect.right_top(), rect.right_bottom(), rect.left_bottom()];
    if let Some(i) = corners.iter().position(|c| c.distance(p) < 12.0) {
        return Grip::Corner(i);
    }
    let near = 7.0;
    let along_x = p.x > rect.left() && p.x < rect.right();
    let along_y = p.y > rect.top() && p.y < rect.bottom();
    if along_y && (p.x - rect.left()).abs() < near {
        Grip::Edge(0)
    } else if along_x && (p.y - rect.top()).abs() < near {
        Grip::Edge(1)
    } else if along_y && (p.x - rect.right()).abs() < near {
        Grip::Edge(2)
    } else if along_x && (p.y - rect.bottom()).abs() < near {
        Grip::Edge(3)
    } else if rect.contains(p) {
        Grip::Inside
    } else {
        Grip::Outside
    }
}

fn grip_cursor(g: Grip) -> egui::CursorIcon {
    match g {
        Grip::Corner(0 | 2) => egui::CursorIcon::ResizeNwSe,
        Grip::Corner(_) => egui::CursorIcon::ResizeNeSw,
        Grip::Edge(0 | 2) => egui::CursorIcon::ResizeHorizontal,
        Grip::Edge(_) => egui::CursorIcon::ResizeVertical,
        Grip::Inside => egui::CursorIcon::Grab,
        Grip::Outside => egui::CursorIcon::Crosshair,
    }
}

/// The frame `r` with one edge (left, top, right, bottom) moved to `at`; with a ratio the other
/// side changes around its middle
fn edge_drag(side: usize, r: [f64; 4], at: Pos2, ratio: Option<f64>) -> [f64; 4] {
    const MIN: f64 = 0.01;
    let [x, y, w, h] = r;
    let (ax, ay) = (at.x as f64, at.y as f64);
    if side.is_multiple_of(2) {
        let (l, right) = if side == 0 { (ax.clamp(0.0, x + w - MIN), x + w) } else { (x, ax.clamp(x + MIN, 1.0)) };
        let mut nw = right - l;
        let Some(a) = ratio else { return [l, y, nw, h] };
        let mut nh = nw / a;
        if nh > 1.0 {
            nh = 1.0;
            nw = a;
        }
        let nx = if side == 0 { right - nw } else { l };
        [nx, (y + h / 2.0 - nh / 2.0).clamp(0.0, 1.0 - nh), nw, nh]
    } else {
        let (t, bottom) = if side == 1 { (ay.clamp(0.0, y + h - MIN), y + h) } else { (y, ay.clamp(y + MIN, 1.0)) };
        let mut nh = bottom - t;
        let Some(a) = ratio else { return [x, t, w, nh] };
        let mut nw = nh * a;
        if nw > 1.0 {
            nw = 1.0;
            nh = 1.0 / a;
        }
        let ny = if side == 1 { bottom - nh } else { t };
        [(x + w / 2.0 - nw / 2.0).clamp(0.0, 1.0 - nw), ny, nw, nh]
    }
}

/// The frame on whole pixels of a picture of `full`, at least one pixel
fn snap(r: [f64; 4], full: (u32, u32)) -> [f64; 4] {
    let (fw, fh) = (full.0.max(1) as f64, full.1.max(1) as f64);
    let (l, t) = ((r[0] * fw).round().clamp(0.0, fw - 1.0), (r[1] * fh).round().clamp(0.0, fh - 1.0));
    let right = ((r[0] + r[2]) * fw).round().clamp(l + 1.0, fw);
    let bottom = ((r[1] + r[3]) * fh).round().clamp(t + 1.0, fh);
    [l / fw, t / fh, (right - l) / fw, (bottom - t) / fh]
}

/// The size of the crop in pixels of a picture of `full`
fn crop_px(r: [f64; 4], full: (u32, u32)) -> (u32, u32) {
    ((r[2] * full.0 as f64).round() as u32, (r[3] * full.1 as f64).round() as u32)
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

/// The frame of the crop: the rest of the picture darker, thirds, corners and the middles of
/// the edges; `full`: the size of the picture, for the tag with the size of the crop
fn draw_crop(p: &egui::Painter, pic: Rect, r: [f64; 4], full: Option<(u32, u32)>) {
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
    let m = rect.center();
    if rect.width() > 48.0 {
        for y in [rect.top(), rect.bottom()] {
            p.rect_filled(Rect::from_center_size(pos2(m.x, y), vec2(14.0, 4.0)), 0.0, k.accent);
        }
    }
    if rect.height() > 48.0 {
        for x in [rect.left(), rect.right()] {
            p.rect_filled(Rect::from_center_size(pos2(x, m.y), vec2(4.0, 14.0)), 0.0, k.accent);
        }
    }
    if let Some(full) = full {
        let (w, h) = crop_px(r, full);
        size_tag(p, pic, rect, format!("{w}×{h}"));
    }
}

/// The size of the crop on a tag under the frame (in it, when there is no room under it)
fn size_tag(p: &egui::Painter, pic: Rect, rect: Rect, text: String) {
    let k = c();
    let font = match k.look {
        Look::Classic => egui::FontId::new(11.5, bold_family()),
        Look::Brutal => egui::FontId::monospace(11.0),
        Look::Y2k => egui::FontId::new(11.0, tech_family()),
    };
    let galley = p.layout_no_wrap(text, font, k.on_accent);
    let pad = vec2(7.0, 3.0);
    let size = galley.size() + pad * 2.0;
    let mut at = pos2(rect.center().x - size.x / 2.0, rect.bottom() + 7.0);
    if at.y + size.y > pic.bottom() - 2.0 {
        at.y = rect.bottom() - 7.0 - size.y;
    }
    at.x = at.x.clamp(pic.left() + 2.0, (pic.right() - 2.0 - size.x).max(pic.left() + 2.0));
    at.y = at.y.max(pic.top() + 2.0);
    let tag = Rect::from_min_size(at, size);
    match k.look {
        Look::Classic => {
            p.rect_filled(tag, 3.0, k.accent);
        }
        Look::Brutal => {
            p.rect_filled(tag, 0.0, k.accent);
        }
        Look::Y2k => super::paint::cut_rect(p, tag, 5.0, super::paint::DIAGONAL, k.accent, Stroke::NONE),
    }
    p.galley(tag.min + pad, galley, k.on_accent);
}

/// The magnifier: the pixels of the picture around `focus` (in parts of the picture), `zoom`
/// points to a pixel of a picture of `full`, in a square of `side` next to the pointer
#[allow(clippy::too_many_arguments)]
fn draw_loupe(ctx: &egui::Context, tex: &TextureHandle, full: (u32, u32), focus: Pos2, pointer: Pos2, crop: Option<[f64; 4]>, zoom: f32, side: f32) {
    let k = c();
    let screen = ctx.screen_rect();
    let gap = 26.0;
    let mut min = pointer + vec2(gap, gap);
    if min.x + side > screen.right() - 8.0 {
        min.x = pointer.x - gap - side;
    }
    if min.y + side > screen.bottom() - 8.0 {
        min.y = pointer.y - gap - side;
    }
    let rect = Rect::from_min_size(pos2(min.x.max(screen.left() + 8.0), min.y.max(screen.top() + 8.0)), vec2(side, side));
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("cmmt_loupe")));
    match k.look {
        Look::Classic => {
            p.add(egui::epaint::Shadow { offset: vec2(0.0, 4.0), blur: 16.0, spread: 0.0, color: Color32::from_black_alpha(110) }.as_shape(rect, 3.0));
        }
        Look::Brutal => {
            p.rect_filled(rect.translate(vec2(5.0, 5.0)), 0.0, k.shadow);
        }
        Look::Y2k => {}
    }
    let inner = p.with_clip_rect(rect);
    inner.rect_filled(rect, 0.0, k.surface);
    let (fw, fh) = (full.0.max(1) as f32, full.1.max(1) as f32);
    let mid = rect.center();
    let img = Rect::from_min_size(mid - vec2(focus.x * fw, focus.y * fh) * zoom, vec2(fw, fh) * zoom);
    inner.image(tex.id(), img, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
    if zoom >= 8.0 {
        let line = Stroke::new(1.0_f32, Color32::from_white_alpha(18));
        let (x0, x1) = (((rect.left() - img.left()) / zoom).floor().max(0.0) as i32, ((rect.right() - img.left()) / zoom).ceil().min(fw) as i32);
        let (y0, y1) = (((rect.top() - img.top()) / zoom).floor().max(0.0) as i32, ((rect.bottom() - img.top()) / zoom).ceil().min(fh) as i32);
        let (top, bottom) = (rect.top().max(img.top()), rect.bottom().min(img.bottom()));
        let (left, right) = (rect.left().max(img.left()), rect.right().min(img.right()));
        for i in x0..=x1 {
            let x = img.left() + i as f32 * zoom;
            inner.line_segment([pos2(x, top), pos2(x, bottom)], line);
        }
        for i in y0..=y1 {
            let y = img.top() + i as f32 * zoom;
            inner.line_segment([pos2(left, y), pos2(right, y)], line);
        }
    }
    if let Some(r) = crop {
        draw_crop(&inner, img, r, None);
    }
    // the point itself: a cross with a gap in its middle
    for (a, b) in [(vec2(-9.0, 0.0), vec2(-3.0, 0.0)), (vec2(3.0, 0.0), vec2(9.0, 0.0)), (vec2(0.0, -9.0), vec2(0.0, -3.0)), (vec2(0.0, 3.0), vec2(0.0, 9.0))] {
        inner.line_segment([mid + a, mid + b], Stroke::new(3.0_f32, Color32::from_black_alpha(170)));
        inner.line_segment([mid + a, mid + b], Stroke::new(1.2_f32, Color32::WHITE));
    }
    let font = match k.look {
        Look::Classic => egui::FontId::proportional(11.0),
        Look::Brutal => egui::FontId::monospace(10.5),
        Look::Y2k => egui::FontId::new(10.5, tech_family()),
    };
    let zoom_text = if zoom < 10.0 { format!("{zoom:.1}") } else { format!("{zoom:.0}") };
    let caption = format!("×{} · {}, {}", zoom_text.trim_end_matches(".0"), (focus.x * fw).round(), (focus.y * fh).round());
    let strip = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 18.0), rect.right_bottom());
    inner.rect_filled(strip, 0.0, Color32::from_black_alpha(160));
    inner.text(strip.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, caption, font, Color32::WHITE);
    match k.look {
        Look::Classic => {
            p.rect_stroke(rect, 3.0, Stroke::new(1.0_f32, k.border));
        }
        Look::Brutal => {
            p.rect_stroke(rect, 0.0, Stroke::new(2.0_f32, k.text));
        }
        Look::Y2k => {
            p.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, k.border));
            // the marks of cut corners and the star of the stickers
            let cut = 12.0;
            let accent = Stroke::new(2.0_f32, k.accent);
            p.line_segment([rect.left_top() + vec2(0.0, cut), rect.left_top() + vec2(cut, 0.0)], accent);
            p.line_segment([rect.right_bottom() - vec2(0.0, cut), rect.right_bottom() - vec2(cut, 0.0)], accent);
            super::paint::star(&p, rect.right_top(), 7.0, k.text);
        }
    }
}
/// A small button in the top right corner of `area`, four corners pointing out; true: clicked
fn expand_button(ui: &mut egui::Ui, area: Rect, tip: &str) -> bool {
    let k = c();
    let rect = Rect::from_min_size(pos2(area.right() - 8.0 - 28.0, area.top() + 8.0), vec2(28.0, 28.0));
    // it takes drags too, so a drag that starts on it does not crop
    let resp = ui.interact(rect, ui.id().with("cmmt_expand"), Sense::click_and_drag()).on_hover_text(tip);
    let hovered = resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }
    let p = ui.painter();
    let fill = if hovered { k.hover } else { k.surface };
    let stroke = Stroke::new(1.0_f32, if hovered { k.accent } else { k.border });
    match k.look {
        Look::Classic => {
            p.rect(rect, 4.0, fill, stroke);
        }
        Look::Brutal => {
            p.rect(rect, 0.0, fill, stroke);
        }
        Look::Y2k => super::paint::cut_rect(p, rect, 6.0, super::paint::DIAGONAL, fill, stroke),
    }
    let line = Stroke::new(1.6_f32, if hovered { k.accent } else { k.text });
    let m = rect.center();
    let (s, l) = (6.5, 4.0);
    for (dx, dy) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let corner = m + vec2(dx * s, dy * s);
        p.line_segment([corner, corner - vec2(dx * l, 0.0)], line);
        p.line_segment([corner, corner - vec2(0.0, dy * l)], line);
    }
    resp.clicked()
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

#[cfg(test)]
mod tests {
    use super::{edge_drag, grip, snap, Grip};
    use eframe::egui::{pos2, Rect};

    fn close(a: [f64; 4], b: [f64; 4]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
    }

    #[test]
    fn the_frame_stays_on_whole_pixels() {
        let r = snap([0.10013, 0.2004, 0.30051, 0.4], (1000, 500));
        assert!(close(r, [0.1, 0.2, 0.301, 0.4]));
        // at least one pixel, never outside the picture
        let r = snap([0.9999, 0.0, 0.00001, 1.2], (100, 100));
        assert!(close(r, [0.99, 0.0, 0.01, 1.0]));
    }

    #[test]
    fn an_edge_moves_alone_or_with_the_ratio() {
        let r = [0.2, 0.2, 0.4, 0.4];
        assert!(close(edge_drag(2, r, pos2(0.8, 0.9), None), [0.2, 0.2, 0.6, 0.4]));
        assert!(close(edge_drag(1, r, pos2(0.0, 0.1), None), [0.2, 0.1, 0.4, 0.5]));
        // the left edge cannot pass the right one
        assert!(close(edge_drag(0, r, pos2(0.9, 0.5), None), [0.59, 0.2, 0.01, 0.4]));
        // 1:1 in parts: the height follows around the middle
        assert!(close(edge_drag(2, r, pos2(0.8, 0.0), Some(1.0)), [0.2, 0.1, 0.6, 0.6]));
        // and stays in the picture
        assert!(close(edge_drag(3, [0.0, 0.0, 0.2, 0.2], pos2(0.0, 0.6), Some(1.0)), [0.0, 0.0, 0.6, 0.6]));
    }

    #[test]
    fn grips_of_the_frame() {
        let rect = Rect::from_min_max(pos2(100.0, 100.0), pos2(300.0, 200.0));
        assert_eq!(grip(rect, pos2(104.0, 97.0)), Grip::Corner(0));
        assert_eq!(grip(rect, pos2(297.0, 203.0)), Grip::Corner(2));
        assert_eq!(grip(rect, pos2(200.0, 103.0)), Grip::Edge(1));
        assert_eq!(grip(rect, pos2(296.0, 150.0)), Grip::Edge(2));
        assert_eq!(grip(rect, pos2(200.0, 150.0)), Grip::Inside);
        assert_eq!(grip(rect, pos2(50.0, 150.0)), Grip::Outside);
    }
}
