//! The tools of the whole window of Compress Image: a panel at the right of the picture (crop,
//! text, blur, colours, effects, captions) with the list of the edits. The picture shows what
//! ffmpeg makes of the edits, made in the background a moment after the last change.

use super::{crop_rect, form_row, grip, hint, muted, one_line, primary_button, secondary_button, segmented, slider_row, Grip};
use super::{Drag, Env, ImageForm, T};
use crate::engine::{self, BlurArea, BlurKind, Caption, ImageEdit, TextMark, Upto};
use crate::fonts::{self, FontFile};
use crate::theme::{c, Look, CONTROL_H};
use eframe::egui::{self, pos2, vec2, Color32, ColorImage, Pos2, Rect, Sense, Shape, Stroke, TextureFilter, TextureHandle, TextureOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The width of the panel of the tools
pub const PANEL_W: f32 = 330.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool {
    Crop,
    Text,
    Blur,
    Colours,
    Effects,
    Canvas,
}

impl Tool {
    const ALL: [Tool; 6] = [Tool::Crop, Tool::Text, Tool::Blur, Tool::Colours, Tool::Effects, Tool::Canvas];

    /// The picture shows the result (cropped, with the captions) instead of the picture the
    /// crop, the text and the areas are placed on
    pub fn shows_result(self) -> bool {
        matches!(self, Tool::Colours | Tool::Effects | Tool::Canvas)
    }
}

/// The fonts of the system, read in the background
#[derive(Default)]
pub struct FontList {
    started: bool,
    slot: Arc<Mutex<Option<Arc<Vec<FontFile>>>>>,
}

impl FontList {
    fn get(&mut self, ctx: &egui::Context) -> Option<Arc<Vec<FontFile>>> {
        if !self.started {
            self.started = true;
            let (slot, ctx) = (self.slot.clone(), ctx.clone());
            std::thread::spawn(move || {
                *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(fonts::system());
                ctx.request_repaint();
            });
        }
        self.slot.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// A picture made by ffmpeg: what it was made of and the result
type Made = Option<(String, Result<ColorImage, String>)>;

/// The picture with the edits as ffmpeg makes it, a moment after the last change
#[derive(Default)]
pub struct Render {
    shown: Option<(String, TextureHandle)>,
    changed: Option<(String, Instant)>,
    asked: Option<String>,
    slot: Arc<Mutex<Made>>,
    busy: Arc<AtomicBool>,
    error: Option<String>,
}

impl Render {
    /// The texture of `edit` (up to `upto`) on the preview picture of `src` (not turned); the one
    /// made before while the new one is made, None before the first
    fn texture(&mut self, ctx: &egui::Context, env: &Env, edit: &ImageEdit, upto: Upto, src: (u32, u32)) -> Option<TextureHandle> {
        let key = format!("{upto:?} {src:?} {edit:?}");
        if let Some((k, r)) = self.slot.lock().unwrap_or_else(|e| e.into_inner()).take() {
            match r {
                Ok(image) => {
                    let options = TextureOptions { mipmap_mode: Some(TextureFilter::Linear), ..TextureOptions::LINEAR };
                    self.shown = Some((k, ctx.load_texture("cmmt_render", image, options)));
                    self.error = None;
                }
                Err(e) => self.error = Some(e),
            }
        }
        if let Some((k, tex)) = &self.shown {
            if *k == key {
                return Some(tex.clone());
            }
        }
        if self.changed.as_ref().is_none_or(|(k, _)| *k != key) {
            self.changed = Some((key.clone(), Instant::now()));
        }
        let since = self.changed.as_ref().map_or(Duration::ZERO, |(_, at)| at.elapsed());
        let wait = Duration::from_millis(120);
        if self.busy.load(Ordering::SeqCst) {
            ctx.request_repaint_after(Duration::from_millis(40));
        } else if self.asked.as_deref() == Some(key.as_str()) {
        } else if since < wait {
            ctx.request_repaint_after(wait - since);
        } else if let (Some(tools), Some(file)) = (env.tools.clone(), env.files.first().cloned()) {
            self.asked = Some(key.clone());
            match engine::build_edit(edit, Some(src), upto) {
                Ok(engine::Built { filters, size: Some(size) }) => {
                    self.busy.store(true, Ordering::SeqCst);
                    let (slot, busy, ctx) = (self.slot.clone(), self.busy.clone(), ctx.clone());
                    std::thread::spawn(move || {
                        let r = render(&tools, &file, src, &filters, size);
                        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some((key, r));
                        busy.store(false, Ordering::SeqCst);
                        ctx.request_repaint();
                    });
                }
                Ok(_) => {}
                Err(e) => self.error = Some(e),
            }
        }
        self.shown.as_ref().map(|(_, t)| t.clone())
    }
}

/// The picture scaled to `src` as the preview is, with `filters`; it comes out `out` large
fn render(tools: &super::super::Tools, file: &Path, src: (u32, u32), filters: &[String], out: (u32, u32)) -> Result<ColorImage, String> {
    let mut vf = format!("scale={}:{}:flags=bicubic", src.0, src.1);
    for f in filters {
        vf.push(',');
        vf.push_str(f);
    }
    vf.push_str(",format=rgba");
    let r = crate::proc::command(&tools.ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(file)
        .args(["-frames:v", "1", "-vf", &vf, "-f", "rawvideo", "-pix_fmt", "rgba", "-"])
        .output()
        .map_err(|e| e.to_string())?;
    let (w, h) = (out.0 as usize, out.1 as usize);
    if r.stdout.len() != w * h * 4 {
        return Err(first_error(&String::from_utf8_lossy(&r.stderr)));
    }
    Ok(ColorImage::from_rgba_unmultiplied([w, h], &r.stdout))
}

/// The first line ffmpeg wrote, which says what went wrong (the last ones only that nothing
/// came out), without the "[Parsed_pad_3 @ 0x...]" in front
fn first_error(stderr: &str) -> String {
    let line = stderr.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("ffmpeg");
    match (line.starts_with('['), line.find("] ")) {
        (true, Some(i)) => line[i + 2..].to_string(),
        _ => line.to_string(),
    }
}

/// A frame in pixels of a picture of `full` on the picture drawn at `pic`
fn on_screen(r: [f64; 4], full: (u32, u32), pic: Rect) -> Rect {
    let (fw, fh) = (full.0.max(1) as f64, full.1.max(1) as f64);
    crop_rect(pic, [r[0] / fw, r[1] / fh, r[2] / fw, r[3] / fh])
}

fn corners(r: Rect) -> [Pos2; 4] {
    [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()]
}

fn dashed(p: &egui::Painter, r: Rect) {
    let pts = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom(), r.left_top()];
    p.add(Shape::dashed_line(&pts, Stroke::new(3.0_f32, Color32::from_black_alpha(90)), 5.0, 4.0));
    p.add(Shape::dashed_line(&pts, Stroke::new(1.2_f32, Color32::from_white_alpha(200)), 5.0, 4.0));
}

/// The frame of a chosen text or area: a line of the accent and its corners
fn chosen_frame(p: &egui::Painter, r: Rect, edges: bool) {
    let k = c();
    p.rect_stroke(r, 0.0, Stroke::new(3.0_f32, Color32::from_black_alpha(90)));
    p.rect_stroke(r, 0.0, Stroke::new(1.5_f32, k.accent));
    for corner in corners(r) {
        p.rect_filled(Rect::from_center_size(corner, vec2(8.0, 8.0)), 0.0, k.accent);
    }
    if edges {
        let m = r.center();
        if r.width() > 48.0 {
            for y in [r.top(), r.bottom()] {
                p.rect_filled(Rect::from_center_size(pos2(m.x, y), vec2(14.0, 4.0)), 0.0, k.accent);
            }
        }
        if r.height() > 48.0 {
            for x in [r.left(), r.right()] {
                p.rect_filled(Rect::from_center_size(pos2(x, m.y), vec2(4.0, 14.0)), 0.0, k.accent);
            }
        }
    }
}

/// A blurred area on the picture drawn at `pic` (`r` in parts of it)
pub fn draw_area(p: &egui::Painter, pic: Rect, r: [f64; 4], chosen: bool) {
    let rect = crop_rect(pic, r);
    if chosen {
        chosen_frame(p, rect, true);
    } else {
        dashed(p, rect);
    }
}

fn kind_label(t: &T, k: BlurKind) -> &'static str {
    match k {
        BlurKind::Gauss => t.img_blur_gauss,
        BlurKind::Box => t.img_blur_box,
        BlurKind::Pixels => t.img_blur_pixels,
    }
}

/// A button with the colour; a click opens the colour picker, which stays inside the window
fn colour_button(ui: &mut egui::Ui, colour: &mut [u8; 3]) -> egui::Response {
    let k = c();
    let (rect, mut resp) = ui.allocate_exact_size(vec2(CONTROL_H * 1.4, CONTROL_H), Sense::click());
    let popup = resp.id.with("picker");
    let radius = if k.look == Look::Classic { 3.0 } else { 0.0 };
    let border = if resp.hovered() { k.accent } else { k.border };
    ui.painter().rect(rect.shrink(1.0), radius, Color32::from_rgb(colour[0], colour[1], colour[2]), Stroke::new(1.0_f32, border));
    if resp.clicked() {
        ui.memory_mut(|m| m.toggle_popup(popup));
    }
    if !ui.memory(|m| m.is_popup_open(popup)) {
        return resp;
    }
    let ctx = ui.ctx().clone();
    let screen = ctx.screen_rect();
    let size = ctx.memory(|m| m.area_rect(popup)).map_or(vec2(320.0, 400.0), |r| r.size());
    // below the button, above it, or else at its left
    let mut at = pos2(rect.right() - size.x, rect.bottom() + 4.0);
    if at.y + size.y > screen.bottom() - 8.0 {
        at.y = rect.top() - 4.0 - size.y;
        if at.y < screen.top() + 8.0 {
            at = pos2(rect.left() - 8.0 - size.x, rect.center().y - size.y / 2.0);
        }
    }
    at.x = at.x.min(screen.right() - size.x - 8.0).max(screen.left() + 8.0);
    at.y = at.y.min(screen.bottom() - size.y - 8.0).max(screen.top() + 8.0);
    let area = egui::Area::new(popup).order(egui::Order::Foreground).fixed_pos(at).show(&ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.spacing_mut().slider_width = 240.0;
            let mut c32 = Color32::from_rgb(colour[0], colour[1], colour[2]);
            if egui::widgets::color_picker::color_picker_color32(ui, &mut c32, egui::widgets::color_picker::Alpha::Opaque) {
                *colour = [c32.r(), c32.g(), c32.b()];
                resp.mark_changed();
            }
        });
    });
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !area.response.rect.contains(p) && !rect.contains(p)));
    if outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        ui.memory_mut(|m| m.close_popup());
    }
    resp
}

