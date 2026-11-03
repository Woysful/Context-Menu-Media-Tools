//! The building blocks every window is made of, drawn in the style of the settings
//! (`theme::Look`): Classic, Brutalist or Y2K.

use super::paint::{self, alpha};
use crate::theme::{bold, bold_family, c, Look, CONTROL_H};
use eframe::egui::{
    self, pos2, text::LayoutJob, vec2, Align, Align2, Color32, FontId, Id, Layout, Margin, Rect, Response, RichText, Sense, Shape, Stroke, TextFormat, Vec2,
};

/// The space between the edges of a window and its content
pub const MARGIN: f32 = 20.0;
/// How far the hard shadow of a Brutalist button falls
const SHADOW: f32 = 3.0;
/// What the corner of every window says (Brutalist, Y2K)
const MARK: &str = concat!("CMMT/", env!("CARGO_PKG_VERSION"));
const PILL: f32 = CONTROL_H / 2.0;

pub fn muted() -> Color32 {
    c().muted
}

/// Text with space between the letters (captions, tags)
pub fn spaced(text: &str, font: FontId, color: Color32, spacing: f32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append(text, 0.0, TextFormat { font_id: font, color, extra_letter_spacing: spacing, ..Default::default() });
    job
}

fn paint_job(ui: &egui::Ui, pos: egui::Pos2, align: Align2, job: LayoutJob) -> Rect {
    let galley = ui.fonts(|f| f.layout_job(job));
    let rect = align.anchor_size(pos, galley.size());
    ui.painter().galley(rect.min, galley, Color32::WHITE);
    rect
}

/// A gel surface of Y2K: the gradient of the accent with a shine and a glow (`on`), or glass
fn gel(p: &egui::Painter, rect: Rect, radius: f32, on: bool, hovered: bool) {
    let k = c();
    if on {
        paint::glow(p, rect, radius, alpha(k.shadow, if hovered { 1.0 } else { 0.55 }), if hovered { 16.0 } else { 10.0 });
        paint::gradient(p, rect, radius, k.accent, k.accent2, true);
        paint::gloss(p, rect, radius, if k.dark { 0.8 } else { 1.0 });
    } else {
        if hovered {
            paint::glow(p, rect, radius, alpha(k.shadow, 0.45), 10.0);
        }
        p.rect(rect, radius, if hovered { k.hover } else { k.glass }, Stroke::new(1.0_f32, if hovered { k.accent } else { k.border }));
    }
}

/// A small label: `[ VIDEO ]` as a block of inverted text (Brutalist), a pill (Y2K) or
/// coloured capitals (Classic). `fill` is the accent or the colour of the text.
pub fn tag(ui: &egui::Ui, at: egui::Pos2, text: &str, fill: Color32, color: Color32) -> Rect {
    let k = c();
    let p = ui.painter();
    match k.look {
        Look::Classic => paint_job(
            ui,
            at,
            Align2::LEFT_TOP,
            spaced(&text.to_uppercase(), FontId::new(11.0, bold_family()), if fill == k.accent { k.accent } else { k.muted }, 0.5),
        ),
        Look::Brutal => {
            let galley = ui.fonts(|f| f.layout_job(spaced(&text.to_uppercase(), FontId::new(10.0, bold_family()), color, 1.2)));
            let rect = Rect::from_min_size(at, galley.size() + vec2(12.0, 4.0));
            p.rect_filled(rect, 0.0, fill);
            p.galley(rect.min + vec2(6.0, 2.0), galley, color);
            rect
        }
        Look::Y2k => {
            let on = fill == k.accent;
            let galley = ui.fonts(|f| f.layout_job(spaced(&text.to_uppercase(), FontId::new(10.0, bold_family()), if on { k.on_accent } else { k.text }, 1.4)));
            let rect = Rect::from_min_size(at, galley.size() + vec2(18.0, 6.0));
            if on {
                paint::gradient(p, rect, rect.height() / 2.0, k.accent, k.accent2, true);
                paint::gloss(p, rect, rect.height() / 2.0, 0.7);
            } else {
                p.rect(rect, rect.height() / 2.0, k.glass, Stroke::new(1.0_f32, k.border));
            }
            p.galley(rect.min + vec2(9.0, 3.0), galley, Color32::WHITE);
            rect
        }
    }
}

/// The number of the next row of a form (they are counted again in every frame)
fn next_index(ui: &egui::Ui) -> usize {
    let id = Id::new("cmmt_row_index");
    ui.ctx().data_mut(|d| {
        let n = d.get_temp::<usize>(id).unwrap_or(0) + 1;
        d.insert_temp(id, n);
        n
    })
}

