//! Windows: the classic context menu of Explorer, written to `cmmt-menu.reg` in %APPDATA%\cmmt
//! and imported with `reg import`; PATH and the Start menu shortcut through PowerShell.

use super::menu::{SUBMENUS, VIDEO};
use crate::media::Kind;
use crate::paths;
use crate::settings::Settings;
use std::fs;
use std::path::Path;

fn reg_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// A verb: its key with the name, and the command below it
fn verb(body: &mut Vec<String>, key: &str, name: &str, command: &str) {
    body.push(format!("[{key}]\r\n\"MUIVerb\"=\"{}\"\r\n", reg_escape(name)));
    body.push(format!("[{key}\\command]\r\n@=\"{}\"\r\n", reg_escape(command)));
}

/// The text of the .reg file: verbs under SystemFileAssociations\.ext\shell, audio and image
/// conversions as a cascade. `uninstall` removes the keys of ALL items (also the
/// switched-off ones, so an update can start from a clean state).
pub fn reg_text(exe: &str, uninstall: bool, s: &Settings) -> String {
    let root = r"HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations";
    let command = |args: &str| format!("\"{exe}\" {args} --batch \"%1\"");
    let mut keys: Vec<String> = Vec::new(); // everything we own, for the uninstall file
    let mut body: Vec<String> = Vec::new();

    for ext in Kind::Video.extensions() {
        for (i, it) in VIDEO.iter().enumerate() {
            let key = format!(r"{root}\.{ext}\shell\cmmt_{:02}", i + 1);
            if s.enabled(it.action) {
                verb(&mut body, &key, it.name(), &command(it.action.cli_name()));
            }
            keys.push(key);
        }
    }
    for sub in &SUBMENUS {
        let with_formats = s.enabled(sub.action);
        let with_stats = s.enabled(crate::action::Action::Stats);
        for ext in sub.kind.extensions() {
            let top = format!(r"{root}\.{ext}\shell\cmmt_01");
            keys.push(top.clone());
            if !with_formats && !with_stats {
                continue;
            }
            body.push(format!("[{top}]\r\n\"MUIVerb\"=\"{}\"\r\n\"SubCommands\"=\"\"\r\n", reg_escape(sub.name)));
            if with_formats {
                for (i, (key, text, _)) in sub.formats.iter().enumerate() {
                    let args = format!("{} --yes --format {key}", sub.action.cli_name());
                    verb(&mut body, &format!("{top}\\shell\\{:02}_{key}", i + 1), text, &command(&args));
                }
            }
            if with_stats {
                let stats = super::menu::STATS;
                verb(&mut body, &format!("{top}\\shell\\{:02}_stats", sub.formats.len() + 1), stats.name(), &command("stats"));
            }
        }
    }
    // own items: one verb per extension of their kind
    for c in s.menu_customs() {
        for ext in c.media_kind().extensions() {
            let key = format!(r"{root}\.{ext}\shell\cmmt_c_{}", c.id);
            verb(&mut body, &key, &c.name, &command(&format!("run {}", c.id)));
            keys.push(key);
        }
    }

    let mut out = String::from("Windows Registry Editor Version 5.00\r\n\r\n");
    if uninstall {
        for key in &keys {
            out.push_str(&format!("[-{key}]\r\n"));
        }
    } else {
        for b in &body {
            out.push_str(b);
            out.push_str("\r\n");
        }
    }
    out
}

/// .reg files are UTF-16 little endian with a byte order mark
pub fn reg_bytes(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    for unit in text.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

pub fn reg_import(file: &Path) -> Result<(), String> {
    let out = crate::proc::command("reg").arg("import").arg(file).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("reg import failed: {}", String::from_utf8_lossy(&out.stderr)))
    }
}

pub fn install(exe: &Path, st: &Settings) -> Result<Vec<String>, String> {
    let dir = paths::config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let exe_text = exe.to_string_lossy().to_string();
    let file = dir.join("cmmt-menu.reg");
    let off = dir.join("cmmt-menu-uninstall.reg");
    fs::write(&file, reg_bytes(&reg_text(&exe_text, false, st))).map_err(|e| e.to_string())?;
    fs::write(&off, reg_bytes(&reg_text(&exe_text, true, st))).map_err(|e| e.to_string())?;
    let mut log = vec![format!("written {} (you can read and edit it)", file.display())];
    if cfg!(windows) {
        // start from a clean state: items that were switched off must disappear
        let _ = reg_import(&off);
        reg_import(&file)?;
        log.push("Windows: imported into the registry".into());
    } else {
        log.push("(not Windows: the file was only written, nothing was imported)".into());
    }
    Ok(log)
}

