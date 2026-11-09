//! The Settings window.

use super::dropdown::{build_groups, codec_combo, dropdown, Col, Group};
use super::forms::audio_combo;
use super::scale_field::{label_of, ScaleField};
use super::widgets::{button_row, choice, field, muted, number_field, primary_button, secondary_button, MARGIN};
use crate::action::Action;
use crate::config::{self, Config, ConvertKind};
use crate::defaults::{self as dv, Kind};
use crate::i18n::T;
use crate::i18n_settings as si;
use crate::install::{self, Options};
use crate::paths;
use crate::proc;
use crate::scale::ScaleMode;
use crate::settings::{self, Custom, Settings};
use crate::syspal::{self, Palette};
use crate::theme::{self, bold, c, CONTROL_H};
use crate::units;
use crate::update;
use eframe::egui::{self, pos2, vec2, Align, Color32, FontId, Layout, Margin, Rect, RichText, Sense, Stroke};
use std::path::Path;
use std::sync::Arc;

pub struct SettingsState {
    /// what the window shows (and previews)
    pub s: Settings,
    saved: Settings,
    tab: usize,
    status: String,
    status_ok: bool,
    palette: Option<Palette>,
    ffmpeg_info: String,
    ffprobe_info: String,
    transfer_path: String,
    auto_close_text: String,
    /// the interface scale while its slider is dragged
    scale_draft: Option<f64>,
    update: update::Checker,
    /// the lists of the Defaults page
    cfg: Arc<Config>,
    p_groups: Vec<Group>,
    l_groups: Vec<Group>,
    defaults_path: String,
}

impl SettingsState {
    pub fn new(cfg: Arc<Config>) -> Self {
        let t = crate::i18n::t();
        let s = cfg.settings.clone();
        let mut st = SettingsState {
            saved: s.clone(),
            auto_close_text: units::fmt_float(s.auto_close),
            p_groups: build_groups(&cfg.compress, t, &cfg),
            l_groups: build_groups(&cfg.limit, t, &cfg),
            defaults_path: paths::home_dir().join("cmmt-defaults.toml").display().to_string(),
            cfg,
            s,
            tab: 0,
            status: String::new(),
            status_ok: true,
            palette: syspal::detect(),
            ffmpeg_info: String::new(),
            ffprobe_info: String::new(),
            transfer_path: paths::home_dir().join("cmmt-commands.toml").display().to_string(),
            scale_draft: None,
            update: update::Checker::default(),
        };
        st.refresh_tools();
        st
    }

    fn refresh_tools(&mut self) {
        let f = |name: &str, conf: &str| paths::find_tool(name, conf).map(|p| p.display().to_string()).unwrap_or_default();
        self.ffmpeg_info = f("ffmpeg", &self.s.ffmpeg);
        self.ffprobe_info = f("ffprobe", &self.s.ffprobe);
    }

    fn set_status(&mut self, text: impl Into<String>, ok: bool) {
        self.status = text.into();
        self.status_ok = ok;
    }
}

/// The buttons of the window that do more than change a value
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Close,
    Save,
    SaveAndInstall,
    Install,
    Uninstall,
    OpenPresets,
    OpenFolder,
}

/// What the window does after a command
pub enum After {
    Stay,
    Close,
    /// Show the lines of install / uninstall
    Report {
        title: String,
        lines: Vec<String>,
        ok: bool,
    },
}

fn report(title: &str, (code, lines): (i32, Vec<String>)) -> After {
    After::Report { title: title.to_string(), lines, ok: code == 0 }
}

pub fn run_command(st: &mut SettingsState, cmd: Command, t: &T) -> After {
    let ts = si::s();
    match cmd {
        Command::Close => After::Close,
        Command::Save | Command::SaveAndInstall => {
            match st.s.save() {
                Ok(()) => {
                    st.saved = st.s.clone();
                    st.set_status(ts.saved, true);
                }
                Err(e) => st.set_status(e, false),
            }
            if cmd == Command::SaveAndInstall && st.status_ok {
                report(t.w_install, install::install(&Options::default()))
            } else {
                After::Stay
            }
        }
        Command::Install => report(t.w_install, install::install(&Options::default())),
        Command::Uninstall => report(t.w_uninstall, install::uninstall(&Options::default())),
        Command::OpenPresets => {
            let path = paths::presets_file();
            if !path.exists() {
                if let Err(e) = config::init_presets() {
                    st.set_status(e, false);
                }
            }
            proc::open_path(&path);
            After::Stay
        }
        Command::OpenFolder => {
            let dir = paths::config_dir();
            let _ = std::fs::create_dir_all(&dir);
            proc::open_path(&dir);
            After::Stay
        }
    }
}