fn reset_index(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(Id::new("cmmt_row_index"), 0usize));
}

pub fn text_width(ctx: &egui::Context, text: &str, font: FontId) -> f32 {
    ctx.fonts(|f| f.layout_no_wrap(text.to_string(), font, Color32::WHITE).size().x)
}

pub fn file_name(path: &std::path::Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// How many characters stay at the end of a cut name: the extension and 5 before it
fn tail_len(name: &str) -> usize {
    let chars = name.chars().count();
    let ext = name.rfind('.').map(|i| name[i..].chars().count()).filter(|&n| n > 1 && n <= 9 && n < chars);
    (ext.unwrap_or(3) + 5).min(chars)
}

/// The name with its middle cut out, `head` characters kept at the start ("Verylongfile…blabla.mp4")
pub fn cut_name(name: &str, head: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    let tail = tail_len(name);
    if head + tail + 1 >= chars.len() {
        return name.to_string();
    }
    let start: String = chars[..head].iter().collect();
    let end: String = chars[chars.len() - tail..].iter().collect();
    format!("{}…{}", start.trim_end(), end)
}

/// The name cut in the middle so that it fits into `width` in `font`
pub fn fit_name_in(ctx: &egui::Context, name: &str, font: FontId, width: f32) -> String {
    if text_width(ctx, name, font.clone()) <= width {
        return name.to_string();
    }
    let (mut lo, mut hi) = (0, name.chars().count());
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if text_width(ctx, &cut_name(name, mid), font.clone()) <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    cut_name(name, lo)
}

/// The name cut in the middle so that it fits into the width left in `ui`
pub fn fit_name(ui: &egui::Ui, name: &str, font: FontId) -> String {
    fit_name_in(ui.ctx(), name, font, ui.available_width() - 2.0)
}

/// The title of a window. Brutalist: a block of colour, capitals, the mark of the app and a
/// heavy rule with the ticks of a ruler. Y2K: capitals with a chromatic fringe, sparkles and a
/// line of the gradient.
pub fn title(ui: &mut egui::Ui, text: &str, color: Color32) {
    let k = c();
    match k.look {
        Look::Classic => {
            ui.label(bold(text, 18.0).color(color));
        }
        Look::Brutal => {
            let w = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
            let p = ui.painter();
            let block = if color == k.text { k.accent } else { color };
            p.rect_filled(Rect::from_min_size(pos2(rect.left(), rect.top() + 3.0), vec2(10.0, 22.0)), 0.0, block);
            let mark = paint_job(ui, rect.right_center(), Align2::RIGHT_CENTER, spaced(MARK, FontId::monospace(10.0), k.muted, 1.0));
            let clip = Rect::from_min_max(rect.min, pos2(mark.left() - 12.0, rect.bottom()));
            p.with_clip_rect(clip).text(
                pos2(rect.left() + 20.0, rect.center().y),
                Align2::LEFT_CENTER,
                text.to_uppercase(),
                FontId::new(20.0, bold_family()),
                color,
            );
            let y = rect.bottom() + 6.0;
            p.line_segment([pos2(rect.left(), y), pos2(rect.right(), y)], Stroke::new(2.0_f32, k.text));
            let mut x = rect.left();
            let mut n = 0;
            while x <= rect.right() + 0.5 {
                let long = n % 10 == 0;
                p.line_segment([pos2(x, y + 1.0), pos2(x, y + if long { 7.0 } else { 4.0 })], Stroke::new(1.0_f32, if long { k.border } else { k.rule }));
                x += 8.0;
                n += 1;
            }
            ui.add_space(14.0);
        }
        Look::Y2k => {
            let w = ui.available_width();
            let (rect, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
            let p = ui.painter();
            let s = rect.right_center() - vec2(10.0, 0.0);
            paint::glow(p, Rect::from_center_size(s, vec2(6.0, 6.0)), 3.0, alpha(k.shadow, 0.9), 14.0);
            paint::sparkle(p, s, 9.0, k.accent);
            paint::sparkle(p, s + vec2(-20.0, -7.0), 4.5, k.accent2);
            paint::sparkle(p, s + vec2(-13.0, 8.0), 3.0, alpha(k.text, 0.8));
            let mark = paint_job(ui, pos2(s.x - 36.0, s.y), Align2::RIGHT_CENTER, spaced(MARK, FontId::monospace(9.5), k.muted, 1.2));
            let clip = Rect::from_min_max(rect.min, pos2(mark.left() - 10.0, rect.bottom()));
            let font = FontId::new(19.0, bold_family());
            let caps = text.to_uppercase();
            let at = pos2(rect.left(), rect.center().y);
            for (off, col) in [(vec2(1.6, 1.0), alpha(k.accent2, 0.75)), (vec2(-1.2, -0.6), alpha(k.accent, 0.75)), (Vec2::ZERO, color)] {
                let job = spaced(&caps, font.clone(), col, 2.0);
                let galley = ui.fonts(|f| f.layout_job(job));
                let r = Align2::LEFT_CENTER.anchor_size(at + off, galley.size());
                p.with_clip_rect(clip).galley(r.min, galley, col);
            }
            let line = Rect::from_min_size(pos2(rect.left(), rect.bottom() + 6.0), vec2(w, 2.0));
            paint::gradient(p, line, 1.0, k.accent, alpha(k.accent2, 0.15), true);
            ui.add_space(12.0);
        }
    }
}

/// A small upper-case caption
pub fn caption(ui: &mut egui::Ui, text: &str) {
    let k = c();
    match k.look {
        Look::Classic => {
            ui.label(RichText::new(text.to_uppercase()).size(11.0).color(muted()));
        }
        Look::Brutal => {
            ui.label(spaced(&text.to_uppercase(), FontId::proportional(10.5), muted(), 1.0));
        }
        Look::Y2k => {
            ui.label(spaced(&text.to_uppercase(), FontId::proportional(10.5), muted(), 1.4));
        }
    }
}

/// A field: the caption above the control
pub fn field<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    caption(ui, label);
    let r = add(ui);
    ui.add_space(8.0);
    r
}

pub fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().color(muted()));
}

