//! presets.toml: the built-in defaults merged with the user's own file.
//! Every ffmpeg command line of the program lives in this file, so the user can change
//! them without rebuilding anything.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use toml::{Table, Value};

pub const DEFAULT_PRESETS: &str = include_str!("../presets.toml");

/// An output format (Convert Video / Audio, Extract Audio, frames, images)
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Format {
    pub text: String,
    pub ext: String,
    /// the complete ffmpeg command with placeholders
    pub command: String,
    /// name of the result, e.g. "{base}.{ext}"
    pub output: String,
    pub default: bool,
}

/// A video codec entry, used by both Compress Video tabs
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Codec {
    pub label: String,
    pub group: String,
    pub backend: String,
    pub vendor: String,
    pub command: String,
    pub output: String,
    pub q: Vec<Value>,
    pub p: Vec<Value>,
    // size limit mode
    pub rc_max: f64,
    pub rc_buf: f64,
    pub aim: f64,
    pub bpp_min: f64,
    pub twopass: String,
    pub chunked: bool,
    pub preset: bool,
    pub limit: f64,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct DeleteSound {
    pub command: String,
    pub output: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct WebpAnim {
    pub size: u32,
    pub fps: u32,
    pub quality: u32,
    pub compression: u32,
    pub command: String,
    pub output: String,
}

/// The items of the menu that the user switched off
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(default)]
pub struct MenuSettings {
    pub disabled: Vec<String>,
}

/// An own menu item: any ffmpeg command for the files of one kind
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Custom {
    pub id: String,
    pub name: String,
    /// video | audio | image
    pub kind: String,
    pub command: String,
    pub output: String,
}

impl Default for Custom {
    fn default() -> Self {
        Custom {
            id: String::new(),
            name: String::new(),
            kind: "video".into(),
            command: "ffmpeg -n -i {input} -c:v copy {output}".into(),
            output: "{base}_custom.{srcext}".into(),
        }
    }
}

impl Custom {
    pub fn media_kind(&self) -> crate::media::Kind {
        match self.kind.as_str() {
            "audio" => crate::media::Kind::Audio,
            "image" => crate::media::Kind::Image,
            _ => crate::media::Kind::Video,
        }
    }
}

/// The file of the export / import of own items: just a list of [[custom]] tables
#[derive(Serialize, Deserialize, Default)]
struct Bundle {
    custom: Vec<Custom>,
}

/// Write the own items to a file (a copy of the [[custom]] part of settings.toml)
pub fn export_custom(items: &[Custom], path: &Path) -> Result<usize, String> {
    let text = toml::to_string_pretty(&Bundle { custom: items.to_vec() }).map_err(|e| e.to_string())?;
    let header = "# Own menu items of mmt (Settings > Menu > Import reads this file)\n\n";
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            let _ = std::fs::create_dir_all(dir);
        }
    }
    std::fs::write(path, format!("{header}{text}")).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(items.len())
}

/// Read own items from such a file; their ids are cleared, so they never clash with existing ones
pub fn import_custom(path: &Path) -> Result<Vec<Custom>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bundle: Bundle = toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e.message()))?;
    Ok(bundle
        .custom
        .into_iter()
        .filter(|c| !c.name.is_empty())
        .map(|mut c| {
            c.id.clear();
            c
        })
        .collect())
}

/// "My GIF maker!" -> "my_gif_maker"
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('_') && !out.is_empty() {
            out.push('_');
        }
    }
    let out = out.trim_end_matches('_').to_string();
    if out.is_empty() {
        "item".into()
    } else {
        out
    }
}

