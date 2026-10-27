//! The Settings window.

use super::widgets::{button_row, choice, field, muted, number_field, primary_button, secondary_button, MARGIN};
use crate::action::Action;
use crate::config;
use crate::i18n::T;
use crate::i18n_settings as si;
use crate::install::{self, Options};
use crate::paths;
use crate::proc;
use crate::settings::{self, Custom, Settings};
use crate::syspal::{self, Palette};
use crate::theme::{self, bold, c, CONTROL_H};
use crate::units;
use eframe::egui::{self, pos2, vec2, Align, Color32, FontId, Layout, Margin, Rect, RichText, Sense, Stroke};
use std::path::Path;

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
}

impl SettingsState {
    pub fn new(s: Settings) -> Self {
        let mut st = SettingsState {
            saved: s.clone(),
            auto_close_text: units::fmt_float(s.auto_close),
            s,
            tab: 0,
            status: String::new(),
            status_ok: true,
            palette: syspal::detect(),
            ffmpeg_info: String::new(),
            ffprobe_info: String::new(),
            transfer_path: paths::home_dir().join("cmmt-commands.toml").display().to_string(),
            scale_draft: None,
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
            for (i, label) in [ts.tab_general, ts.tab_look, ts.tab_menu].iter().enumerate() {
                if nav_item(ui, label, st.tab == i) {
                    st.tab = i;
                }
            }
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                ui.label(RichText::new(format!("{} {}", ts.version, env!("CARGO_PKG_VERSION"))).small().color(k.muted));
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
                    _ => menu_page(ui, st, t),
                }
            });
        },
    );
    pressed
}

fn general_page(ui: &mut egui::Ui, st: &mut SettingsState, pressed: &mut Option<Command>) {
    let ts = si::s();
    let close_ok = units::parse_decimal(&st.auto_close_text).filter(|v| (0.0..=3600.0).contains(v));
    setting_row(ui, ts.auto_close, ts.auto_close_hint, |ui| number_field(ui, &mut st.auto_close_text, close_ok.is_some(), "s"));
    if let Some(v) = close_ok {
        st.s.auto_close = v;
    }
    setting_row(ui, ts.gpu_codecs, ts.gpu_codecs_hint, |ui| {
        ui.checkbox(&mut st.s.show_all_gpus, ts.gpu_codecs_check);
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
                let current = theme::parse_hex(&st.s.accent).unwrap_or_else(|| theme::default_accent(k.dark));
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