fn card_frame(top: f32) -> egui::Frame {
    let k = c();
    match k.look {
        Look::Classic => egui::Frame::none().fill(k.surface).stroke(Stroke::new(1.0_f32, k.border)).rounding(4.0).inner_margin(Margin::symmetric(12.0, 10.0)),
        Look::Brutal => egui::Frame::none().fill(k.surface).stroke(Stroke::new(1.0_f32, k.border)).rounding(0.0).inner_margin(Margin {
            left: 12.0,
            right: 12.0,
            top,
            bottom: 10.0,
        }),
        Look::Y2k => egui::Frame::none().fill(k.glass).stroke(Stroke::new(1.0_f32, k.border)).rounding(14.0).inner_margin(Margin {
            left: 14.0,
            right: 14.0,
            top: top + 2.0,
            bottom: 12.0,
        }),
    }
}

/// What sits on the edge of a card: a solid square (Brutalist) or a line of light (Y2K)
fn card_marks(ui: &egui::Ui, rect: Rect) {
    let k = c();
    match k.look {
        Look::Classic => {}
        Look::Brutal => {
            ui.painter().rect_filled(Rect::from_min_size(rect.left_top(), vec2(6.0, 6.0)), 0.0, k.accent);
        }
        Look::Y2k => {
            let line = Rect::from_min_size(pos2(rect.left() + 16.0, rect.top() + 0.5), vec2(rect.width() * 0.45, 1.0));
            paint::gradient(ui.painter(), line, 0.0, alpha(Color32::WHITE, if k.dark { 0.45 } else { 0.9 }), Color32::TRANSPARENT, true);
        }
    }
}

/// A framed block on the surface colour (the log, the text of a command)
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let r = card_frame(10.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    card_marks(ui, r.response.rect);
    r.inner
}

/// A card with a tag on its top edge (`INPUT`, `LOG`) and an optional note at the right of it.
/// Classic has neither: a plain card.
pub fn tagged_card<R>(ui: &mut egui::Ui, label: &str, note: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let k = c();
    if k.look == Look::Classic {
        return card(ui, add);
    }
    let r = card_frame(28.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    let rect = r.response.rect;
    card_marks(ui, rect);
    let t = match k.look {
        Look::Y2k => tag(ui, rect.left_top() + vec2(12.0, 9.0), label, k.accent, k.on_accent),
        _ => tag(ui, rect.left_top(), label, k.text, k.bg),
    };
    if !note.is_empty() {
        let spacing = if k.look == Look::Y2k { 1.4 } else { 1.0 };
        paint_job(ui, pos2(rect.right() - 12.0, t.center().y), Align2::RIGHT_CENTER, spaced(&note.to_uppercase(), FontId::monospace(10.0), k.muted, spacing));
    }
    r.inner
}

/// A row of right-aligned buttons with a fixed height (with_layout would take all the
/// remaining height of the panel)
pub fn button_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    match c().look {
        Look::Brutal => {
            let size = Vec2::new(ui.available_width(), CONTROL_H + SHADOW);
            ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Min), |ui| {
                ui.spacing_mut().item_spacing.x = 10.0 + SHADOW;
                add(ui)
            });
        }
        _ => {
            let size = Vec2::new(ui.available_width(), CONTROL_H + 2.0);
            ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), add);
        }
    }
}

