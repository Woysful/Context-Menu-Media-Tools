//! The progress of a job, then its result.

use super::widgets::{button_row, caption, card, chars_fitting, primary_button, secondary_button, short, title, window};
use super::Env;
use crate::engine::{self, Job, Line, LineKind, Progress, Request, State};
use crate::media;
use crate::theme::{bold, c};
use crate::units::fmt_duration;
use eframe::egui::{self, RichText};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct Running {
    prog: Arc<Progress>,
    worker: Option<JoinHandle<()>>,
    close_at: Option<Instant>,
    keep_open: bool,
}

impl Running {
    pub fn start(env: &Env, ctx: &egui::Context, req: Request) -> Running {
        let tools = env.tools.clone().expect("the window checks for ffmpeg before a job starts");
        let repaint = ctx.clone();
        let prog = Progress::new(Box::new(move || repaint.request_repaint()), false);
        let job = Job { cfg: env.cfg.clone(), ffmpeg: tools.ffmpeg, ffprobe: tools.ffprobe, prog: prog.clone() };
        let files = env.files.clone();
        let worker = std::thread::spawn(move || engine::run(&job, &req, &files));
        Running { prog, worker: Some(worker), close_at: None, keep_open: false }
    }

    /// Cancel the job and wait until it has stopped (the window is closing)
    pub fn stop(&mut self) {
        self.prog.request_cancel();
        if let Some(h) = self.worker.take() {
            let _ = h.join();
        }
    }

    /// Returns true when the window should close
    pub fn ui(&mut self, ctx: &egui::Context, env: &Env) -> bool {
        let t = env.t;
        let k = c();
        let s = self.prog.snapshot();
        let (heading, color) = if !s.finished {
            (env.title.as_str(), k.text)
        } else if s.cancelled {
            (t.cancelled, k.text)
        } else if s.errors > 0 {
            (t.finished_with_errors, k.err)
        } else {
            (t.all_done, k.ok)
        };

        // a clean run closes itself, the rest stays for the user to read
        let auto = env.cfg.settings.auto_close;
        let left = (s.finished && s.clean() && auto > 0.0 && !self.keep_open).then(|| {
            let deadline = *self.close_at.get_or_insert_with(|| Instant::now() + Duration::from_secs_f64(auto));
            deadline.saturating_duration_since(Instant::now())
        });
        let mut close = left.is_some_and(|l| l.is_zero());
        if left.is_some() {
            ctx.request_repaint_after(Duration::from_millis(200));
        }

        let prog = &self.prog;
        let keep_open = &mut self.keep_open;
        window(
            ctx,
            false,
            |ui| {
                button_row(ui, |ui| {
                    if !s.finished {
                        if ui.add(secondary_button(t.cancel)).clicked() {
                            prog.request_cancel();
                        }
                        return;
                    }
                    if ui.add(primary_button(t.close)).clicked() {
                        close = true;
                    }
                    if let Some(l) = left {
                        if ui.add(secondary_button(t.keep_open)).clicked() {
                            *keep_open = true;
                        }
                        ui.label(RichText::new(t.closing_in.replace("{}", &(l.as_secs() + 1).to_string())).small().color(k.muted));
                    }
                });
            },
            |ui| {
                title(ui, heading, color);
                ui.add_space(10.0);
                if !s.finished {
                    progress_card(ui, env, &s);
                    ui.add_space(10.0);
                }
                if !s.lines.is_empty() {
                    log(ui, &s.lines);
                }
            },
        );
        close || (s.finished && ctx.input(|i| i.key_pressed(egui::Key::Escape)))
    }
}

/// The file being processed, its progress and the numbers about it
fn progress_card(ui: &mut egui::Ui, env: &Env, s: &State) {
    let t = env.t;
    let k = c();
    card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        if s.file_count > 1 {
            caption(ui, &t.file_of.replacen("{}", &(s.file_index + 1).to_string(), 1).replacen("{}", &s.file_count.to_string(), 1));
        }
        if !s.file_name.is_empty() {
            ui.label(bold(short(&s.file_name, chars_fitting(ui)), 13.0));
            if let Some(info) = env.infos.iter().find(|i| i.name == s.file_name) {
                ui.label(RichText::new(media::describe(info, t.audio_word)).small().color(k.muted));
            }
        }
        ui.add_space(6.0);
        let overall = s.overall();
        ui.add(egui::ProgressBar::new(overall as f32).desired_height(6.0).rounding(3.0).fill(k.accent));
        let mut parts = vec![format!("{:.0}%", overall * 100.0)];
        if !s.step.is_empty() {
            parts.push(s.step.clone());
        }
        if let Some(speed) = s.speed {
            parts.push(format!("{} {speed:.2}×", t.speed));
        }
        if let Some(left) = s.remaining() {
            parts.push(format!("{} {}", fmt_duration(left), t.remaining));
        }
        ui.label(RichText::new(parts.join("  ·  ")).small().color(k.muted));
    });
}

/// The messages of the job; fills the rest of the window
fn log(ui: &mut egui::Ui, lines: &[Line]) {
    let k = c();
    // the card adds its margins and border to the height of the list
    let h = (ui.available_height() - 22.0).max(60.0);
    card(ui, |ui| {
        egui::ScrollArea::vertical().max_height(h).auto_shrink([false, false]).stick_to_bottom(true).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            for l in lines {
                let (prefix, col) = match l.kind {
                    LineKind::Info => ("", k.muted),
                    LineKind::Warn => ("⚠ ", k.warn),
                    LineKind::Error => ("✖ ", k.err),
                    LineKind::Result => ("✔ ", k.ok),
                };
                ui.label(RichText::new(format!("{prefix}{}", l.text)).size(12.0).color(col));
            }
        });
    });
}
