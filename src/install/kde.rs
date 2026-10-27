//! KDE (Dolphin): service menus in ~/.local/share/kio/servicemenus/mmt-*.desktop

use super::menu::{desktop_quote, Item, STATS, SUBMENUS, VIDEO};
use super::{remove_matching, set_executable};
use crate::media::Kind;
use crate::paths;
use crate::settings::Settings;
use std::fs;
use std::path::{Path, PathBuf};

fn head(mime: &str, actions: &str, extra: &str) -> String {
    format!("[Desktop Entry]\nType=Service\nX-KDE-ServiceTypes=KonqPopupMenu/Plugin\nMimeType={mime}\nActions={actions}\n{extra}X-KDE-AuthorizeAction=shell_access\n\n")
}

fn action(id: &str, name: &str, exec: &str, icon: &str) -> String {
    format!("[Desktop Action {id}]\nName={name}\nExec={exec}\nIcon={icon}\n")
}

/// All service menu files: (file name, content)
pub fn files(exe: &str, s: &Settings) -> Vec<(String, String)> {
    let exe = desktop_quote(exe);
    let top = "X-KDE-Priority=TopLevel\n";
    let mut files = Vec::new();

    // video: one file per item, all at the top level of the menu
    for it in VIDEO.iter().filter(|i| s.enabled(i.action)) {
        let body = action(it.id, it.name(), &format!("{exe} {} %F", it.action.cli_name()), it.icon);
        files.push((format!("mmt-{}.desktop", it.id), head(Kind::Video.mime(), it.id, top) + &body));
    }

    // audio / image: a submenu with one entry per format (no dialog) and Stats
    for sub in &SUBMENUS {
        let with_formats = s.enabled(sub.action);
        let with_stats = s.enabled(STATS.action);
        if !with_formats && !with_stats {
            continue;
        }
        let mut ids: Vec<String> = if with_formats { sub.formats.iter().map(|(k, _, _)| format!("convert_{k}")).collect() } else { vec![] };
        let mut body = String::new();
        if with_formats {
            for (key, text, icon) in sub.formats {
                body.push_str(&action(&format!("convert_{key}"), text, &format!("{exe} {} --yes --format {key} %F", sub.action.cli_name()), icon));
                body.push('\n');
            }
        }
        if with_stats {
            ids.push("stats".into());
            body.push_str(&stats_action(&exe, &STATS));
        }
        let content = head(sub.kind.mime(), &ids.join(";"), &format!("X-KDE-Submenu={}\n", sub.name)) + &body;
        files.push((format!("mmt-{}.desktop", sub.kind.name()), content));
    }

    // own items
    for c in s.menu_customs() {
        let id = format!("custom_{}", c.id);
        let kind = c.media_kind();
        let body = action(&id, &c.name, &format!("{exe} run {} %F", c.id), kind.icon());
        files.push((format!("mmt-custom-{}.desktop", c.id), head(kind.mime(), &id, top) + &body));
    }
    files
}

fn stats_action(exe: &str, it: &Item) -> String {
    action("stats", it.name(), &format!("{exe} stats %F"), it.icon)
}

fn dir() -> PathBuf {
    paths::xdg_data().join("kio/servicemenus")
}

pub fn install(exe: &Path, st: &Settings) -> Result<Vec<String>, String> {
    let dir = dir();
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut log = Vec::new();
    // The shell-script version of the project used multimedia-*.desktop: it would show everything twice
    for removed in remove_matching(&dir, "multimedia-", ".desktop") {
        log.push(format!("{removed} (old version)"));
    }
    let old_scripts = paths::home_dir().join(".local/share/multimedia-context-menu-scripts");
    if old_scripts.is_dir() && fs::remove_dir_all(&old_scripts).is_ok() {
        log.push(format!("removed the old {}", old_scripts.display()));
    }
    // an earlier install of our own: remove it, so items that were switched off disappear
    remove_matching(&dir, "mmt-", ".desktop");
    for (name, content) in files(&exe.to_string_lossy(), st) {
        let path = dir.join(&name);
        fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
        set_executable(&path); // Plasma 6 only runs service menus that are executable
    }
    log.push(format!("KDE (Dolphin): {}", dir.display()));
    for tool in ["kbuildsycoca6", "kbuildsycoca5"] {
        if std::process::Command::new(tool).arg("--noincremental").output().is_ok() {
            break;
        }
    }
    Ok(log)
}

pub fn uninstall() -> Vec<String> {
    remove_matching(&dir(), "mmt-", ".desktop")
}

#[cfg(test)]
mod tests {
    use super::super::tests::with_custom;
    use super::*;
    use crate::action::Action;

    #[test]
    fn files_are_complete() {
        let files = files("/home/u/.local/bin/mmt", &Settings::default());
        assert_eq!(files.len(), 10); // 8 video items + audio + image submenus
        let compress = &files.iter().find(|(n, _)| n == "mmt-01_compressVideo.desktop").unwrap().1;
        assert!(compress.contains("Exec=/home/u/.local/bin/mmt compress %F"));
        assert!(compress.contains("X-KDE-Priority=TopLevel"));
        assert!(compress.contains("Name=Compress Video"));
        let audio = &files.iter().find(|(n, _)| n == "mmt-audio.desktop").unwrap().1;
        assert!(audio.contains("X-KDE-Submenu=Convert Audio"));
        assert!(audio.contains("Exec=/home/u/.local/bin/mmt convert-audio --yes --format mp3 %F"));
        assert!(audio.contains("Actions=convert_wav;convert_mp3;"));
        assert!(audio.contains("[Desktop Action stats]\nName=Stats\nExec=/home/u/.local/bin/mmt stats %F"));
    }

    #[test]
    fn switched_off_items_and_own_items() {
        let mut s = with_custom();
        for a in [Action::DeleteSound, Action::ConvertImage, Action::Stats] {
            s.set_enabled(a, false);
        }
        let files = files("/bin/mmt", &s);
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert!(!names.contains(&"mmt-05_deleteSound.desktop"));
        assert!(!names.contains(&"mmt-08_stats.desktop"));
        assert!(!names.contains(&"mmt-image.desktop")); // no formats and no stats: no submenu at all
        assert!(names.contains(&"mmt-audio.desktop"));
        let audio = &files.iter().find(|(n, _)| n == "mmt-audio.desktop").unwrap().1;
        assert!(!audio.contains("stats"));
        let custom = &files.iter().find(|(n, _)| n == "mmt-custom-gif.desktop").unwrap().1;
        assert!(custom.contains("Exec=/bin/mmt run gif %F") && custom.contains("Name=Make GIF"));
    }
}