/// A button: flat (Classic), a slab with a hard shadow that sinks into it when pressed
/// (Brutalist) or a gel pill (Y2K)
pub struct Slab {
    text: String,
    primary: bool,
    min: Option<Vec2>,
}

impl Slab {
    pub fn min_size(mut self, min: Vec2) -> Slab {
        self.min = Some(min);
        self
    }
}

pub fn primary_button(text: &str) -> Slab {
    Slab { text: text.to_string(), primary: true, min: None }
}

pub fn secondary_button(text: &str) -> Slab {
    Slab { text: text.to_string(), primary: false, min: None }
}

impl egui::Widget for Slab {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let k = c();
        let (text, font, pad, min_w) = match (k.look, self.primary) {
            (Look::Classic, true) => (self.text, FontId::new(13.5, bold_family()), 24.0, 110.0),
            (Look::Classic, false) => (self.text, FontId::proportional(13.5), 24.0, 96.0),
            (Look::Brutal, true) => (format!("{} →", self.text.to_uppercase()), FontId::new(12.5, bold_family()), 28.0, 120.0),
            (Look::Brutal, false) => (self.text.to_uppercase(), FontId::new(12.5, bold_family()), 28.0, 96.0),
            (Look::Y2k, true) => (self.text, FontId::new(13.0, bold_family()), 40.0, 120.0),
            (Look::Y2k, false) => (self.text, FontId::proportional(13.0), 36.0, 100.0),
        };
        let min = self.min.unwrap_or(vec2(min_w, CONTROL_H));
        let w = text_width(ui.ctx(), &text, font.clone()) + pad;
        let size = vec2(w.max(min.x), min.y.max(CONTROL_H));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let enabled = ui.is_enabled();
        let pressed = enabled && resp.is_pointer_button_down_on();
        let hovered = enabled && resp.hovered();
        let p = ui.painter();
        match k.look {
            Look::Classic => {
                let (fill, stroke, color) = match (self.primary, hovered) {
                    (true, h) => (if h { theme_mix(k.accent, Color32::WHITE, 0.08) } else { k.accent }, Stroke::NONE, k.on_accent),
                    (false, true) => (k.hover, Stroke::new(1.0_f32, k.accent), k.text),
                    (false, false) => (k.surface, Stroke::new(1.0_f32, k.border), k.text),
                };
                let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.45) };
                p.rect(rect, 3.0, fade(fill), Stroke::new(stroke.width, fade(stroke.color)));
                p.text(rect.center(), Align2::CENTER_CENTER, &text, font, fade(color));
            }
            Look::Brutal => {
                let sink = if pressed {
                    SHADOW
                } else if hovered {
                    1.0
                } else {
                    0.0
                };
                let face = rect.translate(vec2(sink, sink));
                if enabled && sink < SHADOW {
                    p.rect_filled(rect.translate(vec2(SHADOW, SHADOW)), 0.0, k.shadow);
                }
                let (fill, stroke, color) = match (enabled, self.primary) {
                    (false, _) => (k.surface, Stroke::new(1.0_f32, k.rule), k.muted),
                    (true, true) => (k.accent, Stroke::new(2.0_f32, k.accent), k.on_accent),
                    (true, false) => (k.bg, Stroke::new(2.0_f32, if hovered { k.accent } else { k.border }), k.text),
                };
                p.rect(face, 0.0, fill, stroke);
                p.text(face.center(), Align2::CENTER_CENTER, &text, font, color);
            }
            Look::Y2k => {
                let face = if pressed { rect.translate(vec2(0.0, 1.0)) } else { rect };
                let r = face.height() / 2.0;
                if enabled {
                    gel(p, face, r, self.primary, hovered);
                } else {
                    p.rect(face, r, k.glass, Stroke::new(1.0_f32, k.rule));
                }
                let color = match (enabled, self.primary) {
                    (false, _) => k.muted,
                    (true, true) => k.on_accent,
                    (true, false) => k.text,
                };
                p.text(face.center(), Align2::CENTER_CENTER, &text, font, color);
            }
        }
        if hovered && k.look != Look::Classic {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }
}

fn theme_mix(a: Color32, b: Color32, t: f32) -> Color32 {
    crate::theme::mix(a, b, t)
}

