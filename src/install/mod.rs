//! `mmt install` / `mmt uninstall`: put the items into the context menu of the file manager.
//!
//! What is written are plain text files, so they can be read and edited by hand:
//!   KDE (Dolphin)  ~/.local/share/kio/servicemenus/mmt-*.desktop
//!   GNOME (Files)  ~/.local/share/nautilus/scripts/Media Tools/*   (small sh scripts)
//!   XFCE (Thunar)  <mmt-…> actions inside ~/.config/Thunar/uca.xml
//!   Raspberry Pi OS / LXDE / LXQt (PCManFM)   ~/.local/share/file-manager/actions/mmt-*.desktop
//!   Windows        mmt-menu.reg in %APPDATA%\mmt, imported with `reg import`
//!
//! The menu follows the Settings window: items that are switched off are left out, own
//! items (settings.toml) are added.

mod gnome;
mod kde;
mod linux;
mod menu;
mod pcmanfm;
mod windows;
mod xfce;

use crate::config::Config;
use crate::paths;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target {
    Kde,
    Gnome,
    Xfce,
    Pcmanfm,
    Windows,
}

impl Target {
    const ALL: [Target; 5] = [Target::Kde, Target::Gnome, Target::Xfce, Target::Pcmanfm, Target::Windows];

    /// The option that chooses it (`--kde` ...)
    fn flag(self) -> &'static str {
        match self {
            Target::Kde => "kde",
            Target::Gnome => "gnome",
            Target::Xfce => "xfce",
            Target::Pcmanfm => "pcmanfm",
            Target::Windows => "windows",
        }
    }

    fn install(self, exe: &Path, st: &crate::settings::Settings) -> Result<Vec<String>, String> {
        match self {
            Target::Kde => kde::install(exe, st),
            Target::Gnome => gnome::install(exe, st),
            Target::Xfce => xfce::install(exe, st),
            Target::Pcmanfm => pcmanfm::install(exe, st),
            Target::Windows => windows::install(exe, st),
        }
    }

    fn uninstall(self) -> Vec<String> {
        match self {
            Target::Kde => kde::uninstall(),
            Target::Gnome => gnome::uninstall(),
            Target::Xfce => xfce::uninstall(),
            Target::Pcmanfm => pcmanfm::uninstall(),
            Target::Windows => windows::uninstall(),
        }
    }
}

/// What `install` / `uninstall` do
#[derive(Clone, Debug)]
pub struct Options {
    /// empty = the file managers found on this computer
    pub targets: Vec<Target>,
    /// copy the program to ~/.local/bin (Windows: %LOCALAPPDATA%\mmt) so the download can be deleted
    pub copy: bool,
    /// add that folder to PATH
    pub path: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { targets: Vec::new(), copy: true, path: true }
    }
}

impl Options {
    /// From the options of the command line (`--kde`, `--no-copy`, ...)
    pub fn from_flags(flags: &[String]) -> Options {
        let has = |f: &str| flags.iter().any(|x| x == f);
        Options { targets: Target::ALL.into_iter().filter(|t| has(t.flag())).collect(), copy: !has("no-copy"), path: !has("no-path") }
    }

    fn targets(&self) -> Vec<Target> {
        if self.targets.is_empty() {
            detect()
        } else {
            self.targets.clone()
        }
    }
}

/// The file managers of this computer
fn detect() -> Vec<Target> {
    if cfg!(windows) {
        return vec![Target::Windows];
    }
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase();
    let desk: Vec<&str> = desktop.split(':').collect();
    let is = |names: &[&str]| desk.iter().any(|x| names.iter().any(|n| x.contains(n)));
    let has = |tools: &[&str]| tools.iter().any(|t| paths::in_path(t).is_some());
    let mut out = Vec::new();
    if is(&["kde"]) || has(&["dolphin"]) {
        out.push(Target::Kde);
    }
    if is(&["gnome", "unity"]) || has(&["nautilus"]) {
        out.push(Target::Gnome);
    }
    if is(&["xfce"]) || has(&["thunar"]) {
        out.push(Target::Xfce);
    }
    if is(&["lxde", "lxqt", "labwc", "wayfire"]) || has(&["pcmanfm", "pcmanfm-qt"]) {
        out.push(Target::Pcmanfm);
    }
    out
}

pub(crate) fn remove_matching(dir: &Path, prefix: &str, suffix: &str) -> Vec<String> {
    let mut log = Vec::new();
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if name.starts_with(prefix) && name.ends_with(suffix) && fs::remove_file(e.path()).is_ok() {
            log.push(format!("removed {}", e.path().display()));
        }
    }
    log
}

