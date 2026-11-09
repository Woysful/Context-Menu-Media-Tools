//! A drop-down list made of columns, and the codec lists of Compress Video.

use crate::config::{Codec, Config};
use crate::gpu;
use crate::i18n::T;
use crate::theme::{self, c, Look, CONTROL_H};
use eframe::egui::{self, pos2, vec2, Align, Align2, FontId, Layout, Margin, Rect, Sense, Stroke};

use super::paint;
use super::widgets::text_width;

/// One column of a drop-down list: items are (key, label)
pub struct Col {
    pub title: Option<String>,
    pub items: Vec<(String, String)>,
}

const ITEM_H: f32 = 28.0;
const TITLE_H: f32 = 22.0;
const POP_PAD: f32 = 8.0;
const COL_GAP: f32 = 8.0;

/// While a form is measured, the lowest edge of an open list is kept under this id,
/// so the window is made tall enough for it
pub fn need_id() -> egui::Id {
    egui::Id::new("cmmt_dropdown_need")
}

/// The room left of the text of a narrow list button
const TEXT_PAD: f32 = 12.0;

/// The room the arrow of a list button takes on each side of the text
fn arrow_margin(look: Look) -> f32 {
    match look {
        Look::Brutal => CONTROL_H + 4.0,
        Look::Classic => 28.0,
        Look::Y2k => 32.0,
    }
}

fn button_text_size(look: Look) -> f32 {
    if look == Look::Classic {
        13.5
    } else {
        13.0
    }
}

/// The width of a narrow list button that shows `shown` whole
pub fn list_button_width(ui: &egui::Ui, shown: &str) -> f32 {
    let look = c().look;
    text_width(ui.ctx(), shown, FontId::proportional(button_text_size(look))) + TEXT_PAD + arrow_margin(look) + 4.0
}
/// The button of a drop-down list showing `shown`, as wide as the room; a click opens or closes
/// the popup `id`. Returns the button and whether the list was open.
pub fn list_button(ui: &mut egui::Ui, id: egui::Id, shown: &str) -> (egui::Response, bool) {
    let k = c();
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, CONTROL_H), Sense::click());
    let open = ui.memory(|m| m.is_popup_open(id));

    // the button
    let hot = open || resp.hovered();
    let painter = ui.painter();
    let look = k.look;
    match look {
        Look::Classic => {
            let stroke = if hot { Stroke::new(1.0_f32, k.accent) } else { Stroke::new(1.0_f32, k.border) };
            painter.rect(rect, 3.0, k.surface, stroke);
        }
        Look::Brutal => {
            let stroke = if hot { Stroke::new(2.0_f32, k.accent) } else { Stroke::new(1.0_f32, k.border) };
            painter.rect(rect, 0.0, k.surface, stroke);
        }
        Look::Y2k => {
            let (fill, stroke) = if hot { (k.hover, Stroke::new(1.0_f32, k.accent)) } else { (k.surface, Stroke::new(1.0_f32, k.border)) };
            paint::cut_rect(painter, rect, 7.0, paint::DIAGONAL, fill, stroke);
            let x = rect.right() - 30.0;
            painter.line_segment([pos2(x, rect.top() + 8.0), pos2(x, rect.bottom() - 8.0)], Stroke::new(1.0_f32, k.rule));
        }
    }
    // the arrow: at the right (Classic, Y2K) or in a square cell of its own (Brutalist)
    let (m, arrow_c) = match look {
        Look::Brutal => {
            let cell = Rect::from_min_max(pos2(rect.right() - CONTROL_H, rect.top()), rect.max);
            if open {
                painter.rect_filled(cell, 0.0, k.accent);
            }
            painter.line_segment([cell.left_top(), cell.left_bottom()], Stroke::new(1.0_f32, if hot { k.accent } else { k.border }));
            (cell.center(), if open { k.on_accent } else { k.text })
        }
        Look::Classic => (pos2(rect.right() - 15.0, rect.center().y), k.muted),
        Look::Y2k => (pos2(rect.right() - 15.0, rect.center().y), k.accent),
    };
    // centred between two margins as wide as the arrow, so it stays clear of it; on a narrow
    // button that has no room for that, centred in what is left of the arrow
    let text_size = button_text_size(look);
    let even = rect.shrink2(vec2(arrow_margin(look), 0.0));
    let fits = text_width(ui.ctx(), shown, FontId::proportional(text_size)) <= even.width();
    let clip = if fits { even } else { Rect::from_min_max(pos2(rect.left() + TEXT_PAD, rect.top()), pos2(rect.right() - arrow_margin(look), rect.bottom())) };
    let mut job = egui::text::LayoutJob::simple_singleline(shown.to_string(), FontId::proportional(text_size), k.text);
    job.wrap = egui::text::TextWrapping { max_width: clip.width(), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let galley = painter.layout_job(job);
    painter.with_clip_rect(clip).galley(Align2::CENTER_CENTER.anchor_size(clip.center(), galley.size()).min, galley, k.text);
    let (hw, hh) = if look == Look::Classic { (4.0, 2.5) } else { (5.0, 3.0) };
    let tri = if open {
        vec![pos2(m.x - hw, m.y + hh), pos2(m.x + hw, m.y + hh), pos2(m.x, m.y - hh)]
    } else {
        vec![pos2(m.x - hw, m.y - hh), pos2(m.x + hw, m.y - hh), pos2(m.x, m.y + hh)]
    };
    painter.add(egui::Shape::convex_polygon(tri, arrow_c, Stroke::NONE));
    if resp.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }
    (resp, open)
}