/// One of several values as one control split into equal parts (the window width or the
/// width of a form row); the response is "changed" when another part was chosen
pub fn segmented(ui: &mut egui::Ui, labels: &[&str], current: &mut usize) -> egui::Response {
    let k = c();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), CONTROL_H), Sense::hover());
    let painter = ui.painter().clone();
    match k.look {
        Look::Classic => painter.rect(rect, 4.0, k.surface, Stroke::new(1.0_f32, k.border)),
        Look::Brutal => painter.rect_filled(rect, 0.0, k.surface),
        Look::Y2k => painter.rect(rect, PILL, k.glass, Stroke::new(1.0_f32, k.border)),
    };
    let seg_w = rect.width() / labels.len().max(1) as f32;
    let mut changed = false;
    let mut all = resp.clone();
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + seg_w * i as f32, rect.top()), vec2(seg_w, CONTROL_H));
        let part = ui.interact(r, resp.id.with(i), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        all = all.union(part.clone());
        let selected = *current == i;
        match k.look {
            Look::Classic => {
                if selected {
                    painter.rect_filled(r.shrink(3.0), 3.0, k.accent);
                } else if part.hovered() {
                    painter.rect_filled(r.shrink(3.0), 3.0, k.hover);
                }
                // a thin line between two parts that are both not selected
                if i > 0 && !selected && *current != i - 1 {
                    painter.line_segment([pos2(r.left(), r.top() + 8.0), pos2(r.left(), r.bottom() - 8.0)], Stroke::new(1.0_f32, k.border));
                }
            }
            Look::Brutal => {
                if selected {
                    painter.rect_filled(r, 0.0, k.accent);
                } else if part.hovered() {
                    painter.rect_filled(r, 0.0, k.hover);
                }
                if i > 0 {
                    painter.line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0_f32, k.border));
                }
            }
            Look::Y2k => {
                let inner = r.shrink(3.0);
                if selected {
                    gel(&painter, inner, inner.height() / 2.0, true, part.hovered());
                } else if part.hovered() {
                    painter.rect_filled(inner, inner.height() / 2.0, k.hover);
                }
            }
        }
        let (text, font, color) = match (k.look, selected) {
            (Look::Brutal, true) => (label.to_uppercase(), FontId::new(12.5, bold_family()), k.on_accent),
            (Look::Brutal, false) => (label.to_uppercase(), FontId::proportional(12.5), k.text),
            (_, true) => (label.to_string(), FontId::new(13.0, bold_family()), k.on_accent),
            (_, false) => (label.to_string(), FontId::proportional(13.0), k.text),
        };
        painter.text(r.center(), Align2::CENTER_CENTER, text, font, color);
        if part.clicked() && !selected {
            *current = i;
            changed = true;
        }
    }
    if k.look == Look::Brutal {
        painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, k.border));
    }
    if changed {
        all.mark_changed();
    }
    all
}

/// The width of the names in front of the controls of a form
fn label_w() -> f32 {
    match c().look {
        Look::Classic => 104.0,
        Look::Brutal => 116.0,
        Look::Y2k => 112.0,
    }
}

/// A row of a form: the name at the left, the control (and a line about it) at the right.
/// Brutalist: a line of a spec sheet, numbered between dashed rules. Y2K: a diamond before the name.
pub fn form_row<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let k = c();
    let label_w = label_w();
    if k.look == Look::Brutal {
        let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
        ui.painter().add(Shape::dashed_line(&[line.left_center(), line.right_center()], Stroke::new(1.0_f32, k.rule), 3.0, 3.0));
        ui.add_space(2.0);
    }
    let n = next_index(ui);
    let inner = ui
        .horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(label_w, CONTROL_H), Sense::hover());
            match k.look {
                Look::Classic => {
                    ui.painter().text(r.left_center(), Align2::LEFT_CENTER, label, FontId::proportional(13.0), muted());
                }
                Look::Brutal => {
                    let num = paint_job(ui, r.left_center(), Align2::LEFT_CENTER, spaced(&format!("{n:02}"), FontId::new(10.5, bold_family()), k.accent, 0.5));
                    let clip = Rect::from_min_max(pos2(num.right() + 8.0, r.top()), r.max);
                    let galley = ui.fonts(|f| f.layout_job(spaced(&label.to_uppercase(), FontId::proportional(11.0), k.muted, 0.8)));
                    let at = Align2::LEFT_CENTER.anchor_size(pos2(clip.left(), r.center().y), galley.size()).min;
                    ui.painter().with_clip_rect(clip).galley(at, galley, k.muted);
                }
                Look::Y2k => {
                    paint::diamond(ui.painter(), r.left_center() + vec2(4.0, 0.0), 3.5, k.accent2);
                    let clip = Rect::from_min_max(pos2(r.left() + 14.0, r.top()), r.max);
                    ui.painter().with_clip_rect(clip).text(pos2(clip.left(), r.center().y), Align2::LEFT_CENTER, label, FontId::proportional(12.5), k.muted);
                }
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                add(ui)
            })
            .inner
        })
        .inner;
    ui.add_space(if k.look == Look::Brutal { 5.0 } else { 8.0 });
    inner
}

