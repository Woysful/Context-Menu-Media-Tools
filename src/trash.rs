//! Moving a file to the trash of the system (Delete the original when done).
//!
//! Linux and other free desktops: the trash of the home folder (freedesktop.org Trash spec)
//! for files on its drive, else `gio trash`, `kioclient move … trash:/` or `trash-put`.
//! Windows: the Recycle Bin through PowerShell. macOS: the Finder.

use std::path::Path;

/// Move `file` to the trash; an error leaves it where it is
pub fn put(file: &Path) -> Result<(), String> {
    let file = std::path::absolute(file).map_err(|e| e.to_string())?;
    platform::put(&file)?;
    if file.exists() {
        return Err("the file is still there".into());
    }
    Ok(())
}

/// Run a program that moves the file; Ok when it is gone afterwards
#[allow(dead_code)]
fn by_program(program: &str, before: &[&str], file: &Path, after: &[&str]) -> Result<(), String> {
    let out = crate::proc::command(program).args(before).arg(file).args(after).output().map_err(|e| format!("{program}: {e}"))?;
    if out.status.success() && !file.exists() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
    Err(match err {
        e if e.is_empty() => format!("{program} exited with {}", out.status),
        e if e.starts_with(program) => e,
        e => format!("{program}: {e}"),
    })
}

#[cfg(windows)]
mod platform {
    use std::path::Path;

    pub fn put(file: &Path) -> Result<(), String> {
        // the path goes through the environment, so no quoting can break the script
        let script = "Add-Type -AssemblyName Microsoft.VisualBasic; \
            [Microsoft.VisualBasic.FileIO.FileSystem]::DeleteFile($env:CMMT_TRASH, 'OnlyErrorDialogs', 'SendToRecycleBin')";
        let out = crate::proc::command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("CMMT_TRASH", file)
            .output()
            .map_err(|e| format!("powershell: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use std::path::Path;

    pub fn put(file: &Path) -> Result<(), String> {
        let script = ["-e", "on run argv", "-e", "tell application \"Finder\" to delete POSIX file (item 1 of argv)", "-e", "end run"];
        super::by_program("osascript", &script, file, &[])
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use std::io::Write;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    pub fn put(file: &Path) -> Result<(), String> {
        let mut errors = Vec::new();
        match home_trash(file, &crate::paths::xdg_data().join("Trash")) {
            Ok(true) => return Ok(()),
            Ok(false) => {}
            Err(e) => errors.push(e),
        }
        let tools: [(&str, &[&str], &[&str]); 4] =
            [("gio", &["trash"], &[]), ("kioclient6", &["move"], &["trash:/"]), ("kioclient5", &["move"], &["trash:/"]), ("trash-put", &[], &[])];
        for (program, before, after) in tools {
            match super::by_program(program, before, file, after) {
                Ok(()) => return Ok(()),
                // a program that is not installed is no news
                Err(e) if e.contains("No such file") => {}
                Err(e) => errors.push(e),
            }
        }
        if errors.is_empty() {
            errors.push("no trash for this drive".into());
        }
        Err(errors.join("; "))
    }

    /// The trash of the home folder (`dir`); Ok(false) when the file is on another drive
    pub(super) fn home_trash(file: &Path, dir: &Path) -> Result<bool, String> {
        let (files, info) = (dir.join("files"), dir.join("info"));
        if std::fs::create_dir_all(&files).is_err() || std::fs::create_dir_all(&info).is_err() {
            return Ok(false);
        }
        let dev = |p: &Path| std::fs::metadata(p).map(|m| m.dev()).ok();
        if dev(file).is_none() || dev(file) != dev(&files) {
            return Ok(false);
        }
        let name = file.file_name().ok_or("no file name")?.to_string_lossy().to_string();
        let (stem, ext) = match name.rsplit_once('.') {
            Some((s, e)) if !s.is_empty() => (s.to_string(), format!(".{e}")),
            _ => (name.clone(), String::new()),
        };
        for n in 0..10_000 {
            let candidate = if n == 0 { name.clone() } else { format!("{stem}.{n}{ext}") };
            let info_path = info.join(format!("{candidate}.trashinfo"));
            if files.join(&candidate).exists() {
                continue;
            }
            // the info file is made first and exclusively: it reserves the name
            let mut f = match std::fs::OpenOptions::new().write(true).create_new(true).open(&info_path) {
                Ok(f) => f,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("{}: {e}", info_path.display())),
            };
            let text = format!("[Trash Info]\nPath={}\nDeletionDate={}\n", encode(file), local_time());
            if let Err(e) = f.write_all(text.as_bytes()) {
                let _ = std::fs::remove_file(&info_path);
                return Err(e.to_string());
            }
            drop(f);
            return match std::fs::rename(file, files.join(&candidate)) {
                Ok(()) => Ok(true),
                Err(e) => {
                    let _ = std::fs::remove_file(&info_path);
                    Err(e.to_string())
                }
            };
        }
        Err("no free name in the trash".into())
    }

    /// The path as the Trash spec wants it: percent-encoded bytes, "/" kept
    pub(super) fn encode(path: &Path) -> String {
        let mut out = String::new();
        for &b in path.as_os_str().as_bytes() {
            if b.is_ascii_alphanumeric() || b"/-_.~".contains(&b) {
                out.push(b as char);
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    }

    /// "2024-05-01T13:45:00" in local time
    fn local_time() -> String {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as libc::time_t;
        // SAFETY: localtime_r only writes the tm it is given
        let mut tm: libc::tm = unsafe { std::mem::zeroed() };
        if unsafe { libc::localtime_r(&now, &mut tm) }.is_null() {
            return String::new();
        }
        format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}", tm.tm_year + 1900, tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec)
    }
}

#[cfg(all(test, unix, not(target_os = "macos")))]
mod tests {
    use super::*;

    #[test]
    fn paths_are_encoded() {
        assert_eq!(platform::encode(Path::new("/home/a/my clip (1).mp4")), "/home/a/my%20clip%20%281%29.mp4");
        assert_eq!(platform::encode(Path::new("/x/ä")), "/x/%C3%A4");
    }

    #[test]
    fn a_file_goes_to_the_home_trash() {
        let base = std::env::temp_dir().join(format!("cmmt-trash-{}", std::process::id()));
        let trash = base.join("Trash");
        std::fs::create_dir_all(&base).unwrap();
        let file = base.join("clip.mp4");
        for body in [b"x", b"y"] {
            std::fs::write(&file, body).unwrap();
            assert!(platform::home_trash(&file, &trash).unwrap());
        }
        assert!(!file.exists());
        assert_eq!(std::fs::read(trash.join("files/clip.mp4")).unwrap(), b"x");
        assert_eq!(std::fs::read(trash.join("files/clip.1.mp4")).unwrap(), b"y");
        let info = std::fs::read_to_string(trash.join("info/clip.1.mp4.trashinfo")).unwrap();
        assert!(info.starts_with("[Trash Info]\nPath=/") && info.contains("clip.mp4\nDeletionDate=20"), "{info}");
        let _ = std::fs::remove_dir_all(&base);
    }
}
