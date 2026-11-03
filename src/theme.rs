//! Look of the windows: technical brutalism. JetBrains Mono, square corners, hard lines and
//! offset shadows, colours from the settings (dark / light / the system palette) and one loud
//! accent colour.

use crate::settings::Settings;
use crate::syspal::Palette;
use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily, FontId, RichText, Rounding, Stroke, TextStyle, Vec2};
use std::sync::RwLock;

/// Height of every main control (buttons, drop-downs, text fields)
pub const CONTROL_H: f32 = 30.0;

#[derive(Clone, Copy, Debug)]
pub struct Colors {
    pub dark: bool,
    pub bg: Color32,
    pub surface: Color32,
    pub hover: Color32,
    /// the strong lines: frames of the controls and the cards
    pub border: Color32,
    /// the faint lines between the rows of a form
    pub rule: Color32,
    /// the hard shadow under buttons and lists
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
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

pub fn to_hex(c: Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r(), c.g(), c.b())
}

pub fn default_accent(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(0xd4, 0xff, 0x00)
    } else {
        Color32::from_rgb(0xff, 0x4a, 0x00)
    }
}

fn luma(c: Color32) -> f32 {
    (0.2126 * c.r() as f32 + 0.7152 * c.g() as f32 + 0.0722 * c.b() as f32) / 255.0
}

/// Colours from the settings. `system_dark`: what the system asks for (None = unknown);
/// `palette`: the colours of the system when `system_palette` is switched on.
pub fn compute(s: &Settings, system_dark: Option<bool>, palette: Option<Palette>) -> Colors {
    let pal = if s.system_palette { palette } else { None };
    let dark = match (&pal, s.theme.as_str()) {
        (Some(p), _) => p.dark,
        (None, "dark") => true,
        (None, "light") => false,
        (None, _) => system_dark.unwrap_or(true),
    };
    let (bg, surface, text) = match &pal {
        Some(p) => (rgb(p.bg), rgb(p.surface), rgb(p.text)),
        None if dark => (Color32::from_rgb(0x0c, 0x0c, 0x0c), Color32::from_rgb(0x16, 0x16, 0x16), Color32::from_rgb(0xee, 0xec, 0xe5)),
        None => (Color32::from_rgb(0xe7, 0xe5, 0xde), Color32::from_rgb(0xf4, 0xf3, 0xee), Color32::from_rgb(0x0c, 0x0c, 0x0c)),
    };
    let accent = match &pal {
        Some(p) => rgb(p.accent),
        None => parse_hex(&s.accent).unwrap_or_else(|| default_accent(dark)),
    };
    let toward = if dark { Color32::WHITE } else { Color32::BLACK };
    Colors {
        dark,
        bg,
        surface,
        hover: mix(surface, toward, 0.08),
        border: mix(bg, text, 0.62),
        rule: mix(bg, text, 0.2),
        // a light shadow on black would glow, so the dark theme takes a grey one
        shadow: if dark { mix(bg, text, 0.3) } else { text },
        text,
        muted: mix(text, bg, 0.42),
        accent,
        on_accent: if luma(accent) > 0.6 { Color32::BLACK } else { Color32::WHITE },
        ok: if dark { Color32::from_rgb(0x3a, 0xe0, 0x7a) } else { Color32::from_rgb(0x0b, 0x8f, 0x43) },
        warn: if dark { Color32::from_rgb(0xff, 0xb0, 0x20) } else { Color32::from_rgb(0xb8, 0x6e, 0x00) },
        err: if dark { Color32::from_rgb(0xff, 0x45, 0x45) } else { Color32::from_rgb(0xd0, 0x22, 0x22) },
    }
}

pub fn bold_family() -> FontFamily {
    FontFamily::Name("bold".into())
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
    f.families.insert(bold_family(), b);
    ctx.set_fonts(f);
}