/// A line of small text that is cut at the edge instead of wrapping (keeps the height of a form fixed)
pub fn one_line(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.add(egui::Label::new(RichText::new(text).small().color(color)).truncate());
}

/// A one-line text field as high as the buttons and drop-down lists
pub fn line_edit<'a>(text: &'a mut String, width: f32, valid: bool) -> egui::TextEdit<'a> {
    let mut edit =
        egui::TextEdit::singleline(text).desired_width(width).min_size(vec2(width, CONTROL_H)).vertical_align(Align::Center).horizontal_align(Align::Center);
    if !valid {
        edit = edit.text_color(c().err);
    }
    edit
}

/// One of several values, as a row of buttons: (value, label)
pub fn choice(ui: &mut egui::Ui, current: &mut String, options: &[(&str, &str)]) {
    let k = c();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(if k.look == Look::Brutal { 0.0 } else { 6.0 }, 6.0);
        for (value, label) in options {
            let selected = current == value;
            let b = match k.look {
                Look::Classic => {
                    let text = if selected { bold(*label, 13.0).color(k.on_accent) } else { RichText::new(*label) };
                    let b = egui::Button::new(text).min_size(vec2(0.0, CONTROL_H));
                    if selected {
                        b.fill(k.accent).stroke(Stroke::NONE)
                    } else {
                        b
                    }
                }
                Look::Brutal => {
                    let text = if selected { bold(label.to_uppercase(), 12.5).color(k.on_accent) } else { RichText::new(label.to_uppercase()).size(12.5) };
                    let b = egui::Button::new(text).min_size(vec2(0.0, CONTROL_H)).stroke(Stroke::new(1.0_f32, k.border));
                    if selected {
                        b.fill(k.accent).stroke(Stroke::new(1.0_f32, k.accent))
                    } else {
                        b
                    }
                }
                Look::Y2k => {
                    let text = if selected { bold(*label, 13.0).color(k.on_accent) } else { RichText::new(*label) };
                    let b = egui::Button::new(text).min_size(vec2(64.0, CONTROL_H)).rounding(PILL);
                    if selected {
                        b.fill(k.accent).stroke(Stroke::NONE)
                    } else {
                        b.fill(k.glass)
                    }
                }
            };
            let r = ui.add(b);
            if selected && k.look == Look::Y2k {
                gel(ui.painter(), r.rect, PILL, true, r.hovered());
                ui.painter().text(r.rect.center(), Align2::CENTER_CENTER, *label, FontId::new(13.0, bold_family()), k.on_accent);
            }
            if r.clicked() {
                *current = value.to_string();
            }
        }
    });
}

