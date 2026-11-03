//! The building blocks every window is made of: square, hard-edged, technical.

use crate::theme::{bold, bold_family, c, CONTROL_H};
use eframe::egui::{
    self, pos2, text::LayoutJob, vec2, Align, Align2, Color32, FontId, Id, Layout, Margin, Rect, Response, RichText, Sense, Shape, Stroke, TextFormat, Vec2,
};

/// The space between the edges of a window and its content
pub const MARGIN: f32 = 22.0;
const BODY_TOP: f32 = 16.0;
/// How far the hard shadow of a button falls
const SHADOW: f32 = 3.0;
/// What the corner of every window says
const MARK: &str = concat!("CMMT/", env!("CARGO_PKG_VERSION"));

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

/// A small block of inverted text: `[ VIDEO ]` as a label on a frame
pub fn tag(ui: &egui::Ui, at: egui::Pos2, text: &str, fill: Color32, color: Color32) -> Rect {
    let job = spaced(&text.to_uppercase(), FontId::new(10.0, bold_family()), color, 1.2);
    let galley = ui.fonts(|f| f.layout_job(job));
    let rect = Rect::from_min_size(at, galley.size() + vec2(12.0, 4.0));
    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter().galley(rect.min + vec2(6.0, 2.0), galley, color);
    rect
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

/// The title of a window: a block of colour, the name in capitals, the mark of the app at the
/// right and a heavy rule with the ticks of a ruler under it
pub fn title(ui: &mut egui::Ui, text: &str, color: Color32) {
    let k = c();
    let w = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(vec2(w, 28.0), Sense::hover());
    let p = ui.painter();
    let block = if color == k.text { k.accent } else { color };
    p.rect_filled(Rect::from_min_size(pos2(rect.left(), rect.top() + 3.0), vec2(10.0, 22.0)), 0.0, block);
    let mark = paint_job(ui, rect.right_center(), Align2::RIGHT_CENTER, spaced(MARK, FontId::monospace(10.0), k.muted, 1.0));
    let clip = Rect::from_min_max(rect.min, pos2(mark.left() - 12.0, rect.bottom()));
    p.with_clip_rect(clip).text(pos2(rect.left() + 20.0, rect.center().y), Align2::LEFT_CENTER, text.to_uppercase(), FontId::new(20.0, bold_family()), color);
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

/// A small upper-case caption with space between the letters
pub fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(spaced(&text.to_uppercase(), FontId::proportional(10.5), muted(), 1.0));
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
    egui::Frame::none().fill(k.surface).stroke(Stroke::new(1.0_f32, k.border)).rounding(0.0).inner_margin(Margin { left: 12.0, right: 12.0, top, bottom: 10.0 })
}

/// The mark of a frame on a technical drawing: a solid square in the top left corner
fn corner(ui: &egui::Ui, rect: Rect) {
    ui.painter().rect_filled(Rect::from_min_size(rect.left_top(), vec2(6.0, 6.0)), 0.0, c().accent);
}

/// A framed block on the surface colour (the log, the text of a command)
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let r = card_frame(10.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    corner(ui, r.response.rect);
    r.inner
}

/// A card with a tag on its top edge (`INPUT`, `LOG`) and an optional note at the right of it
pub fn tagged_card<R>(ui: &mut egui::Ui, label: &str, note: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let k = c();
    let r = card_frame(28.0).show(ui, |ui| {
        ui.set_width(ui.available_width());
        add(ui)
    });
    let rect = r.response.rect;
    let t = tag(ui, rect.left_top(), label, k.text, k.bg);
    if !note.is_empty() {
        paint_job(ui, pos2(rect.right() - 10.0, t.center().y), Align2::RIGHT_CENTER, spaced(&note.to_uppercase(), FontId::monospace(10.0), k.muted, 1.0));
    }
    r.inner
}

/// A row of right-aligned buttons with a fixed height (with_layout would take all the
/// remaining height of the panel)
pub fn button_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let size = Vec2::new(ui.available_width(), CONTROL_H + SHADOW);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 10.0 + SHADOW;
        add(ui)
    });
}

/// A slab of a button with a hard shadow; it sinks into the shadow when pressed
pub struct Slab {
    text: String,
    primary: bool,
    min: Vec2,
}

impl Slab {
    pub fn min_size(mut self, min: Vec2) -> Slab {
        self.min = min;
        self
    }
}

pub fn primary_button(text: &str) -> Slab {
    Slab { text: format!("{} →", text.to_uppercase()), primary: true, min: vec2(120.0, CONTROL_H) }
}

pub fn secondary_button(text: &str) -> Slab {
    Slab { text: text.to_uppercase(), primary: false, min: vec2(96.0, CONTROL_H) }
}