const CONTROL_W: f32 = 270.0;

/// One setting: the name and a hint on the left, the control in a column of fixed width on the right
fn setting_row(ui: &mut egui::Ui, title: &str, hint_text: &str, add: impl FnOnce(&mut egui::Ui)) {
    // the width of the page, not of what the rows above have grown to
    let total = ui.max_rect().width();
    // the left block, the control, and the three gaps between and around them must fit exactly
    let gaps = ui.spacing().item_spacing.x * 2.0 + 24.0;
    let left_w = (total - CONTROL_W - gaps - 2.0).max(140.0);
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(vec2(left_w, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(left_w);
            ui.label(bold(title, 13.5));
            if !hint_text.is_empty() {
                ui.label(RichText::new(hint_text).small().color(muted()));
            }
        });
        ui.add_space(24.0);
        ui.allocate_ui_with_layout(vec2(CONTROL_W, 0.0), Layout::top_down(Align::Min), |ui| {
            ui.set_width(CONTROL_W);
            add(ui);
        });
    });
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
}

/// A setting whose control is as wide as the page: the name and hint above it
fn wide_row(ui: &mut egui::Ui, title: &str, hint_text: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.label(bold(title, 13.5));
    if !hint_text.is_empty() {
        ui.label(RichText::new(hint_text).small().color(muted()));
    }
    ui.add_space(6.0);
    add(ui);
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
}

/// An entry of the navigation at the left
fn nav_item(ui: &mut egui::Ui, label: &str, selected: bool) -> bool {
    let k = c();
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), 34.0), Sense::click());
    if selected {
        ui.painter().rect_filled(rect, 3.0, k.hover);
        ui.painter().rect_filled(Rect::from_min_size(rect.min, vec2(3.0, rect.height())), 1.0, k.accent);
    } else if resp.hovered() {
        let h = k.hover;
        ui.painter().rect_filled(rect, 3.0, Color32::from_rgba_unmultiplied(h.r(), h.g(), h.b(), 120));
    }
    let (font, color) = if selected { (FontId::new(13.5, theme::bold_family()), k.text) } else { (FontId::proportional(13.5), k.muted) };
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, color);
    ui.painter().galley(pos2(rect.left() + 14.0, rect.center().y - galley.size().y / 2.0), galley, color);
    resp.clicked()
}

