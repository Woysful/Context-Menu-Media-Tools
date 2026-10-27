//! presets.toml: the built-in defaults merged with the user's own file.
//! Every ffmpeg command line of the program lives in this file, so the user can change
//! them without rebuilding anything.

use crate::media::Kind;
use crate::paths;
use crate::settings::Settings;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use toml::{Table, Value};

pub const DEFAULT_PRESETS: &str = include_str!("../presets.toml");

/// An output format (Convert Video / Audio, Extract Audio, frames, images)
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Format {
    pub text: String,
    /// a few words about the format, next to its name
    pub hint: String,
    pub ext: String,
    /// the complete ffmpeg command with placeholders
    pub command: String,
    /// name of the result, e.g. "{base}.{ext}"
    pub output: String,
    pub default: bool,
}

impl Format {
    /// What a list shows for the format with the key `key`
    pub fn label<'a>(&'a self, key: &'a str) -> &'a str {
        if self.text.is_empty() {
            key
        } else {
            &self.text
        }
    }

    /// The extension of the result: `ext`, else the key itself
    pub fn extension<'a>(&'a self, key: &'a str) -> &'a str {
        if self.ext.is_empty() {
            key
        } else {
            &self.ext
        }
    }
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

impl Codec {
    /// Value of the `q` / `p` list for the compression level (0 = low, 1 = medium, 2 = high)
    pub fn pick(list: &[Value], level: usize) -> String {
        match list.get(level).or_else(|| list.last()) {
            Some(Value::Integer(i)) => i.to_string(),
            Some(Value::Float(f)) => format!("{f}"),
            Some(Value::String(s)) => s.clone(),
            _ => "0".into(),
        }
    }

    /// The size limit of a preset ("Discord — 20 MB"); None = the user types it
    pub fn fixed_limit(&self) -> Option<f64> {
        (self.preset && self.limit > 0.0).then_some(self.limit)
    }

    /// Two-pass encoding is possible (and not replaced by the chunked mode)
    pub fn supports_twopass(&self) -> bool {
        !self.twopass.is_empty() && !self.chunked
    }
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

/// The actions that only pick an output format; each one has its own list in presets.toml
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConvertKind {
    Video,
    /// re-encode the audio tracks of a video
    VideoAudio,
    Extract,
    Frames,
    AudioFile,
    Image,
}

impl ConvertKind {
    pub const ALL: [ConvertKind; 6] =
        [ConvertKind::Video, ConvertKind::VideoAudio, ConvertKind::Extract, ConvertKind::Frames, ConvertKind::AudioFile, ConvertKind::Image];

    /// The table of presets.toml
    pub fn section(self) -> &'static str {
        match self {
            ConvertKind::Video => "video_convert",
            ConvertKind::VideoAudio => "video_audio",
            ConvertKind::Extract => "extract",
            ConvertKind::Frames => "frames",
            ConvertKind::AudioFile => "audio_file",
            ConvertKind::Image => "image",
        }
    }