/// A drop-down list. The list is a set of columns (groups side by side), so it is short enough to
/// open completely inside the window: no scrolling, and it never covers its own button. It opens
/// below the button, or above it when there is no room below. The response of the button is
/// "changed" when an item was chosen.
pub fn dropdown(ui: &mut egui::Ui, id: impl std::hash::Hash, shown: &str, cols: &[Col], selected: &mut String) -> egui::Response {
    let k = c();
    let ctx = ui.ctx().clone();
    let id = ui.make_persistent_id(id);
    let width = ui.available_width();
    let (mut resp, open) = list_button(ui, id, shown);
    let rect = resp.rect;
    let look = k.look;
    let text_size = if look == Look::Classic { 13.5 } else { 13.0 };

    // the size of the list
    let font = FontId::proportional(text_size);
    let title_font = match look {
        Look::Classic => FontId::proportional(10.5),
        Look::Brutal => FontId::new(10.0, theme::bold_family()),
        Look::Y2k => FontId::new(10.5, theme::tech_family()),
    };
    let title_text = |t: &str| match look {
        Look::Classic => t.to_uppercase(),
        Look::Brutal => format!("// {}", t.to_uppercase()),
        Look::Y2k => format!("[ {} ]", t.to_uppercase()),
    };
    let screen = ctx.screen_rect();
    let max_w = (screen.width() - 16.0).max(160.0);
    let natural: Vec<f32> = cols
        .iter()
        .map(|col| {
            let title_w = col.title.as_ref().map_or(0.0, |t| text_width(&ctx, &title_text(t), title_font.clone()));
            col.items.iter().map(|(_, label)| text_width(&ctx, label, font.clone())).fold(title_w, f32::max) + 28.0
        })
        .collect();
    let col_h: Vec<f32> = cols.iter().map(|col| col.title.as_ref().map_or(0.0, |_| TITLE_H) + col.items.len() as f32 * ITEM_H).collect();
    // columns are packed into rows no wider than the window
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut cur_w = 0.0;
    for (i, w) in natural.iter().enumerate() {
        let joined = cur_w + COL_GAP + w;
        match rows.last_mut() {
            Some(row) if joined + 2.0 * POP_PAD <= max_w => {
                row.push(i);
                cur_w = joined;
            }
            _ => {
                rows.push(vec![i]);
                cur_w = *w;
            }
        }
    }
    let row_w = |row: &Vec<usize>| row.iter().map(|&i| natural[i]).sum::<f32>() + COL_GAP * (row.len().saturating_sub(1)) as f32;
    let inner_w = rows.iter().map(row_w).fold(0.0, f32::max).max(width - 2.0 * POP_PAD).min(max_w - 2.0 * POP_PAD);
    let popup_w = inner_w + 2.0 * POP_PAD;
    let popup_h =
        rows.iter().map(|r| r.iter().map(|&i| col_h[i]).fold(0.0, f32::max)).sum::<f32>() + COL_GAP * rows.len().saturating_sub(1) as f32 + 2.0 * POP_PAD + 2.0;
    // a list that fits above its button opens there in a shorter window, so it needs no room below
    let fits_above = rect.top() - 4.0 - popup_h >= screen.top() + 8.0;
    if !fits_above {
        ctx.data_mut(|d| {
            let cur = d.get_temp::<f32>(need_id()).unwrap_or(0.0);
            d.insert_temp(need_id(), cur.max(rect.bottom() + 4.0 + popup_h + 8.0));
        });
    }

    let mut chosen = false;
    if open {
        let below = screen.bottom() - rect.bottom() - 12.0;
        let y = if popup_h <= below || !fits_above { rect.bottom() + 4.0 } else { rect.top() - 4.0 - popup_h };
        let x = rect.left().min(screen.right() - popup_w - 8.0).max(8.0);
        let area = egui::Area::new(id.with("list")).order(egui::Order::Foreground).fixed_pos(pos2(x, y)).constrain(false).show(&ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(Margin::same(POP_PAD)).show(ui, |ui| {
                ui.spacing_mut().item_spacing = vec2(COL_GAP, COL_GAP);
                for row in &rows {
                    // a row of columns shares the free width
                    let extra = ((inner_w - row_w(row)) / row.len() as f32).max(0.0);
                    ui.horizontal_top(|ui| {
                        for &ci in row {
                            let col = &cols[ci];
                            let w = natural[ci] + extra;
                            ui.allocate_ui_with_layout(vec2(w, 0.0), Layout::top_down(Align::Min), |ui| {
                                ui.set_width(w);
                                ui.spacing_mut().item_spacing.y = 0.0;
                                if let Some(title) = &col.title {
                                    let (r, _) = ui.allocate_exact_size(vec2(w, TITLE_H), Sense::hover());
                                    if !title.is_empty() {
                                        let color = match look {
                                            Look::Classic => k.muted,
                                            Look::Brutal => k.accent,
                                            Look::Y2k => k.accent,
                                        };
                                        ui.painter().text(
                                            pos2(r.left() + 10.0, r.center().y),
                                            Align2::LEFT_CENTER,
                                            title_text(title),
                                            title_font.clone(),
                                            color,
                                        );
                                    }
                                }
                                for (key, label) in &col.items {
                                    let (r, resp) = ui.allocate_exact_size(vec2(w, ITEM_H), Sense::click());
                                    let chosen_here = key == selected;
                                    let p = ui.painter();
                                    let color = match (look, chosen_here) {
                                        (Look::Classic, true) => {
                                            p.rect_filled(r, 3.0, theme::mix(k.surface, k.accent, 0.45));
                                            k.text
                                        }
                                        (Look::Brutal, true) => {
                                            p.rect_filled(r, 0.0, k.accent);
                                            k.on_accent
                                        }
                                        (Look::Y2k, true) => {
                                            paint::cut_rect(p, r, 6.0, paint::DIAGONAL, k.accent, Stroke::NONE);
                                            k.on_accent
                                        }
                                        (_, false) => {
                                            if resp.hovered() {
                                                p.rect_filled(r, if look == Look::Classic { 3.0 } else { 0.0 }, k.hover);
                                                if look != Look::Classic {
                                                    p.rect_filled(Rect::from_min_size(r.left_top(), vec2(3.0, r.height())), 0.0, k.accent);
                                                }
                                            }
                                            k.text
                                        }
                                    };
                                    ui.painter().text(pos2(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, label, font.clone(), color);
                                    if resp.clicked() {
                                        selected.clone_from(key);
                                        chosen = true;
                                    }
                                }
                            });
                        }
                    });
                }
            });
        });
        let list_rect = area.response.rect;
        let outside = ctx.input(|i| i.pointer.any_pressed() && i.pointer.interact_pos().is_some_and(|p| !list_rect.contains(p) && !rect.contains(p)));
        if chosen || outside || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.memory_mut(|mem| mem.close_popup());
        }
    }
    if chosen {
        resp.mark_changed();
    }
    resp
}