/// The Settings window; returns the button that was pressed
pub fn screen(ctx: &egui::Context, st: &mut SettingsState, t: &T) -> Option<Command> {
    let ts = si::s();
    let k = c();
    let fill = ctx.style().visuals.panel_fill;
    let mut pressed: Option<Command> = None;

    // top: the title and the menu buttons
    egui::TopBottomPanel::top("s_top")
        .frame(egui::Frame::none().fill(fill).inner_margin(Margin { left: MARGIN, right: MARGIN, top: 16.0, bottom: 12.0 }))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(bold(ts.title, 18.0));
                let buttons = 110.0 + 96.0 + ui.spacing().item_spacing.x * 2.0 + 4.0;
                let free = ui.available_width() - buttons;
                if free > 0.0 {
                    ui.add_space(free);
                }
                if ui.add(primary_button(t.w_install)).clicked() {
                    pressed = Some(Command::Install);
                }
                if ui.add(secondary_button(t.w_uninstall)).clicked() {
                    pressed = Some(Command::Uninstall);
                }
            });
        });

    // bottom: the state of the settings and the buttons
    let dirty = st.s != st.saved;
    egui::TopBottomPanel::bottom("s_foot")
        .frame(egui::Frame::none().fill(fill).inner_margin(Margin { left: MARGIN, right: MARGIN, top: 12.0, bottom: 16.0 }))
        .show(ctx, |ui| {
            button_row(ui, |ui| {
                if ui.add(secondary_button(t.close)).clicked() {
                    pressed = Some(Command::Close);
                }
                if ui.add(primary_button(ts.save_menu).min_size(vec2(170.0, CONTROL_H))).clicked() {
                    pressed = Some(Command::SaveAndInstall);
                }
                if ui.add_enabled(dirty, secondary_button(ts.save)).clicked() {
                    pressed = Some(Command::Save);
                }
                let (text, color) = if !st.status.is_empty() {
                    (st.status.as_str(), if st.status_ok { k.ok } else { k.err })
                } else if dirty {
                    (ts.unsaved, k.warn)
                } else {
                    ("", k.muted)
                };
                ui.label(RichText::new(text).small().color(color));
            });
        });

    // left: the pages
    egui::SidePanel::left("s_nav").exact_width(190.0).resizable(false).frame(egui::Frame::none().fill(k.surface).inner_margin(Margin::same(12.0))).show(
        ctx,
        |ui| {
            for (i, label) in [ts.tab_general, ts.tab_look, ts.tab_menu, ts.tab_defaults].iter().enumerate() {
                if nav_item(ui, label, st.tab == i) {
                    st.tab = i;
                }
            }
            let repaint = {
                let ctx = ctx.clone();
                move || ctx.request_repaint()
            };
            let release = st.update.poll(st.s.check_updates, repaint).cloned();
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                ui.label(RichText::new(format!("{} {}", ts.version, env!("CARGO_PKG_VERSION"))).small().color(k.muted));
                if let Some(r) = release {
                    update_notice(ui, &r);
                }
            });
        },
    );

    // the page
    egui::CentralPanel::default().frame(egui::Frame::none().fill(fill).inner_margin(Margin { left: 24.0, right: 16.0, top: 12.0, bottom: 0.0 })).show(
        ctx,
        |ui| {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                let page_w = ui.available_width() - 14.0;
                ui.set_max_width(page_w);
                ui.set_min_width(page_w);
                match st.tab {
                    0 => general_page(ui, st, &mut pressed),
                    1 => look_page(ui, st),
                    2 => menu_page(ui, st, t),
                    _ => defaults_page(ui, st, t),
                }
            });
        },
    );
    pressed
}

/// A small accent dot and "Version x is out"; a click opens the page of the release
fn update_notice(ui: &mut egui::Ui, r: &update::Release) {
    let k = c();
    ui.horizontal(|ui| {
        let (dot, _) = ui.allocate_exact_size(vec2(8.0, 8.0), Sense::hover());
        ui.painter().circle_filled(dot.center(), 4.0, k.accent);
        let text = si::s().update_available.replace("{}", &r.version);
        let link = ui.add(egui::Link::new(RichText::new(text).small().color(k.accent)));
        if link.on_hover_text(&r.url).clicked() {
            proc::open_path(Path::new(&r.url));
        }
    });
}

fn general_page(ui: &mut egui::Ui, st: &mut SettingsState, pressed: &mut Option<Command>) {
    let ts = si::s();
    let close_ok = units::parse_decimal(&st.auto_close_text).filter(|v| (0.0..=3600.0).contains(v));
    setting_row(ui, ts.auto_close, ts.auto_close_hint, |ui| {
        number_field(ui, &mut st.auto_close_text, close_ok.is_some(), "s");
    });
    if let Some(v) = close_ok {
        st.s.auto_close = v;
    }
    setting_row(ui, ts.gpu_codecs, ts.gpu_codecs_hint, |ui| {
        ui.checkbox(&mut st.s.show_all_gpus, ts.gpu_codecs_check);
    });
    setting_row(ui, ts.updates, ts.updates_hint, |ui| {
        ui.checkbox(&mut st.s.check_updates, ts.updates_check);
    });
    setting_row(ui, ts.originals, ts.originals_hint, |ui| {
        choice(ui, &mut st.s.delete_mode, &[("trash", ts.to_trash), ("permanent", ts.permanently)]);
    });
    let mut changed = false;
    for (title, path, info) in [(ts.ffmpeg, &mut st.s.ffmpeg, &st.ffmpeg_info), (ts.ffprobe, &mut st.s.ffprobe, &st.ffprobe_info)] {
        let status = if info.is_empty() { format!("{} — {}", ts.not_found, ts.tool_hint) } else { format!("{}: {info}", ts.found) };
        setting_row(ui, title, &status, |ui| {
            if ui.add(egui::TextEdit::singleline(path).desired_width(CONTROL_W).hint_text("auto")).changed() {
                changed = true;
            }
        });
    }
    if changed {
        st.refresh_tools();
    }
    setting_row(ui, ts.commands, &paths::presets_file().display().to_string(), |ui| {
        ui.horizontal(|ui| {
            if ui.add(secondary_button(ts.open_file).min_size(vec2(150.0, CONTROL_H))).clicked() {
                *pressed = Some(Command::OpenPresets);
            }
            if ui.add(secondary_button(ts.open_folder).min_size(vec2(104.0, CONTROL_H))).clicked() {
                *pressed = Some(Command::OpenFolder);
            }
        });
    });
}