    /// The files it works on
    pub fn input_kind(self) -> Kind {
        match self {
            ConvertKind::AudioFile => Kind::Audio,
            ConvertKind::Image => Kind::Image,
            _ => Kind::Video,
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

pub type Formats = Vec<(String, Format)>;

#[derive(Debug, Clone, Default)]
pub struct Config {
    pub settings: Settings,
    pub compress: Vec<(String, Codec)>,
    pub limit: Vec<(String, Codec)>,
    formats: [Formats; 6],
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

/// Write the complete built-in commands to the user's presets.toml (when it does not exist yet)
pub fn init_presets() -> Result<PathBuf, String> {
    let path = paths::presets_file();
    if !path.exists() {
        std::fs::create_dir_all(paths::config_dir()).map_err(|e| e.to_string())?;
        std::fs::write(&path, DEFAULT_PRESETS).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(path)
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
        let Some(Value::Table(own)) = section.get(&key).cloned() else { continue };
        let Some(parent) = own.get("inherit").and_then(|v| v.as_str()) else { continue };
        if let Some(Value::Table(mut merged)) = section.get(parent).cloned() {
            merge(&mut merged, &own);
            merged.remove("inherit");
            section.insert(key, Value::Table(merged));
        }
    }
}

fn section<T: DeserializeOwned>(root: &Table, name: &str, warnings: &mut Vec<String>) -> Vec<(String, T)> {
    let Some(Value::Table(t)) = root.get(name) else { return Vec::new() };
    let mut out = Vec::new();
    for (key, value) in t {
        match value.clone().try_into::<T>() {
            Ok(v) => out.push((key.clone(), v)),
            Err(e) => warnings.push(format!("[{name}.{key}]: {e}")),
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
    #[cfg(test)]
    pub fn defaults() -> Config {
        Config::from_sources(DEFAULT_PRESETS, None, Settings::default())
    }

    pub fn load() -> Config {
        let (settings, problem) = Settings::load();
        let path = paths::presets_file();
        let user = std::fs::read_to_string(&path).ok();
        let mut cfg = Config::from_sources(DEFAULT_PRESETS, user.as_deref().map(|t| (path.as_path(), t)), settings);
        cfg.warnings.extend(problem);
        cfg
    }

    pub fn from_sources(defaults: &str, user: Option<(&Path, &str)>, settings: Settings) -> Config {
        let mut warnings = Vec::new();
        let mut root: Table = defaults.parse().expect("the built-in presets.toml is valid");
        if let Some((path, text)) = user {
            match text.parse::<Table>() {
                Ok(over) => merge(&mut root, &over),
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
            formats: ConvertKind::ALL.map(|k| section(&root, k.section(), &mut warnings)),
            delete_sound: single(&root, "delete_sound", &mut warnings),
            webp: single(&root, "webp_anim", &mut warnings),
            warnings,
        };
        // A user entry that merges into something unusable must not break the program
        if cfg.compress.is_empty() || cfg.limit.is_empty() {
            let mut fallback = Config::from_sources(defaults, None, cfg.settings.clone());
            fallback.warnings = cfg.warnings;
            fallback.warnings.push("the codec lists are empty, built-in presets are used".into());
            return fallback;
        }
        cfg.check_commands();
        cfg
    }

    fn check_commands(&mut self) {
        let mut w = Vec::new();
        for (sec, list) in [("compress", &self.compress), ("limit", &self.limit)] {
            for (k, c) in list {
                check_command(&format!("[{sec}.{k}]"), &c.command, &mut w);
            }
        }
        for kind in ConvertKind::ALL {
            for (k, f) in self.formats(kind) {
                check_command(&format!("[{}.{k}]", kind.section()), &f.command, &mut w);
            }
        }
        check_command("[delete_sound]", &self.delete_sound.command, &mut w);
        check_command("[webp_anim]", &self.webp.command, &mut w);
        self.warnings.extend(w);
    }

    pub fn formats(&self, kind: ConvertKind) -> &[(String, Format)] {
        &self.formats[kind.index()]
    }

    pub fn format(&self, kind: ConvertKind, key: &str) -> Option<&Format> {
        self.formats(kind).iter().find(|(k, _)| k == key).map(|(_, f)| f)
    }

    /// The format marked `default = true`, else the first one
    pub fn default_format(&self, kind: ConvertKind) -> Option<&str> {
        let list = self.formats(kind);
        list.iter().find(|(_, f)| f.default).or_else(|| list.first()).map(|(k, _)| k.as_str())
    }

    pub fn compress_codec(&self, key: &str) -> Option<&Codec> {
        self.compress.iter().find(|(k, _)| k == key).map(|(_, c)| c)
    }

    pub fn limit_codec(&self, key: &str) -> Option<&Codec> {
        self.limit.iter().find(|(k, _)| k == key).map(|(_, c)| c)
    }
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
        assert_eq!(c.formats(ConvertKind::Video).len(), 6);
        assert_eq!(c.formats(ConvertKind::AudioFile).len(), 8);
        assert_eq!(c.formats(ConvertKind::Image).len(), 7);
        assert_eq!(c.webp.size, 720);
        assert_eq!(c.compress[0].0, "h264_cpu");
        assert_eq!(c.default_format(ConvertKind::Video), Some("mp4"));
    }

    #[test]
    fn every_default_command_is_complete() {
        let c = defaults();
        for (k, e) in c.compress.iter().chain(c.limit.iter()) {
            assert!(e.command.starts_with("ffmpeg "), "{k}");
            assert!(!e.output.is_empty(), "{k} has no output");
        }
        for kind in ConvertKind::ALL {
            for (k, f) in c.formats(kind) {
                assert!(f.command.starts_with("ffmpeg ") && !f.output.is_empty() && !f.ext.is_empty(), "{k}");
            }
        }
    }

    #[test]
    fn discord_inherits_hevc_cpu() {
        let c = defaults();
        let d = c.limit_codec("discord").unwrap();
        assert!(d.preset);
        assert_eq!(d.fixed_limit(), Some(20.0));
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
        assert_eq!(c.formats(ConvertKind::Video).last().unwrap().0, "gif");
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
    fn level_pick() {
        let list = vec![Value::Integer(22), Value::Integer(30), Value::Integer(35)];
        assert_eq!(Codec::pick(&list, 0), "22");
        assert_eq!(Codec::pick(&list, 2), "35");
        assert_eq!(Codec::pick(&list, 9), "35");
    }
}