impl egui::Widget for Slab {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let k = c();
        let font = FontId::new(12.5, bold_family());
        let w = text_width(ui.ctx(), &self.text, font.clone()) + 28.0;
        let size = vec2(w.max(self.min.x), self.min.y.max(CONTROL_H));
        let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
        let enabled = ui.is_enabled();
        let pressed = enabled && resp.is_pointer_button_down_on();
        let hovered = enabled && resp.hovered();
        let sink = if pressed {
            SHADOW
        } else if hovered {
            1.0
        } else {
            0.0
        };
        let face = rect.translate(vec2(sink, sink));
        let p = ui.painter();
        if enabled && sink < SHADOW {
            p.rect_filled(rect.translate(vec2(SHADOW, SHADOW)), 0.0, k.shadow);
        }
        let (fill, stroke, color) = match (enabled, self.primary) {
            (false, _) => (k.surface, Stroke::new(1.0_f32, k.rule), k.muted),
            (true, true) => (k.accent, Stroke::new(2.0_f32, k.accent), k.on_accent),
            (true, false) => (k.bg, Stroke::new(2.0_f32, if hovered { k.accent } else { k.border }), k.text),
        };
        p.rect(face, 0.0, fill, stroke);
        p.text(face.center(), Align2::CENTER_CENTER, &self.text, font, color);
        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        resp
    }
}

/// One of several values as one control split into equal cells (the window width or the
/// width of a form row); the response is "changed" when another cell was chosen
pub fn segmented(ui: &mut egui::Ui, labels: &[&str], current: &mut usize) -> egui::Response {
    let k = c();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), CONTROL_H), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect_filled(rect, 0.0, k.surface);
    let seg_w = rect.width() / labels.len().max(1) as f32;
    let mut changed = false;
    let mut all = resp.clone();
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + seg_w * i as f32, rect.top()), vec2(seg_w, CONTROL_H));
        let part = ui.interact(r, resp.id.with(i), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        all = all.union(part.clone());
        let selected = *current == i;
        if selected {
            painter.rect_filled(r, 0.0, k.accent);
        } else if part.hovered() {
            painter.rect_filled(r, 0.0, k.hover);
        }
        if i > 0 {
            painter.line_segment([r.left_top(), r.left_bottom()], Stroke::new(1.0_f32, k.border));
        }
        let (font, color) = if selected { (FontId::new(12.5, bold_family()), k.on_accent) } else { (FontId::proportional(12.5), k.text) };
        painter.text(r.center(), Align2::CENTER_CENTER, label.to_uppercase(), font, color);
        if part.clicked() && !selected {
            *current = i;
            changed = true;
        }
    }
    painter.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, k.border));
    if changed {
        all.mark_changed();
    }
    all
}

/// The width of the names in front of the controls of a form
pub const LABEL_W: f32 = 116.0;

/// A row of a form, like a line of a spec sheet: a dashed rule, the number of the row and its
/// name at the left, the control (and a line about it) at the right
pub fn form_row<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let k = c();
    let n = next_index(ui);
    let (line, _) = ui.allocate_exact_size(vec2(ui.available_width(), 1.0), Sense::hover());
    ui.painter().add(Shape::dashed_line(&[line.left_center(), line.right_center()], Stroke::new(1.0_f32, k.rule), 3.0, 3.0));
    ui.add_space(2.0);
    let inner = ui
        .horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(LABEL_W, CONTROL_H), Sense::hover());
            let num = paint_job(ui, r.left_center(), Align2::LEFT_CENTER, spaced(&format!("{n:02}"), FontId::new(10.5, bold_family()), k.accent, 0.5));
            let clip = Rect::from_min_max(pos2(num.right() + 8.0, r.top()), r.max);
            let galley = ui.fonts(|f| f.layout_job(spaced(&label.to_uppercase(), FontId::proportional(11.0), k.muted, 0.8)));
            let at = Align2::LEFT_CENTER.anchor_size(pos2(clip.left(), r.center().y), galley.size()).min;
            ui.painter().with_clip_rect(clip).galley(at, galley, k.muted);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                add(ui)
            })
            .inner
        })
        .inner;
    ui.add_space(5.0);
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

