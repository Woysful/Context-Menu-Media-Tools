//! PCManFM (Raspberry Pi OS, LXDE, LXQt): libfm "file manager actions" in
//! ~/.local/share/file-manager/actions/mmt-*.desktop (flat entries: PCManFM has no
//! dialog-less submenus we could rely on)

use super::menu::{desktop_quote, flat_entries};
use super::remove_matching;
use crate::paths;
use crate::settings::Settings;
use std::fs;
use std::path::{Path, PathBuf};

pub fn files(exe: &str, s: &Settings) -> Vec<(String, String)> {
    let exe = desktop_quote(exe);
    flat_entries(s)
        .into_iter()
        .map(|e| {
            let mime: Vec<&str> = e.kinds.iter().map(|k| k.mime()).collect();
            let content = format!(
                "[Desktop Entry]\nType=Action\nName={}\nIcon={}\nProfiles=on-all;\n\n[X-Action-Profile on-all]\nMimeTypes={}\nExec={exe} {} %F\nName=Default profile\n",
                e.name,
                e.icon,
                mime.join(";"),
                e.args
            );
            (format!("mmt-{}.desktop", e.file_id), content)
        })
        .collect()
}

fn dir() -> PathBuf {
    paths::xdg_data().join("file-manager/actions")
}

pub fn install(exe: &Path, st: &Settings) -> Result<Vec<String>, String> {
    let dir = dir();
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    remove_matching(&dir, "mmt-", ".desktop");
    for (name, content) in files(&exe.to_string_lossy(), st) {
        let path = dir.join(&name);
        fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(vec![format!("PCManFM (Raspberry Pi OS / LXDE / LXQt): {}  (restart the file manager: pcmanfm -q)", dir.display())])
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
    fn actions_are_flat_entries_with_mime_types() {
        let files = files("/bin/mmt", &Settings::default());
        let compress = &files.iter().find(|(n, _)| n == "mmt-01_compressVideo.desktop").unwrap().1;
        assert!(compress.starts_with("[Desktop Entry]\nType=Action\n"));
        assert!(compress.contains("Profiles=on-all;"));
        assert!(compress.contains("[X-Action-Profile on-all]\nMimeTypes=video/mp4;"));
        assert!(compress.contains("Exec=/bin/mmt compress %F"));
        let audio = &files.iter().find(|(n, _)| n == "mmt-convertAudio.desktop").unwrap().1;
        assert!(audio.contains("audio/mpeg") && audio.contains("video/mp4"));
    }

    #[test]
    fn follow_the_settings() {
        let mut s = with_custom();
        s.set_enabled(Action::DeleteSound, false);
        let p = files("/bin/mmt", &s);
        assert!(p.iter().any(|(n, _)| n == "mmt-custom-gif.desktop"));
        assert!(!p.iter().any(|(n, _)| n == "mmt-05_deleteSound.desktop"));
    }
}
