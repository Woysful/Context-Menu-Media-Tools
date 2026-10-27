//! The Stats window: what ffprobe says about each file.

use super::widgets::{button_row, card, primary_button, secondary_button, title, window};
use super::Env;
use crate::engine;
use crate::theme::{bold, c};
use eframe::egui::{self, RichText};
use std::sync::{Arc, Mutex};

type Texts = Vec<(String, String)>;

pub struct Stats {
    /// (file name, text), once ffprobe has answered
    slot: Arc<Mutex<Option<Texts>>>,
}

impl Stats {
    pub fn spawn(env: &Env, ctx: &egui::Context) -> Stats {
        let slot = Arc::new(Mutex::new(None));
        let tools = env.tools.clone().expect("the window checks for ffprobe first");
        let (out, files, ctx) = (slot.clone(), env.files.clone(), ctx.clone());
        std::thread::spawn(move || {
            let texts: Texts = files.iter().map(|f| (super::widgets::file_name(f), engine::stats_text(&tools.ffprobe, f))).collect();
            *out.lock().unwrap_or_else(|e| e.into_inner()) = Some(texts);
            ctx.request_repaint();
        });
        Stats { slot }
    }

    /// Returns true when the window should close
    pub fn ui(&self, ctx: &egui::Context, env: &Env) -> bool {
        let t = env.t;
        let data = self.slot.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let mut close = false;
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
                            let all: Vec<String> = list.iter().map(|(n, s)| format!("== {n}\n{s}\n")).collect();
                            ui.output_mut(|o| o.copied_text = all.join("\n"));
                        }
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
                    for (i, (name, text)) in list.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(10.0);
                        }
                        card(ui, |ui| {
                            ui.label(bold(name.as_str(), 13.0));
                            ui.add_space(4.0);
                            ui.add(egui::Label::new(RichText::new(text).monospace().size(12.0)).wrap());
                        });
                    }
                });
            },
        );
        close || ctx.input(|i| i.key_pressed(egui::Key::Escape))
    }
}