/// One of several values, as a row of square cells: (value, label)
pub fn choice(ui: &mut egui::Ui, current: &mut String, options: &[(&str, &str)]) {
    let k = c();
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(0.0, 6.0);
        for (value, label) in options {
            let selected = current == value;
            let text = if selected { bold(label.to_uppercase(), 12.5).color(k.on_accent) } else { RichText::new(label.to_uppercase()).size(12.5) };
            let mut b = egui::Button::new(text).min_size(vec2(0.0, CONTROL_H)).stroke(Stroke::new(1.0_f32, k.border));
            if selected {
                b = b.fill(k.accent).stroke(Stroke::new(1.0_f32, k.accent));
            }
            if ui.add(b).clicked() {
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
    let name_font = FontId::new(13.0, bold_family());
    let hint_font = FontId::proportional(11.0);
    let width = ui.available_width();
    let cell_w = (width - GAP * (COLS - 1) as f32) / COLS as f32;
    let name_w = options.iter().map(|(_, n, _)| text_width(ui.ctx(), &n.to_uppercase(), name_font.clone())).fold(0.0, f32::max);
    let rows = options.len().div_ceil(COLS);
    let (area, mut all) = ui.allocate_exact_size(vec2(width, rows as f32 * (CONTROL_H + GAP) - GAP), Sense::hover());
    let painter = ui.painter().clone();
    for (i, (value, name, hint)) in options.iter().enumerate() {
        let at = vec2((i % COLS) as f32 * (cell_w + GAP), (i / COLS) as f32 * (CONTROL_H + GAP));
        let r = Rect::from_min_size(area.min + at, vec2(cell_w, CONTROL_H));
        let resp = ui.interact(r, ui.id().with(("option", i)), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        all = all.union(resp.clone());
        let selected = current == value;
        let (fill, stroke, text_c, hint_c) = if selected {
            (k.accent, Stroke::new(1.0_f32, k.accent), k.on_accent, k.on_accent.gamma_multiply(0.75))
        } else if resp.hovered() {
            (k.hover, Stroke::new(2.0_f32, k.accent), k.text, k.muted)
        } else {
            (k.surface, Stroke::new(1.0_f32, k.border), k.text, k.muted)
        };
        painter.rect(r, 0.0, fill, stroke);
        // a notch at the left: the cell that is chosen gets a solid one
        painter.rect_filled(Rect::from_min_size(r.left_top(), vec2(4.0, CONTROL_H)), 0.0, if selected { k.on_accent } else { k.rule });
        painter.text(pos2(r.left() + 14.0, r.center().y), Align2::LEFT_CENTER, name.to_uppercase(), name_font.clone(), text_c);
        if !hint.is_empty() {
            let x = r.left() + 14.0 + name_w + 12.0;
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
    ui.horizontal(|ui| {
        let w = (ui.available_width() - if suffix.is_empty() { 0.0 } else { 36.0 }).max(40.0);
        let mut edit = egui::TextEdit::singleline(text).desired_width(w).horizontal_align(Align::Center);
        if !valid {
            edit = edit.text_color(c().err);
        }
        let r = ui.add(edit);
        if !suffix.is_empty() {
            ui.label(RichText::new(suffix.to_uppercase()).color(muted()));
        }
        r
    })
    .inner
}

/// A slider with its value in a box at the right, like a readout
pub fn slider_row(ui: &mut egui::Ui, value: &mut u32, range: std::ops::RangeInclusive<u32>) -> egui::Response {
    let k = c();
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 60.0).max(80.0);
        let before = *value;
        let r = ui.add(egui::Slider::new(value, range).show_value(false));
        // a slider follows every mouse button: the right one opens the context menu instead
        let secondary = ui.input(|i| i.pointer.button_down(egui::PointerButton::Secondary) || i.pointer.button_released(egui::PointerButton::Secondary));
        if secondary {
            *value = before;
        }
        let (rect, _) = ui.allocate_exact_size(vec2(44.0, 22.0), Sense::hover());
        ui.painter().rect(rect, 0.0, k.bg, Stroke::new(1.0_f32, k.border));
        ui.painter().text(rect.center(), Align2::CENTER_CENTER, format!("{:03}", *value), FontId::new(12.0, bold_family()), k.text);
        r
    })
    .inner
}

/// The crop marks in the corners of a window
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

/// The layout of the job windows: the buttons at the bottom under a rule, the content above
/// them. `scroll`: the content scrolls when the window is smaller than it (forms); without it
/// the content may fill the height (a log). Returns what `body` returned and the height the
/// window needs for all of it.
pub fn window<R>(ctx: &egui::Context, scroll: bool, footer: impl FnOnce(&mut egui::Ui), body: impl FnOnce(&mut egui::Ui) -> R) -> (R, f32) {
    reset_index(ctx);
    let fill = ctx.style().visuals.panel_fill;
    let foot = egui::TopBottomPanel::bottom("footer")
        .frame(egui::Frame::none().fill(fill).inner_margin(Margin { left: MARGIN, right: MARGIN, top: 14.0, bottom: 16.0 }))
        .show_separator_line(false)
        .show(ctx, |ui| {
            let r = ui.max_rect();
            ui.painter().line_segment([pos2(r.left(), r.top() - 14.0), pos2(r.right(), r.top() - 14.0)], Stroke::new(1.0_f32, c().border));
            footer(ui)
        });
    let frame = egui::Frame::none().fill(fill).inner_margin(Margin { left: MARGIN, right: MARGIN, top: BODY_TOP, bottom: 0.0 });
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
    crop_marks(ctx);
    (inner, BODY_TOP + content_h + 8.0 + foot.response.rect.height())
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
