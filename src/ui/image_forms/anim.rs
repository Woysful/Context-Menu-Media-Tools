//! The form of Make Anim: an animated WebP or GIF of a video. One video is shown at the right,
//! a frame of it chosen with a slider, with the editor of Compress Image.

use super::super::forms::{default_menu, original_check, parse_in, result_row_with, small_number};
use super::{form_row, muted, one_line, segmented, slider_row, Editor, Env, FIELDS_W, GAP, PREVIEW_MIN_W};
use crate::defaults::{self as dv, DITHERS};
use crate::engine::{Anim, AnimFormat, Request};
use eframe::egui;

pub struct AnimForm {
    /// 0 = WebP, 1 = GIF
    pub(in crate::ui) format: usize,
    size_text: String,
    fps_text: String,
    quality: u32,
    compression: u32,
    /// a place in DITHERS
    dither: usize,
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
        let r = form_row(ui, t.webp_size, |ui| {
            let r = small_number(ui, &mut self.size_text, size.is_some(), "px");
            one_line(ui, t.webp_size_hint, muted());
            r
        });
        if let Some(v) = default_menu(&r, env, dv::WEBP_SIZE, size.map(|v| v.to_string()).as_deref()) {
            self.size_text = v;
        }
        let r = form_row(ui, t.webp_fps, |ui| {
            let r = small_number(ui, &mut self.fps_text, fps.is_some(), "fps");
            one_line(ui, t.webp_fps_hint, muted());
            r
        });
        if let Some(v) = default_menu(&r, env, dv::WEBP_FPS, fps.map(|v| v.to_string()).as_deref()) {
            self.fps_text = v;
        }
        if self.format == 0 {
            let r = form_row(ui, t.webp_quality, |ui| {
                let r = slider_row(ui, &mut self.quality, 0..=100);
                one_line(ui, t.webp_quality_hint, muted());
                r
            });
            if let Some(v) = default_menu(&r, env, dv::WEBP_QUALITY, Some(&self.quality.to_string())) {
                self.quality = v.parse().unwrap_or(self.quality);
            }
            let r = form_row(ui, t.webp_compression, |ui| {
                let r = slider_row(ui, &mut self.compression, 0..=6);
                one_line(ui, t.webp_compression_hint, muted());
                r
            });
            if let Some(v) = default_menu(&r, env, dv::WEBP_COMPRESSION, Some(&self.compression.to_string())) {
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
        let req = match (size, fps) {
            (Some(size), Some(fps)) => Some(Request::Anim(Box::new(Anim {
                format: if self.format == 0 { AnimFormat::Webp } else { AnimFormat::Gif },
                size,
                fps,
                quality: self.quality,
                compression: self.compression,
                dither: DITHERS[self.dither.min(3)].to_string(),
                edit: self.ed.edit(env),
            }))),
            _ => None,
        };
        result_row_with(ui, env, req.as_ref(), None);
        req
    }
}

fn dither_index(key: &str) -> usize {
    DITHERS.iter().position(|d| *d == key).unwrap_or(0)
}
