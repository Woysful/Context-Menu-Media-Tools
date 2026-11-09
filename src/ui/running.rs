//! The progress of a job, then its result.

use super::paint;
use super::widgets::{button_row, caption, card, fit_name, primary_button, secondary_button, spaced, tagged_card, text_width, title, window};
use super::Env;
use crate::engine::{self, Job, Line, LineKind, Progress, Request, State};
use crate::media;
use crate::theme::{bold, bold_family, c, display_family, tech_family, Look};
use crate::units::fmt_duration;
use eframe::egui::{self, pos2, text::LayoutJob, vec2, Align2, FontId, Rect, RichText, Sense, Stroke, TextFormat};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub struct Running {
    prog: Arc<Progress>,
    worker: Option<JoinHandle<()>>,
    close_at: Option<Instant>,
    keep_open: bool,
    /// the height the window was last given: as high as its content, or `LOG_H` once the
    /// messages do not fit (they scroll then)
    sized: Option<Size>,
}

#[derive(Clone, Copy, PartialEq)]
enum Size {
    Fit(u32),
    Log,
}

/// The height of the window once there are messages
const LOG_H: f32 = 380.0;

impl Running {
    pub fn start(env: &Env, ctx: &egui::Context, req: Request, delete_original: bool) -> Running {
        let tools = env.tools.clone().expect("the window checks for ffmpeg before a job starts");
        let repaint = ctx.clone();
        let prog = Progress::new(Box::new(move || repaint.request_repaint()), false);
        let job = Job { cfg: env.cfg.clone(), ffmpeg: tools.ffmpeg, ffprobe: tools.ffprobe, prog: prog.clone(), delete_original };
        let files = env.files.clone();
        let worker = std::thread::spawn(move || engine::run(&job, &req, &files));
        Running { prog, worker: Some(worker), close_at: None, keep_open: false, sized: None }
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
        let mut overflow = false;
        let (_, needed) = window(
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
                    overflow = log(ui, &s.lines);
                }
            },
        );
        // No empty space under the content. The log is shown whole while it fits; once it does
        // not, the window gets taller and stays so until the job ends.
        let fit = Size::Fit(needed.ceil() as u32 + 2);
        let want = match (overflow, self.sized) {
            (true, _) => Size::Log,
            (false, Some(Size::Log)) if !s.finished => Size::Log,
            _ => fit,
        };
        if self.sized != Some(want) {
            let h = match want {
                Size::Fit(h) => h as f32,
                Size::Log => LOG_H,
            };
            let z = ctx.zoom_factor();
            let w = ctx.screen_rect().width().max(560.0);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w * z, h * z)));
            self.sized = Some(want);
        }
        close || (s.finished && ctx.input(|i| i.key_pressed(egui::Key::Escape)))
    }
}

/// The bar of the progress: separate blocks like the level meter of a device (Brutalist) or a
/// row of squares in a frame with cut corners (Y2K)
fn blocks(ui: &mut egui::Ui, done: f32, height: f32, sweep: Option<f32>) {
    let k = c();
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());
    let p = ui.painter();
    if k.look == Look::Y2k {
        paint::cut_rect(p, rect, 5.0, paint::DIAGONAL, k.surface, Stroke::new(1.0_f32, k.border));
        paint::squares(p, rect.shrink2(vec2(8.0, 5.0)), done, k.accent, crate::theme::mix(k.surface, k.text, 0.3), sweep);
        return;
    }
    p.rect_stroke(rect, 0.0, Stroke::new(1.0_f32, k.border));
    let inner = rect.shrink(3.0);
    const W: f32 = 6.0;
    const GAP: f32 = 2.0;
    let n = ((inner.width() + GAP) / (W + GAP)).floor().max(1.0) as usize;
    let w = (inner.width() + GAP) / n as f32 - GAP;
    let lit = (done.clamp(0.0, 1.0) * n as f32).round() as usize;
    for i in 0..n {
        let r = Rect::from_min_size(pos2(inner.left() + i as f32 * (w + GAP), inner.top()), vec2(w, inner.height()));
        let color = if i < lit {
            k.accent
        } else if sweep.is_some_and(|phase| paint::swept(i, n, lit, phase)) {
            k.accent.gamma_multiply(0.55)
        } else {
            k.rule
        };
        p.rect_filled(r, 0.0, color);
    }
}

