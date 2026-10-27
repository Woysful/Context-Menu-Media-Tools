//! Linux: the folder of the program in PATH (a marked block in the shell files) and the
//! "Media Tools" entry of the application menu, which opens the Settings window.

use super::menu::desktop_quote;
use super::set_executable;
use crate::paths;
use std::fs;
use std::path::{Path, PathBuf};

const BLOCK_START: &str = "# >>> mmt >>>";
const BLOCK_END: &str = "# <<< mmt <<<";

/// The text with our marked block added (or replaced when it is there already)
pub fn with_block(text: &str, block: &str) -> String {
    let mut out = without_block(text).trim_end_matches('\n').to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(&format!("{BLOCK_START}\n{block}\n{BLOCK_END}\n"));
    out
}

/// The text without our marked block
pub fn without_block(text: &str) -> String {
    let (Some(a), Some(b)) = (text.find(BLOCK_START), text.find(BLOCK_END)) else { return text.to_string() };
    if b < a {
        return text.to_string();
    }
    let before = text[..a].trim_end_matches('\n');
    let after = text[b + BLOCK_END.len()..].trim_start_matches('\n');
    match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (true, false) => after.to_string(),
        (false, true) => format!("{before}\n"),
        (false, false) => format!("{before}\n\n{after}"),
    }
}

/// The folder as the shells should see it ("$HOME/.local/bin")
fn home_text(dir: &Path) -> String {
    match dir.strip_prefix(paths::home_dir()) {
        Ok(rel) => format!("$HOME/{}", rel.to_string_lossy()),
        Err(_) => dir.to_string_lossy().to_string(),
    }
}

fn fish_file() -> PathBuf {
    paths::home_dir().join(".config/fish/conf.d/mmt-path.fish")
}

/// Add the folder of the program to PATH for the shells of the user: ~/.profile, ~/.bashrc,
/// ~/.zshenv and the fish configuration. Each file gets a marked block that `uninstall` removes.
pub fn path_install(bin_dir: &Path) -> Vec<String> {
    let in_path = std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d == bin_dir));
    let home = paths::home_dir();
    let dir = home_text(bin_dir);
    let sh_block = format!("case \":$PATH:\" in *\":{dir}:\"*) ;; *) export PATH=\"{dir}:$PATH\" ;; esac");
    let mut done = Vec::new();
    let mut touch = |path: PathBuf, create: bool, text: &dyn Fn(&str) -> String| {
        let existing = fs::read_to_string(&path);
        if existing.is_err() && !create {
            return;
        }
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::write(&path, text(&existing.unwrap_or_default())).is_ok() {
            done.push(path.display().to_string());
        }
    };
    let sh = |old: &str| with_block(old, &sh_block);
    touch(home.join(".profile"), true, &sh);
    touch(home.join(".bashrc"), false, &sh);
    if home.join(".zshenv").exists() || paths::in_path("zsh").is_some() {
        touch(home.join(".zshenv"), true, &sh);
    }
    if home.join(".config/fish").exists() || paths::in_path("fish").is_some() {
        let fish = format!("if not contains -- {dir} $PATH\n    set -gx PATH {dir} $PATH\nend\n");
        touch(fish_file(), true, &|_| fish.clone());
    }
    if in_path {
        vec![format!("PATH: {} is already in PATH, `mmt` works in the terminal", bin_dir.display())]
    } else {
        vec![
            format!("PATH: added {} to {}", bin_dir.display(), done.join(", ")),
            "PATH: log out and in (or open a new terminal) to use `mmt` in the terminal".into(),
        ]
    }
}

pub fn path_uninstall() -> Vec<String> {
    let home = paths::home_dir();
    let mut log = Vec::new();
    for name in [".profile", ".bashrc", ".zshenv"] {
        let path = home.join(name);
        let Ok(text) = fs::read_to_string(&path) else { continue };
        if text.contains(BLOCK_START) {
            let cleaned = without_block(&text);
            let _ = if cleaned.trim().is_empty() { fs::remove_file(&path) } else { fs::write(&path, cleaned) };
            log.push(format!("PATH: removed from {}", path.display()));
        }
    }
    let fish = fish_file();
    if fish.exists() && fs::remove_file(&fish).is_ok() {
        log.push(format!("PATH: removed {}", fish.display()));
    }
    log
}

const ICON: &[u8] = include_bytes!("../../assets/mmt.png");

/// The entry of the application menu that opens the Settings window
pub fn launcher_desktop(exe: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Media Tools\nComment=Settings of the Media Tools context menu\nExec={} settings\nIcon=mmt\nTerminal=false\nCategories=AudioVideo;Settings;\n",
        desktop_quote(exe)
    )
}

fn launcher_file() -> PathBuf {
    paths::xdg_data().join("applications/mmt.desktop")
}

fn icon_file() -> PathBuf {
    paths::xdg_data().join("icons/hicolor/256x256/apps/mmt.png")
}

fn write(path: &Path, bytes: &[u8]) -> bool {
    path.parent().is_some_and(|d| fs::create_dir_all(d).is_ok()) && fs::write(path, bytes).is_ok()
}

pub fn launcher_install(exe: &Path) -> Vec<String> {
    let file = launcher_file();
    let _ = write(&icon_file(), ICON);
    if write(&file, launcher_desktop(&exe.to_string_lossy()).as_bytes()) {
        set_executable(&file);
        vec![format!("application menu: \"Media Tools\" opens the Settings ({})", file.display())]
    } else {
        vec![]
    }
}

pub fn launcher_uninstall() -> Vec<String> {
    let _ = fs::remove_file(icon_file());
    let file = launcher_file();
    if fs::remove_file(&file).is_ok() {
        vec![format!("removed {}", file.display())]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_block_is_added_replaced_and_removed() {
        let block = "export PATH=\"$HOME/.local/bin:$PATH\"";
        let original = "alias ll='ls -l'\n";
        let once = with_block(original, block);
        assert!(once.starts_with("alias ll='ls -l'\n\n# >>> mmt >>>\n"));
        assert_eq!(with_block(&once, block), once); // idempotent
        assert_eq!(without_block(&once), original);
        assert_eq!(without_block(&with_block("", block)), "");
        assert_eq!(without_block("nothing here\n"), "nothing here\n");
    }

    #[test]
    fn launcher_entry_opens_the_settings() {
        let d = launcher_desktop("/home/u/.local/bin/mmt");
        assert!(d.contains("Exec=/home/u/.local/bin/mmt settings") && d.contains("Categories=AudioVideo;Settings;"));
        assert!(d.contains("Icon=mmt\n"));
        assert!(ICON.starts_with(b"\x89PNG"));
    }
}