fn look_page(ui: &mut egui::Ui, st: &mut SettingsState) {
    let ts = si::s();
    let k = c();
    setting_row(ui, ts.style, ts.style_hint, |ui| {
        choice(ui, &mut st.s.style, &[("classic", ts.style_classic), ("brutalist", ts.style_brutal), ("y2k", ts.style_y2k)]);
    });
    setting_row(ui, ts.theme, ts.theme_hint, |ui| {
        choice(ui, &mut st.s.theme, &[("auto", ts.theme_auto), ("dark", ts.theme_dark), ("light", ts.theme_light)]);
    });
    let have_palette = st.palette.is_some();
    if !have_palette {
        st.s.system_palette = false;
    }
    setting_row(ui, ts.system_colours, if have_palette { ts.system_colours_hint } else { ts.system_colours_na }, |ui| {
        ui.add_enabled(have_palette, egui::Checkbox::new(&mut st.s.system_palette, ts.system_colours_check));
    });
    setting_row(ui, ts.accent, ts.accent_hint, |ui| {
        ui.add_enabled_ui(!st.s.system_palette, |ui| {
            ui.horizontal(|ui| {
                let current = theme::parse_hex(&st.s.accent).unwrap_or_else(|| theme::default_accent(k.look, k.dark));
                let mut rgb = [current.r(), current.g(), current.b()];
                if ui.color_edit_button_srgb(&mut rgb).changed() {
                    st.s.accent = theme::to_hex(Color32::from_rgb(rgb[0], rgb[1], rgb[2]));
                }
                ui.label(RichText::new(theme::to_hex(current)).monospace());
                if ui.add(secondary_button(ts.reset).min_size(vec2(70.0, CONTROL_H))).clicked() {
                    st.s.accent.clear();
                }
            });
        });
    });
    setting_row(ui, ts.transparency, ts.transparency_hint, |ui| {
        choice(ui, &mut st.s.transparency, &[("auto", ts.tr_auto), ("on", ts.tr_on), ("off", ts.tr_off)]);
    });
    let slider = |ui: &mut egui::Ui, value: &mut f64, range: std::ops::RangeInclusive<f64>, step: f64| {
        ui.horizontal(|ui| {
            ui.spacing_mut().slider_width = (ui.available_width() - 52.0).max(80.0);
            let r = ui.add(egui::Slider::new(value, range).step_by(step).show_value(false));
            ui.label(RichText::new(format!("{value:.2}")).monospace());
            r
        })
        .inner
    };
    setting_row(ui, ts.opacity, ts.opacity_hint, |ui| {
        ui.add_enabled_ui(st.s.transparency != "off", |ui| slider(ui, &mut st.s.opacity, 0.5..=1.0, 0.01));
    });
    setting_row(ui, ts.scale, ts.scale_hint, |ui| {
        // the scale resizes the slider under the pointer, so it takes effect when the slider is let go
        let mut v = st.scale_draft.unwrap_or(st.s.ui_scale);
        if slider(ui, &mut v, 0.8..=1.6, 0.05).dragged() {
            st.scale_draft = Some(v);
        } else {
            st.scale_draft = None;
            st.s.ui_scale = v;
        }
    });
}

