//! Where things are: the folders of the settings and of the desktop files, the program
//! itself, ffmpeg / ffprobe.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub fn home_dir() -> PathBuf {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

fn xdg(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var).filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(|| home_dir().join(fallback))
}

/// `$XDG_CONFIG_HOME` or `~/.config`
pub fn xdg_config() -> PathBuf {
    xdg("XDG_CONFIG_HOME", ".config")
}

/// `$XDG_DATA_HOME` or `~/.local/share`
pub fn xdg_data() -> PathBuf {
    xdg("XDG_DATA_HOME", ".local/share")
}

/// Directory of the user's settings (created on demand)
pub fn config_dir() -> PathBuf {
    if cfg!(windows) {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("mmt");
        }
    }
    xdg_config().join("mmt")
}

pub fn presets_file() -> PathBuf {
    config_dir().join("presets.toml")
}

pub fn settings_file() -> PathBuf {
    config_dir().join("settings.toml")
}

/// The file the user started. In an AppImage that is the AppImage itself: the binary inside
/// lives in a temporary mount that disappears when the program exits, so nothing (menu
/// entries, the copy made by `install`) may point at it.
pub fn program() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    Some(appimage_of(&exe, std::env::var_os("APPIMAGE"), std::env::var_os("APPDIR")).unwrap_or(exe))
}

/// The AppImage that holds `exe`. `APPIMAGE` alone is not trusted: a program started from
/// another AppImage inherits it, so the binary must also be inside the mount (`APPDIR`).
fn appimage_of(exe: &Path, image: Option<OsString>, mount: Option<OsString>) -> Option<PathBuf> {
    let image = PathBuf::from(image.filter(|v| !v.is_empty())?);
    let mount = PathBuf::from(mount.filter(|v| !v.is_empty())?);
    (exe.starts_with(&mount) && image.is_file()).then_some(image)
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
    let beside = program().and_then(|p| p.parent().map(|d| d.join(&file))).filter(|p| p.is_file());
    beside.or_else(|| in_path(&file))
}

/// The first `file` in a folder of PATH
pub fn in_path(file: &str) -> Option<PathBuf> {
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths).map(|dir| dir.join(file)).find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appimage_is_recognised_only_from_inside_its_mount() {
        let dir = std::env::temp_dir().join(format!("mmt-appimage-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let image = dir.join("mmt.AppImage");
        std::fs::write(&image, b"x").unwrap();
        let mount = PathBuf::from("/tmp/.mount_mmtXYZ");
        let inside = mount.join("usr/bin/mmt");
        let some = |p: &Path| Some(p.as_os_str().to_os_string());
        assert_eq!(appimage_of(&inside, some(&image), some(&mount)), Some(image.clone()));
        // inherited from another AppImage: the binary is not in that mount
        assert_eq!(appimage_of(Path::new("/usr/bin/mmt"), some(&image), some(&mount)), None);
        assert_eq!(appimage_of(&inside, None, some(&mount)), None);
        assert_eq!(appimage_of(&inside, some(&dir.join("gone.AppImage")), some(&mount)), None);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