/// Settings of the program itself (the Settings window writes them to settings.toml).
/// The order of the fields matters: values first, tables last.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// auto | dark | light
    pub theme: String,
    /// take the colours of the system (KDE, GNOME, Windows) where they can be read
    pub system_palette: bool,
    /// "#rrggbb"; empty = the default accent
    pub accent: String,
    /// auto | on | off
    pub transparency: String,
    pub opacity: f64,
    pub ui_scale: f64,
    /// seconds until the window of a successful job closes (0 = stays open)
    pub auto_close: f64,
    pub show_all_gpus: bool,
    /// GNOME: "01 Compress Video" ... instead of plain names (Files sorts scripts by name)
    pub numbered_scripts: bool,
    pub ffmpeg: String,
    pub ffprobe: String,
    pub menu: MenuSettings,
    pub custom: Vec<Custom>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: "auto".into(),
            system_palette: false,
            accent: String::new(),
            transparency: "auto".into(),
            opacity: 0.90,
            ui_scale: 1.0,
            auto_close: 4.0,
            show_all_gpus: false,
            numbered_scripts: false,
            ffmpeg: String::new(),
            ffprobe: String::new(),
            menu: MenuSettings::default(),
            custom: Vec::new(),
        }
    }
}

impl Settings {
    /// (settings, problem found in the file)
    pub fn load() -> (Settings, Option<String>) {
        let path = settings_file();
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(s) => (s, None),
                Err(e) => (Settings::default(), Some(format!("{}: {}", path.display(), e.message()))),
            },
            Err(_) => (Settings::default(), None),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = settings_file();
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        let body = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let text = format!("# Settings of mmt. The Settings window (run `mmt settings`) writes this file; you can edit it, too.\n\n{body}");
        std::fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn enabled(&self, action: &str) -> bool {
        !self.menu.disabled.iter().any(|d| d == action)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub settings: Settings,
    pub compress: Vec<(String, Codec)>,
    pub limit: Vec<(String, Codec)>,
    pub video_convert: Vec<(String, Format)>,
    pub video_audio: Vec<(String, Format)>,
    pub extract: Vec<(String, Format)>,
    pub frames: Vec<(String, Format)>,
    pub audio_file: Vec<(String, Format)>,
    pub image: Vec<(String, Format)>,
    pub delete_sound: DeleteSound,
    pub webp: WebpAnim,
    /// Problems found in the user's files (shown in the window; a broken file is ignored)
    pub warnings: Vec<String>,
}

pub fn current_os() -> &'static str {
    if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux"
    }
}

/// Directory of the user's settings (created on demand)
pub fn config_dir() -> PathBuf {
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("mmt");
        }
    }
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        if !xdg.is_empty() {
            return PathBuf::from(xdg).join("mmt");
        }
    }
    home_dir().join(".config").join("mmt")
}

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn user_file() -> PathBuf {
    config_dir().join("presets.toml")
}