/// The font of the system to draw with: a list with a search field
fn font_picker(ui: &mut egui::Ui, id: &str, list: &[FontFile], current: &mut PathBuf, t: &T) -> bool {
    let ctx = ui.ctx().clone();
    let id = ui.make_persistent_id(id);
    let shown = fonts::describe(list, current)
        .map(|(f, s)| format!("{f} {s}"))
        .unwrap_or_else(|| current.file_stem().map_or_else(|| "—".into(), |s| s.to_string_lossy().to_string()));
    let (button, open) = super::super::dropdown::list_button(ui, id, &shown);
    let mut changed = false;
    if !open {
        return false;
    }
    let search_id = id.with("search");
    let mut search: String = ctx.data(|d| d.get_temp(search_id)).unwrap_or_default();
    let w = button.rect.width().max(280.0);
    let screen = ctx.screen_rect();
    let list_h = 300.0;
    let below = button.rect.bottom() + 4.0;
    let y = if below + list_h + 60.0 < screen.bottom() { below } else { (button.rect.top() - 4.0 - list_h - 60.0).max(screen.top() + 8.0) };
    let x = button.rect.left().min(screen.right() - w - 24.0).max(8.0);
    let area = egui::Area::new(id.with("list")).order(egui::Order::Foreground).fixed_pos(pos2(x, y)).show(&ctx, |ui| {
        egui::Frame::popup(ui.style()).show(ui, |ui| {
            ui.set_width(w);
            let r = ui.add(egui::TextEdit::singleline(&mut search).hint_text(t.img_font_search).desired_width(w));
            if !r.has_focus() && search.is_empty() {
                r.request_focus();
            }
            ui.add_space(4.0);
            let needle = search.to_lowercase();
            let items: Vec<&FontFile> =
                list.iter().filter(|f| needle.is_empty() || format!("{} {}", f.family, f.style).to_lowercase().contains(&needle)).collect();
            let row_h = 24.0;
            egui::ScrollArea::vertical().max_height(list_h).auto_shrink([false, true]).show_rows(ui, row_h, items.len(), |ui, range| {
                for f in &items[range] {
                    let on = f.path == *current;
                    let (r, resp) = ui.allocate_exact_size(vec2(ui.available_width(), row_h), Sense::click());
                    let k = c();
                    if on {
                        ui.painter().rect_filled(r, 2.0, k.accent);
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 2.0, k.hover);
                    }
                    let color = if on { k.on_accent } else { k.text };
                    let style_c = if on { k.on_accent } else { k.muted };
                    let galley = ui.painter().layout_no_wrap(f.family.clone(), egui::FontId::proportional(13.0), color);
                    let gw = galley.size().x;
                    ui.painter().galley(pos2(r.left() + 8.0, r.center().y - galley.size().y / 2.0), galley, color);
                    ui.painter().text(pos2(r.left() + 16.0 + gw, r.center().y), egui::Align2::LEFT_CENTER, &f.style, egui::FontId::proportional(11.5), style_c);
                    if resp.clicked() {
                        current.clone_from(&f.path);
                        changed = true;
                    }
                }
            });
        });
    });
    ctx.data_mut(|d| d.insert_temp(search_id, search));
    let list_rect = area.response.rect;
    let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !list_rect.contains(p) && !button.rect.contains(p)));
    if changed || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        ui.memory_mut(|m| m.close_popup());
        ctx.data_mut(|d| d.remove::<String>(search_id));
    }
    changed
}

