//! The application: one window that goes through its stages.

use super::forms::{self, Form};
use super::running::Running;
use super::settings::{self, After, SettingsState};
use super::stats::Stats;
use super::widgets::{button_row, card, primary_button, title, window};
use super::{Env, Start};
use crate::effects;
use crate::engine::Request;
use crate::i18n::T;
use crate::settings::Settings;
use crate::syspal::{self, Palette};
use crate::theme::{self, c};
use eframe::egui::{self, vec2, RichText};

enum Stage {
    Settings(Box<SettingsState>),
    /// Text of a command (install / uninstall)
    Message {
        title: String,
        lines: Vec<String>,
        ok: bool,
    },
    /// ffmpeg or ffprobe cannot be found
    Missing,
    Form(Form),
    Running(Running),
    Stats(Stats),
}

pub struct App {
    env: Env,
    stage: Stage,
    /// The request the form makes right now (None = the form is not valid)
    valid: Option<Request>,
    // look
    transparent: bool,
    palette: Option<Palette>,
    applied: Option<Settings>,
    blur_state: Option<bool>,
}

impl App {
    pub fn new(env: Env, start: Start, request: Option<Request>, ctx: &egui::Context, transparent: bool) -> App {
        let stage = match start {
            Start::Settings => Stage::Settings(Box::new(SettingsState::new(env.cfg.settings.clone()))),
            Start::Message { title, lines, ok } => Stage::Message { title, lines, ok },
            Start::Normal if env.tools.is_none() => Stage::Missing,
            Start::Normal if env.is_stats() => Stage::Stats(Stats::spawn(&env, ctx)),
            Start::Normal => match request {
                Some(req) => Stage::Running(Running::start(&env, ctx, req)),
                None => Stage::Form(Form::new(&env)),
            },
        };
        App { env, stage, valid: None, transparent, palette: syspal::detect(), applied: None, blur_state: None }
    }

    /// Put the look of `settings` into egui (only when something changed)
    fn restyle(&mut self, ctx: &egui::Context, settings: &Settings) {
        if self.applied.as_ref() == Some(settings) {
            return;
        }
        let system_dark = ctx.input(|i| i.raw.system_theme).map(|t| t == egui::Theme::Dark);
        let colors = theme::compute(settings, system_dark, self.palette);
        let alpha = if self.transparent && settings.transparency != "off" { settings.opacity as f32 } else { 1.0 };
        theme::apply(ctx, colors, alpha);
        ctx.set_zoom_factor(settings.ui_scale.clamp(0.5, 3.0) as f32);
        self.applied = Some(settings.clone());
    }

    /// The window changes its content (form -> progress, settings -> result): one resize, at the user's action
    fn resize_to(ctx: &egui::Context, min_w: f32, h: f32) {
        let z = ctx.zoom_factor();
        let w = (ctx.screen_rect().width() * z).max(min_w * z);
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(vec2(w, h * z)));
    }
}

/// A text with a Close button; returns true when the window should close
fn notice(ctx: &egui::Context, t: &T, heading: &str, color: egui::Color32, body: impl FnOnce(&mut egui::Ui)) -> bool {
    let mut close = false;
    window(
        ctx,
        false,
        |ui| {
            button_row(ui, |ui| {
                close = ui.add(primary_button(t.close)).clicked();
            })
        },
        |ui| {
            title(ui, heading, color);
            ui.add_space(10.0);
            body(ui);
        },
    );
    close || ctx.input(|i| i.key_pressed(egui::Key::Escape) || i.key_pressed(egui::Key::Enter))
}

impl eframe::App for App {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        if self.transparent {
            [0.0, 0.0, 0.0, 0.0]
        } else {
            let b = c().bg;
            [b.r() as f32 / 255.0, b.g() as f32 / 255.0, b.b() as f32 / 255.0, 1.0]
        }
    }

    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        // look: the Settings window previews its changes live
        let look = match &self.stage {
            Stage::Settings(st) => st.s.clone(),
            _ => self.env.cfg.settings.clone(),
        };
        self.restyle(ctx, &look);
        // the blur behind a transparent window is always asked for where the system can do it
        let want_blur = self.transparent && look.transparency != "off" && effects::blur_supported();
        if effects::blur_supported() && self.blur_state != Some(want_blur) {
            if let Err(e) = effects::set_blur(frame, want_blur, c().dark) {
                eprintln!("mmt: blur not changed: {e}");
            }
            self.blur_state = Some(want_blur);
        }

        let k = c();
        let t = self.env.t;
        let mut close = false;
        let mut next: Option<(Stage, f32, f32)> = None;

        match &mut self.stage {
            Stage::Settings(st) => {
                if let Some(cmd) = settings::screen(ctx, st, t) {
                    match settings::run_command(st, cmd, t) {
                        After::Stay => {}
                        After::Close => close = true,
                        After::Report { title, lines, ok } => next = Some((Stage::Message { title, lines, ok }, 640.0, 300.0)),
                    }
                }
            }
            Stage::Form(form) => {
                let shown = forms::show(ctx, &self.env, form, self.valid.is_some());
                if shown.request.is_some() != self.valid.is_some() {
                    ctx.request_repaint();
                }
                self.valid = shown.request;
                let enter = ctx.input(|i| i.key_pressed(egui::Key::Enter));
                if shown.start || enter {
                    if let Some(req) = self.valid.clone() {
                        next = Some((Stage::Running(Running::start(&self.env, ctx, req)), 560.0, 380.0));
                    }
                }
                close = shown.cancel || (ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !ctx.memory(|m| m.any_popup_open()));
            }
            Stage::Running(run) => close = run.ui(ctx, &self.env),
            Stage::Stats(stats) => close = stats.ui(ctx, &self.env),
            Stage::Message { title, lines, ok } => {
                let color = if *ok { k.ok } else { k.err };
                close = notice(ctx, t, title, color, |ui| {
                    let h = (ui.available_height() - 22.0).max(60.0);
                    card(ui, |ui| {
                        egui::ScrollArea::vertical().max_height(h).auto_shrink([false, false]).show(ui, |ui| {
                            for l in lines.iter() {
                                ui.add(egui::Label::new(RichText::new(l.as_str()).monospace().size(12.0)).wrap());
                            }
                        });
                    });
                });
            }
            Stage::Missing => {
                close = notice(ctx, t, &self.env.title, k.text, |ui| {
                    card(ui, |ui| ui.label(RichText::new(t.ffmpeg_missing).color(k.err)));
                });
            }
        }

        if let Some((stage, w, h)) = next {
            self.stage = stage;
            Self::resize_to(ctx, w, h);
        }
        if close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if let Stage::Running(run) = &mut self.stage {
            run.stop();
        }
    }
}