pub(crate) fn set_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o755));
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Where `install` puts the program
fn local_bin() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(paths::config_dir).join("mmt").join("mmt.exe")
    } else {
        paths::home_dir().join(".local/bin/mmt")
    }
}

/// Copy the program (the AppImage, when it runs from one) to a permanent place, so the
/// downloaded file can be deleted. The menu entries point at the result.
fn place_program(copy: bool) -> Result<PathBuf, String> {
    let me = paths::program().ok_or("the program cannot find its own file")?;
    if !copy {
        return Ok(me);
    }
    let target = local_bin();
    let same = matches!((fs::canonicalize(&me), fs::canonicalize(&target)), (Ok(a), Ok(b)) if a == b);
    if !same {
        let dir = target.parent().expect("a folder");
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        // a running program cannot be overwritten in place on every system: write a new file, then swap
        let tmp = target.with_extension("new");
        fs::copy(&me, &tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        set_executable(&tmp);
        fs::rename(&tmp, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    Ok(target)
}

/// Returns (exit code, lines to show). A program without a console (Windows) shows them in a window.
pub fn install(opts: &Options) -> (i32, Vec<String>) {
    let targets = opts.targets();
    let mut out: Vec<String> = Vec::new();
    if targets.is_empty() {
        out.push("no supported file manager found; choose one with --kde, --gnome, --xfce or --pcmanfm".into());
        return (1, out);
    }
    let exe = match place_program(opts.copy) {
        Ok(p) => p,
        Err(e) => return (1, vec![e]),
    };
    out.push(format!("program: {}", exe.display()));
    let cfg = Config::load();
    let mut ok = true;
    for t in &targets {
        match t.install(&exe, &cfg.settings) {
            Ok(log) => out.extend(log),
            Err(e) => {
                out.push(format!("{}: {e}", t.flag()));
                ok = false;
            }
        }
    }

    // `mmt` in the terminal, and a "Media Tools" entry (the Settings window) in the application menu
    let bin_dir = exe.parent().unwrap_or(Path::new("")).to_path_buf();
    if cfg!(windows) {
        if opts.path {
            match windows::powershell(&windows::ps_path_script(&bin_dir.to_string_lossy(), true)) {
                Ok(()) => out.push(format!("PATH: {} added to the user PATH (open a new terminal)", bin_dir.display())),
                Err(e) => out.push(format!("PATH: could not change it ({e})")),
            }
        }
        match windows::powershell(&windows::ps_shortcut_script(&exe.to_string_lossy(), true)) {
            Ok(()) => out.push("Start menu: \"Media Tools\" opens the Settings".into()),
            Err(e) => out.push(format!("Start menu: shortcut not created ({e})")),
        }
    } else {
        if opts.path {
            out.extend(linux::path_install(&bin_dir));
        }
        out.extend(linux::launcher_install(&exe));
    }
    if paths::find_tool("ffmpeg", &cfg.settings.ffmpeg).is_none() || paths::find_tool("ffprobe", &cfg.settings.ffprobe).is_none() {
        out.push("warning: ffmpeg / ffprobe not found; install ffmpeg (or put ffmpeg and ffprobe next to mmt)".into());
    }
    (if ok { 0 } else { 1 }, out)
}

pub fn uninstall(opts: &Options) -> (i32, Vec<String>) {
    let mut out: Vec<String> = opts.targets().into_iter().flat_map(Target::uninstall).collect();
    if cfg!(windows) {
        let exe = local_bin();
        let _ = windows::powershell(&windows::ps_path_script(&exe.parent().unwrap_or(Path::new("")).to_string_lossy(), false));
        let _ = windows::powershell(&windows::ps_shortcut_script(&exe.to_string_lossy(), false));
    } else {
        out.extend(linux::launcher_uninstall());
        out.extend(linux::path_uninstall());
    }
    out.push(format!("done (the program itself and {} were left in place)", paths::config_dir().display()));
    (0, out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::settings::{Custom, Settings};

    pub fn with_custom() -> Settings {
        let mut s = Settings::default();
        s.custom.push(Custom { id: "gif".into(), name: "Make GIF".into(), kind: "video".into(), ..Default::default() });
        s
    }

    #[test]
    fn options_from_the_command_line() {
        let o = Options::from_flags(&["kde".into(), "no-path".into()]);
        assert_eq!(o.targets, [Target::Kde]);
        assert!(o.copy && !o.path);
        let d = Options::from_flags(&[]);
        assert!(d.targets.is_empty() && d.copy && d.path);
    }
}
