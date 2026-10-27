//! The resolution field of Compress Video: a drop-down list of the usual sizes and a field
//! for an own value, with a line that tells what the size becomes.

use crate::i18n::T;
use crate::media::Info;
use crate::scale::{self, ScaleMode};
use crate::theme::{c, CONTROL_H};
use eframe::egui::{self, vec2, Align, Layout};

use super::dropdown::{dropdown, Col};
use super::widgets::{line_edit, muted, one_line};

const OWN_W: f32 = 118.0;
const HEIGHTS: [u32; 7] = [2160, 1440, 1080, 720, 540, 480, 360];
const PERCENTS: [u32; 4] = [75, 50, 33, 25];

#[derive(Clone)]
pub struct ScaleField {
    pub mode: ScaleMode,
    custom: String,
    pub invalid: bool,
}

/// The key of a mode in the list: the codes of `--scale`
fn key_of(mode: ScaleMode) -> String {
    match mode {
        ScaleMode::Orig => "orig".into(),
        ScaleMode::Auto => "auto".into(),
        ScaleMode::Height(h) => format!("h{h}"),
        ScaleMode::Factor(f) => format!("s{f}"),
    }
}

fn label_of(mode: ScaleMode, t: &T) -> String {
    match mode {
        ScaleMode::Orig => t.res_original.into(),
        ScaleMode::Auto => t.res_auto.into(),
        ScaleMode::Height(h) => format!("{h}p"),
        ScaleMode::Factor(f) => format!("{}%", (f * 100.0).round()),
    }
}

impl ScaleField {
    pub fn new() -> Self {
        ScaleField { mode: ScaleMode::Orig, custom: String::new(), invalid: false }
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

    fn columns(t: &T, with_auto: bool) -> Vec<Col> {
        let item = |m: ScaleMode| (key_of(m), label_of(m, t));
        let mut main = vec![item(ScaleMode::Orig)];
        if with_auto {
            main.push(item(ScaleMode::Auto));
        }
        let heights: Vec<(String, String)> = HEIGHTS.iter().map(|h| item(ScaleMode::Height(*h))).collect();
        let (tall, short) = heights.split_at(4);
        vec![
            Col { title: Some(t.res_group_main.into()), items: main },
            Col { title: Some(t.res_group_size.into()), items: tall.to_vec() },
            // continues the column before it: an empty title keeps the rows level
            Col { title: Some(String::new()), items: short.to_vec() },
            Col { title: Some(t.res_group_scale.into()), items: PERCENTS.iter().map(|p| item(ScaleMode::Factor(*p as f64 / 100.0))).collect() },
        ]
    }

    /// What comes out of the choice, in one line
    fn note(&self, t: &T, infos: &[Info]) -> String {
        if self.mode == ScaleMode::Auto {
            return t.res_auto_text.into();
        }
        let Some(i) = infos.iter().find(|i| i.width > 0) else { return t.res_hint.into() };
        match (self.mode, scale::target_size(i.width, i.height, self.mode)) {
            (ScaleMode::Orig, _) => t.res_hint.into(),
            (_, Some((w, h))) => format!("{}×{}  →  {w}×{h}", i.width, i.height),
            (_, None) => format!("{}×{}  →  {}", i.width, i.height, t.res_unchanged),
        }
    }

    /// The list and the own value side by side, the note under them
    pub fn ui(&mut self, ui: &mut egui::Ui, id: &str, t: &T, with_auto: bool, infos: &[Info]) {
        ui.horizontal(|ui| {
            let list_w = (ui.available_width() - OWN_W - ui.spacing().item_spacing.x).max(120.0);
            ui.allocate_ui_with_layout(vec2(list_w, CONTROL_H), Layout::left_to_right(Align::Center), |ui| {
                let shown = if self.invalid { "—".to_string() } else { label_of(self.mode, t) };
                let mut key = key_of(self.mode);
                if dropdown(ui, id, &shown, &Self::columns(t, with_auto), &mut key) {
                    if let Some(m) = ScaleMode::from_code(&key) {
                        self.mode = m;
                        self.custom.clear();
                        self.invalid = false;
                    }
                }
            });
            if ui.add(line_edit(&mut self.custom, OWN_W, !self.invalid).hint_text(t.res_custom)).changed() {
                self.typed();
            }
        });
        if self.invalid {
            one_line(ui, t.res_invalid, c().err);
        } else {
            one_line(ui, &self.note(t, infos), muted());
        }
    }
}
