//! The program was called mmt before it became cmmt. `install` and `uninstall` remove what an
//! install under the old name left behind, so the menus do not show every item twice.

use super::{linux, remove_matching, windows, xfce};
use crate::paths;
use std::fs;
use std::path::Path;

const OLD: &str = "mmt";

/// A line of the help text, found in every build of the program under either name
const SIGNATURE: &[u8] = b"Media Tools: ffmpeg front end for the file manager context menu";

/// A file in the place of the old copy is removed only when it is that copy (or a link)
fn remove_old_program(path: &Path, log: &mut Vec<String>) {
    let ours = path.is_symlink() || fs::read(path).is_ok_and(|bytes| bytes.windows(SIGNATURE.len()).any(|w| w == SIGNATURE));
    if ours && fs::remove_file(path).is_ok() {
        log.push(format!("removed {} (old name)", path.display()));
    }
}

pub fn remove() -> Vec<String> {
    let mut log = Vec::new();
    if cfg!(windows) {
        let dir = paths::config_dir();
        let off = dir.join(format!("{OLD}-menu-uninstall.reg"));
        if off.exists() && windows::reg_import(&off).is_ok() {
            log.push("Windows: menu items of the old name removed".into());
            let _ = fs::remove_file(&off);
            let _ = fs::remove_file(dir.join(format!("{OLD}-menu.reg")));
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            let old_dir = Path::new(&local).join(OLD);
            let _ = windows::powershell(&windows::ps_path_script(&old_dir.to_string_lossy(), false));
            remove_old_program(&old_dir.join(format!("{OLD}.exe")), &mut log);
            let _ = fs::remove_dir(&old_dir);
        }
        return log;
    }
    let data = paths::xdg_data();
    for dir in ["kio/servicemenus", "file-manager/actions"] {
        for removed in remove_matching(&data.join(dir), &format!("{OLD}-"), ".desktop") {
            log.push(format!("{removed} (old name)"));
        }
    }
    log.extend(xfce::remove_named(OLD));
    for file in [data.join(format!("applications/{OLD}.desktop")), data.join(format!("icons/hicolor/256x256/apps/{OLD}.png"))] {
        if fs::remove_file(&file).is_ok() {
            log.push(format!("removed {} (old name)", file.display()));
        }
    }
    log.extend(linux::remove_path_blocks(OLD));
    remove_old_program(&paths::home_dir().join(".local/bin").join(OLD), &mut log);
    log
}
