//! The form of Make Anim: an animated WebP or GIF of a video. One video is shown at the right,
//! a frame of it chosen with a slider, with the editor of Compress Image.

use super::super::dropdown::{dropdown, Col};
use super::super::forms::{default_menu, original_check, parse_in, result_row_with, small_number};
use super::super::widgets::{caption, caption_width};
use super::frame_time;
use super::{form_row, muted, one_line, segmented, slider_row, Editor, Env, FIELDS_W, GAP, PREVIEW_MIN_W};
use crate::defaults::{self as dv, DITHERS, SLOWS};
use crate::engine::{Anim, AnimFormat, Request};
use crate::theme::CONTROL_H;
use crate::units::{parse_clock, parse_decimal};
use eframe::egui::{self, RichText};

pub struct AnimForm {
    /// 0 = WebP, 1 = GIF
    pub(in crate::ui) format: usize,
    size_text: String,
    fps_text: String,
    quality: u32,
    compression: u32,
    /// a place in DITHERS
    dither: usize,
    speed_text: String,
    /// a place in SLOWS
    slow: usize,
    /// the part of the video; an empty end: to the end
    from_text: String,
    to_text: String,
    /// the part the two fields were last filled with (the slider under the picture moves it too)
    trim_shown: (f64, f64),
    pub(in crate::ui) ed: Editor,
}

impl AnimForm {
    pub fn new(env: &Env) -> AnimForm {
        let cfg = &env.cfg;
        let d = |key: &str| dv::value(cfg, key);
        let length = env.infos.first().and_then(|i| i.duration).filter(|d| *d > 0.0);
        AnimForm {
            format: 0,
            size_text: d(dv::WEBP_SIZE),
            fps_text: d(dv::WEBP_FPS),
            quality: d(dv::WEBP_QUALITY).parse().unwrap_or(cfg.webp.quality),
            compression: d(dv::WEBP_COMPRESSION).parse().unwrap_or(cfg.webp.compression),
            dither: dither_index(&d(dv::GIF_DITHER)),
            speed_text: "1".into(),
            slow: SLOWS.iter().position(|s| *s == d(dv::ANIM_SLOW)).unwrap_or(0),
            from_text: frame_time(0.0),
            to_text: length.filter(|_| env.files.len() == 1).map(frame_time).unwrap_or_default(),
            trim_shown: (0.0, length.unwrap_or(0.0)),
            // without a known length: the first frame, as of a picture
            ed: Editor::new(length),
        }
    }