/// Write the complete built-in commands to the user's presets.toml (when it does not exist yet)
pub fn init_presets() -> Result<PathBuf, String> {
    let path = user_file();
    if !path.exists() {
        std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
        std::fs::write(&path, DEFAULT_PRESETS).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(path)
}

pub fn settings_file() -> PathBuf {
    config_dir().join("settings.toml")
}

/// Merge `over` into `base`: tables are merged key by key, everything else is replaced
fn merge(base: &mut Table, over: &Table) {
    for (key, value) in over {
        match (base.get_mut(key), value) {
            (Some(Value::Table(b)), Value::Table(o)) => merge(b, o),
            _ => {
                base.insert(key.clone(), value.clone());
            }
        }
    }
}

/// `[entry.windows]` / `[entry.linux]` sub-tables override the entry on that system only
fn apply_os_overrides(section: &mut Table) {
    let os = current_os();
    for (_, entry) in section.iter_mut() {
        if let Value::Table(t) = entry {
            let mut chosen: Option<Table> = None;
            for name in ["windows", "linux", "macos"] {
                if let Some(Value::Table(sub)) = t.remove(name) {
                    if name == os {
                        chosen = Some(sub);
                    }
                }
            }
            if let Some(sub) = chosen {
                merge(t, &sub);
            }
        }
    }
}

/// `inherit = "other"` starts from a copy of another entry of the same section
fn apply_inherit(section: &mut Table) {
    let keys: Vec<String> = section.keys().cloned().collect();
    for key in keys {
        let parent = match section.get(&key) {
            Some(Value::Table(t)) => t.get("inherit").and_then(|v| v.as_str()).map(str::to_string),
            _ => None,
        };
        if let Some(parent) = parent {
            if let Some(Value::Table(base)) = section.get(&parent).cloned() {
                let mut merged = base;
                if let Some(Value::Table(own)) = section.get(&key) {
                    merge(&mut merged, own);
                }
                merged.remove("inherit");
                section.insert(key, Value::Table(merged));
            }
        }
    }
}

fn section<T: DeserializeOwned>(root: &Table, name: &str, warnings: &mut Vec<String>) -> Vec<(String, T)> {
    let mut out = Vec::new();
    if let Some(Value::Table(t)) = root.get(name) {
        for (key, value) in t {
            match value.clone().try_into::<T>() {
                Ok(v) => out.push((key.clone(), v)),
                Err(e) => warnings.push(format!("[{name}.{key}]: {e}")),
            }
        }
    }
    out
}

fn single<T: DeserializeOwned + Default>(root: &Table, name: &str, warnings: &mut Vec<String>) -> T {
    match root.get(name) {
        Some(v) => v.clone().try_into::<T>().unwrap_or_else(|e| {
            warnings.push(format!("[{name}]: {e}"));
            T::default()
        }),
        None => T::default(),
    }
}

/// A command must be a valid command line with {input} and {output}
fn check_command(what: &str, command: &str, warnings: &mut Vec<String>) {
    match crate::cmdline::placeholders(command) {
        Ok(p) => {
            for need in ["input", "output"] {
                if !p.iter().any(|x| x == need) {
                    warnings.push(format!("{what}: the command has no {{{need}}}"));
                }
            }
        }
        Err(e) => warnings.push(format!("{what}: {e}")),
    }
}

impl Config {
    /// Built-in defaults only
    #[allow(dead_code)]
    pub fn defaults() -> Config {
        Config::from_sources(DEFAULT_PRESETS, None, Settings::default())
    }

    pub fn load() -> Config {
        let (settings, problem) = Settings::load();
        let path = user_file();
        let user = std::fs::read_to_string(&path).ok().map(|text| (path, text));
        let mut cfg = Config::from_sources(DEFAULT_PRESETS, user.as_ref().map(|(p, t)| (p.as_path(), t.as_str())), settings);
        cfg.warnings.extend(problem);
        cfg
    }

    pub fn from_sources(defaults: &str, user: Option<(&Path, &str)>, settings: Settings) -> Config {
        let mut warnings = Vec::new();
        let mut root: Table = defaults.parse().expect("the built-in presets.toml is valid");
        if let Some((path, text)) = user {
            match text.parse::<Table>() {
                Ok(over) => {
                    let mut candidate = root.clone();
                    merge(&mut candidate, &over);
                    root = candidate;
                }
                Err(e) => warnings.push(format!("{}: {}", path.display(), e.message())),
            }
        }
        for name in ["compress", "limit"] {
            if let Some(Value::Table(t)) = root.get_mut(name) {
                apply_os_overrides(t);
                apply_inherit(t);
            }
        }
        let mut cfg = Config {
            settings,
            compress: section(&root, "compress", &mut warnings),
            limit: section(&root, "limit", &mut warnings),
            video_convert: section(&root, "video_convert", &mut warnings),
            video_audio: section(&root, "video_audio", &mut warnings),
            extract: section(&root, "extract", &mut warnings),
            frames: section(&root, "frames", &mut warnings),
            audio_file: section(&root, "audio_file", &mut warnings),
            image: section(&root, "image", &mut warnings),
            delete_sound: single(&root, "delete_sound", &mut warnings),
            webp: single(&root, "webp_anim", &mut warnings),
            warnings,
        };
        // A user entry that merges into something unusable must not break the program
        if cfg.compress.is_empty() || cfg.limit.is_empty() {
            let mut fallback = Config::from_sources(defaults, None, cfg.settings.clone());
            fallback.warnings = cfg.warnings;
            fallback.warnings.push("the codec lists are empty, built-in presets are used".into());
            cfg = fallback;
        }
        let mut w = Vec::new();
        for (k, c) in &cfg.compress {
            check_command(&format!("[compress.{k}]"), &c.command, &mut w);
        }
        for (k, c) in &cfg.limit {
            check_command(&format!("[limit.{k}]"), &c.command, &mut w);
        }
        for (sec, list) in [
            ("video_convert", &cfg.video_convert),
            ("video_audio", &cfg.video_audio),
            ("extract", &cfg.extract),
            ("frames", &cfg.frames),
            ("audio_file", &cfg.audio_file),
            ("image", &cfg.image),
        ] {
            for (k, f) in list {
                check_command(&format!("[{sec}.{k}]"), &f.command, &mut w);
            }
        }
        check_command("[delete_sound]", &cfg.delete_sound.command, &mut w);
        check_command("[webp_anim]", &cfg.webp.command, &mut w);
        cfg.warnings.extend(w);
        cfg
    }

    pub fn compress_codec(&self, key: &str) -> Option<&Codec> {
        self.compress.iter().find(|(k, _)| k == key).map(|(_, c)| c)
    }

    pub fn limit_codec(&self, key: &str) -> Option<&Codec> {
        self.limit.iter().find(|(k, _)| k == key).map(|(_, c)| c)
    }
}

impl Codec {
    /// Value of the `q` / `p` list for the compression level (0 = low, 1 = medium, 2 = high)
    pub fn pick(list: &[Value], level: usize) -> String {
        let v = list.get(level).or_else(|| list.last());
        match v {
            Some(Value::Integer(i)) => i.to_string(),
            Some(Value::Float(f)) => format!("{f}"),
            Some(Value::String(s)) => s.clone(),
            _ => "0".into(),
        }
    }
}

/// ffmpeg / ffprobe: the configured path, then next to the program, then PATH
pub fn find_tool(name: &str, configured: &str) -> Option<PathBuf> {
    let file = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
    if !configured.is_empty() {
        let p = PathBuf::from(configured);
        if p.is_file() {
            return Some(p);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let p = dir.join(&file);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let p = dir.join(&file);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> Config {
        Config::defaults()
    }

    #[test]
    fn defaults_parse_without_warnings() {
        let c = defaults();
        assert!(c.warnings.is_empty(), "{:?}", c.warnings);
        assert_eq!(c.compress.len(), 14);
        assert_eq!(c.limit.len(), 14);
        assert_eq!(c.video_convert.len(), 6);
        assert_eq!(c.audio_file.len(), 8);
        assert_eq!(c.image.len(), 7);
        assert_eq!(c.webp.size, 720);
        assert_eq!(c.compress[0].0, "h264_cpu");
    }

    #[test]
    fn every_default_command_is_complete() {
        let c = defaults();
        for (k, e) in c.compress.iter().chain(c.limit.iter()) {
            assert!(e.command.starts_with("ffmpeg "), "{k}");
            assert!(!e.output.is_empty(), "{k} has no output");
        }
        for (k, f) in c.video_convert.iter().chain(&c.video_audio).chain(&c.extract).chain(&c.frames).chain(&c.audio_file).chain(&c.image) {
            assert!(f.command.starts_with("ffmpeg ") && !f.output.is_empty() && !f.ext.is_empty(), "{k}");
        }
    }

    #[test]
    fn discord_inherits_hevc_cpu() {
        let c = defaults();
        let d = c.limit_codec("discord").unwrap();
        assert!(d.preset);
        assert_eq!(d.limit, 20.0);
        assert!(d.command.contains("libx265"));
        assert_eq!(d.output, "{base}_discord.mp4");
        assert_eq!(d.twopass, "x265");
    }

    #[test]
    fn os_override_is_applied() {
        let c = defaults();
        let amd = c.compress_codec("h264_amd").unwrap();
        if cfg!(windows) {
            assert_eq!(amd.backend, "amf");
            assert!(amd.command.contains("h264_amf"));
        } else {
            assert_eq!(amd.backend, "vaapi");
            assert!(amd.command.contains("{render_node}"));
        }
    }

    #[test]
    fn user_file_overrides_and_extends() {
        let user = r#"
            [compress.h264_cpu]
            q = [18, 24, 30]
            [video_convert.gif]
            text = "GIF"
            ext = "gif"
            command = "ffmpeg -n -i {input} -an {output}"
            output = "{base}.{ext}"
        "#;
        let c = Config::from_sources(DEFAULT_PRESETS, Some((Path::new("u.toml"), user)), Settings::default());
        assert!(c.warnings.is_empty(), "{:?}", c.warnings);
        let h = c.compress_codec("h264_cpu").unwrap();
        assert_eq!(Codec::pick(&h.q, 1), "24");
        assert!(h.command.contains("libx264"));
        assert_eq!(c.video_convert.last().unwrap().0, "gif");
    }

    #[test]
    fn broken_user_file_is_ignored_with_a_warning() {
        let c = Config::from_sources(DEFAULT_PRESETS, Some((Path::new("u.toml"), "this is [not toml")), Settings::default());
        assert_eq!(c.warnings.len(), 1);
        assert_eq!(c.compress.len(), 14);
    }

    #[test]
    fn a_command_without_output_is_reported() {
        let user = "[compress.h264_cpu]\ncommand = \"ffmpeg -n -i {input} -c:v libx264\"\n";
        let c = Config::from_sources(DEFAULT_PRESETS, Some((Path::new("u.toml"), user)), Settings::default());
        assert!(c.warnings.iter().any(|w| w.contains("compress.h264_cpu") && w.contains("{output}")), "{:?}", c.warnings);
    }

    #[test]
    fn settings_roundtrip_through_toml() {
        let mut s = Settings::default();
        s.theme = "dark".into();
        s.menu.disabled = vec!["delete-sound".into()];
        s.custom.push(Custom { id: "gif".into(), name: "GIF".into(), ..Default::default() });
        let text = toml::to_string_pretty(&s).unwrap();
        let back: Settings = toml::from_str(&text).unwrap();
        assert_eq!(s, back);
        assert!(!back.enabled("delete-sound") && back.enabled("compress"));
        // an old / partial file is fine
        let partial: Settings = toml::from_str("theme = \"light\"\n").unwrap();
        assert_eq!(partial.theme, "light");
        assert_eq!(partial.opacity, 0.90);
    }

    #[test]
    fn own_items_export_and_import() {
        let dir = std::env::temp_dir().join(format!("mmt-export-{}", std::process::id()));
        let file = dir.join("items.toml");
        let items = vec![
            Custom { id: "a".into(), name: "Make GIF".into(), command: "ffmpeg -n -i {input} {output}".into(), ..Default::default() },
            Custom { id: "b".into(), name: "Quiet".into(), kind: "audio".into(), ..Default::default() },
        ];
        assert_eq!(export_custom(&items, &file).unwrap(), 2);
        let back = import_custom(&file).unwrap();
        assert_eq!(back.len(), 2);
        assert!(back.iter().all(|c| c.id.is_empty()));
        assert_eq!(back[0].name, "Make GIF");
        assert_eq!(back[1].kind, "audio");
        assert!(import_custom(&dir.join("missing.toml")).is_err());
        std::fs::write(&file, "custom = 5").unwrap();
        assert!(import_custom(&file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("My GIF maker!"), "my_gif_maker");
        assert_eq!(slug("Сжать"), "item");
    }

    #[test]
    fn level_pick() {
        let list = vec![Value::Integer(22), Value::Integer(30), Value::Integer(35)];
        assert_eq!(Codec::pick(&list, 0), "22");
        assert_eq!(Codec::pick(&list, 2), "35");
        assert_eq!(Codec::pick(&list, 9), "35");
    }
}