fn menu_page(ui: &mut egui::Ui, st: &mut SettingsState, t: &T) {
    let ts = si::s();
    let k = c();
    wide_row(ui, ts.standard, ts.standard_hint, |ui| {
        ui.columns(3, |cols| {
            for (i, a) in Action::ALL.into_iter().enumerate() {
                let mut on = st.s.enabled(a);
                if cols[i % 3].checkbox(&mut on, a.title(t)).changed() {
                    st.s.set_enabled(a, on);
                }
            }
        });
    });
    setting_row(ui, ts.numbered, ts.numbered_hint, |ui| {
        ui.checkbox(&mut st.s.numbered_scripts, ts.numbered_check);
    });

    wide_row(ui, ts.custom, ts.custom_hint, |ui| {
        let mut remove: Option<usize> = None;
        for (i, item) in st.s.custom.iter_mut().enumerate() {
            egui::Frame::none().fill(k.surface).stroke(Stroke::new(1.0_f32, k.border)).rounding(3.0).inner_margin(Margin::same(12.0)).show(ui, |ui| {
                ui.columns(2, |cols| {
                    field(&mut cols[0], ts.name, |ui| {
                        ui.add(egui::TextEdit::singleline(&mut item.name).desired_width(f32::INFINITY));
                    });
                    field(&mut cols[1], ts.kind, |ui| {
                        choice(ui, &mut item.kind, &[("video", ts.kind_video), ("audio", ts.kind_audio), ("image", ts.kind_image)]);
                    });
                });
                field(ui, ts.command, |ui| {
                    ui.add(egui::TextEdit::multiline(&mut item.command).desired_rows(2).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                });
                field(ui, ts.output, |ui| {
                    ui.add(egui::TextEdit::singleline(&mut item.output).desired_width(f32::INFINITY).font(egui::TextStyle::Monospace));
                });
                button_row(ui, |ui| {
                    if ui.add(secondary_button(ts.remove)).clicked() {
                        remove = Some(i);
                    }
                });
            });
            ui.add_space(8.0);
        }
        if let Some(i) = remove {
            st.s.custom.remove(i);
        }
        if ui.add(secondary_button(&format!("+ {}", ts.add)).min_size(vec2(150.0, CONTROL_H))).clicked() {
            st.s.custom.push(Custom { name: ts.new_item.into(), ..Default::default() });
        }
    });

    wide_row(ui, ts.transfer, ts.transfer_hint, |ui| {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 2.0 * 96.0 - 2.0 * ui.spacing().item_spacing.x).max(120.0);
            ui.add(egui::TextEdit::singleline(&mut st.transfer_path).desired_width(w));
            if ui.add(secondary_button(ts.export)).clicked() {
                export(st);
            }
            if ui.add(secondary_button(ts.import)).clicked() {
                import(st);
            }
        });
    });

    settings::assign_ids(&mut st.s.custom);
}

fn export(st: &mut SettingsState) {
    let ts = si::s();
    if st.s.custom.is_empty() {
        st.set_status(ts.nothing_to_export, false);
        return;
    }
    let path = st.transfer_path.trim().to_string();
    match settings::export_custom(&st.s.custom, Path::new(&path)) {
        Ok(n) => st.set_status(format!("{} {path}", ts.exported.replace("{}", &n.to_string())), true),
        Err(e) => st.set_status(e, false),
    }
}

fn import(st: &mut SettingsState) {
    let ts = si::s();
    match settings::import_custom(Path::new(st.transfer_path.trim())) {
        Ok(items) => {
            st.set_status(ts.imported.replace("{}", &items.len().to_string()), true);
            st.s.custom.extend(items);
        }
        Err(e) => st.set_status(format!("{} {e}", ts.import_failed), false),
    }
}