/// Put the colours into egui. `alpha`: opacity of the window background (1 = opaque).
pub fn apply(ctx: &egui::Context, colors: Colors, alpha: f32) {
    *CURRENT.write().unwrap() = Some(colors);
    ctx.set_theme(if colors.dark { egui::ThemePreference::Dark } else { egui::ThemePreference::Light });
    let k = colors;
    let mut v = if k.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
    let r = Rounding::ZERO;
    let stroke = |w: f32, c: Color32| Stroke::new(w, c);
    v.panel_fill = Color32::from_rgba_unmultiplied(k.bg.r(), k.bg.g(), k.bg.b(), (alpha.clamp(0.0, 1.0) * 255.0) as u8);
    v.window_fill = k.surface;
    v.extreme_bg_color = k.surface;
    v.faint_bg_color = k.surface;
    v.code_bg_color = k.surface;
    v.window_stroke = stroke(2.0, k.text);
    v.window_rounding = r;
    v.menu_rounding = r;
    v.window_shadow = egui::epaint::Shadow { offset: Vec2::new(6.0, 6.0), blur: 0.0, spread: 0.0, color: k.shadow };
    v.popup_shadow = egui::epaint::Shadow { offset: Vec2::new(5.0, 5.0), blur: 0.0, spread: 0.0, color: k.shadow };
    v.hyperlink_color = k.accent;
    v.selection.bg_fill = k.accent;
    v.selection.stroke = stroke(1.0, k.on_accent);
    v.text_cursor.stroke = stroke(2.0, k.accent);
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.45 };
    v.interact_cursor = Some(egui::CursorIcon::PointingHand);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = k.bg;
    w.noninteractive.weak_bg_fill = k.bg;
    w.noninteractive.bg_stroke = stroke(1.0, k.rule);
    w.noninteractive.fg_stroke = stroke(1.0, k.text);
    w.noninteractive.rounding = r;
    w.inactive.bg_fill = k.surface;
    w.inactive.weak_bg_fill = k.surface;
    w.inactive.bg_stroke = stroke(1.0, k.border);
    w.inactive.fg_stroke = stroke(2.0, k.text);
    w.inactive.rounding = r;
    w.hovered.bg_fill = k.hover;
    w.hovered.weak_bg_fill = k.hover;
    w.hovered.bg_stroke = stroke(2.0, k.accent);
    w.hovered.fg_stroke = stroke(2.0, k.text);
    w.hovered.rounding = r;
    w.hovered.expansion = 0.0;
    w.active.bg_fill = k.accent;
    w.active.weak_bg_fill = mix(k.surface, k.accent, 0.35);
    w.active.bg_stroke = stroke(2.0, k.accent);
    w.active.fg_stroke = stroke(2.0, k.text);
    w.active.rounding = r;
    w.active.expansion = 0.0;
    w.open.bg_fill = k.hover;
    w.open.weak_bg_fill = k.hover;
    w.open.bg_stroke = stroke(2.0, k.accent);
    w.open.fg_stroke = stroke(2.0, k.text);
    w.open.rounding = r;
    ctx.set_visuals(v);
    ctx.style_mut(|s| {
        s.spacing.item_spacing = Vec2::new(8.0, 7.0);
        s.spacing.button_padding = Vec2::new(12.0, 6.0);
        s.spacing.interact_size.y = CONTROL_H;
        s.spacing.combo_height = 380.0;
        s.spacing.slider_rail_height = 4.0;
        s.spacing.icon_width = 15.0;
        s.spacing.icon_width_inner = 7.0;
        // between a checkbox and its text
        s.spacing.icon_spacing = 9.0;
        s.spacing.scroll = egui::style::ScrollStyle::solid();
        s.spacing.scroll.bar_width = 6.0;
        s.text_styles.insert(TextStyle::Body, FontId::proportional(13.0));
        s.text_styles.insert(TextStyle::Button, FontId::proportional(13.0));
        s.text_styles.insert(TextStyle::Heading, FontId::new(20.0, bold_family()));
        s.text_styles.insert(TextStyle::Small, FontId::proportional(11.0));
        s.text_styles.insert(TextStyle::Monospace, FontId::monospace(12.5));
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
}