pub fn uninstall() -> Vec<String> {
    let file = paths::config_dir().join("cmmt-menu-uninstall.reg");
    if !cfg!(windows) || !file.exists() {
        return vec![];
    }
    match reg_import(&file) {
        Ok(()) => vec!["Windows: menu items removed".into()],
        Err(_) => vec![format!("could not import {}; double-click it to remove the items", file.display())],
    }
}

/// PowerShell that adds / removes a folder in the user's PATH
pub fn ps_path_script(dir: &str, add: bool) -> String {
    let d = dir.replace('\'', "''");
    if add {
        format!("$d='{d}'; $p=[Environment]::GetEnvironmentVariable('Path','User'); if ($null -eq $p) {{ $p='' }}; if (($p -split ';') -notcontains $d) {{ [Environment]::SetEnvironmentVariable('Path', (($p.TrimEnd(';') + ';' + $d).TrimStart(';')), 'User') }}")
    } else {
        format!("$d='{d}'; $p=[Environment]::GetEnvironmentVariable('Path','User'); if ($null -ne $p) {{ [Environment]::SetEnvironmentVariable('Path', ((($p -split ';') | Where-Object {{ $_ -ne $d -and $_ -ne '' }}) -join ';'), 'User') }}")
    }
}

/// PowerShell that makes / removes the Start menu shortcut of the Settings window
pub fn ps_shortcut_script(exe: &str, add: bool) -> String {
    let e = exe.replace('\'', "''");
    let lnk = "$env:APPDATA + '\\Microsoft\\Windows\\Start Menu\\Programs\\Media Tools.lnk'";
    if add {
        format!("$s=(New-Object -ComObject WScript.Shell).CreateShortcut({lnk}); $s.TargetPath='{e}'; $s.Arguments='settings'; $s.Description='Media Tools'; $s.Save()")
    } else {
        format!("Remove-Item -LiteralPath ({lnk}) -ErrorAction SilentlyContinue")
    }
}

pub fn powershell(script: &str) -> Result<(), String> {
    let out = crate::proc::command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::with_custom;
    use super::*;
    use crate::action::Action;

    #[test]
    fn reg_file_structure() {
        let t = reg_text(r"C:\Users\me\AppData\Local\cmmt\cmmt.exe", false, &Settings::default());
        assert!(t.starts_with("Windows Registry Editor Version 5.00\r\n"));
        assert!(t.contains(r"[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.mp4\shell\cmmt_01]"));
        assert!(t.contains("\"MUIVerb\"=\"Compress Video\""));
        assert!(t.contains(r#"@="\"C:\\Users\\me\\AppData\\Local\\cmmt\\cmmt.exe\" compress --batch \"%1\"""#));
        assert!(t.contains(r"SystemFileAssociations\.mp3\shell\cmmt_01\shell\02_mp3\command]"));
        assert!(t.contains("convert-audio --yes --format mp3 --batch"));
        assert!(t.contains("\"SubCommands\"=\"\""));
        let off = reg_text("x.exe", true, &Settings::default());
        assert!(off.contains(r"[-HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.mp4\shell\cmmt_08]"));
        assert!(off.contains(r"[-HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.png\shell\cmmt_01]"));
        assert!(!off.contains("MUIVerb"));
    }

    #[test]
    fn reg_follows_the_settings() {
        let mut s = with_custom();
        s.set_enabled(Action::DeleteSound, false);
        let t = reg_text("x.exe", false, &s);
        assert!(!t.contains(r"\.mp4\shell\cmmt_05]")); // numbering stays stable, the item is gone
        assert!(t.contains(r"\.mp4\shell\cmmt_06]"));
        assert!(t.contains(r"\.mkv\shell\cmmt_c_gif]") && t.contains("run gif --batch"));
        // the uninstall file still knows the switched-off item and the own one
        let off = reg_text("x.exe", true, &s);
        assert!(off.contains(r"[-HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.mp4\shell\cmmt_05]"));
        assert!(off.contains(r"\.mp4\shell\cmmt_c_gif]"));
    }

    #[test]
    fn reg_is_utf16le_with_bom() {
        assert_eq!(reg_bytes("Aя"), vec![0xFF, 0xFE, b'A', 0, 0x4F, 0x04]);
    }

    #[test]
    fn powershell_scripts_quote_the_paths() {
        let add = ps_path_script(r"C:\Users\O'Neil\cmmt", true);
        assert!(add.contains(r"$d='C:\Users\O''Neil\cmmt'") && add.contains("SetEnvironmentVariable('Path'"));
        assert!(ps_path_script("C:\\cmmt", false).contains("Where-Object"));
        assert!(ps_shortcut_script(r"C:\cmmt\cmmt.exe", true).contains("$s.Arguments='settings'"));
        assert!(ps_shortcut_script("x", false).starts_with("Remove-Item"));
    }
}
