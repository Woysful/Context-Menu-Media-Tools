//! The colours of the system, where they can be read: KDE (kdeglobals), GNOME (gsettings),
//! Windows (registry). Elsewhere (XFCE, ...) there is no palette and None is returned.

use crate::paths;
use crate::proc::output_of as run;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Palette {
    pub dark: bool,
    pub bg: [u8; 3],
    pub surface: [u8; 3],
    pub text: [u8; 3],
    pub accent: [u8; 3],
}

fn luminance(c: [u8; 3]) -> f32 {
    (0.2126 * c[0] as f32 + 0.7152 * c[1] as f32 + 0.0722 * c[2] as f32) / 255.0
}

fn rgb3(text: &str) -> Option<[u8; 3]> {
    let v: Vec<u8> = text.split(',').filter_map(|p| p.trim().parse().ok()).collect();
    (v.len() >= 3).then(|| [v[0], v[1], v[2]])
}

/// Value of `key` in `[section]` of an ini file
fn ini_value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut inside = false;
    for line in text.lines() {
        let l = line.trim();
        if l.starts_with('[') {
            inside = l == format!("[{section}]");
        } else if inside {
            if let Some((k, v)) = l.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

pub fn parse_kdeglobals(text: &str) -> Option<Palette> {
    let bg = ini_value(text, "Colors:Window", "BackgroundNormal").and_then(|v| rgb3(&v))?;
    let surface = ini_value(text, "Colors:View", "BackgroundNormal").and_then(|v| rgb3(&v)).unwrap_or(bg);
    let fg = ini_value(text, "Colors:Window", "ForegroundNormal").and_then(|v| rgb3(&v)).unwrap_or([128, 128, 128]);
    // Plasma 6 has an accent colour of its own; older versions use the selection colour
    let accent = ini_value(text, "General", "AccentColor")
        .and_then(|v| rgb3(&v))
        .or_else(|| ini_value(text, "Colors:Selection", "BackgroundNormal").and_then(|v| rgb3(&v)))
        .unwrap_or([61, 174, 233]);
    Some(Palette { dark: luminance(bg) < 0.5, bg, surface, text: fg, accent })
}

fn gnome_accent(name: &str) -> [u8; 3] {
    match name.trim_matches('\'') {
        "teal" => [0x21, 0x90, 0xa4],
        "green" => [0x3a, 0x94, 0x4a],
        "yellow" => [0xc8, 0x88, 0x00],
        "orange" => [0xed, 0x5b, 0x00],
        "red" => [0xe6, 0x2d, 0x42],
        "pink" => [0xd5, 0x61, 0x99],
        "purple" => [0x91, 0x41, 0xac],
        "slate" => [0x6f, 0x83, 0x96],
        _ => [0x35, 0x84, 0xe4],
    }
}

/// libadwaita colours: `scheme` is the value of color-scheme, `accent` of accent-color
pub fn gnome_palette(scheme: &str, accent: &str) -> Palette {
    let dark = scheme.contains("dark");
    if dark {
        Palette { dark, bg: [0x24, 0x24, 0x24], surface: [0x30, 0x30, 0x30], text: [0xff, 0xff, 0xff], accent: gnome_accent(accent) }
    } else {
        Palette { dark, bg: [0xfa, 0xfa, 0xfa], surface: [0xff, 0xff, 0xff], text: [0x24, 0x24, 0x24], accent: gnome_accent(accent) }
    }
}

/// `reg query` prints "    AppsUseLightTheme    REG_DWORD    0x0": the number after 0x
fn reg_dword(output: &str, name: &str) -> Option<u32> {
    for line in output.lines() {
        if line.trim_start().starts_with(name) {
            let hex = line.split("0x").nth(1)?.trim();
            return u32::from_str_radix(hex, 16).ok();
        }
    }
    None
}

pub fn windows_palette(personalize: &str, dwm: &str) -> Option<Palette> {
    let light = reg_dword(personalize, "AppsUseLightTheme")?;
    // AccentColor is 0xAABBGGRR
    let accent = reg_dword(dwm, "AccentColor").map_or([0x00, 0x78, 0xd4], |v| [(v & 0xff) as u8, ((v >> 8) & 0xff) as u8, ((v >> 16) & 0xff) as u8]);
    Some(if light == 0 {
        Palette { dark: true, bg: [0x20, 0x20, 0x20], surface: [0x2b, 0x2b, 0x2b], text: [0xff, 0xff, 0xff], accent }
    } else {
        Palette { dark: false, bg: [0xf3, 0xf3, 0xf3], surface: [0xff, 0xff, 0xff], text: [0x1b, 0x1b, 0x1b], accent }
    })
}

fn desktop() -> String {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase()
}

/// Does this system have a palette that can be read? (the Settings window enables its button by this)
pub fn detect() -> Option<Palette> {
    if cfg!(windows) {
        let p = run("reg", &["query", r"HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize", "/v", "AppsUseLightTheme"])?;
        let d = run("reg", &["query", r"HKCU\Software\Microsoft\Windows\DWM", "/v", "AccentColor"]).unwrap_or_default();
        return windows_palette(&p, &d);
    }
    let desk = desktop();
    if desk.contains("kde") {
        return parse_kdeglobals(&std::fs::read_to_string(paths::xdg_config().join("kdeglobals")).ok()?);
    }
    if desk.contains("gnome") || desk.contains("unity") {
        let scheme = run("gsettings", &["get", "org.gnome.desktop.interface", "color-scheme"])?;
        let accent = run("gsettings", &["get", "org.gnome.desktop.interface", "accent-color"]).unwrap_or_default();
        return Some(gnome_palette(scheme.trim(), accent.trim()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kde_colours() {
        let text = "[Colors:Window]\nBackgroundNormal=35,38,41\nForegroundNormal=252,252,252\n[Colors:View]\nBackgroundNormal=27,30,32\n[General]\nAccentColor=61,174,233\nColorScheme=BreezeDark\n";
        let p = parse_kdeglobals(text).unwrap();
        assert!(p.dark);
        assert_eq!(p.bg, [35, 38, 41]);
        assert_eq!(p.surface, [27, 30, 32]);
        assert_eq!(p.accent, [61, 174, 233]);
        let light = "[Colors:Window]\nBackgroundNormal=239,240,241\n[Colors:Selection]\nBackgroundNormal=1,2,3\n";
        let p = parse_kdeglobals(light).unwrap();
        assert!(!p.dark);
        assert_eq!(p.accent, [1, 2, 3]);
        assert!(parse_kdeglobals("[General]\nx=1\n").is_none());
    }

    #[test]
    fn gnome_colours() {
        assert!(gnome_palette("'prefer-dark'", "'green'").dark);
        assert_eq!(gnome_palette("'default'", "'red'").accent, [0xe6, 0x2d, 0x42]);
        assert!(!gnome_palette("'default'", "").dark);
    }

    #[test]
    fn windows_colours() {
        let p = "    AppsUseLightTheme    REG_DWORD    0x0\n";
        let d = "    AccentColor    REG_DWORD    0xffd77800\n";
        let pal = windows_palette(p, d).unwrap();
        assert!(pal.dark);
        assert_eq!(pal.accent, [0x00, 0x78, 0xd7]);
        assert!(!windows_palette("    AppsUseLightTheme    REG_DWORD    0x1\n", "").unwrap().dark);
        assert!(windows_palette("nothing", d).is_none());
    }
}
