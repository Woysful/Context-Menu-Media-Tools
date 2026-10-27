//! The resolution field of Compress Video.

use crate::i18n::T;
use crate::media::Info;
use crate::scale::{self, ScaleMode};
use crate::theme::c;
use eframe::egui::{self, vec2, Align2, FontId, RichText, Sense};

use super::widgets::{chip, hint, short};

#[derive(Clone)]
pub struct ScaleField {
    pub mode: ScaleMode,
    custom: String,
    pub invalid: bool,
}

impl ScaleField {
    pub fn new() -> Self {
        ScaleField { mode: ScaleMode::Orig, custom: String::new(), invalid: false }
    }

    fn pick(&mut self, mode: ScaleMode) {
        self.mode = mode;
        self.custom.clear();
        self.invalid = false;
    }

    fn typed(&mut self) {
        if self.custom.trim().is_empty() {
            self.mode = ScaleMode::Orig;
            self.invalid = false;
        } else if let Some(m) = scale::parse_typed(&self.custom) {
            self.mode = m;
            self.invalid = false;
        } else {
            self.invalid = true;
        }
    }

    /// Three groups: original (+ auto) and an own value; sizes by the shorter side; scale in percent.
    /// A group that does not fit into a line continues in the next one, under its own chips.
    pub fn ui(&mut self, ui: &mut egui::Ui, t: &T, with_auto: bool, infos: &[Info]) {
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
                self.typed();
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