/// What a line of the list of edits removes
#[derive(Clone, Copy)]
enum Item {
    Crop,
    Colours,
    Sharpen,
    Vignette,
    Blur(usize),
    Text(usize),
    Caption,
}

impl ImageForm {
    /// The size of the preview picture as it was loaded (not turned)
    fn src_size(&self, base: &TextureHandle) -> (u32, u32) {
        let [w, h] = base.size().map(|v| v as u32);
        if self.turns % 2 == 1 {
            (h, w)
        } else {
            (w, h)
        }
    }

    /// The picture the crop, the text and the areas are placed on: as ffmpeg makes it when
    /// there are edits before the crop
    pub(super) fn workspace(&mut self, ctx: &egui::Context, env: &Env, base: &TextureHandle) -> TextureHandle {
        let e = self.edit(env);
        let plain = e.colours == engine::Colours::default() && e.sharpen == 0 && e.blurs.is_empty() && e.texts.is_empty();
        if plain {
            return base.clone();
        }
        let src = self.src_size(base);
        self.work.texture(ctx, env, &e, Upto::Crop, src).filter(|t| t.size() == base.size()).unwrap_or_else(|| base.clone())
    }

    /// The result in `room`: cropped, with every edit (the crop of the plain picture until
    /// ffmpeg has made it)
    pub(super) fn draw_result(&mut self, ctx: &egui::Context, env: &Env, p: &egui::Painter, room: Rect, base: &TextureHandle) {
        let e = self.edit(env);
        let src = self.src_size(base);
        let [bw, bh] = base.size().map(|v| v as f32);
        let expected = engine::build_edit(&e, Some(src), Upto::All).ok().and_then(|b| b.size);
        let only_crop = e == ImageEdit { turns: e.turns, flip_h: e.flip_h, crop: e.crop, ..Default::default() };
        let made = if only_crop { None } else { self.result.texture(ctx, env, &e, Upto::All, src) };
        let made = made.filter(|t| expected.is_some_and(|(w, h)| t.size() == [w as usize, h as usize]));
        let [x, y, w, h] = e.crop.unwrap_or([0.0, 0.0, 1.0, 1.0]).map(|v| v as f32);
        let (id, uv, size) = match &made {
            Some(t) => (t.id(), Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), vec2(t.size()[0] as f32, t.size()[1] as f32)),
            None => {
                let size = expected.map_or(vec2(w * bw, h * bh), |(w, h)| vec2(w as f32, h as f32));
                (base.id(), Rect::from_min_size(pos2(x, y), vec2(w, h)), size)
            }
        };
        let s = (room.width() / size.x).min(room.height() / size.y);
        let pic = Rect::from_center_size(room.center(), size * s);
        if made.is_none() && expected.is_some_and(|(_, eh)| (eh as f32) > h * bh + 0.5) {
            // the strips of the captions are not there yet: the cropped picture between them
            let strip = Rect::from_center_size(pic.center(), vec2(pic.width(), pic.width() * h * bh / (w * bw).max(1.0)));
            p.rect_filled(pic, 0.0, Color32::WHITE);
            p.image(id, strip, uv, Color32::WHITE);
        } else {
            p.image(id, pic, uv, Color32::WHITE);
        }
    }

    /// The frames of the texts in pixels of a picture of `full`
    fn text_frames(&self, full: (u32, u32)) -> Vec<Option<[f64; 4]>> {
        self.edits.texts.iter().map(|t| engine::text_block(t, full).ok().map(|b| b.rect)).collect()
    }

    /// Drag on the picture with the text tool: a text moves, the corner of the chosen one makes
    /// it larger or smaller; a click chooses a text
    pub(super) fn text_tool(&mut self, ctx: &egui::Context, resp: &egui::Response, pic: Rect, full: (u32, u32)) {
        let frames: Vec<Option<Rect>> = self.text_frames(full).into_iter().map(|r| r.map(|r| on_screen(r, full, pic))).collect();
        let hit = |p: Pos2| frames.iter().rposition(|r| r.is_some_and(|r| r.expand(4.0).contains(p)));
        let chosen_frame = self.chosen.and_then(|i| frames.get(i).copied().flatten());
        let on_corner = |p: Pos2| chosen_frame.and_then(|r| corners(r).iter().position(|c| c.distance(p) < 12.0));
        let middle = |i: usize, texts: &[TextMark]| pic.left_top() + vec2(texts[i].at[0] as f32 * pic.width(), texts[i].at[1] as f32 * pic.height());
        let origin = ctx.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
        if let (true, Some(p)) = (resp.drag_started(), origin) {
            self.drag = None;
            if let (Some(i), Some(_)) = (self.chosen, on_corner(p)) {
                self.drag = Some(Drag::TextSize(self.edits.texts[i].size, middle(i, &self.edits.texts).distance(p).max(4.0)));
            } else if let Some(i) = hit(p) {
                self.chosen = Some(i);
                self.drag = Some(Drag::Text(p - middle(i, &self.edits.texts)));
            }
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.chosen = hit(p);
            }
        }
        if let (true, Some(p), Some(i)) = (resp.dragged(), resp.interact_pointer_pos(), self.chosen.filter(|i| *i < self.edits.texts.len())) {
            match self.drag {
                Some(Drag::Text(off)) => {
                    let m = p - off;
                    let at = [((m.x - pic.left()) / pic.width()).clamp(0.0, 1.0) as f64, ((m.y - pic.top()) / pic.height()).clamp(0.0, 1.0) as f64];
                    self.edits.texts[i].at = at;
                }
                Some(Drag::TextSize(size, dist)) => {
                    let d = middle(i, &self.edits.texts).distance(p);
                    self.edits.texts[i].size = (size * (d / dist) as f64).clamp(0.004, 0.5);
                }
                _ => {}
            }
        }
        if !resp.dragged() {
            self.drag = None;
        }
        if resp.hovered() || resp.dragged() {
            let icon = match (self.drag, resp.hover_pos()) {
                (Some(Drag::Text(_)), _) => egui::CursorIcon::Grabbing,
                (Some(Drag::TextSize(..)), _) => egui::CursorIcon::ResizeNwSe,
                (_, Some(p)) => match on_corner(p) {
                    Some(0 | 2) => egui::CursorIcon::ResizeNwSe,
                    Some(_) => egui::CursorIcon::ResizeNeSw,
                    None if hit(p).is_some() => egui::CursorIcon::Grab,
                    None => egui::CursorIcon::Default,
                },
                _ => egui::CursorIcon::Default,
            };
            ctx.set_cursor_icon(icon);
        }
    }

    pub(super) fn draw_texts(&self, p: &egui::Painter, pic: Rect, full: (u32, u32)) {
        for (i, r) in self.text_frames(full).into_iter().enumerate() {
            let Some(r) = r else { continue };
            let rect = on_screen(r, full, pic);
            if self.chosen == Some(i) {
                chosen_frame(p, rect, false);
            } else {
                dashed(p, rect);
            }
        }
    }

    /// The area that is chosen, in parts of the picture
    pub(super) fn chosen_area(&self) -> Option<[f64; 4]> {
        self.chosen.and_then(|i| self.edits.blurs.get(i)).map(|b| b.rect)
    }

    /// Drag on the picture with the blur tool: a new area, or a corner, an edge or the whole of
    /// one; a click chooses an area
    pub(super) fn blur_tool(&mut self, ctx: &egui::Context, resp: &egui::Response, pic: Rect, full: (u32, u32)) {
        let n = self.edits.blurs.len();
        let grip_of = |i: usize, p: Pos2, blurs: &[BlurArea]| grip(crop_rect(pic, blurs[i].rect), p);
        // the chosen area first, then the others from the top
        let target = |p: Pos2, chosen: Option<usize>, blurs: &[BlurArea]| {
            chosen.filter(|i| *i < n && grip_of(*i, p, blurs) != Grip::Outside).or_else(|| (0..n).rev().find(|i| grip_of(*i, p, blurs) == Grip::Inside))
        };
        let origin = ctx.input(|i| i.pointer.press_origin()).or(resp.interact_pointer_pos());
        if let (true, Some(p)) = (resp.drag_started(), origin) {
            self.chosen = target(p, self.chosen, &self.edits.blurs);
        }
        if resp.clicked() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.chosen = target(p, self.chosen, &self.edits.blurs);
            }
        }
        let mut rect = self.chosen_area();
        let before = rect;
        super::drag_rect(&mut self.drag, &mut self.focus, &mut rect, resp, pic, full, None);
        if let (Some(r), true) = (rect, rect != before) {
            match self.chosen {
                Some(i) => self.edits.blurs[i].rect = r,
                None => {
                    let (kind, strength) = self.blur_last;
                    self.edits.blurs.push(BlurArea { rect: r, kind, strength });
                    self.chosen = Some(self.edits.blurs.len() - 1);
                }
            }
        }
        let hovered = resp.hover_pos().and_then(|p| target(p, self.chosen, &self.edits.blurs));
        super::rect_cursor(ctx, resp, pic, hovered.or(self.chosen).and_then(|i| self.edits.blurs.get(i)).map(|b| b.rect), self.drag);
        if let Some(b) = self.chosen.and_then(|i| self.edits.blurs.get(i)) {
            self.blur_last = (b.kind, b.strength);
        }
    }

    pub(super) fn draw_areas(&self, p: &egui::Painter, pic: Rect) {
        for (i, b) in self.edits.blurs.iter().enumerate() {
            draw_area(p, pic, b.rect, self.chosen == Some(i));
        }
    }

    /// The panel at the right of the picture: the tools in two rows, what the chosen one
    /// changes, the list of the edits
    pub(super) fn tools_panel(&mut self, ui: &mut egui::Ui, env: &Env, full: Option<(u32, u32)>) {
        let t = env.t;
        let list = self.fonts.get(ui.ctx());
        let rows = [[t.img_tool_crop, t.img_tool_text, t.img_tool_blur], [t.img_tool_colours, t.img_tool_effects, t.img_tool_canvas]];
        let at = Tool::ALL.iter().position(|x| *x == self.tool).unwrap_or(0);
        for (row, labels) in rows.iter().enumerate() {
            let mut part = if at / 3 == row { at % 3 } else { usize::MAX };
            if segmented(ui, labels, &mut part).changed() {
                self.choose_tool(Tool::ALL[row * 3 + part]);
            }
            ui.add_space(2.0);
        }
        ui.add_space(12.0);
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 6.0;
            match self.tool {
                Tool::Crop => self.crop_tool_panel(ui, t, full),
                Tool::Text => self.text_panel(ui, t, full, list.as_ref().map(|l| l.as_slice())),
                Tool::Blur => self.blur_panel(ui, t, full),
                Tool::Colours => self.colours_panel(ui, t),
                Tool::Effects => self.effects_panel(ui, t),
                Tool::Canvas => self.canvas_panel(ui, t, list.as_ref().map(|l| l.as_slice())),
            }
            let error = if self.tool.shows_result() { &self.result.error } else { &self.work.error };
            if let Some(e) = error {
                ui.add_space(4.0);
                ui.label(egui::RichText::new(format!("{} {e}", t.img_render_failed)).small().color(c().warn));
            }
            ui.add_space(16.0);
            self.edits_list(ui, t, full);
        });
    }

    fn choose_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.chosen = None;
        self.drag = None;
    }

    fn crop_tool_panel(&mut self, ui: &mut egui::Ui, t: &T, full: Option<(u32, u32)>) {
        let labels = super::ratio_labels(t);
        let before = self.ratio;
        let mut clicked = false;
        let mut changed = false;
        for row in 0..2 {
            let mut part = if self.ratio / 3 == row { self.ratio % 3 } else { usize::MAX };
            let r = ui.add_enabled_ui(full.is_some(), |ui| segmented(ui, &labels[row * 3..row * 3 + 3], &mut part)).inner.on_hover_text(t.img_crop_upright);
            if r.changed() {
                self.ratio = row * 3 + part;
                changed = true;
            }
            clicked |= r.clicked();
            ui.add_space(2.0);
        }
        let again = clicked && !changed && self.ratio == before && self.ratio > 1;
        if again {
            self.upright = !self.upright;
        }
        if let (Some(full), true) = (full, changed || again) {
            self.fit_ratio(full);
        }
        ui.add_space(4.0);
        one_line(ui, &self.crop_note(t, full), muted());
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.allocate_ui(vec2(150.0, CONTROL_H), |ui| self.loupe_toggle(ui, t, full.is_some()));
            if ui.add_enabled(self.crop.is_some(), secondary_button(t.img_crop_reset)).clicked() {
                self.reset_crop();
            }
        });
    }

    fn text_panel(&mut self, ui: &mut egui::Ui, t: &T, full: Option<(u32, u32)>, list: Option<&[FontFile]>) {
        let Some(list) = list else {
            hint(ui, t.img_fonts_loading);
            return;
        };
        let chosen = self.chosen.filter(|i| *i < self.edits.texts.len());
        ui.horizontal(|ui| {
            if ui.add_enabled(full.is_some(), primary_button(t.img_text_add)).clicked() {
                // like the text before, a little lower
                let mut mark =
                    self.edits.texts.last().cloned().unwrap_or_else(|| TextMark { font: fonts::default_text(list).unwrap_or_default(), ..Default::default() });
                mark.text = t.img_text_new.to_string();
                mark.at = if self.edits.texts.is_empty() { [0.5, 0.5] } else { [mark.at[0], (mark.at[1] + 0.12).min(0.95)] };
                self.edits.texts.push(mark);
                self.chosen = Some(self.edits.texts.len() - 1);
            }
            if let Some(i) = chosen {
                if ui.add(secondary_button(t.img_delete)).clicked() {
                    self.edits.texts.remove(i);
                    self.chosen = None;
                }
            }
        });
        let Some(i) = self.chosen.filter(|i| *i < self.edits.texts.len()) else {
            hint(ui, if self.edits.texts.is_empty() { t.img_text_none } else { t.img_text_pick });
            return;
        };
        hint(ui, t.img_text_hint);
        ui.add_space(4.0);
        let mark = &mut self.edits.texts[i];
        ui.add(egui::TextEdit::multiline(&mut mark.text).desired_rows(2).desired_width(f32::INFINITY));
        ui.add_space(4.0);
        form_row(ui, t.img_font, |ui| font_picker(ui, "cmmt_text_font", list, &mut mark.font, t));
        if let Some(full) = full {
            let side = full.0.max(full.1).max(1) as f64;
            let mut px = (mark.size * side).round().max(1.0) as u32;
            let hi = ((side / 3.0) as u32).max(24);
            form_row(ui, t.img_size, |ui| {
                if slider_row(ui, &mut px, 4.min(hi)..=hi).changed() {
                    mark.size = px as f64 / side;
                }
            });
        }
        form_row(ui, t.img_colour, |ui| colour_button(ui, &mut mark.color));
        form_row(ui, t.img_outline, |ui| {
            ui.horizontal(|ui| {
                colour_button(ui, &mut mark.outline_color);
                slider_row(ui, &mut mark.outline, 0..=30);
            });
        });
        form_row(ui, t.img_background, |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut mark.boxed, "");
                ui.add_enabled_ui(mark.boxed, |ui| {
                    colour_button(ui, &mut mark.box_color);
                    slider_row(ui, &mut mark.box_opacity, 0..=100).on_hover_text(t.img_opacity);
                });
            });
        });
    }

    fn blur_panel(&mut self, ui: &mut egui::Ui, t: &T, full: Option<(u32, u32)>) {
        hint(ui, t.img_blur_hint);
        ui.add_space(4.0);
        ui.allocate_ui(vec2(150.0, CONTROL_H), |ui| self.loupe_toggle(ui, t, full.is_some()));
        let Some(i) = self.chosen.filter(|i| *i < self.edits.blurs.len()) else { return };
        ui.add_space(8.0);
        let b = &mut self.edits.blurs[i];
        let labels: Vec<&str> = BlurKind::ALL.iter().map(|k| kind_label(t, *k)).collect();
        let mut kind = BlurKind::ALL.iter().position(|k| *k == b.kind).unwrap_or(0);
        form_row(ui, t.img_blur_kind, |ui| {
            if segmented(ui, &labels, &mut kind).changed() {
                b.kind = BlurKind::ALL[kind];
            }
        });
        form_row(ui, t.img_strength, |ui| slider_row(ui, &mut b.strength, 1..=100));
        self.blur_last = (b.kind, b.strength);
        if ui.add(secondary_button(t.img_delete)).clicked() {
            self.edits.blurs.remove(i);
            self.chosen = None;
        }
    }

    fn colours_panel(&mut self, ui: &mut egui::Ui, t: &T) {
        hint(ui, t.img_result_hint);
        ui.add_space(4.0);
        let c = &mut self.edits.colours;
        for (label, v, lo, hi) in [
            (t.img_brightness, &mut c.brightness, -100, 100),
            (t.img_contrast, &mut c.contrast, -100, 100),
            (t.img_saturation, &mut c.saturation, -100, 100),
            (t.img_vibrance, &mut c.vibrance, -100, 100),
            (t.img_gamma, &mut c.gamma, -100, 100),
            (t.img_temperature, &mut c.temperature, -100, 100),
            (t.img_hue, &mut c.hue, -180, 180),
        ] {
            form_row(ui, label, |ui| slider_row(ui, v, lo..=hi));
        }
        for (title, values) in [(t.img_shadows, &mut c.shadows), (t.img_highlights, &mut c.highlights)] {
            super::super::widgets::caption(ui, title);
            for (label, v) in [t.img_red, t.img_green, t.img_blue].into_iter().zip(values.iter_mut()) {
                form_row(ui, label, |ui| slider_row(ui, v, -100..=100));
            }
        }
        if ui.add_enabled(*c != engine::Colours::default(), secondary_button(t.img_reset_tool)).clicked() {
            *c = engine::Colours::default();
        }
    }

    fn effects_panel(&mut self, ui: &mut egui::Ui, t: &T) {
        hint(ui, t.img_result_hint);
        ui.add_space(4.0);
        form_row(ui, t.img_sharpen, |ui| slider_row(ui, &mut self.edits.sharpen, 0..=100));
        form_row(ui, t.img_vignette, |ui| slider_row(ui, &mut self.edits.vignette, 0..=100));
        if ui.add_enabled(self.edits.sharpen > 0 || self.edits.vignette > 0, secondary_button(t.img_reset_tool)).clicked() {
            self.edits.sharpen = 0;
            self.edits.vignette = 0;
        }
    }

    fn canvas_panel(&mut self, ui: &mut egui::Ui, t: &T, list: Option<&[FontFile]>) {
        let Some(list) = list else {
            hint(ui, t.img_fonts_loading);
            return;
        };
        hint(ui, t.img_caption_hint);
        ui.add_space(4.0);
        let cap = self.edits.caption.get_or_insert_with(Caption::default);
        if cap.font.as_os_str().is_empty() {
            cap.font = fonts::default_caption(list).unwrap_or_default();
        }
        let mut style = usize::from(!cap.strip);
        if segmented(ui, &[t.img_caption_strip, t.img_caption_over], &mut style).changed() {
            cap.strip = style == 0;
        }
        ui.add_space(2.0);
        form_row(ui, t.img_caption_top, |ui| ui.add(egui::TextEdit::multiline(&mut cap.top).desired_rows(1).desired_width(f32::INFINITY)));
        form_row(ui, t.img_caption_bottom, |ui| ui.add(egui::TextEdit::multiline(&mut cap.bottom).desired_rows(1).desired_width(f32::INFINITY)));
        form_row(ui, t.img_size, |ui| slider_row(ui, &mut cap.size, 3..=20));
        form_row(ui, t.img_font, |ui| font_picker(ui, "cmmt_caption_font", list, &mut cap.font, t));
        if ui.add_enabled(!cap.is_empty(), secondary_button(t.img_reset_tool)).clicked() {
            cap.top.clear();
            cap.bottom.clear();
        }
    }

    /// Every edit, one per line: a click shows its tool, the cross removes it
    fn edits_list(&mut self, ui: &mut egui::Ui, t: &T, full: Option<(u32, u32)>) {
        let k = c();
        super::super::widgets::caption(ui, t.img_edits);
        let mut items: Vec<(String, Tool, Option<usize>, Item)> = Vec::new();
        if let Some(r) = self.crop {
            let size = full.map(|f| super::crop_px(r, f)).map_or(String::new(), |(w, h)| format!(" {w}×{h}"));
            items.push((format!("{}{size}", t.img_tool_crop), Tool::Crop, None, Item::Crop));
        }
        for (i, m) in self.edits.texts.iter().enumerate() {
            let first = m.text.lines().next().unwrap_or("").trim();
            items.push((format!("{}: {first}", t.img_tool_text), Tool::Text, Some(i), Item::Text(i)));
        }
        for (i, b) in self.edits.blurs.iter().enumerate() {
            items.push((format!("{} {}: {}", t.img_tool_blur, i + 1, kind_label(t, b.kind)), Tool::Blur, Some(i), Item::Blur(i)));
        }
        if self.edits.colours != engine::Colours::default() {
            items.push((t.img_tool_colours.to_string(), Tool::Colours, None, Item::Colours));
        }
        if self.edits.sharpen > 0 {
            items.push((format!("{} {}", t.img_sharpen, self.edits.sharpen), Tool::Effects, None, Item::Sharpen));
        }
        if self.edits.vignette > 0 {
            items.push((format!("{} {}", t.img_vignette, self.edits.vignette), Tool::Effects, None, Item::Vignette));
        }
        if self.edits.caption.as_ref().is_some_and(|c| !c.is_empty()) {
            items.push((t.img_captions.to_string(), Tool::Canvas, None, Item::Caption));
        }
        if items.is_empty() {
            hint(ui, t.img_no_edits);
            return;
        }
        let mut remove = None;
        for (label, tool, index, item) in items {
            let on = self.tool == tool && (index.is_none() || self.chosen == index);
            let (row, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click());
            let cross = Rect::from_center_size(pos2(row.right() - 13.0, row.center().y), vec2(22.0, 22.0));
            let cross_resp = ui.interact(cross, resp.id.with("x"), Sense::click()).on_hover_text(t.img_delete);
            let p = ui.painter();
            let radius = if k.look == Look::Classic { 3.0 } else { 0.0 };
            if on {
                p.rect_filled(row, radius, crate::theme::mix(k.surface, k.accent, 0.35));
            } else if resp.hovered() {
                p.rect_filled(row, radius, k.hover);
            }
            let clip = Rect::from_min_max(row.min, pos2(cross.left() - 4.0, row.bottom()));
            p.with_clip_rect(clip).text(pos2(row.left() + 8.0, row.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), k.text);
            let x_color = if cross_resp.hovered() { c().err } else { k.muted };
            let m = cross.center();
            for d in [vec2(4.0, 4.0), vec2(4.0, -4.0)] {
                p.line_segment([m - d, m + d], Stroke::new(1.5_f32, x_color));
            }
            if cross_resp.clicked() {
                remove = Some(item);
            } else if resp.clicked() {
                self.choose_tool(tool);
                self.chosen = index;
            }
        }
        match remove {
            Some(Item::Crop) => self.reset_crop(),
            Some(Item::Colours) => self.edits.colours = engine::Colours::default(),
            Some(Item::Sharpen) => self.edits.sharpen = 0,
            Some(Item::Vignette) => self.edits.vignette = 0,
            Some(Item::Blur(i)) => {
                self.edits.blurs.remove(i);
                self.chosen = None;
            }
            Some(Item::Text(i)) => {
                self.edits.texts.remove(i);
                self.chosen = None;
            }
            Some(Item::Caption) => self.edits.caption = None,
            None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{first_error, on_screen};
    use eframe::egui::{pos2, Rect};

    #[test]
    fn the_first_error_of_ffmpeg() {
        let err = "[Parsed_pad_2 @ 0x7f77] Input area 0:100:720:1181 not within the padded area
[out#0/rawvideo @ 0x55] Nothing was written into output file
";
        assert_eq!(first_error(err), "Input area 0:100:720:1181 not within the padded area");
        assert_eq!(first_error(""), "ffmpeg");
    }

    #[test]
    fn frames_in_pixels_go_on_the_screen() {
        let pic = Rect::from_min_max(pos2(10.0, 20.0), pos2(210.0, 120.0));
        let r = on_screen([100.0, 50.0, 200.0, 100.0], (1000, 500), pic);
        assert_eq!(r, Rect::from_min_max(pos2(30.0, 30.0), pos2(70.0, 50.0)));
    }
}