/// One of several values as a grid of equal cells, two in a row: the name and a few words
/// about it, the words of all cells starting at the same place. Options: (value, name, hint)
pub fn option_grid(ui: &mut egui::Ui, current: &mut String, options: &[(&str, &str, &str)]) -> egui::Response {
    const COLS: usize = 2;
    const GAP: f32 = 6.0;
    let k = c();
    let brutal = k.look == Look::Brutal;
    let caps = |n: &str| if brutal { n.to_uppercase() } else { n.to_string() };
    let name_font = FontId::new(13.0, bold_family());
    let hint_font = FontId::proportional(if brutal { 11.0 } else { 11.5 });
    let pad = match k.look {
        Look::Classic => 12.0,
        Look::Brutal => 14.0,
        Look::Y2k => 14.0,
    };
    let width = ui.available_width();
    let cell_w = (width - GAP * (COLS - 1) as f32) / COLS as f32;
    let name_w = options.iter().map(|(_, n, _)| text_width(ui.ctx(), &caps(n), name_font.clone())).fold(0.0, f32::max);
    let rows = options.len().div_ceil(COLS);
    let (area, mut all) = ui.allocate_exact_size(vec2(width, rows as f32 * (CONTROL_H + GAP) - GAP), Sense::hover());
    let painter = ui.painter().clone();
    for (i, (value, name, hint)) in options.iter().enumerate() {
        let at = vec2((i % COLS) as f32 * (cell_w + GAP), (i / COLS) as f32 * (CONTROL_H + GAP));
        let r = Rect::from_min_size(area.min + at, vec2(cell_w, CONTROL_H));
        let resp = ui.interact(r, ui.id().with(("option", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        all = all.union(resp.clone());
        let selected = current == value;
        let hovered = resp.hovered();
        let (text_c, hint_c) = if selected { (k.on_accent, k.on_accent.gamma_multiply(0.75)) } else { (k.text, k.muted) };
        match k.look {
            Look::Classic => {
                let (fill, stroke) =
                    if selected { (k.accent, Stroke::NONE) } else { (if hovered { k.hover } else { k.surface }, Stroke::new(1.0_f32, k.border)) };
                painter.rect(r, 4.0, fill, stroke);
            }
            Look::Brutal => {
                let (fill, stroke) = if selected {
                    (k.accent, Stroke::new(1.0_f32, k.accent))
                } else if hovered {
                    (k.hover, Stroke::new(2.0_f32, k.accent))
                } else {
                    (k.surface, Stroke::new(1.0_f32, k.border))
                };
                painter.rect(r, 0.0, fill, stroke);
                // a notch at the left: the cell that is chosen gets a solid one
                painter.rect_filled(Rect::from_min_size(r.left_top(), vec2(4.0, CONTROL_H)), 0.0, if selected { k.on_accent } else { k.rule });
            }
            Look::Y2k => gel(&painter, r, 10.0, selected, hovered),
        }
        painter.text(pos2(r.left() + pad, r.center().y), Align2::LEFT_CENTER, caps(name), name_font.clone(), text_c);
        if !hint.is_empty() {
            let x = r.left() + pad + name_w + 12.0;
            let clip = Rect::from_min_max(pos2(x, r.top()), pos2(r.right() - 8.0, r.bottom()));
            painter.with_clip_rect(clip).text(pos2(x, r.center().y), Align2::LEFT_CENTER, *hint, hint_font.clone(), hint_c);
        }
        if resp.clicked() {
            *current = value.to_string();
            all.mark_changed();
        }
    }
    all
}

/// A text field for a number, red when it is not valid
pub fn number_field(ui: &mut egui::Ui, text: &mut String, valid: bool, suffix: &str) -> egui::Response {
    let brutal = c().look == Look::Brutal;
    ui.horizontal(|ui| {
        let w = (ui.available_width() - if suffix.is_empty() { 0.0 } else { 36.0 }).max(40.0);
        let mut edit = egui::TextEdit::singleline(text).desired_width(w).horizontal_align(Align::Center);
        if !valid {
            edit = edit.text_color(c().err);
        }
        let r = ui.add(edit);
        if !suffix.is_empty() {
            ui.label(RichText::new(if brutal { suffix.to_uppercase() } else { suffix.to_string() }).color(muted()));
        }
        r
    })
    .inner
}

/// A slider with its value at the right (a readout in a box: Brutalist, a pill: Y2K)
pub fn slider_row(ui: &mut egui::Ui, value: &mut u32, range: std::ops::RangeInclusive<u32>) -> egui::Response {
    let k = c();
    ui.horizontal(|ui| {
        let room = if k.look == Look::Classic { 48.0 } else { 60.0 };
        ui.spacing_mut().slider_width = (ui.available_width() - room).max(80.0);
        let before = *value;
        let r = ui.add(egui::Slider::new(value, range).show_value(false));
        // a slider follows every mouse button: the right one opens the context menu instead
        let secondary = ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary) || i.pointer.button_released(egui::PointerButton::Secondary));
        if secondary {
            *value = before;
        }
        match k.look {
            Look::Classic => {
                ui.label(RichText::new(value.to_string()).monospace());
            }
            Look::Brutal => {
                let (rect, _) = ui.allocate_exact_size(vec2(44.0, 22.0), Sense::hover());
                ui.painter().rect(rect, 0.0, k.bg, Stroke::new(1.0_f32, k.border));
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, format!("{:03}", *value), FontId::new(12.0, bold_family()), k.text);
            }
            Look::Y2k => {
                let (rect, _) = ui.allocate_exact_size(vec2(46.0, 22.0), Sense::hover());
                ui.painter().rect(rect, 11.0, k.glass, Stroke::new(1.0_f32, k.border));
                ui.painter().text(rect.center(), Align2::CENTER_CENTER, value.to_string(), FontId::new(12.0, bold_family()), k.accent);
            }
        }
        r
    })
    .inner
}