/// A step that tells nothing of its progress: the phase of the part that moves over the bar
fn sweep(ui: &egui::Ui, s: &State) -> Option<f32> {
    s.busy.then(|| {
        ui.ctx().request_repaint();
        (ui.input(|i| i.time) * 0.6).fract() as f32
    })
}

/// The thin bar of Classic; while busy, a part of the bar moves over the rest of it
fn thin_bar(ui: &mut egui::Ui, done: f32, sweep: Option<f32>) {
    let k = c();
    let Some(phase) = sweep else {
        ui.add(egui::ProgressBar::new(done).desired_height(6.0).rounding(3.0).fill(k.accent));
        return;
    };
    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 6.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 3.0, ui.visuals().extreme_bg_color);
    let start = rect.left() + rect.width() * done.clamp(0.0, 1.0);
    if start > rect.left() {
        p.rect_filled(Rect::from_x_y_ranges(rect.left()..=start, rect.y_range()), 3.0, k.accent);
    }
    let free = rect.right() - start;
    let w = free * 0.25;
    let x = start + (free + w) * phase - w;
    let (a, b) = (x.max(start), (x + w).min(rect.right()));
    if b > a {
        p.rect_filled(Rect::from_x_y_ranges(a..=b, rect.y_range()), 3.0, k.accent.gamma_multiply(0.55));
    }
}

/// A readout: the name in small capitals over the value
fn readout(ui: &mut egui::Ui, name: &str, value: &str) {
    let k = c();
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 1.0;
        if k.look == Look::Y2k {
            ui.label(spaced(&format!("[{}]", name.to_uppercase()), FontId::new(10.0, tech_family()), k.muted, 1.0));
            ui.label(RichText::new(value).font(FontId::new(14.0, tech_family())).color(k.text));
        } else {
            ui.label(spaced(&name.to_uppercase(), FontId::proportional(9.5), k.muted, 1.0));
            ui.label(bold(value, 12.5).color(k.text));
        }
    });
}

