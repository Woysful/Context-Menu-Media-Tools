//! Starting other programs.

use std::path::Path;
use std::process::Command;

/// A command that does not flash up a console window when a GUI program starts it on Windows
pub fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut cmd = Command::new(program);
    hide_console(&mut cmd);
    cmd
}

fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))]
    let _ = cmd;
}

/// Open a file or folder with the program the system has for it
pub fn open_path(path: &Path) {
    let mut cmd = if cfg!(windows) {
        let mut c = command("cmd");
        c.args(["/C", "start", ""]);
        c
    } else if cfg!(target_os = "macos") {
        command("open")
    } else {
        command("xdg-open")
    };
    let _ = cmd.arg(path).spawn();
}

/// stdout of a program that succeeded
pub fn output_of(program: &str, args: &[&str]) -> Option<String> {
    let out = command(program).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).to_string())
}