/// The codecs of one kind of hardware (or the presets of the size limit tab)
pub struct Group {
    label: String,
    items: Vec<(String, String)>, // (key, label)
}

fn group_label(t: &T, group: &str) -> String {
    match group {
        "cpu" => t.group_cpu.into(),
        "nvidia" => t.group_nvidia.into(),
        "amd" => t.group_amd.into(),
        "intel" => t.group_intel.into(),
        "pi" => t.group_pi.into(),
        "preset" => t.group_preset.into(),
        other => other.to_string(),
    }
}

/// The codecs grouped by hardware, then the presets (size limit tab). Groups of GPUs that
/// are not in this computer are left out (unless `show_all_gpus` is set or nothing is known).
pub fn build_groups(list: &[(String, Codec)], t: &T, cfg: &Config) -> Vec<Group> {
    let show_all = cfg.settings.show_all_gpus;
    let present = if show_all { None } else { gpu::vendors_present() };
    let pi = show_all || gpu::raspberry_pi_encoder();
    let mut groups: Vec<Group> = Vec::new();
    let mut add = |group: &str, key: &str, label: String| {
        let name = group_label(t, group);
        match groups.iter_mut().find(|g| g.label == name) {
            Some(g) => g.items.push((key.to_string(), label)),
            None => groups.push(Group { label: name, items: vec![(key.to_string(), label)] }),
        }
    };
    for (key, c) in list.iter().filter(|(_, c)| !c.preset) {
        if c.group == "pi" && !pi {
            continue;
        }
        if let Some(set) = &present {
            if matches!(c.group.as_str(), "nvidia" | "amd" | "intel") && !set.contains(c.group.as_str()) {
                continue;
            }
        }
        add(&c.group, key, c.label.clone());
    }
    for (key, c) in list.iter().filter(|(_, c)| c.preset) {
        add("preset", key, c.label.clone());
    }
    groups
}

pub fn codec_combo(ui: &mut egui::Ui, id: &str, groups: &[Group], selected: &mut String, t: &T) -> egui::Response {
    let preset_label = group_label(t, "preset");
    let shown = groups
        .iter()
        .find_map(|g| {
            g.items.iter().find(|(k, _)| k == selected).map(|(_, label)| if g.label == preset_label { label.clone() } else { format!("{label} — {}", g.label) })
        })
        .unwrap_or_else(|| selected.clone());
    let cols: Vec<Col> = groups.iter().map(|g| Col { title: Some(g.label.clone()), items: g.items.clone() }).collect();
    dropdown(ui, id, &shown, &cols, selected)
}