/// The name of a field of the option windows and where it is: ("Video codec", "Compress Video · Presets")
fn field_names(t: &T, key: &str) -> (String, String) {
    let ts = si::s();
    let presets = format!("{} · {}", t.t_compress, t.tab_presets);
    let limit = format!("{} · {}", t.t_compress, t.tab_limit);
    let (name, place) = match key {
        dv::MODE => (ts.field_tab, t.t_compress.to_string()),
        dv::LEVEL => (t.level, presets),
        dv::CODEC => (t.codec, presets),
        dv::RESOLUTION => (t.resolution, presets),
        dv::AUDIO => (t.audio, presets),
        dv::MERGE => (t.merge_tracks, t.t_compress.to_string()),
        dv::LIMIT_SIZE => (t.limit, limit),
        dv::TWO_PASS => (t.two_pass, limit),
        dv::LIMIT_CODEC => (t.codec, limit),
        dv::LIMIT_RESOLUTION => (t.resolution, limit),
        dv::LIMIT_AUDIO => (t.audio, limit),
        dv::WEBP_SIZE => (t.webp_size, t.t_anim.to_string()),
        dv::WEBP_FPS => (t.webp_fps, t.t_anim.to_string()),
        dv::WEBP_QUALITY => (t.webp_quality, format!("{} · WebP", t.t_anim)),
        dv::WEBP_COMPRESSION => (t.webp_compression, format!("{} · WebP", t.t_anim)),
        dv::GIF_DITHER => (t.anim_dither, format!("{} · GIF", t.t_anim)),
        dv::ANIM_SLOW => (t.anim_slow, t.t_anim.to_string()),
        dv::DELETE_MERGE => (t.merge_kept, t.t_delete_audio.to_string()),
        dv::DELETE_ORIGINAL => (t.delete_original, ts.every_window.to_string()),
        dv::IMAGE_MODE => (ts.field_tab, t.t_compress_image.to_string()),
        dv::IMAGE_FORMAT => (t.img_format, t.t_compress_image.to_string()),
        dv::IMAGE_QUALITY => (t.img_quality, t.t_compress_image.to_string()),
        dv::IMAGE_LIMIT => (t.img_limit, t.t_compress_image.to_string()),
        dv::IMAGE_RESOLUTION => (t.resolution, t.t_compress_image.to_string()),
        dv::ICON_SIZES => (t.icon_sizes, t.t_make_icon.to_string()),
        dv::ICON_SQUARE => (t.icon_square, t.t_make_icon.to_string()),
        dv::FRAMES_FPS => (t.fv_fps, t.t_frames_video.to_string()),
        dv::FRAMES_CODEC => (t.codec, t.t_frames_video.to_string()),
        dv::FRAMES_LEVEL => (t.level, t.t_frames_video.to_string()),
        _ => match dv::kind_of(key) {
            Some(Kind::Format(k)) => {
                let action = match k {
                    ConvertKind::Video => t.t_convert_video.to_string(),
                    ConvertKind::VideoAudio => format!("{} · {}", t.t_convert_audio, ts.kind_video),
                    ConvertKind::Extract => t.t_extract_audio.to_string(),
                    ConvertKind::Frames => t.t_frames.to_string(),
                    ConvertKind::AudioFile => t.t_convert_audio.to_string(),
                    ConvertKind::Image => t.t_convert_image.to_string(),
                };
                (t.format, action)
            }
            _ => return (key.to_string(), String::new()),
        },
    };
    (name.to_string(), place)
}

