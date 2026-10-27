//! GNOME (Files): small sh scripts in ~/.local/share/nautilus/scripts/Media Tools/

use super::menu::sh_quote;
use super::set_executable;
use crate::action::Action;
use crate::i18n;
use crate::paths;
use crate::settings::Settings;
use std::fs;
use std::path::{Path, PathBuf};

/// (file name, content). Files has no file type filter for scripts, so the program itself
/// picks what fits ("Convert Audio" works for audio files and for videos). Files sorts the
/// scripts by name: with `numbered_scripts` the names start with 01, 02 ... to keep the order.
pub fn files(exe: &str, s: &Settings) -> Vec<(String, String)> {
    let mut entries: Vec<(String, String)> = Vec::new(); // (name, arguments)
    for action in Action::ALL.into_iter().filter(|a| s.enabled(*a)) {
        entries.push((action.title(i18n::t()).to_string(), action.cli_name().to_string()));
    }
    for c in s.menu_customs() {
        entries.push((c.name.clone(), format!("run {}", sh_quote(&c.id))));
    }
    entries
        .iter()
        .enumerate()
        .map(|(i, (name, args))| {
            let file = if s.numbered_scripts { format!("{:02} {name}", i + 1) } else { name.clone() };
            (file, format!("#!/bin/sh\n# Media Tools: {name}\nexec {} {args} \"$@\"\n", sh_quote(exe)))
        })
        .collect()
}

fn dir() -> PathBuf {
    paths::xdg_data().join("nautilus/scripts/Media Tools")
}

pub fn install(exe: &Path, st: &Settings) -> Result<Vec<String>, String> {
    let dir = dir();
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir); // replace our own folder, and only that
    }
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, content) in files(&exe.to_string_lossy(), st) {
        let path = dir.join(name);
        fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
        set_executable(&path);
    }
    Ok(vec![format!("GNOME (Files): right-click > Scripts > Media Tools  ({})", dir.display())])
}

pub fn uninstall() -> Vec<String> {
    let dir = dir();
    if dir.is_dir() && fs::remove_dir_all(&dir).is_ok() {
        vec![format!("removed {}", dir.display())]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::with_custom;
    use super::*;

    #[test]
    fn scripts_pass_the_selection_and_numbers_are_optional() {
        let files = files("/opt/my mmt/mmt", &Settings::default());
        assert_eq!(files.len(), 9);
        assert_eq!(files[0].0, "Compress Video"); // plain names by default
        assert_eq!(files[7].0, "Convert Image");
        assert_eq!(files[8].0, "Stats");
        assert!(files[0].1.contains("exec '/opt/my mmt/mmt' compress \"$@\""));
        assert!(files[0].1.starts_with("#!/bin/sh\n"));
        let s = Settings { numbered_scripts: true, ..Default::default() };
        assert_eq!(super::files("/bin/mmt", &s)[0].0, "01 Compress Video");
    }

    #[test]
    fn follow_the_settings() {
        let mut s = with_custom();
        s.set_enabled(Action::DeleteSound, false);
        let g = files("/bin/mmt", &s);
        assert!(g.iter().all(|(n, _)| n != "Delete sound"));
        assert!(g.iter().any(|(n, c)| n == "Make GIF" && c.contains("run 'gif'")));
    }
}
