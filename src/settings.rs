//! settings.toml: the settings of the program itself (the Settings window writes them),
//! including the own menu items.

use crate::media::Kind;
use crate::paths;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

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
    /// the name of the command `cmmt run <id>`; made from the name when empty
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
    pub fn media_kind(&self) -> Kind {
        Kind::from_name(&self.kind)
    }
}

/// Settings of the program. The order of the fields matters: values first, tables last.
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
    /// look for a newer version on GitHub (once a day, when the Settings window opens)
    pub check_updates: bool,
    /// what "Delete the original when done" does: trash | permanent
    pub delete_mode: String,
    pub ffmpeg: String,
    pub ffprobe: String,
    pub menu: MenuSettings,
    /// own default values of the fields of the option windows (see defaults.rs)
    pub defaults: BTreeMap<String, String>,
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
            check_updates: true,
            delete_mode: "trash".into(),
            ffmpeg: String::new(),
            ffprobe: String::new(),
            menu: MenuSettings::default(),
            defaults: BTreeMap::new(),
            custom: Vec::new(),
        }
    }
}

impl Settings {
    /// (settings, problem found in the file)
    pub fn load() -> (Settings, Option<String>) {
        let path = paths::settings_file();
        match std::fs::read_to_string(&path) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(mut s) => {
                    s.renamed_items();
                    (s, None)
                }
                Err(e) => (Settings::default(), Some(format!("{}: {}", path.display(), e.message()))),
            },
            Err(_) => (Settings::default(), None),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let path = paths::settings_file();
        let body = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        let text = format!("# Settings of Media Tools (cmmt). The Settings window (run `cmmt settings`) writes this file; you can edit it, too.\n\n{body}");
        write_creating_dir(&path, text.as_bytes())
    }

    /// Delete Audio was called Delete sound (cli name delete-sound)
    fn renamed_items(&mut self) {
        for d in &mut self.menu.disabled {
            if d == "delete-sound" {
                *d = "delete-audio".into();
            }
        }
    }

    pub fn enabled(&self, action: crate::action::Action) -> bool {
        let name = action.cli_name();
        !self.menu.disabled.iter().any(|d| d == name)
    }

    pub fn set_enabled(&mut self, action: crate::action::Action, on: bool) {
        let name = action.cli_name();
        self.menu.disabled.retain(|d| d != name);
        if !on {
            self.menu.disabled.push(name.to_string());
        }
    }

    /// The own items that can go into a menu (they have a name and an id)
    pub fn menu_customs(&self) -> impl Iterator<Item = &Custom> {
        self.custom.iter().filter(|c| !c.id.is_empty() && !c.name.is_empty())
    }

    pub fn custom(&self, id: &str) -> Option<&Custom> {
        self.custom.iter().find(|c| c.id == id)
    }
}

/// Save one own default value (None = remove it) right away: an option window has no Save
/// button. The file is read again first, so nothing else in it changes.
pub fn store_default(key: &str, value: Option<&str>) -> Result<(), String> {
    let (mut s, problem) = Settings::load();
    if let Some(p) = problem {
        return Err(p);
    }
    match value {
        Some(v) => s.defaults.insert(key.to_string(), v.to_string()),
        None => s.defaults.remove(key),
    };
    s.save()
}

/// Every own item needs a unique id (it is the name of the command `cmmt run <id>`):
/// an empty one is made from the name, a repeated one gets a number.
pub fn assign_ids(items: &mut [Custom]) {
    let mut seen: Vec<String> = Vec::new();
    for item in items.iter_mut() {
        let wanted = if item.id.is_empty() { slug(&item.name) } else { item.id.clone() };
        let mut id = wanted.clone();
        let mut n = 2;
        while seen.contains(&id) {
            id = format!("{wanted}_{n}");
            n += 1;
        }
        item.id.clone_from(&id);
        seen.push(id);
    }
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
    let out = out.trim_end_matches('_');
    if out.is_empty() {
        "item".into()
    } else {
        out.to_string()
    }
}

fn write_creating_dir(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))
}

/// The file of the export / import of own items: just a list of [[custom]] tables
#[derive(Serialize, Deserialize, Default)]
struct Bundle {
    custom: Vec<Custom>,
}