/// The crop marks in the corners of a window (Brutalist)
fn crop_marks(ctx: &egui::Context) {
    let k = c();
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::PanelResizeLine, Id::new("cmmt_marks")));
    let r = ctx.screen_rect().shrink(6.0);
    let s = Stroke::new(1.0_f32, k.border);
    let l = 9.0;
    for (corner, dx, dy) in [(r.left_top(), 1.0, 1.0), (r.right_top(), -1.0, 1.0), (r.left_bottom(), 1.0, -1.0), (r.right_bottom(), -1.0, -1.0)] {
        p.line_segment([corner, corner + vec2(l * dx, 0.0)], s);
        p.line_segment([corner, corner + vec2(0.0, l * dy)], s);
    }
}

/// The background of a window. Y2K paints a gradient with two spots of light under the panels
/// and returns a clear fill for them; the other styles return the fill of the panels.
pub fn backdrop(ctx: &egui::Context) -> Color32 {
    let k = c();
    let fill = ctx.style().visuals.panel_fill;
    if k.look != Look::Y2k {
        return fill;
    }
    let a = fill.a() as f32 / 255.0;
    let p = ctx.layer_painter(egui::LayerId::background());
    let r = ctx.screen_rect();
    paint::gradient(&p, r, 0.0, alpha(k.bg, a), alpha(k.bg2, a), false);
    let strength = if k.dark { 0.22 } else { 0.28 };
    paint::aura(&p, pos2(r.right() - r.width() * 0.1, r.top() + 20.0), r.width().max(r.height()) * 0.55, alpha(k.accent, strength * a));
    paint::aura(&p, pos2(r.left() + r.width() * 0.05, r.bottom()), r.width().max(r.height()) * 0.6, alpha(k.accent2, strength * a));
    Color32::TRANSPARENT
}

/// The layout of the job windows: the buttons at the bottom, the content above them.
/// `scroll`: the content scrolls when the window is smaller than it (forms); without it the
/// content may fill the height (a log). Returns what `body` returned and the height the
/// window needs for all of it.
pub fn window<R>(ctx: &egui::Context, scroll: bool, footer: impl FnOnce(&mut egui::Ui), body: impl FnOnce(&mut egui::Ui) -> R) -> (R, f32) {
    reset_index(ctx);
    let look = c().look;
    let fill = backdrop(ctx);
    let (body_top, foot_top, extra) = match look {
        Look::Classic => (18.0, 12.0, 0.0),
        Look::Brutal => (16.0, 14.0, 8.0),
        Look::Y2k => (18.0, 14.0, 6.0),
    };
    let foot = egui::TopBottomPanel::bottom("footer")
        .frame(egui::Frame::none().fill(fill).inner_margin(Margin { left: MARGIN, right: MARGIN, top: foot_top, bottom: 16.0 }))
        .show_separator_line(false)
        .show(ctx, |ui| {
            let k = c();
            let r = ui.max_rect();
            let y = r.top() - foot_top;
            match look {
                Look::Classic => {}
                Look::Brutal => {
                    ui.painter().line_segment([pos2(r.left(), y), pos2(r.right(), y)], Stroke::new(1.0_f32, k.border));
                }
                Look::Y2k => paint::gradient(
                    ui.painter(),
                    Rect::from_min_size(pos2(r.left(), y), vec2(r.width(), 1.0)),
                    0.0,
                    alpha(k.accent2, 0.1),
                    alpha(k.accent, 0.6),
                    true,
                ),
            }
            footer(ui)
        });
    let frame = egui::Frame::none().fill(fill).inner_margin(Margin { left: MARGIN, right: MARGIN, top: body_top, bottom: 0.0 });
    let (inner, content_h) = egui::CentralPanel::default()
        .frame(frame)
        .show(ctx, |ui| {
            let measured = |ui: &mut egui::Ui| {
                let resp = ui.vertical(body);
                (resp.inner, resp.response.rect.height())
            };
            if scroll {
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, measured).inner
            } else {
                measured(ui)
            }
        })
        .inner;
    if look == Look::Brutal {
        crop_marks(ctx);
    }
    (inner, body_top + content_h + extra + foot.response.rect.height())
}

#[cfg(test)]
mod tests {
    use super::cut_name;

    #[test]
    fn cut_keeps_the_end_of_the_name() {
        assert_eq!(cut_name("Verylongfilename_blabla.mp4", 8), "Verylong…labla.mp4");
        assert_eq!(cut_name("short.mp4", 3), "short.mp4");
        assert_eq!(cut_name("no extension at all", 4), "no e…n at all");
        assert_eq!(cut_name("clip.unusually-long-ext", 4), "clip…long-ext");
    }
}
