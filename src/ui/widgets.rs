//! The building blocks every window is made of.

use crate::theme::{bold, bold_family, c, CONTROL_H};
use eframe::egui::{self, pos2, vec2, Align, Align2, Color32, FontId, Layout, Margin, Rect, RichText, Sense, Stroke, Vec2};

/// The space between the edges of a window and its content
pub const MARGIN: f32 = 20.0;
const BODY_TOP: f32 = 18.0;

pub fn muted() -> Color32 {
    c().muted
}

pub fn text_width(ctx: &egui::Context, text: &str, font: FontId) -> f32 {
    ctx.fonts(|f| f.layout_no_wrap(text.to_string(), font, Color32::WHITE).size().x)
}

/// How many characters of a 13 px name fit into the width of `ui` (the font is monospaced)
pub fn chars_fitting(ui: &egui::Ui) -> usize {
    ((ui.available_width() / 7.8) as usize).clamp(16, 160)
}

pub fn file_name(path: &std::path::Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// A long name with its middle cut out, so the extension stays visible
pub fn short(name: &str, max: usize) -> String {
    let chars: Vec<char> = name.chars().collect();
    if chars.len() <= max {
        name.to_string()
    } else {
        let head: String = chars[..max.saturating_sub(9)].iter().collect();
        let tail: String = chars[chars.len() - 6..].iter().collect();
        format!("{head}…{tail}")
    }
}

/// The title of a window
pub fn title(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.label(bold(text, 18.0).color(color));
}

/// A small upper-case caption
pub fn caption(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text.to_uppercase()).size(11.0).color(muted()));
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

/// A framed block on the surface colour (the file, the log, the text of a command)
pub fn card<R>(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let k = c();
    egui::Frame::none()
        .fill(k.surface)
        .stroke(Stroke::new(1.0_f32, k.border))
        .rounding(4.0)
        .inner_margin(Margin::symmetric(12.0, 10.0))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A row of right-aligned buttons with a fixed height (with_layout would take all the
/// remaining height of the panel)
pub fn button_row(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let size = Vec2::new(ui.available_width(), CONTROL_H + 2.0);
    ui.allocate_ui_with_layout(size, Layout::right_to_left(Align::Center), add);
}

pub fn primary_button(text: &str) -> egui::Button<'static> {
    let k = c();
    egui::Button::new(bold(text, 13.5).color(k.on_accent)).fill(k.accent).stroke(Stroke::NONE).min_size(vec2(110.0, CONTROL_H))
}

pub fn secondary_button(text: &str) -> egui::Button<'static> {
    egui::Button::new(text.to_string()).min_size(vec2(96.0, CONTROL_H))
}

/// One of several values as one control split into equal parts (the window width or the
/// width of a form row); returns true when another part was chosen
pub fn segmented(ui: &mut egui::Ui, labels: &[&str], current: &mut usize) -> bool {
    let k = c();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), CONTROL_H), Sense::hover());
    let painter = ui.painter().clone();
    painter.rect(rect, 4.0, k.surface, Stroke::new(1.0_f32, k.border));
    let seg_w = rect.width() / labels.len().max(1) as f32;
    let mut changed = false;
    for (i, label) in labels.iter().enumerate() {
        let r = Rect::from_min_size(pos2(rect.left() + seg_w * i as f32, rect.top()), vec2(seg_w, CONTROL_H));
        let part = ui.interact(r, resp.id.with(i), Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand);
        let selected = *current == i;
        if selected {
            painter.rect_filled(r.shrink(3.0), 3.0, k.accent);
        } else if part.hovered() {
            painter.rect_filled(r.shrink(3.0), 3.0, k.hover);
        }
        // a thin line between two parts that are both not selected
        if i > 0 && !selected && *current != i - 1 {
            painter.line_segment([pos2(r.left(), r.top() + 8.0), pos2(r.left(), r.bottom() - 8.0)], Stroke::new(1.0_f32, k.border));
        }
        let (font, color) = if selected { (FontId::new(13.0, bold_family()), k.on_accent) } else { (FontId::proportional(13.0), k.text) };
        painter.text(r.center(), Align2::CENTER_CENTER, *label, font, color);
        if part.clicked() && !selected {
            *current = i;
            changed = true;
        }
    }
    changed
}

/// The width of the names in front of the controls of a form
pub const LABEL_W: f32 = 104.0;

/// A row of a form: the name at the left, the control (and a line about it) at the right
pub fn form_row<R>(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let inner = ui
        .horizontal_top(|ui| {
            let (r, _) = ui.allocate_exact_size(vec2(LABEL_W, CONTROL_H), Sense::hover());
            ui.painter().text(r.left_center(), Align2::LEFT_CENTER, label, FontId::proportional(13.0), muted());
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                add(ui)
            })
            .inner
        })
        .inner;
    ui.add_space(8.0);
    inner
}

/// A line of small text that is cut at the edge instead of wrapping (keeps the height of a form fixed)
pub fn one_line(ui: &mut egui::Ui, text: &str, color: Color32) {
    ui.add(egui::Label::new(RichText::new(text).small().color(color)).truncate());
}

/// A one-line text field as high as the buttons and drop-down lists
pub fn line_edit<'a>(text: &'a mut String, width: f32, valid: bool) -> egui::TextEdit<'a> {
    let mut edit = egui::TextEdit::singleline(text).desired_width(width).min_size(vec2(width, CONTROL_H)).vertical_align(Align::Center);
    if !valid {
        edit = edit.text_color(c().err);
    }
    edit
}

/// One of several values, as a row of flat buttons: (value, label)
pub fn choice(ui: &mut egui::Ui, current: &mut String, options: &[(&str, &str)]) {
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

/// A text field for a number: left aligned like every other field, red when it is not valid
pub fn number_field(ui: &mut egui::Ui, text: &mut String, valid: bool, suffix: &str) {
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

/// A slider with its value written at the right
pub fn slider_row(ui: &mut egui::Ui, value: &mut u32, range: std::ops::RangeInclusive<u32>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().slider_width = (ui.available_width() - 48.0).max(80.0);
        ui.add(egui::Slider::new(value, range).show_value(false));
        ui.label(RichText::new(value.to_string()).monospace());
    });
}

/// The layout of the job windows: the buttons at the bottom, the content above them.
/// `scroll`: the content scrolls when the window is smaller than it (forms); without it the
/// content may fill the height (a log). Returns what `body` returned and the height the
/// window needs for all of it.
pub fn window<R>(ctx: &egui::Context, scroll: bool, footer: impl FnOnce(&mut egui::Ui), body: impl FnOnce(&mut egui::Ui) -> R) -> (R, f32) {
    let fill = ctx.style().visuals.panel_fill;
    let foot = egui::TopBottomPanel::bottom("footer")
        .frame(egui::Frame::none().fill(fill).inner_margin(Margin { left: MARGIN, right: MARGIN, top: 12.0, bottom: 16.0 }))
        .show_separator_line(false)
        .show(ctx, footer);
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
    (inner, BODY_TOP + content_h + foot.response.rect.height())
}