/// Write the own items to a file (a copy of the [[custom]] part of settings.toml)
pub fn export_custom(items: &[Custom], path: &Path) -> Result<usize, String> {
    let text = toml::to_string_pretty(&Bundle { custom: items.to_vec() }).map_err(|e| e.to_string())?;
    let header = "# Own menu items of Media Tools (cmmt) (Settings > Menu > Import reads this file)\n\n";
    write_creating_dir(path, format!("{header}{text}").as_bytes())?;
    Ok(items.len())
}

/// Read own items from such a file; their ids are cleared, so they never clash with existing ones
pub fn import_custom(path: &Path) -> Result<Vec<Custom>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bundle: Bundle = toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e.message()))?;
    Ok(bundle.custom.into_iter().filter(|c| !c.name.is_empty()).map(|c| Custom { id: String::new(), ..c }).collect())
}

/// The file of the export / import of the default values: a [defaults] table
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct DefaultsBundle {
    defaults: BTreeMap<String, String>,
}

pub fn export_defaults(values: &BTreeMap<String, String>, path: &Path) -> Result<usize, String> {
    let text = toml::to_string_pretty(&DefaultsBundle { defaults: values.clone() }).map_err(|e| e.to_string())?;
    let header = "# Default values of the option windows of Media Tools (cmmt) (Settings > Defaults > Import reads this file)\n\n";
    write_creating_dir(path, format!("{header}{text}").as_bytes())?;
    Ok(values.len())
}

pub fn import_defaults(path: &Path) -> Result<BTreeMap<String, String>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let bundle: DefaultsBundle = toml::from_str(&text).map_err(|e| format!("{}: {}", path.display(), e.message()))?;
    Ok(bundle.defaults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;

    #[test]
    fn settings_roundtrip_through_toml() {
        let mut s = Settings { theme: "dark".into(), ..Default::default() };
        s.set_enabled(Action::DeleteAudio, false);
        s.custom.push(Custom { id: "gif".into(), name: "GIF".into(), ..Default::default() });
        s.defaults.insert("compress.codec".into(), "h265_cpu".into());
        let text = toml::to_string_pretty(&s).unwrap();
        let back: Settings = toml::from_str(&text).unwrap();
        assert_eq!(s, back);
        assert!(!back.enabled(Action::DeleteAudio) && back.enabled(Action::Compress));
        // an old / partial file is fine
        let partial: Settings = toml::from_str("theme = \"light\"\n").unwrap();
        assert_eq!(partial.theme, "light");
        assert_eq!(partial.opacity, 0.90);
    }

    #[test]
    fn switching_an_item_on_and_off() {
        let mut s = Settings::default();
        s.set_enabled(Action::Stats, false);
        s.set_enabled(Action::Stats, false);
        assert_eq!(s.menu.disabled, ["stats"]);
        s.set_enabled(Action::Stats, true);
        assert!(s.menu.disabled.is_empty());
    }

    #[test]
    fn own_items_export_and_import() {
        let dir = std::env::temp_dir().join(format!("cmmt-export-{}", std::process::id()));
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
    fn defaults_export_and_import() {
        let dir = std::env::temp_dir().join(format!("cmmt-defaults-{}", std::process::id()));
        let file = dir.join("defaults.toml");
        let values: BTreeMap<String, String> = [("limit.size", "8"), ("compress.codec", "h265_cpu")].map(|(k, v)| (k.into(), v.into())).into();
        assert_eq!(export_defaults(&values, &file).unwrap(), 2);
        assert_eq!(import_defaults(&file).unwrap(), values);
        std::fs::write(&file, "defaults = 3").unwrap();
        assert!(import_defaults(&file).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_old_name_of_delete_audio_is_switched_off_too() {
        let mut s: Settings = toml::from_str("[menu]\ndisabled = [\"delete-sound\"]\n").unwrap();
        s.renamed_items();
        assert!(!s.enabled(Action::DeleteAudio));
    }

    #[test]
    fn ids_are_made_unique() {
        let mut items = vec![
            Custom { name: "Make GIF".into(), ..Default::default() },
            Custom { name: "make gif".into(), ..Default::default() },
            Custom { id: "keep".into(), name: "x".into(), ..Default::default() },
        ];
        assign_ids(&mut items);
        let ids: Vec<&str> = items.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, ["make_gif", "make_gif_2", "keep"]);
    }

    #[test]
    fn slugs() {
        assert_eq!(slug("My GIF maker!"), "my_gif_maker");
        assert_eq!(slug("Сжать"), "item");
    }
}