/// A value as the field shows it ("H.264 — CPU" for h264_cpu)
fn shown_value(cfg: &Config, t: &T, key: &str, value: &str) -> String {
    let ts = si::s();
    let named = |label: &str| if label.is_empty() { value.to_string() } else { label.to_string() };
    match dv::kind_of(key) {
        Some(Kind::Mode) => (if value == "limit" { t.tab_limit } else { t.tab_presets }).to_string(),
        Some(Kind::Level) => [t.level_low, t.level_medium, t.level_high][dv::level_index(value)].to_string(),
        Some(Kind::Codec { limit }) => {
            let c = if limit { cfg.limit_codec(value) } else { cfg.compress_codec(value) };
            c.map_or_else(|| value.to_string(), |c| named(&c.label))
        }
        Some(Kind::Audio) => cfg.audio_codec(value).map_or_else(|| value.to_string(), |a| named(&a.label)),
        Some(Kind::Resolution { .. }) => ScaleMode::from_code(value).map_or_else(|| value.to_string(), |m| label_of(m, t)),
        Some(Kind::Flag) => (if value == "true" { ts.tr_on } else { ts.tr_off }).to_string(),
        Some(Kind::Format(k)) => cfg.format(k, value).map_or_else(|| value.to_string(), |f| f.label(value).to_string()),
        Some(Kind::Megabytes) => format!("{value} MB"),
        Some(Kind::Number { .. }) if key == dv::WEBP_SIZE => format!("{value} px"),
        Some(Kind::Choice(_)) => choice_options(t, key).iter().find(|(v, _)| *v == value).map_or_else(|| value.to_string(), |(_, l)| l.to_string()),
        Some(Kind::ImageFormat) if value == crate::engine::SOURCE => t.img_source.to_string(),
        Some(Kind::ImageFormat) => cfg.image_format(value).map_or_else(|| value.to_string(), |f| f.label(value).to_string()),
        Some(Kind::Sizes) => format!("{} px", value.replace(',', ", ")),
        Some(Kind::Fps) => format!("{value} fps"),
        _ => value.to_string(),
    }
}

/// The values of a field of a few words, with their names
fn choice_options(t: &T, key: &str) -> Vec<(&'static str, &'static str)> {
    match key {
        dv::IMAGE_MODE => vec![("quality", t.img_tab_quality), ("limit", t.img_tab_limit)],
        dv::ICON_SQUARE => vec![("pad", t.icon_pad), ("crop", t.icon_crop)],
        dv::GIF_DITHER => dv::DITHERS.into_iter().zip(t.anim_dithers).collect(),
        dv::ANIM_SLOW => dv::SLOWS.into_iter().zip(t.anim_slows).collect(),
        _ => vec![],
    }
}

/// The control of one default value: the same kind of control as in the option window
fn value_control(ui: &mut egui::Ui, key: &str, value: &mut String, cfg: &Config, groups: (&[Group], &[Group]), t: &T) {
    let ts = si::s();
    let id = ("default", key.to_string());
    match dv::kind_of(key) {
        Some(Kind::Mode) => choice(ui, value, &[("presets", t.tab_presets), ("limit", t.tab_limit)]),
        Some(Kind::Level) => choice(ui, value, &[("low", t.level_low), ("medium", t.level_medium), ("high", t.level_high)]),
        Some(Kind::Codec { limit }) => {
            codec_combo(ui, &format!("default_{key}"), if limit { groups.1 } else { groups.0 }, value, t);
        }
        Some(Kind::Audio) => {
            audio_combo(ui, &format!("default_{key}"), t, cfg, value);
        }
        Some(Kind::Resolution { auto }) => {
            let mut field = ScaleField::from_code(value);
            if field.list(ui, &format!("default_{key}"), t, auto).changed() {
                *value = field.mode.code();
            }
        }
        Some(Kind::Flag) => {
            let mut on = value == "true";
            if ui.checkbox(&mut on, ts.tr_on).changed() {
                *value = on.to_string();
            }
        }
        Some(Kind::Format(k)) => {
            let items: Vec<(String, String)> = cfg.formats(k).iter().map(|(key, f)| (key.clone(), f.label(key).to_string())).collect();
            let shown = shown_value(cfg, t, key, value);
            dropdown(ui, id, &shown, &[Col { title: None, items }], value);
        }
        Some(Kind::Choice(_)) => choice(ui, value, &choice_options(t, key)),
        Some(Kind::ImageFormat) => {
            let mut items = vec![(crate::engine::SOURCE.to_string(), t.img_source.to_string())];
            items.extend(cfg.image_compress.iter().map(|(key, f)| (key.clone(), f.label(key).to_string())));
            let shown = shown_value(cfg, t, key, value);
            dropdown(ui, id, &shown, &[Col { title: None, items }], value);
        }
        Some(kind @ (Kind::Number { .. } | Kind::Megabytes | Kind::Size | Kind::Sizes | Kind::Fps)) => {
            let suffix = match key {
                dv::LIMIT_SIZE => "MB",
                dv::WEBP_SIZE | dv::ICON_SIZES => "px",
                dv::FRAMES_FPS => "fps",
                _ => "",
            };
            number_field(ui, value, dv::valid(cfg, kind, value), suffix);
        }
        None => {
            ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY));
        }
    }
}

