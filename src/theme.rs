//! Look of the windows: JetBrains Mono and one of three styles (Classic: flat and quiet;
//! Brutalist: square corners, hard lines and offset shadows; Y2K: cyber stickers, thin light
//! lines, cut corners and wide display type), with colours from the settings (dark / light /
//! the system palette) and an accent.

use crate::settings::Settings;
use crate::syspal::Palette;
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Rounding, Stroke, TextStyle, Vec2};
use std::sync::RwLock;

/// Height of every main control (buttons, drop-downs, text fields)
pub const CONTROL_H: f32 = 30.0;

/// The style of the windows (`style` of the settings)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Look {
    Classic,
    Brutal,
    Y2k,
}

impl Look {
    pub fn from_name(name: &str) -> Look {
        match name {
            "brutalist" => Look::Brutal,
            "y2k" => Look::Y2k,
            _ => Look::Classic,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Colors {
    pub look: Look,
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub hover: Color32,
    /// the frames of the controls and the cards
    pub border: Color32,
    /// the faint lines between rows
    pub rule: Color32,
    /// the shadow under buttons and lists (Brutalist: hard; Y2K: none)
    pub shadow: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub on_accent: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
}

static CURRENT: RwLock<Option<Colors>> = RwLock::new(None);

/// The colours in use (the UI code asks for them while drawing)
pub fn c() -> Colors {
    CURRENT.read().ok().and_then(|g| *g).unwrap_or_else(|| compute(&Settings::default(), None, None))
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn hex(v: u32) -> Color32 {
    Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8)
}

pub fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let f = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round().clamp(0.0, 255.0) as u8;
    Color32::from_rgb(f(a.r(), b.r()), f(a.g(), b.g()), f(a.b(), b.b()))
}

pub fn parse_hex(text: &str) -> Option<Color32> {
    let t = text.trim().trim_start_matches('#');
    if t.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(t, 16).ok()?;
    Some(hex(v))
}

pub fn to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

pub fn default_accent(look: Look, dark: bool) -> Color32 {
    hex(match (look, dark) {
        (Look::Classic, true) => 0x4c8dff,
        (Look::Classic, false) => 0x2f6bed,
        (Look::Brutal, true) => 0xd4ff00,
        (Look::Brutal, false) => 0xff4a00,
        (Look::Y2k, true) => 0x9db4ff,
        (Look::Y2k, false) => 0x3d5afe,
    })
}

fn luma(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

/// Colours from the settings. `system_dark`: what the system asks for (None = unknown);
/// `palette`: the colours of the system when `system_palette` is switched on.
pub fn compute(s: &Settings, system_dark: Option<bool>, palette: Option<Palette>) -> Colors {
    let look = Look::from_name(&s.style);
    let pal = if s.system_palette { palette } else { None };
    let dark = match (&pal, s.theme.as_str()) {
        (Some(p), _) => p.dark,
        (None, "dark") => true,
        (None, "light") => false,
        (None, _) => system_dark.unwrap_or(true),
    };
    let (bg, surface, text) = match (&pal, look, dark) {
        (Some(p), _, _) => (rgb(p.bg), rgb(p.surface), rgb(p.text)),
        (None, Look::Classic, true) => (hex(0x0f1115), hex(0x171a21), hex(0xe6e8ee)),
        (None, Look::Classic, false) => (hex(0xf4f5f7), Color32::WHITE, hex(0x14171f)),
        (None, Look::Brutal, true) => (hex(0x0c0c0c), hex(0x161616), hex(0xeeece5)),
        (None, Look::Brutal, false) => (hex(0xe7e5de), hex(0xf4f3ee), hex(0x0c0c0c)),
        (None, Look::Y2k, true) => (hex(0x121316), hex(0x0a0a0c), hex(0xeceef3)),
        (None, Look::Y2k, false) => (hex(0xd9dadf), hex(0xf3f3f5), hex(0x0c0c0e)),
    };
    let accent = match &pal {
        Some(p) => rgb(p.accent),
        None => parse_hex(&s.accent).unwrap_or_else(|| default_accent(look, dark)),
    };
    let toward = if dark { Color32::WHITE } else { Color32::BLACK };
    let base = Colors {
        look,
        dark,
        bg,
        surface,
        hover: mix(surface, toward, 0.07),
        border: mix(bg, text, 0.20),
        rule: mix(bg, text, 0.20),
        shadow: Color32::from_black_alpha(80),
        text,
        muted: mix(text, bg, 0.45),
        accent,
        on_accent: if luma(accent) > 0.6 { Color32::BLACK } else { Color32::WHITE },
        ok: hex(0x3fb96b),
        warn: hex(0xe0a13a),
        err: hex(0xe55b5b),
    };
    match look {
        Look::Classic => base,
        Look::Brutal => Colors {
            hover: mix(surface, toward, 0.08),
            border: mix(bg, text, 0.62),
            rule: mix(bg, text, 0.2),
            // a light shadow on black would glow, so the dark theme takes a grey one
            shadow: if dark { mix(bg, text, 0.3) } else { text },
            muted: mix(text, bg, 0.42),
            ok: hex(if dark { 0x3ae07a } else { 0x0b8f43 }),
            warn: hex(if dark { 0xffb020 } else { 0xb86e00 }),
            err: hex(if dark { 0xff4545 } else { 0xd02222 }),
            ..base
        },
        Look::Y2k => Colors {
            hover: mix(surface, text, 0.08),
            border: mix(bg, text, 0.78),
            rule: mix(bg, text, 0.16),
            shadow: Color32::TRANSPARENT,
            muted: mix(text, bg, 0.45),
            on_accent: if luma(accent) > 0.6 { hex(0x0a0a0c) } else { Color32::WHITE },
            ok: hex(if dark { 0x8affc1 } else { 0x0d8a4e }),
            warn: hex(if dark { 0xffd36b } else { 0xa86b00 }),
            err: hex(if dark { 0xff6b6b } else { 0xd02a2a }),
            ..base
        },
    }
}

pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
}

/// Wide and heavy (Unbounded Black): the titles and big numbers of Y2K
pub fn display_family() -> FontFamily {
    FontFamily::Name("display".into())
}

/// Square and technical (Chakra Petch Bold): the buttons and labels of Y2K
pub fn tech_family() -> FontFamily {
    FontFamily::Name("tech".into())
}

/// Bold text (JetBrains Mono Bold)
pub fn bold(text: impl Into<String>, size: f32) -> RichText {
    RichText::new(text.into()).font(FontId::new(size, bold_family()))
}

/// JetBrains Mono for everything; the fonts that egui brings are kept as a fallback for the
/// symbols JetBrains Mono does not have.
pub fn install_fonts(ctx: &egui::Context) {
    let mut f = FontDefinitions::default();
    f.font_data.insert("jbm".into(), FontData::from_static(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")));
    f.font_data.insert("jbm-bold".into(), FontData::from_static(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf")));
    let prop: Vec<String> = f.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let mono: Vec<String> = f.families.get(&FontFamily::Monospace).cloned().unwrap_or_default();
    let mut p = vec!["jbm".to_string()];
    p.extend(prop.clone());
    p.push("Hack".into());
    let mut m = vec!["jbm".to_string()];
    m.extend(mono);
    let mut b = vec!["jbm-bold".to_string(), "jbm".to_string()];
    b.extend(prop);
    b.push("Hack".into());
    f.families.insert(FontFamily::Proportional, p);
    f.families.insert(FontFamily::Monospace, m);
    f.families.insert(bold_family(), b.clone());
    f.font_data.insert("unbounded".into(), FontData::from_static(include_bytes!("../assets/fonts/Unbounded-Black-Latin.ttf")));
    f.font_data.insert("unbounded-cyr".into(), FontData::from_static(include_bytes!("../assets/fonts/Unbounded-Black-Cyrillic.ttf")));
    f.font_data.insert("chakra".into(), FontData::from_static(include_bytes!("../assets/fonts/ChakraPetch-Bold.ttf")));
    // the symbols and letters these fonts do not have come from JetBrains Mono Bold
    let mut d = vec!["unbounded".to_string(), "unbounded-cyr".to_string()];
    d.extend(b.clone());
    let mut tech = vec!["chakra".to_string()];
    tech.extend(b);
    f.families.insert(display_family(), d);
    f.families.insert(tech_family(), tech);
    ctx.set_fonts(f);
}

/// Put the colours into egui. `alpha`: opacity of the window background (1 = opaque).
pub fn apply(ctx: &egui::Context, colors: Colors, alpha: f32) {
    *CURRENT.write().unwrap() = Some(colors);
    ctx.set_theme(if colors.dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
    let k = colors;
    let mut v = if k.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let stroke = |w: f32, c: Color32| Stroke::new(w, c);
    v.panel_fill = Color32::from_rgba_unmultiplied(k.bg.r(), k.bg.g(), k.bg.b(), (alpha.clamp(0.0, 1.0) * 255.0) as u8);
    v.window_fill = k.surface;
    v.extreme_bg_color = k.surface;
    v.faint_bg_color = k.surface;
    v.code_bg_color = k.surface;
    v.hyperlink_color = k.accent;
    v.text_cursor.stroke = stroke(1.0, k.accent);
    // (rounding of the controls, stroke of the frames, stroke of hovered ones)
    let (r, frame_w, hover_w) = match k.look {
        Look::Classic => (Rounding::same(3.0), 1.0, 1.0),
        Look::Brutal => (Rounding::ZERO, 1.0, 2.0),
        Look::Y2k => (Rounding::ZERO, 1.0, 1.5),
    };
    match k.look {
        Look::Classic => {
            v.window_stroke = stroke(1.0, k.border);
            v.window_rounding = Rounding::same(6.0);
            v.menu_rounding = Rounding::same(4.0);
            v.popup_shadow = egui::epaint::Shadow { offset: Vec2::new(0.0, 4.0), blur: 12.0, spread: 0.0, color: Color32::from_black_alpha(80) };
            v.selection.bg_fill = mix(k.surface, k.accent, 0.55);
            v.selection.stroke = stroke(1.0, k.text);
        }
        Look::Brutal => {
            v.window_stroke = stroke(2.0, k.text);
            v.window_rounding = Rounding::ZERO;
            v.menu_rounding = Rounding::ZERO;
            v.window_shadow = egui::epaint::Shadow { offset: Vec2::new(6.0, 6.0), blur: 0.0, spread: 0.0, color: k.shadow };
            v.popup_shadow = egui::epaint::Shadow { offset: Vec2::new(5.0, 5.0), blur: 0.0, spread: 0.0, color: k.shadow };
            v.selection.bg_fill = k.accent;
            v.selection.stroke = stroke(1.0, k.on_accent);
            v.text_cursor.stroke = stroke(2.0, k.accent);
            v.slider_trailing_fill = true;
            v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.45 };
        }
        Look::Y2k => {
            v.window_stroke = stroke(1.5, k.border);
            v.window_rounding = Rounding::ZERO;
            v.menu_rounding = Rounding::ZERO;
            v.window_shadow = egui::epaint::Shadow::NONE;
            v.popup_shadow = egui::epaint::Shadow { offset: Vec2::new(4.0, 4.0), blur: 0.0, spread: 0.0, color: k.rule };
            v.selection.bg_fill = mix(k.surface, k.accent, 0.6);
            v.selection.stroke = stroke(1.0, k.text);
            v.slider_trailing_fill = true;
            v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.35 };
        }
    }
    if k.look != Look::Classic {
        v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    }
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = k.bg;
    w.noninteractive.weak_bg_fill = k.bg;
    w.noninteractive.bg_stroke = stroke(1.0, if k.look == Look::Classic { k.border } else { k.rule });
    w.noninteractive.fg_stroke = stroke(1.0, k.text);
    w.noninteractive.rounding = r;
    w.inactive.bg_fill = k.surface;
    w.inactive.weak_bg_fill = k.surface;
    w.inactive.bg_stroke = stroke(frame_w, k.border);
    w.inactive.fg_stroke = stroke(if k.look == Look::Brutal { 2.0 } else { 1.0 }, k.text);
    w.inactive.rounding = r;
    w.hovered.bg_fill = k.hover;
    w.hovered.weak_bg_fill = k.hover;
    w.hovered.bg_stroke = stroke(hover_w, k.accent);
    w.hovered.fg_stroke = stroke(if k.look == Look::Brutal { 2.0 } else { 1.0 }, k.text);
    w.hovered.rounding = r;
    w.active.bg_fill = if k.look == Look::Brutal { k.accent } else { mix(k.surface, k.accent, 0.35) };
    w.active.weak_bg_fill = mix(k.surface, k.accent, 0.35);
    w.active.bg_stroke = stroke(hover_w, k.accent);
    w.active.fg_stroke = stroke(if k.look == Look::Brutal { 2.0 } else { 1.0 }, k.text);
    w.active.rounding = r;
    w.open.bg_fill = k.hover;
    w.open.weak_bg_fill = k.hover;
    w.open.bg_stroke = stroke(hover_w, k.accent);
    w.open.fg_stroke = stroke(1.0, k.text);
    w.open.rounding = r;
    if k.look != Look::Classic {
        w.hovered.expansion = 0.0;
        w.active.expansion = 0.0;
    }
    ctx.set_visuals(v);
    let look = k.look;
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 7.0);
        s.spacing.button_padding = Vec2::new(12.0, 6.0);
        s.spacing.interact_size.y = CONTROL_H;
        s.spacing.combo_height = 380.0;
        // between a checkbox and its text
        s.spacing.icon_spacing = 8.0;
        let (body, small) = if look == Look::Classic { (13.5, 11.5) } else { (13.0, 11.0) };
        s.text_styles.insert(TextStyle::Body, FontId::proportional(body));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(body));
        s.text_styles.insert(TextStyle::Heading, FontId::new(18.0, bold_family()));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(small));
        s.text_styles.insert(TextStyle::Monospace, FontId::monospace(12.5));
        s.spacing.scroll = egui::style::ScrollStyle::default();
        match look {
            Look::Classic => s.spacing.slider_rail_height = 3.0,
            Look::Brutal => {
                s.spacing.slider_rail_height = 4.0;
                s.spacing.icon_width = 15.0;
                s.spacing.icon_width_inner = 7.0;
                s.spacing.icon_spacing = 9.0;
                s.spacing.scroll = egui::style::ScrollStyle::solid();
                s.spacing.scroll.bar_width = 6.0;
            }
            Look::Y2k => {
                s.spacing.slider_rail_height = 2.0;
                s.spacing.icon_width = 14.0;
                s.spacing.icon_width_inner = 6.0;
                s.spacing.scroll = egui::style::ScrollStyle::solid();
                s.spacing.scroll.bar_width = 4.0;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_roundtrip() {
        assert_eq!(parse_hex("#4c8dff"), Some(Color32::from_rgb(0x4c, 0x8d, 0xff)));
        assert_eq!(to_hex(Color32::from_rgb(1, 2, 255)), "#0102ff");
        assert_eq!(parse_hex("nope"), None);
    }

    #[test]
    fn theme_decision() {
        let mut s = Settings::default();
        assert!(compute(&s, None, None).dark); // nothing is known: dark
        assert!(!compute(&s, Some(false), None).dark); // follows the system
        s.theme = "light".into();
        assert!(!compute(&s, Some(true), None).dark);
        // the system palette wins over the theme, but only when it is switched on and exists
        let pal = Palette { dark: true, bg: [1, 2, 3], surface: [4, 5, 6], text: [250, 250, 250], accent: [9, 9, 9] };
        assert!(!compute(&s, None, Some(pal)).dark);
        s.system_palette = true;
        let c = compute(&s, None, Some(pal));
        assert!(c.dark && c.accent == Color32::from_rgb(9, 9, 9));
        assert!(!compute(&s, None, None).dark); // button unavailable -> own theme
                                                // a custom accent
        s.system_palette = false;
        s.accent = "#ff0000".into();
        assert_eq!(compute(&s, None, None).accent, Color32::from_rgb(255, 0, 0));
    }

    #[test]
    fn styles() {
        let mut s = Settings::default();
        assert_eq!(compute(&s, None, None).look, Look::Classic);
        s.style = "brutalist".into();
        assert_eq!(compute(&s, None, None).accent, default_accent(Look::Brutal, true));
        s.style = "y2k".into();
        let k = compute(&s, None, None);
        assert!(k.look == Look::Y2k && k.accent == default_accent(Look::Y2k, true));
        s.style = "unknown".into();
        assert_eq!(compute(&s, None, None).look, Look::Classic);
    }
}
