//! A drop-down list made of columns, and the codec lists of Compress Video.

use crate::config::{Codec, Config};
use crate::gpu;
use crate::i18n::T;
use crate::theme::{self, c, CONTROL_H};
use eframe::egui::{self, pos2, vec2, Align, Align2, FontId, Layout, Margin, Rect, Sense, Stroke};

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
    egui::Id::new("mmt_dropdown_need")
}

/// A drop-down list. The list is a set of columns (groups side by side), so it is short enough to
/// open completely inside the window: no scrolling, and it never covers its own button. It opens
/// below the button, or above it when there is no room below. Returns true when an item was chosen.
pub fn dropdown(ui: &mut egui::Ui, id: impl std::hash::Hash, shown: &str, cols: &[Col], selected: &mut String) -> bool {
    let k = c();
    let ctx = ui.ctx().clone();
    let id = ui.make_persistent_id(id);
    let width = ui.available_width();
    let (rect, resp) = ui.allocate_exact_size(vec2(width, CONTROL_H), Sense::click());
    let open = ui.memory(|m| m.is_popup_open(id));

    // the button
    let stroke = if open || resp.hovered() { Stroke::new(1.0_f32, k.accent) } else { Stroke::new(1.0_f32, k.border) };
    let painter = ui.painter();
    painter.rect(rect, 3.0, k.surface, stroke);
    let clip = Rect::from_min_max(rect.min, pos2(rect.right() - 28.0, rect.max.y));
    painter.with_clip_rect(clip).text(pos2(rect.left() + 10.0, rect.center().y), Align2::LEFT_CENTER, shown, FontId::proportional(13.5), k.text);
    let m = pos2(rect.right() - 15.0, rect.center().y);
    let tri = if open {
        vec![pos2(m.x - 4.0, m.y + 2.0), pos2(m.x + 4.0, m.y + 2.0), pos2(m.x, m.y - 3.0)]
    } else {
        vec![pos2(m.x - 4.0, m.y - 2.0), pos2(m.x + 4.0, m.y - 2.0), pos2(m.x, m.y + 3.0)]
    };
    painter.add(egui::Shape::convex_polygon(tri, k.muted, Stroke::NONE));
    if resp.clicked() {
        ui.memory_mut(|mem| mem.toggle_popup(id));
    }

    // the size of the list
    let font = FontId::proportional(13.5);
    let title_font = FontId::proportional(10.5);
    let screen = ctx.screen_rect();
    let max_w = (screen.width() - 16.0).max(160.0);
    let natural: Vec<f32> = cols
        .iter()
        .map(|col| {
            let title_w = col.title.as_ref().map_or(0.0, |t| text_width(&ctx, &t.to_uppercase(), title_font.clone()));
            col.items.iter().map(|(_, label)| text_width(&ctx, label, font.clone())).fold(title_w, f32::max) + 28.0
        })
        .collect();
    let col_h: Vec<f32> = cols.iter().map(|col| col.title.as_ref().map_or(0.0, |_| TITLE_H) + col.items.len() as f32 * ITEM_H).collect();
    // columns are packed into rows no wider than the window, from the last one: the first columns
    // (a wide one of presets) get a row of their own instead of leaving one column alone at the end
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut cur_w = 0.0;
    for (i, w) in natural.iter().enumerate().rev() {
        let joined = cur_w + COL_GAP + w;
        match rows.last_mut() {
            Some(row) if joined + 2.0 * POP_PAD <= max_w => {
                row.insert(0, i);
                cur_w = joined;
            }
            _ => {
                rows.push(vec![i]);
                cur_w = *w;
            }
        }
    }
    rows.reverse();
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
                                    ui.painter().text(
                                        pos2(r.left() + 10.0, r.center().y),
                                        Align2::LEFT_CENTER,
                                        title.to_uppercase(),
                                        title_font.clone(),
                                        k.muted,
                                    );
                                }
                                for (key, label) in &col.items {
                                    let (r, resp) = ui.allocate_exact_size(vec2(w, ITEM_H), Sense::click());
                                    if key == selected {
                                        ui.painter().rect_filled(r, 3.0, theme::mix(k.surface, k.accent, 0.45));
                                    } else if resp.hovered() {
                                        ui.painter().rect_filled(r, 3.0, k.hover);
                                    }
                                    ui.painter().text(pos2(r.left() + 10.0, r.center().y), Align2::LEFT_CENTER, label, font.clone(), k.text);
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
    chosen
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

/// Presets first (size limit tab), then the codecs grouped by hardware. Groups of GPUs that
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
    for (key, c) in list.iter().filter(|(_, c)| c.preset) {
        add("preset", key, c.label.clone());
    }
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
    groups
}

pub fn codec_combo(ui: &mut egui::Ui, id: &str, groups: &[Group], selected: &mut String, t: &T) {
    let preset_label = group_label(t, "preset");
    let shown = groups
        .iter()
        .find_map(|g| {
            g.items.iter().find(|(k, _)| k == selected).map(|(_, label)| if g.label == preset_label { label.clone() } else { format!("{label} — {}", g.label) })
        })
        .unwrap_or_else(|| selected.clone());
    let cols: Vec<Col> = groups.iter().map(|g| Col { title: Some(g.label.clone()), items: g.items.clone() }).collect();
    dropdown(ui, id, &shown, &cols, selected);
}