/// A small square button with a cross
fn remove_button(ui: &mut egui::Ui) -> egui::Response {
    let k = c();
    let (rect, resp) = ui.allocate_exact_size(vec2(CONTROL_H, CONTROL_H), Sense::click());
    let color = if resp.hovered() { k.err } else { k.muted };
    if resp.hovered() {
        ui.painter().rect_filled(rect, 3.0, k.hover);
    }
    let (m, d) = (rect.center(), 4.5);
    let stroke = Stroke::new(1.5_f32, color);
    ui.painter().line_segment([m + vec2(-d, -d), m + vec2(d, d)], stroke);
    ui.painter().line_segment([m + vec2(-d, d), m + vec2(d, -d)], stroke);
    resp.on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn defaults_page(ui: &mut egui::Ui, st: &mut SettingsState, t: &T) {
    let ts = si::s();
    wide_row(ui, ts.defaults, ts.defaults_hint, |ui| {
        if st.s.defaults.is_empty() {
            ui.label(RichText::new(ts.defaults_empty).color(muted()));
        }
    });
    // in the order of the windows; keys this version does not know come last
    let mut keys: Vec<String> = dv::fields().into_iter().map(|(k, _)| k).filter(|k| st.s.defaults.contains_key(k)).collect();
    keys.extend(st.s.defaults.keys().filter(|k| dv::kind_of(k).is_none()).cloned());
    let mut remove: Option<String> = None;
    for key in keys {
        let (name, place) = field_names(t, &key);
        let standard = shown_value(&st.cfg, t, &key, &dv::standard(&st.cfg, &key));
        let about = if place.is_empty() { String::new() } else { format!("{place}\n{}: {standard}", ts.standard_value) };
        let Some(value) = st.s.defaults.get_mut(&key) else { continue };
        setting_row(ui, &name, &about, |ui| {
            ui.horizontal(|ui| {
                let w = CONTROL_W - CONTROL_H - ui.spacing().item_spacing.x;
                ui.allocate_ui_with_layout(vec2(w, CONTROL_H), Layout::top_down(Align::Min), |ui| {
                    ui.set_width(w);
                    value_control(ui, &key, value, &st.cfg, (&st.p_groups, &st.l_groups), t);
                });
                if remove_button(ui).on_hover_text(ts.remove).clicked() {
                    remove = Some(key.clone());
                }
            });
        });
    }
    if let Some(k) = remove {
        st.s.defaults.remove(&k);
    }

    wide_row(ui, ts.transfer, ts.defaults_transfer_hint, |ui| {
        ui.horizontal(|ui| {
            let w = (ui.available_width() - 2.0 * 96.0 - 2.0 * ui.spacing().item_spacing.x).max(120.0);
            ui.add(egui::TextEdit::singleline(&mut st.defaults_path).desired_width(w));
            if ui.add(secondary_button(ts.export)).clicked() {
                export_defaults(st);
            }
            if ui.add(secondary_button(ts.import)).clicked() {
                import_defaults(st);
            }
        });
    });
}

fn export_defaults(st: &mut SettingsState) {
    let ts = si::s();
    if st.s.defaults.is_empty() {
        st.set_status(ts.no_values, false);
        return;
    }
    let path = st.defaults_path.trim().to_string();
    match settings::export_defaults(&st.s.defaults, Path::new(&path)) {
        Ok(n) => st.set_status(format!("{} {path}", ts.exported_values.replace("{}", &n.to_string())), true),
        Err(e) => st.set_status(e, false),
    }
}

fn import_defaults(st: &mut SettingsState) {
    let ts = si::s();
    match settings::import_defaults(Path::new(st.defaults_path.trim())) {
        Ok(values) => {
            st.set_status(ts.imported_values.replace("{}", &values.len().to_string()), true);
            st.s.defaults.extend(values);
        }
        Err(e) => st.set_status(format!("{} {e}", ts.import_failed), false),
    }
}