    pub fn ui(&mut self, ui: &mut egui::Ui, env: &Env, delete_original: &mut bool) -> Option<Request> {
        if !Editor::one_file(env) {
            return self.rows(ui, env, delete_original);
        }
        // the fields keep their width, a wider window widens the picture
        let avail = ui.available_width();
        let left = FIELDS_W.min(avail - GAP - PREVIEW_MIN_W).max(300.0);
        let right = (avail - left - GAP).max(PREVIEW_MIN_W);
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let fields = ui.vertical(|ui| {
                ui.set_width(left);
                ui.spacing_mut().item_spacing.x = 8.0;
                self.rows(ui, env, delete_original)
            });
            ui.add_space(GAP);
            ui.vertical(|ui| {
                ui.set_width(right);
                ui.spacing_mut().item_spacing.x = 8.0;
                self.ed.crop_panel(ui, env, fields.response.rect.height());
            });
            fields.inner
        })
        .inner
    }

    fn rows(&mut self, ui: &mut egui::Ui, env: &Env, delete_original: &mut bool) -> Option<Request> {
        let t = env.t;
        form_row(ui, t.anim_format, |ui| {
            segmented(ui, &["WebP", "GIF"], &mut self.format);
            one_line(ui, if self.format == 0 { t.anim_webp_hint } else { t.anim_gif_hint }, muted());
        });
        let size = parse_in(&self.size_text, 64, 4320);
        let fps = parse_in(&self.fps_text, 1, 120);
        // two small fields: the frame rate at the right of the size
        let (rs, rf) = form_row(ui, t.webp_size, |ui| {
            let r = ui
                .horizontal(|ui| {
                    let rs = small_number(ui, &mut self.size_text, size.is_some(), "px");
                    ui.add_space(10.0);
                    caption(ui, t.webp_fps);
                    let rf = small_number(ui, &mut self.fps_text, fps.is_some(), "fps").on_hover_text(t.webp_fps_hint);
                    (rs, rf)
                })
                .inner;
            one_line(ui, t.webp_size_hint, muted());
            r
        });
        if let Some(v) = default_menu(&rs, env, dv::WEBP_SIZE, size.map(|v| v.to_string()).as_deref()) {
            self.size_text = v;
        }
        if let Some(v) = default_menu(&rf, env, dv::WEBP_FPS, fps.map(|v| v.to_string()).as_deref()) {
            self.fps_text = v;
        }
        let speed = parse_decimal(&self.speed_text).filter(|v| (0.1..=10.0).contains(v));
        let slower = speed.is_some_and(|s| s < 1.0);
        // the way to slow down is chosen at the right of the speed, for a speed below 1
        let r = form_row(ui, t.anim_speed, |ui| {
            let r = ui
                .horizontal(|ui| {
                    small_number(ui, &mut self.speed_text, speed.is_some(), "×");
                    ui.add_space(4.0);
                    ui.add_enabled_ui(slower, |ui| segmented(ui, &t.anim_slows, &mut self.slow)).inner
                })
                .inner;
            one_line(ui, if slower { t.anim_slow_hints[self.slow.min(1)] } else { t.anim_speed_hint }, muted());
            r
        });
        if let Some(v) = default_menu(&r, env, dv::ANIM_SLOW, Some(SLOWS[self.slow.min(1)])) {
            self.slow = SLOWS.iter().position(|s| *s == v).unwrap_or(0);
        }
        let part = self.trim_rows(ui, env);
        if self.format == 0 {
            // the quality, and at its right the effort of the encoder as a short list
            let (rq, rp) = form_row(ui, t.webp_quality, |ui| {
                let r = ui
                    .horizontal(|ui| {
                        let list_w = 112.0;
                        let gap = ui.spacing().item_spacing.x;
                        let slider_w = ui.available_width() - list_w - caption_width(ui, t.webp_compression) - 3.0 * gap - 6.0;
                        let rq = ui.allocate_ui(egui::vec2(slider_w.max(100.0), CONTROL_H), |ui| slider_row(ui, &mut self.quality, 0..=100)).inner;
                        ui.add_space(6.0);
                        caption(ui, t.webp_compression);
                        let rp = ui
                            .allocate_ui(egui::vec2(list_w, CONTROL_H), |ui| {
                                let items = (0..=6u32).map(|n| (n.to_string(), t.anim_presets[n as usize].to_string())).collect();
                                let mut key = self.compression.min(6).to_string();
                                let shown = t.anim_presets[self.compression.min(6) as usize];
                                let r = dropdown(ui, "anim_preset", shown, &[Col { title: None, items }], &mut key);
                                self.compression = key.parse().unwrap_or(self.compression);
                                r
                            })
                            .inner
                            .on_hover_text(t.webp_compression_hint);
                        (rq, rp)
                    })
                    .inner;
                one_line(ui, t.webp_quality_hint, muted());
                r
            });
            if let Some(v) = default_menu(&rq, env, dv::WEBP_QUALITY, Some(&self.quality.to_string())) {
                self.quality = v.parse().unwrap_or(self.quality);
            }
            if let Some(v) = default_menu(&rp, env, dv::WEBP_COMPRESSION, Some(&self.compression.to_string())) {
                self.compression = v.parse().unwrap_or(self.compression);
            }
        } else {
            let r = form_row(ui, t.anim_dither, |ui| {
                let r = segmented(ui, &t.anim_dithers, &mut self.dither);
                one_line(ui, t.anim_dither_hints[self.dither.min(3)], muted());
                r
            });
            if let Some(v) = default_menu(&r, env, dv::GIF_DITHER, Some(DITHERS[self.dither.min(3)])) {
                self.dither = dither_index(&v);
            }
        }
        form_row(ui, t.img_turn, |ui| self.ed.turn_row(ui, t));
        if !Editor::one_file(env) {
            form_row(ui, t.img_crop, |ui| {
                ui.add_enabled_ui(false, |ui| segmented(ui, &super::ratio_labels(t), &mut 0));
                one_line(ui, t.img_crop_many, muted());
            });
        }
        form_row(ui, t.original, |ui| original_check(ui, env, delete_original, t.delete_after));
        let req = match (size, fps, speed, part) {
            (Some(size), Some(fps), Some(speed), Some((from, to))) => Some(Request::Anim(Box::new(Anim {
                format: if self.format == 0 { AnimFormat::Webp } else { AnimFormat::Gif },
                size,
                fps,
                quality: self.quality,
                compression: self.compression,
                dither: DITHERS[self.dither.min(3)].to_string(),
                speed,
                smooth: self.slow == 1,
                from,
                to,
                edit: self.ed.edit(env),
            }))),
            _ => None,
        };
        result_row_with(ui, env, req.as_ref(), None);
        req
    }
}

impl AnimForm {
    /// The part of the video: from, and to (None: to the end), when both fields are right.
    /// With one video the fields follow the slider under the picture, and it follows them.
    fn trim_rows(&mut self, ui: &mut egui::Ui, env: &Env) -> Option<(f64, Option<f64>)> {
        let t = env.t;
        let length = self.ed.length.filter(|_| Editor::one_file(env));
        if length.is_some() && self.ed.trim != self.trim_shown {
            self.from_text = frame_time(self.ed.trim.0);
            self.to_text = frame_time(self.ed.trim.1);
            self.trim_shown = self.ed.trim;
        }
        let parse = |from: &str, to: &str| {
            let from = parse_clock(from).filter(|f| length.is_none_or(|l| *f < l));
            let to = match to.trim() {
                "" => Some(None),
                text => parse_clock(text).filter(|v| length.is_none_or(|l| *v <= l + 0.05)).map(Some),
            };
            let ordered = matches!((from, to), (Some(f), Some(Some(e))) if e <= f);
            (from, to, ordered)
        };
        let (from, to, ordered) = parse(&self.from_text, &self.to_text);
        let changed = form_row(ui, t.anim_trim, |ui| {
            let changed = ui
                .horizontal(|ui| {
                    let a = small_number(ui, &mut self.from_text, from.is_some() && !ordered, "");
                    ui.label(RichText::new("–").color(muted()));
                    let b = small_number(ui, &mut self.to_text, to.is_some() && !ordered, "");
                    a.changed() || b.changed()
                })
                .inner;
            one_line(ui, if length.is_some() { t.anim_trim_one } else { t.anim_trim_many }, muted());
            changed
        });
        // what was typed just now
        let (from, to, ordered) = parse(&self.from_text, &self.to_text);
        let (from, to) = (from?, to?);
        if ordered {
            return None;
        }
        let Some(length) = length else { return Some((from, to)) };
        let end = to.unwrap_or(length).min(length);
        if changed {
            self.ed.trim = (from, end);
            self.trim_shown = self.ed.trim;
        }
        // the very end is no cut
        Some((from, Some(end).filter(|e| length - e > 0.05)))
    }
}

fn dither_index(key: &str) -> usize {
    DITHERS.iter().position(|d| *d == key).unwrap_or(0)
}
