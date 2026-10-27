//! The building blocks every window is made of.

use crate::theme::{bold, c, CONTROL_H};
use eframe::egui::{self, pos2, vec2, Align, Color32, FontId, Layout, Margin, RichText, Stroke, Vec2};

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

/// Tabs with an underline under the selected one
pub fn tabs(ui: &mut egui::Ui, labels: &[&str], current: &mut usize) {
    let k = c();
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 22.0;
        for (i, label) in labels.iter().enumerate() {
            let selected = *current == i;
            let text = if selected { bold(*label, 13.5).color(k.text) } else { RichText::new(*label).color(k.muted) };
            let resp = ui.add(egui::Button::new(text).frame(false));
            if selected {
                let r = resp.rect;
                ui.painter().line_segment([pos2(r.left(), r.bottom() + 3.0), pos2(r.right(), r.bottom() + 3.0)], Stroke::new(2.0_f32, k.accent));
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

/// A small flat button; the selected one is filled with the accent colour
pub fn chip(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let k = c();
    let text = if selected { bold(label, 12.5).color(k.on_accent) } else { RichText::new(label).size(12.5) };
    let mut b = egui::Button::new(text);
    if selected {
        b = b.fill(k.accent).stroke(Stroke::NONE);
    }
    ui.add(b).clicked()
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
