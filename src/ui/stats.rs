//! The Stats window: what is in each file, from ffprobe. The overview is made of the numbers
//! that matter and one section per stream; the full text of ffprobe is one click away.

use super::widgets::{button_row, caption, card, file_name, muted, primary_button, secondary_button, title, window};
use super::Env;
use crate::engine;
use crate::media;
use crate::report::{self, Report, Section};
use crate::theme::{bold, c};
use eframe::egui::{self, vec2, RichText};
use std::sync::{Arc, Mutex};

struct FileStats {
    name: String,
    /// None = ffprobe could not read it: the text says why
    report: Option<Report>,
    text: String,
}

pub struct Stats {
    /// filled once ffprobe has answered for every file
    slot: Arc<Mutex<Option<Arc<Vec<FileStats>>>>>,
    raw: bool,
}

const KEY_W: f32 = 96.0;

impl Stats {
    pub fn spawn(env: &Env, ctx: &egui::Context) -> Stats {
        let slot = Arc::new(Mutex::new(None));
        let tools = env.tools.clone().expect("the window checks for ffprobe first");
        let (out, files, ctx, t) = (slot.clone(), env.files.clone(), ctx.clone(), env.t);
        std::thread::spawn(move || {
            let list: Vec<FileStats> = files
                .iter()
                .map(|f| FileStats {
                    name: file_name(f),
                    report: report::build(&media::probe_json(&tools.ffprobe, f), t),
                    text: engine::stats_text(&tools.ffprobe, f),
                })
                .collect();
            *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(list));
            ctx.request_repaint();
        });
        Stats { slot, raw: false }
    }

    /// Returns true when the window should close
    pub fn ui(&mut self, ctx: &egui::Context, env: &Env) -> bool {
        let t = env.t;
        let data = self.slot.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let mut close = false;
        let raw = self.raw;
        let mut toggle = false;
        window(
            ctx,
            false,
            |ui| {
                button_row(ui, |ui| {
                    if ui.add(primary_button(t.close)).clicked() {
                        close = true;
                    }
                    if let Some(list) = &data {
                        if ui.add(secondary_button(t.copy_all)).clicked() {
                            let all: Vec<String> = list.iter().map(|f| format!("== {}\n{}\n", f.name, f.text)).collect();
                            ui.output_mut(|o| o.copied_text = all.join("\n"));
                        }
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            toggle = ui.add(secondary_button(if raw { t.st_overview } else { t.st_raw })).clicked();
                        });
                    }
                });
            },
            |ui| {
                title(ui, &env.title, c().text);
                ui.add_space(10.0);
                let Some(list) = &data else {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(t.stats_loading);
                    });
                    return;
                };
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for (i, f) in list.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(10.0);
                        }
                        card(ui, |ui| {
                            ui.label(bold(f.name.as_str(), 14.0));
                            ui.add_space(6.0);
                            match (&f.report, raw) {
                                (Some(r), false) => overview(ui, i, r),
                                _ => {
                                    ui.add(egui::Label::new(RichText::new(&f.text).monospace().size(12.0)).wrap());
                                }
                            }
                        });
                    }
                });
            },
        );
        if toggle {
            self.raw = !self.raw;
        }
        close || ctx.input(|i| i.key_pressed(egui::Key::Escape))
    }
}

/// The numbers in a row, then the sections two side by side (the tags across the whole width)
fn overview(ui: &mut egui::Ui, file: usize, r: &Report) {
    if !r.metrics.is_empty() {
        ui.columns(r.metrics.len(), |cols| {
            for (col, (label, value)) in cols.iter_mut().zip(&r.metrics) {
                col.spacing_mut().item_spacing.y = 2.0;
                caption(col, label);
                col.add(egui::Label::new(bold(value.as_str(), 15.0).color(c().text)).truncate());
            }
        });
        ui.add_space(8.0);
    }
    let (wide, pairs): (Vec<&Section>, Vec<&Section>) = r.sections.iter().partition(|s| s.rows.len() > 8);
    for (n, pair) in pairs.chunks(2).enumerate() {
        ui.separator();
        ui.columns(2, |cols| {
            for (k, s) in pair.iter().enumerate() {
                section(&mut cols[k], (file, n, k), s);
            }
        });
    }
    for (n, s) in wide.iter().enumerate() {
        ui.separator();
        section(ui, (file, 1000 + n, 0), s);
    }
}

fn section(ui: &mut egui::Ui, id: (usize, usize, usize), s: &Section) {
    ui.label(RichText::new(s.title.to_uppercase()).size(11.0).color(c().accent).strong());
    ui.add_space(2.0);
    // rows as high as the text, not as the buttons
    ui.spacing_mut().interact_size.y = 16.0;
    egui::Grid::new(("stats", id)).num_columns(2).min_col_width(KEY_W).spacing(vec2(10.0, 5.0)).show(ui, |ui| {
        for (key, value) in &s.rows {
            ui.label(RichText::new(key).color(muted()));
            ui.add(egui::Label::new(RichText::new(value).color(c().text)).wrap());
            ui.end_row();
        }
    });
    ui.add_space(4.0);
}