/// The file being processed, its progress and the numbers about it (Classic: a thin bar and
/// one line of numbers)
fn classic_progress_card(ui: &mut egui::Ui, env: &Env, s: &State) {
    let t = env.t;
    let k = c();
    card(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        if s.file_count > 1 {
            caption(ui, &t.file_of.replacen("{}", &(s.file_index + 1).to_string(), 1).replacen("{}", &s.file_count.to_string(), 1));
        }
        if !s.file_name.is_empty() {
            ui.label(bold(fit_name(ui, &s.file_name, FontId::new(13.0, bold_family())), 13.0));
            if let Some(info) = env.infos.iter().find(|i| i.name == s.file_name) {
                ui.label(RichText::new(media::describe(info, t.audio_word)).small().color(k.muted));
            }
        }
        ui.add_space(6.0);
        let overall = s.overall();
        thin_bar(ui, overall as f32, sweep(ui, s));
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

/// The file being processed, its progress and the numbers about it: a big percentage, the bar
/// and readouts
fn progress_card(ui: &mut egui::Ui, env: &Env, s: &State) {
    let t = env.t;
    let k = c();
    if k.look == Look::Classic {
        return classic_progress_card(ui, env, s);
    }
    let note = if s.file_count > 1 {
        t.file_of.replacen("{}", &(s.file_index + 1).to_string(), 1).replacen("{}", &s.file_count.to_string(), 1)
    } else {
        String::new()
    };
    tagged_card(ui, "process", &note, |ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        if !s.file_name.is_empty() {
            ui.label(bold(fit_name(ui, &s.file_name, FontId::new(13.0, bold_family())), 13.0));
            if let Some(info) = env.infos.iter().find(|i| i.name == s.file_name) {
                let about = media::describe(info, t.audio_word);
                let about = if k.look == Look::Brutal { about.replace(" · ", " / ") } else { about };
                ui.label(RichText::new(about).small().color(k.muted));
            }
        }
        ui.add_space(8.0);
        let overall = s.overall();
        ui.horizontal(|ui| {
            let y2k = k.look == Look::Y2k;
            let font = if y2k { FontId::new(22.0, display_family()) } else { FontId::new(28.0, bold_family()) };
            let w = if y2k { text_width(ui.ctx(), "100%", font.clone()) + 14.0 } else { 78.0 };
            let (r, _) = ui.allocate_exact_size(vec2(w, 30.0), Sense::hover());
            ui.painter().text(r.left_center(), Align2::LEFT_CENTER, format!("{:.0}%", overall * 100.0), font, k.text);
            ui.vertical(|ui| {
                ui.add_space(if y2k { 2.0 } else { 6.0 });
                blocks(ui, overall as f32, 18.0, sweep(ui, s));
                if y2k {
                    // the dots count up while the job runs
                    let dots = (ui.input(|i| i.time) * 2.5) as usize % 4;
                    ui.ctx().request_repaint_after(Duration::from_millis(400));
                    ui.add_space(2.0);
                    ui.label(spaced(&format!("NOW LOADING{}", ".".repeat(dots)), FontId::new(9.5, tech_family()), k.muted, 1.4));
                }
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 26.0;
            if !s.step.is_empty() {
                readout(ui, "step", &s.step);
            }
            if let Some(speed) = s.speed {
                readout(ui, t.speed, &format!("{speed:.2}×"));
            }
            if let Some(left) = s.remaining() {
                readout(ui, t.remaining, &fmt_duration(left));
            }
        });
    });
}

/// The messages of the job: as high as they are, at most the rest of the window. Returns
/// true when they do not fit.
fn log(ui: &mut egui::Ui, lines: &[Line]) -> bool {
    let k = c();
    // the card adds its margins and border to the height of the list
    let look = k.look;
    let h = (ui.available_height() - if look == Look::Classic { 22.0 } else { 40.0 }).max(20.0);
    tagged_card(ui, "log", &format!("{:03}", lines.len()), |ui| {
        let out = egui::ScrollArea::vertical().max_height(h).auto_shrink([false, true]).stick_to_bottom(true).show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 5.0;
            for l in lines {
                if look == Look::Classic {
                    let (prefix, col) = match l.kind {
                        LineKind::Info => ("", k.muted),
                        LineKind::Warn => ("⚠ ", k.warn),
                        LineKind::Error => ("✖ ", k.err),
                        LineKind::Result => ("✔ ", k.ok),
                    };
                    ui.label(RichText::new(format!("{prefix}{}", l.text)).size(12.0).color(col));
                    continue;
                }
                let (tag, col, body) = match l.kind {
                    LineKind::Info => ("[ -- ]", k.muted, k.muted),
                    LineKind::Warn => ("[WARN]", k.warn, k.warn),
                    LineKind::Error => ("[FAIL]", k.err, k.err),
                    LineKind::Result => ("[ OK ]", k.ok, k.text),
                };
                let mut job = LayoutJob::default();
                let tag_font = if look == Look::Y2k { FontId::new(12.0, tech_family()) } else { FontId::new(11.5, bold_family()) };
                job.append(tag, 0.0, TextFormat { font_id: tag_font, color: col, ..Default::default() });
                job.append(&l.text, 8.0, TextFormat { font_id: FontId::monospace(11.5), color: body, ..Default::default() });
                ui.add(egui::Label::new(job).wrap());
            }
        });
        out.content_size.y > out.inner_rect.height() + 2.0
    })
}
