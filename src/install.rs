//! `mmt install` / `mmt uninstall`: put the items into the context menu of the file manager.
//!
//! What is written are plain text files (the same kind the project always used), so they
//! can be read and edited by hand:
//!   KDE (Dolphin)  ~/.local/share/kio/servicemenus/mmt-*.desktop
//!   GNOME (Files)  ~/.local/share/nautilus/scripts/Media Tools/*   (small sh scripts)
//!   XFCE (Thunar)  <mmt-…> actions inside ~/.config/Thunar/uca.xml
//!   Raspberry Pi OS / LXDE / LXQt (PCManFM)   ~/.local/share/file-manager/actions/mmt-*.desktop
//!   Windows        mmt-menu.reg in %APPDATA%\mmt, imported with `reg import`
//!
//! The menu follows the Settings window: items that are switched off are left out, own
//! items (settings.toml) are added.

use crate::config::{self, Config, Custom, Settings};
use std::fs;
use std::path::{Path, PathBuf};

const VIDEO_MIME: &str = "video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm";
const AUDIO_MIME: &str = "audio/wav;audio/mpeg;audio/flac;audio/aac;audio/ogg;audio/x-ms-wma;audio/mp4;audio/webm;audio/opus";
const IMAGE_MIME: &str = "image/jpeg;image/png;image/bmp;image/tiff;image/gif;image/webp;image/x-tga;image/vnd.adobe.photoshop";

pub const VIDEO_EXT: &[&str] = &["mp4", "avi", "mov", "mkv", "webm", "m4v"];
pub const AUDIO_EXT: &[&str] = &["wav", "mp3", "flac", "aac", "ogg", "wma", "m4a", "weba", "opus"];
pub const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "bmp", "tiff", "tif", "gif", "webp", "tga", "psd"];

struct Item {
    id: &'static str,
    action: &'static str,
    name: &'static str,
    icon: &'static str,
}

const fn item(id: &'static str, action: &'static str, name: &'static str, icon: &'static str) -> Item {
    Item { id, action, name, icon }
}

/// The video items, in menu order
const VIDEO: &[Item] = &[
    item("01_compressVideo", "compress", "Compress Video", "video-compress"),
    item("02_convertVideo", "convert-video", "Convert Video", "video-mp4"),
    item("03_convertAudio", "convert-audio", "Convert Audio", "audio-x-generic"),
    item("04_extractAudio", "extract-audio", "Extract Audio", "audio-x-generic"),
    item("05_deleteSound", "delete-sound", "Delete sound", "media-track-remove-amarok"),
    item("06_makeFrames", "frames", "Make frame sequence", "image-x-generic"),
    item("07_convertWebPAnim", "webp", "Convert to WebP anim", "image-webp"),
    item("08_stats", "stats", "Stats", "documentinfo"),
];

const CONVERT_IMAGE: Item = item("convertImage", "convert-image", "Convert Image", "image-x-generic");
const STATS: Item = item("stats", "stats", "Stats", "documentinfo");

/// (format key, text, KDE icon)
const AUDIO_FORMATS: &[(&str, &str, &str)] = &[
    ("wav", "WAV", "audio-wav"),
    ("mp3", "MP3", "audio-mpeg"),
    ("flac", "FLAC", "audio-flac"),
    ("aac", "AAC", "audio-aac"),
    ("ogg", "OGG", "audio-ogg"),
    ("wma", "WMA", "audio-x-ms-wma"),
    ("m4a", "M4A", "audio-mp4"),
    ("opus", "Opus", "audio-opus"),
];
const IMAGE_FORMATS: &[(&str, &str, &str)] = &[
    ("jpg", "JPG", "image-jpeg"),
    ("png", "PNG", "image-png"),
    ("bmp", "BMP", "image-bmp"),
    ("tiff", "TIFF", "image-tiff"),
    ("gif", "GIF", "image-gif"),
    ("webp", "WebP", "image-webp"),
    ("tga", "TGA", "image-x-tga"),
];

fn kind_mime(kind: &str) -> &'static str {
    match kind {
        "audio" => AUDIO_MIME,
        "image" => IMAGE_MIME,
        _ => VIDEO_MIME,
    }
}

fn kind_icon(kind: &str) -> &'static str {
    match kind {
        "audio" => "audio-x-generic",
        "image" => "image-x-generic",
        _ => "video-x-generic",
    }
}

fn kind_exts(kind: &str) -> &'static [&'static str] {
    match kind {
        "audio" => AUDIO_EXT,
        "image" => IMAGE_EXT,
        _ => VIDEO_EXT,
    }
}

/// The own items that make sense (they have a name and an id)
fn customs(s: &Settings) -> Vec<&Custom> {
    s.custom.iter().filter(|c| !c.id.is_empty() && !c.name.is_empty()).collect()
}

fn submenu_name(kind: &str) -> &'static str {
    match kind {
        "audio" => "Convert Audio",
        "image" => "Convert Image",
        _ => "Convert",
    }
}

// ------------------------------------------------------------------ text helpers

/// Exec= field of a desktop file: quoted when the path has spaces
fn desktop_quote(arg: &str) -> String {
    if arg.chars().any(|c| c.is_whitespace() || "\"'\\<>~|&;$*?#()`".contains(c)) {
        format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "\\$").replace('`', "\\`"))
    } else {
        arg.to_string()
    }
}

fn sh_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', "'\\''"))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

// ------------------------------------------------------------------ KDE

/// All service menu files: (file name, content)
pub fn kde_files(exe: &str, s: &Settings) -> Vec<(String, String)> {
    let exe = desktop_quote(exe);
    let head = |mime: &str, actions: &str, extra: &str| {
        format!(
            "[Desktop Entry]\nType=Service\nX-KDE-ServiceTypes=KonqPopupMenu/Plugin\nMimeType={mime}\nActions={actions}\n{extra}X-KDE-AuthorizeAction=shell_access\n\n"
        )
    };
    let names = |it: &Item| -> String { format!("Name={}\n", it.name) };
    let mut files = Vec::new();

    // video: one file per item, all at the top level of the menu
    for it in VIDEO.iter().filter(|i| s.enabled(i.action)) {
        let mut c = head(VIDEO_MIME, it.id, "X-KDE-Priority=TopLevel\n");
        c.push_str(&format!("[Desktop Action {}]\n{}Exec={exe} {} %F\nIcon={}\n", it.id, names(it), it.action, it.icon));
        files.push((format!("mmt-{}.desktop", it.id), c));
    }

    // audio / image: a submenu with one entry per format (no dialog) and Stats
    let sub = |kind: &str, mime: &str, formats: &[(&str, &str, &str)], action: &str| -> Option<(String, String)> {
        let with_formats = s.enabled(action);
        let with_stats = s.enabled("stats");
        if !with_formats && !with_stats {
            return None;
        }
        let mut ids: Vec<String> = if with_formats { formats.iter().map(|(k, _, _)| format!("convert_{k}")).collect() } else { vec![] };
        if with_stats {
            ids.push("stats".into());
        }
        let sm = format!("X-KDE-Submenu={}\n", submenu_name(kind));
        let mut c = head(mime, &ids.join(";"), &sm);
        if with_formats {
            for (key, text, icon) in formats {
                c.push_str(&format!("[Desktop Action convert_{key}]\nName={text}\nExec={exe} {action} --yes --format {key} %F\nIcon={icon}\n\n"));
            }
        }
        if with_stats {
            c.push_str(&format!("[Desktop Action stats]\n{}Exec={exe} stats %F\nIcon=documentinfo\n", names(&STATS)));
        }
        Some((format!("mmt-{kind}.desktop"), c))
    };
    files.extend(sub("audio", AUDIO_MIME, AUDIO_FORMATS, "convert-audio"));
    files.extend(sub("image", IMAGE_MIME, IMAGE_FORMATS, "convert-image"));

    // own items
    for c in customs(s) {
        let id = format!("custom_{}", c.id);
        let mut f = head(kind_mime(&c.kind), &id, "X-KDE-Priority=TopLevel\n");
        f.push_str(&format!("[Desktop Action {id}]\nName={}\nExec={exe} run {} %F\nIcon={}\n", c.name, c.id, kind_icon(&c.kind)));
        files.push((format!("mmt-custom-{}.desktop", c.id), f));
    }
    files
}

fn kde_dir() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home_dir().join(".local/share"));
    data.join("kio/servicemenus")
}

fn kde_install(exe: &Path, st: &Settings) -> Result<Vec<String>, String> {
    let dir = kde_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut log = Vec::new();
    // The shell-script version of the project used multimedia-*.desktop: it would show everything twice
    if let Ok(rd) = fs::read_dir(&dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with("multimedia-") && name.ends_with(".desktop") && fs::remove_file(e.path()).is_ok() {
                log.push(format!("removed the old {name}"));
            }
        }
    }
    let old_scripts = config::home_dir().join(".local/share/multimedia-context-menu-scripts");
    if old_scripts.is_dir() && fs::remove_dir_all(&old_scripts).is_ok() {
        log.push(format!("removed the old {}", old_scripts.display()));
    }
    // an earlier install of our own: remove it, so items that were switched off disappear
    remove_matching(&dir, "mmt-", ".desktop");
    for (name, content) in kde_files(&exe.to_string_lossy(), st) {
        let path = dir.join(&name);
        fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
        set_mode(&path, 0o755); // Plasma 6 only runs service menus that are executable
    }
    log.push(format!("KDE (Dolphin): {}", dir.display()));
    for tool in ["kbuildsycoca6", "kbuildsycoca5"] {
        if std::process::Command::new(tool).arg("--noincremental").output().is_ok() {
            break;
        }
    }
    Ok(log)
}

fn kde_uninstall() -> Vec<String> {
    remove_matching(&kde_dir(), "mmt-", ".desktop")
}

fn remove_matching(dir: &Path, prefix: &str, suffix: &str) -> Vec<String> {
    let mut log = Vec::new();
    if let Ok(rd) = fs::read_dir(dir) {
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with(prefix) && name.ends_with(suffix) && fs::remove_file(e.path()).is_ok() {
                log.push(format!("removed {}", e.path().display()));
            }
        }
    }
    log
}

#[allow(unused_variables)]
fn set_mode(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode));
    }
}

// ------------------------------------------------------------------ GNOME (Files / Nautilus scripts)

const GNOME_ITEMS: &[&str] =
    &["compress", "convert-video", "convert-audio", "extract-audio", "delete-sound", "frames", "webp", "convert-image", "stats"];

fn item_for_action(action: &str) -> &'static Item {
    if action == "convert-image" {
        return &CONVERT_IMAGE;
    }
    VIDEO.iter().find(|i| i.action == action).unwrap_or(&STATS)
}

/// (file name, content). Files has no file type filter for scripts, so the program itself
/// picks what fits ("Convert Audio" works for audio files and for videos). Files sorts the
/// scripts by name: with `numbered_scripts` the names start with 01, 02 ... to keep the order.
pub fn gnome_files(exe: &str, s: &Settings) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut entries: Vec<(String, String, String)> = Vec::new(); // (name, comment, command line)
    for action in GNOME_ITEMS.iter().filter(|a| s.enabled(a)) {
        let it = item_for_action(action);
        entries.push((it.name.to_string(), it.name.to_string(), (*action).to_string()));
    }
    for c in customs(s) {
        entries.push((c.name.clone(), c.name.clone(), format!("run {}", sh_quote(&c.id))));
    }
    for (i, (name, comment, command)) in entries.iter().enumerate() {
        let file = if s.numbered_scripts { format!("{:02} {name}", i + 1) } else { name.clone() };
        let content = format!("#!/bin/sh\n# Media Tools: {comment}\nexec {} {command} \"$@\"\n", sh_quote(exe));
        out.push((file, content));
    }
    out
}

fn gnome_dir() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home_dir().join(".local/share"));
    data.join("nautilus/scripts/Media Tools")
}

fn gnome_install(exe: &Path, st: &Settings) -> Result<Vec<String>, String> {
    let dir = gnome_dir();
    if dir.exists() {
        let _ = fs::remove_dir_all(&dir); // replace our own folder, and only that
    }
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for (name, content) in gnome_files(&exe.to_string_lossy(), st) {
        let path = dir.join(name);
        fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
        set_mode(&path, 0o755);
    }
    Ok(vec![format!("GNOME (Files): right-click > Scripts > Media Tools  ({})", dir.display())])
}

fn gnome_uninstall() -> Vec<String> {
    let dir = gnome_dir();
    if dir.is_dir() && fs::remove_dir_all(&dir).is_ok() {
        vec![format!("removed {}", dir.display())]
    } else {
        vec![]
    }
}

// ------------------------------------------------------------------ XFCE (Thunar custom actions)

/// The <action> blocks of Thunar's uca.xml
pub fn thunar_actions(exe: &str, s: &Settings) -> String {
    let mut out = String::new();
    let mut add = |id: &str, name: &str, command: &str, types: &[&str], icon: &str| {
        let types: String = types.iter().map(|t| format!("\t<{t}/>\n")).collect();
        out.push_str(&format!(
            "<action>\n\t<icon>{}</icon>\n\t<name>{}</name>\n\t<unique-id>mmt-{}</unique-id>\n\t<command>{} {} %F</command>\n\t<description>{}</description>\n\t<patterns>*</patterns>\n{}</action>\n",
            icon,
            xml_escape(name),
            id,
            xml_escape(&desktop_quote(exe)),
            command,
            xml_escape(name),
            types
        ));
    };
    for it in VIDEO.iter().filter(|i| !matches!(i.action, "convert-audio" | "stats") && s.enabled(i.action)) {
        add(it.action, it.name, it.action, &["video-files"], it.icon);
    }
    if s.enabled("convert-audio") {
        add("convert-audio", item_for_action("convert-audio").name, "convert-audio", &["audio-files", "video-files"], "audio-x-generic");
    }
    if s.enabled("convert-image") {
        add("convert-image", CONVERT_IMAGE.name, "convert-image", &["image-files"], "image-x-generic");
    }
    if s.enabled("stats") {
        add("stats", STATS.name, "stats", &["audio-files", "image-files", "video-files"], "documentinfo");
    }
    for c in customs(s) {
        let types = match c.kind.as_str() {
            "audio" => "audio-files",
            "image" => "image-files",
            _ => "video-files",
        };
        add(&format!("custom-{}", c.id), &c.name, &format!("run {}", c.id), &[types], kind_icon(&c.kind));
    }
    out
}

/// Insert our actions into the user's uca.xml (replacing the ones of an earlier install),
/// everything else in the file stays untouched.
pub fn thunar_merge(existing: Option<&str>, ours: &str) -> String {
    let base = match existing {
        Some(text) if text.contains("</actions>") => strip_mmt_actions(text),
        _ => "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n</actions>\n".to_string(),
    };
    let pos = base.rfind("</actions>").expect("checked above");
    format!("{}{}{}", &base[..pos], ours, &base[pos..])
}

pub fn strip_mmt_actions(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<action>") {
        let Some(len) = rest[start..].find("</action>") else { break };
        let end = start + len + "</action>".len();
        let block = &rest[start..end];
        if block.contains("<unique-id>mmt-") {
            out.push_str(&rest[..start]);
            // also swallow the line break after the block
            rest = rest[end..].strip_prefix('\n').unwrap_or(&rest[end..]);
        } else {
            out.push_str(&rest[..end]);
            rest = &rest[end..];
        }
    }
    out.push_str(rest);
    out
}

fn thunar_file() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home_dir().join(".config"));
    base.join("Thunar/uca.xml")
}

fn xfce_install(exe: &Path, st: &Settings) -> Result<Vec<String>, String> {
    let path = thunar_file();
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    let existing = fs::read_to_string(&path).ok();
    let merged = thunar_merge(existing.as_deref(), &thunar_actions(&exe.to_string_lossy(), st));
    fs::write(&path, merged).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(vec![format!("XFCE (Thunar): {}  (restart Thunar: thunar -q)", path.display())])
}

fn xfce_uninstall() -> Vec<String> {
    let path = thunar_file();
    match fs::read_to_string(&path) {
        Ok(text) if text.contains("<unique-id>mmt-") => {
            if fs::write(&path, strip_mmt_actions(&text)).is_ok() {
                vec![format!("removed the mmt actions from {}", path.display())]
            } else {
                vec![]
            }
        }
        _ => vec![],
    }
}

// ------------------------------------------------------------------ Windows (.reg)

fn reg_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// The text of the .reg file. The classic context menu of Explorer: verbs under
/// SystemFileAssociations\.ext\shell, audio and image conversions as a cascade.
/// `uninstall` removes the keys of ALL items (also the switched-off ones, so an update
/// can start from a clean state).
pub fn reg_text(exe: &str, uninstall: bool, s: &Settings) -> String {
    let mut out = String::from("Windows Registry Editor Version 5.00\r\n\r\n");
    let root = r"HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations";
    let command = |action_args: &str| format!("\"{}\" {action_args} --batch \"%1\"", exe);
    let mut keys: Vec<String> = Vec::new(); // everything we own, for the uninstall file
    let mut body: Vec<String> = Vec::new();

    for (exts, kind) in [(VIDEO_EXT, "video"), (AUDIO_EXT, "audio"), (IMAGE_EXT, "image")] {
        for ext in exts {
            let base = format!(r"{root}\.{ext}\shell");
            if kind == "video" {
                for (i, it) in VIDEO.iter().enumerate() {
                    let key = format!(r"{base}\mmt_{:02}", i + 1);
                    keys.push(key.clone());
                    if s.enabled(it.action) {
                        body.push(format!("[{key}]\r\n\"MUIVerb\"=\"{}\"\r\n", reg_escape(it.name)));
                        body.push(format!("[{key}\\command]\r\n@=\"{}\"\r\n", reg_escape(&command(it.action))));
                    }
                }
            } else {
                let (formats, action): (&[(&str, &str, &str)], &str) =
                    if kind == "audio" { (AUDIO_FORMATS, "convert-audio") } else { (IMAGE_FORMATS, "convert-image") };
                let top = format!("{base}\\mmt_01");
                keys.push(top.clone());
                let with_formats = s.enabled(action);
                let with_stats = s.enabled("stats");
                if with_formats || with_stats {
                    body.push(format!("[{top}]\r\n\"MUIVerb\"=\"{}\"\r\n\"SubCommands\"=\"\"\r\n", reg_escape(submenu_name(kind))));
                    if with_formats {
                        for (i, (key, text, _)) in formats.iter().enumerate() {
                            let sub = format!("{top}\\shell\\{:02}_{key}", i + 1);
                            body.push(format!("[{sub}]\r\n\"MUIVerb\"=\"{text}\"\r\n"));
                            body.push(format!("[{sub}\\command]\r\n@=\"{}\"\r\n", reg_escape(&command(&format!("{action} --yes --format {key}")))));
                        }
                    }
                    if with_stats {
                        let sub = format!("{top}\\shell\\{:02}_stats", formats.len() + 1);
                        body.push(format!("[{sub}]\r\n\"MUIVerb\"=\"{}\"\r\n", reg_escape(STATS.name)));
                        body.push(format!("[{sub}\\command]\r\n@=\"{}\"\r\n", reg_escape(&command("stats"))));
                    }
                }
            }
        }
    }
    // own items: one verb per extension of their kind
    for c in customs(s) {
        for ext in kind_exts(&c.kind) {
            let key = format!(r"{root}\.{ext}\shell\mmt_c_{}", c.id);
            keys.push(key.clone());
            body.push(format!("[{key}]\r\n\"MUIVerb\"=\"{}\"\r\n", reg_escape(&c.name)));
            body.push(format!("[{key}\\command]\r\n@=\"{}\"\r\n", reg_escape(&command(&format!("run {}", c.id)))));
        }
    }
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

fn reg_import(file: &Path) -> Result<(), String> {
    let mut cmd = std::process::Command::new("reg");
    cmd.arg("import").arg(file);
    crate::engine::hide_console(&mut cmd);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!("reg import failed: {}", String::from_utf8_lossy(&out.stderr)))
    }
}

fn windows_install(exe: &Path, st: &Settings) -> Result<Vec<String>, String> {
    let dir = config::config_dir();
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let exe_text = exe.to_string_lossy().to_string();
    let file = dir.join("mmt-menu.reg");
    let off = dir.join("mmt-menu-uninstall.reg");
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

fn windows_uninstall() -> Vec<String> {
    let file = config::config_dir().join("mmt-menu-uninstall.reg");
    if cfg!(windows) && file.exists() {
        match reg_import(&file) {
            Ok(()) => vec!["Windows: menu items removed".into()],
            Err(_) => vec![format!("could not import {}; double-click it to remove the items", file.display())],
        }
    } else {
        vec![]
    }
}

// ------------------------------------------------------------------ PCManFM (Raspberry Pi OS, LXDE, LXQt)

/// libfm "file manager actions": flat entries (PCManFM has no dialog-less submenus we could rely on)
pub fn pcmanfm_files(exe: &str, s: &Settings) -> Vec<(String, String)> {
    let exe = desktop_quote(exe);
    let mut out = Vec::new();
    let mut add = |id: &str, name: &str, icon: &str, mime: &str, command: &str| {
        let content = format!(
            "[Desktop Entry]\nType=Action\nName={name}\nIcon={icon}\nProfiles=on-all;\n\n[X-Action-Profile on-all]\nMimeTypes={mime}\nExec={exe} {command} %F\nName=Default profile\n"
        );
        out.push((format!("mmt-{id}.desktop"), content));
    };
    for it in VIDEO.iter().filter(|i| i.action != "convert-audio" && i.action != "stats" && s.enabled(i.action)) {
        add(it.id, it.name, it.icon, VIDEO_MIME, it.action);
    }
    if s.enabled("convert-audio") {
        let it = item_for_action("convert-audio");
        add("convertAudio", it.name, "audio-x-generic", &format!("{AUDIO_MIME};{VIDEO_MIME}"), "convert-audio");
    }
    if s.enabled("convert-image") {
        add("convertImage", CONVERT_IMAGE.name, "image-x-generic", IMAGE_MIME, "convert-image");
    }
    if s.enabled("stats") {
        add("stats", STATS.name, "documentinfo", &format!("{VIDEO_MIME};{AUDIO_MIME};{IMAGE_MIME}"), "stats");
    }
    for c in customs(s) {
        add(&format!("custom-{}", c.id), &c.name, kind_icon(&c.kind), kind_mime(&c.kind), &format!("run {}", c.id));
    }
    out
}

fn pcmanfm_dir() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home_dir().join(".local/share"));
    data.join("file-manager/actions")
}

fn pcmanfm_install(exe: &Path, st: &Settings) -> Result<Vec<String>, String> {
    let dir = pcmanfm_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    remove_matching(&dir, "mmt-", ".desktop");
    for (name, content) in pcmanfm_files(&exe.to_string_lossy(), st) {
        let path = dir.join(&name);
        fs::write(&path, content).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(vec![format!("PCManFM (Raspberry Pi OS / LXDE / LXQt): {}  (restart the file manager: pcmanfm -q)", dir.display())])
}

fn pcmanfm_uninstall() -> Vec<String> {
    remove_matching(&pcmanfm_dir(), "mmt-", ".desktop")
}

// ------------------------------------------------------------------ PATH and the launcher

const BLOCK_START: &str = "# >>> mmt >>>";
const BLOCK_END: &str = "# <<< mmt <<<";

/// The text with our marked block added (or replaced when it is there already)
pub fn with_block(text: &str, block: &str) -> String {
    let cleaned = without_block(text);
    let mut out = cleaned.trim_end_matches('\n').to_string();
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
    let end = b + BLOCK_END.len();
    let before = text[..a].trim_end_matches('\n');
    let after = text[end..].trim_start_matches('\n');
    match (before.is_empty(), after.is_empty()) {
        (true, true) => String::new(),
        (true, false) => format!("{after}"),
        (false, true) => format!("{before}\n"),
        (false, false) => format!("{before}\n\n{after}"),
    }
}

fn home_text(dir: &Path) -> String {
    let home = config::home_dir();
    match dir.strip_prefix(&home) {
        Ok(rel) => format!("$HOME/{}", rel.to_string_lossy()),
        Err(_) => dir.to_string_lossy().to_string(),
    }
}

/// Add the folder of the program to PATH for the shells of the user (Linux): ~/.profile, ~/.bashrc,
/// ~/.zshenv and the fish configuration. Each file gets a marked block that `uninstall` removes.
fn linux_path_install(bin_dir: &Path) -> Vec<String> {
    let in_path = std::env::var_os("PATH").map_or(false, |p| std::env::split_paths(&p).any(|d| d == bin_dir));
    let home = config::home_dir();
    let dir = home_text(bin_dir);
    let sh_block = format!("case \":$PATH:\" in *\":{dir}:\"*) ;; *) export PATH=\"{dir}:$PATH\" ;; esac");
    let mut done = Vec::new();
    let mut touch = |path: PathBuf, create: bool, block: &str| {
        let existing = fs::read_to_string(&path);
        if existing.is_err() && !create {
            return;
        }
        let text = existing.unwrap_or_default();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if fs::write(&path, with_block(&text, block)).is_ok() {
            done.push(path.display().to_string());
        }
    };
    touch(home.join(".profile"), true, &sh_block);
    touch(home.join(".bashrc"), false, &sh_block);
    if home.join(".zshenv").exists() || in_path_tool("zsh") {
        touch(home.join(".zshenv"), true, &sh_block);
    }
    let fish_dir = home.join(".config/fish");
    if fish_dir.exists() || in_path_tool("fish") {
        let fish_block = format!("if not contains -- {dir} $PATH\n    set -gx PATH {dir} $PATH\nend");
        let fish_block = fish_block.replace("$HOME", "$HOME");
        let path = fish_dir.join("conf.d/mmt-path.fish");
        let _ = fs::create_dir_all(path.parent().unwrap());
        if fs::write(&path, format!("{fish_block}\n")).is_ok() {
            done.push(path.display().to_string());
        }
    }
    if in_path {
        vec![format!("PATH: {} is already in PATH, `mmt` works in the terminal", bin_dir.display())]
    } else {
        vec![format!("PATH: added {} to {}", bin_dir.display(), done.join(", ")), "PATH: log out and in (or open a new terminal) to use `mmt` in the terminal".into()]
    }
}

fn linux_path_uninstall() -> Vec<String> {
    let home = config::home_dir();
    let mut log = Vec::new();
    for name in [".profile", ".bashrc", ".zshenv"] {
        let path = home.join(name);
        if let Ok(text) = fs::read_to_string(&path) {
            if text.contains(BLOCK_START) {
                let cleaned = without_block(&text);
                let _ = if cleaned.trim().is_empty() { fs::remove_file(&path) } else { fs::write(&path, cleaned) };
                log.push(format!("PATH: removed from {}", path.display()));
            }
        }
    }
    let fish = home.join(".config/fish/conf.d/mmt-path.fish");
    if fish.exists() && fs::remove_file(&fish).is_ok() {
        log.push(format!("PATH: removed {}", fish.display()));
    }
    log
}

fn in_path_tool(tool: &str) -> bool {
    config::find_tool(tool, "").is_some()
}

/// PowerShell that adds / removes a folder in the user's PATH (Windows)
pub fn ps_path_script(dir: &str, add: bool) -> String {
    let d = dir.replace('\'', "''");
    if add {
        format!("$d='{d}'; $p=[Environment]::GetEnvironmentVariable('Path','User'); if ($null -eq $p) {{ $p='' }}; if (($p -split ';') -notcontains $d) {{ [Environment]::SetEnvironmentVariable('Path', (($p.TrimEnd(';') + ';' + $d).TrimStart(';')), 'User') }}")
    } else {
        format!("$d='{d}'; $p=[Environment]::GetEnvironmentVariable('Path','User'); if ($null -ne $p) {{ [Environment]::SetEnvironmentVariable('Path', ((($p -split ';') | Where-Object {{ $_ -ne $d -and $_ -ne '' }}) -join ';'), 'User') }}")
    }
}

/// PowerShell that makes / removes the Start menu shortcut of the Settings window (Windows)
pub fn ps_shortcut_script(exe: &str, add: bool) -> String {
    let e = exe.replace('\'', "''");
    let lnk = "$env:APPDATA + '\\Microsoft\\Windows\\Start Menu\\Programs\\Media Tools.lnk'";
    if add {
        format!("$s=(New-Object -ComObject WScript.Shell).CreateShortcut({lnk}); $s.TargetPath='{e}'; $s.Arguments='settings'; $s.Description='Media Tools'; $s.Save()")
    } else {
        format!("Remove-Item -LiteralPath ({lnk}) -ErrorAction SilentlyContinue")
    }
}

fn powershell(script: &str) -> Result<(), String> {
    let mut cmd = std::process::Command::new("powershell");
    cmd.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script]);
    crate::engine::hide_console(&mut cmd);
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

/// The entry of the application menu that opens the Settings window
pub fn launcher_desktop(exe: &str) -> String {
    format!(
        "[Desktop Entry]\nType=Application\nName=Media Tools\nComment=Settings of the Media Tools context menu\nExec={} settings\nIcon=applications-multimedia\nTerminal=false\nCategories=AudioVideo;Settings;\n",
        desktop_quote(exe)
    )
}

fn launcher_path() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| config::home_dir().join(".local/share"));
    data.join("applications/mmt.desktop")
}

// ------------------------------------------------------------------ the command

fn desktop_names() -> Vec<String> {
    std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default().to_lowercase().split(':').map(str::to_string).collect()
}

fn in_path(tool: &str) -> bool {
    config::find_tool(tool, "").is_some()
}

fn local_bin() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(config::config_dir).join("mmt").join("mmt.exe")
    } else {
        config::home_dir().join(".local/bin/mmt")
    }
}

/// Copy the program to a permanent place so the downloaded file can be deleted
fn place_binary(no_copy: bool) -> Result<PathBuf, String> {
    let me = std::env::current_exe().map_err(|e| e.to_string())?;
    if no_copy {
        return Ok(me);
    }
    let target = local_bin();
    let same = match (fs::canonicalize(&me), fs::canonicalize(&target)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    };
    if !same {
        fs::create_dir_all(target.parent().unwrap()).map_err(|e| e.to_string())?;
        let tmp = target.with_extension("new");
        fs::copy(&me, &tmp).map_err(|e| format!("{}: {e}", tmp.display()))?;
        set_mode(&tmp, 0o755);
        fs::rename(&tmp, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    Ok(target)
}

/// Returns (exit code, lines to show). A program without a console (Windows) shows them in a window.
pub fn run(action: &str, flags: &[String]) -> (i32, Vec<String>) {
    let has = |f: &str| flags.iter().any(|x| x == f);
    let cfg = Config::load();
    let st = &cfg.settings;
    let mut out: Vec<String> = Vec::new();
    let mut targets: Vec<&str> = ["kde", "gnome", "xfce", "pcmanfm", "windows"].into_iter().filter(|t| has(t)).collect();
    if targets.is_empty() {
        if cfg!(windows) {
            targets.push("windows");
        } else {
            let desk = desktop_names();
            let is = |d: &str| desk.iter().any(|x| x.contains(d));
            if is("kde") || in_path("dolphin") {
                targets.push("kde");
            }
            if is("gnome") || is("unity") || in_path("nautilus") {
                targets.push("gnome");
            }
            if is("xfce") || in_path("thunar") {
                targets.push("xfce");
            }
            if is("lxde") || is("lxqt") || is("labwc") || is("wayfire") || in_path("pcmanfm") || in_path("pcmanfm-qt") {
                targets.push("pcmanfm");
            }
        }
    }
    if targets.is_empty() {
        out.push("no supported file manager found; choose one with --kde, --gnome, --xfce or --pcmanfm".into());
        return (1, out);
    }

    if action == "uninstall" {
        for t in &targets {
            let log = match *t {
                "kde" => kde_uninstall(),
                "gnome" => gnome_uninstall(),
                "xfce" => xfce_uninstall(),
                "pcmanfm" => pcmanfm_uninstall(),
                _ => windows_uninstall(),
            };
            out.extend(log);
        }
        let launcher = launcher_path();
        if !cfg!(windows) {
            if fs::remove_file(&launcher).is_ok() {
                out.push(format!("removed {}", launcher.display()));
            }
            out.extend(linux_path_uninstall());
        } else {
            let exe = local_bin();
            let _ = powershell(&ps_path_script(&exe.parent().unwrap_or(Path::new("")).to_string_lossy(), false));
            let _ = powershell(&ps_shortcut_script(&exe.to_string_lossy(), false));
        }
        out.push(format!("done (the program itself and {} were left in place)", config::config_dir().display()));
        return (0, out);
    }

    let mut ok = true;
    let exe = match place_binary(has("no-copy")) {
        Ok(p) => p,
        Err(e) => {
            out.push(e);
            return (1, out);
        }
    };
    out.push(format!("program: {}", exe.display()));
    let bin_dir = exe.parent().unwrap_or(Path::new("")).to_path_buf();
    for t in &targets {
        let res = match *t {
            "kde" => kde_install(&exe, st),
            "gnome" => gnome_install(&exe, st),
            "xfce" => xfce_install(&exe, st),
            "pcmanfm" => pcmanfm_install(&exe, st),
            _ => windows_install(&exe, st),
        };
        match res {
            Ok(log) => out.extend(log),
            Err(e) => {
                out.push(format!("{t}: {e}"));
                ok = false;
            }
        }
    }

    // `mmt` in the terminal, and a "Media Tools" entry (the Settings window) in the application menu
    if cfg!(windows) {
        if !has("no-path") {
            match powershell(&ps_path_script(&bin_dir.to_string_lossy(), true)) {
                Ok(()) => out.push(format!("PATH: {} added to the user PATH (open a new terminal)", bin_dir.display())),
                Err(e) => out.push(format!("PATH: could not change it ({e})")),
            }
        }
        match powershell(&ps_shortcut_script(&exe.to_string_lossy(), true)) {
            Ok(()) => out.push("Start menu: \"Media Tools\" opens the Settings".into()),
            Err(e) => out.push(format!("Start menu: shortcut not created ({e})")),
        }
    } else {
        if !has("no-path") {
            out.extend(linux_path_install(&bin_dir));
        }
        let launcher = launcher_path();
        if let Some(dir) = launcher.parent() {
            let _ = fs::create_dir_all(dir);
        }
        if fs::write(&launcher, launcher_desktop(&exe.to_string_lossy())).is_ok() {
            set_mode(&launcher, 0o755);
            out.push(format!("application menu: \"Media Tools\" opens the Settings ({})", launcher.display()));
        }
    }
    if !in_path("ffmpeg") || !in_path("ffprobe") {
        out.push("warning: ffmpeg / ffprobe not found in PATH; install ffmpeg (or put ffmpeg and ffprobe next to mmt)".into());
    }
    (if ok { 0 } else { 1 }, out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings::default()
    }

    fn with_custom() -> Settings {
        let mut s = settings();
        s.custom.push(Custom { id: "gif".into(), name: "Make GIF".into(), kind: "video".into(), ..Default::default() });
        s
    }

    #[test]
    fn kde_files_are_complete() {
        let files = kde_files("/home/u/.local/bin/mmt", &settings());
        assert_eq!(files.len(), 10); // 8 video items + audio + image submenus
        let compress = &files.iter().find(|(n, _)| n == "mmt-01_compressVideo.desktop").unwrap().1;
        assert!(compress.contains("Exec=/home/u/.local/bin/mmt compress %F"));
        assert!(compress.contains("X-KDE-Priority=TopLevel"));
        assert!(compress.contains("Name=Compress Video"));
        let audio = &files.iter().find(|(n, _)| n == "mmt-audio.desktop").unwrap().1;
        assert!(audio.contains("X-KDE-Submenu=Convert Audio"));
        assert!(audio.contains("Exec=/home/u/.local/bin/mmt convert-audio --yes --format mp3 %F"));
        assert!(audio.contains("Actions=convert_wav;convert_mp3;"));
    }

    #[test]
    fn switched_off_items_and_own_items() {
        let mut s = with_custom();
        s.menu.disabled = vec!["delete-sound".into(), "convert-image".into(), "stats".into()];
        let files = kde_files("/bin/mmt", &s);
        let names: Vec<&str> = files.iter().map(|(n, _)| n.as_str()).collect();
        assert!(!names.contains(&"mmt-05_deleteSound.desktop"));
        assert!(!names.contains(&"mmt-08_stats.desktop"));
        assert!(!names.contains(&"mmt-image.desktop")); // no formats and no stats: no submenu at all
        assert!(names.contains(&"mmt-audio.desktop"));
        let audio = &files.iter().find(|(n, _)| n == "mmt-audio.desktop").unwrap().1;
        assert!(!audio.contains("stats"));
        let custom = &files.iter().find(|(n, _)| n == "mmt-custom-gif.desktop").unwrap().1;
        assert!(custom.contains("Exec=/bin/mmt run gif %F") && custom.contains("Name=Make GIF"));
        // GNOME, XFCE, PCManFM follow the same rules
        let g = gnome_files("/bin/mmt", &s);
        assert!(g.iter().all(|(n, _)| n != "Delete sound") && g.iter().any(|(n, c)| n == "Make GIF" && c.contains("run 'gif'")));
        assert!(!thunar_actions("/bin/mmt", &s).contains("mmt-delete-sound"));
        assert!(thunar_actions("/bin/mmt", &s).contains("<unique-id>mmt-custom-gif</unique-id>"));
        let p = pcmanfm_files("/bin/mmt", &s);
        assert!(p.iter().any(|(n, _)| n == "mmt-custom-gif.desktop") && !p.iter().any(|(n, _)| n == "mmt-05_deleteSound.desktop"));
    }

    #[test]
    fn exec_path_with_spaces_is_quoted() {
        assert_eq!(desktop_quote("/home/my user/mmt"), "\"/home/my user/mmt\"");
        assert_eq!(desktop_quote("/usr/bin/mmt"), "/usr/bin/mmt");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
    }

    #[test]
    fn gnome_scripts_pass_the_selection_and_numbers_are_optional() {
        let files = gnome_files("/opt/my mmt/mmt", &settings());
        assert_eq!(files.len(), 9);
        assert_eq!(files[0].0, "Compress Video"); // plain names by default
        assert!(files[0].1.contains("exec '/opt/my mmt/mmt' compress \"$@\""));
        assert!(files[0].1.starts_with("#!/bin/sh\n"));
        let mut s = settings();
        s.numbered_scripts = true;
        assert_eq!(gnome_files("/bin/mmt", &s)[0].0, "01 Compress Video");
    }

    #[test]
    fn pcmanfm_actions_are_flat_entries_with_mime_types() {
        let files = pcmanfm_files("/bin/mmt", &settings());
        let compress = &files.iter().find(|(n, _)| n == "mmt-01_compressVideo.desktop").unwrap().1;
        assert!(compress.starts_with("[Desktop Entry]\nType=Action\n"));
        assert!(compress.contains("Profiles=on-all;"));
        assert!(compress.contains("[X-Action-Profile on-all]\nMimeTypes=video/mp4;"));
        assert!(compress.contains("Exec=/bin/mmt compress %F"));
        let audio = &files.iter().find(|(n, _)| n == "mmt-convertAudio.desktop").unwrap().1;
        assert!(audio.contains("audio/mpeg") && audio.contains("video/mp4"));
    }

    #[test]
    fn thunar_merge_keeps_foreign_actions_and_is_idempotent() {
        let foreign = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n<action>\n\t<name>Open Terminal</name>\n\t<unique-id>123-1</unique-id>\n\t<command>xterm</command>\n</action>\n</actions>\n";
        let ours = thunar_actions("/bin/mmt", &settings());
        let once = thunar_merge(Some(foreign), &ours);
        let twice = thunar_merge(Some(&once), &ours);
        assert_eq!(once, twice);
        assert!(once.contains("<unique-id>123-1</unique-id>"));
        assert_eq!(once.matches("<unique-id>mmt-").count(), 9);
        assert!(once.contains("<command>/bin/mmt compress %F</command>"));
        assert!(once.contains("<video-files/>"));
        assert_eq!(strip_mmt_actions(&once), foreign);
        assert!(thunar_merge(None, &ours).starts_with("<?xml"));
    }

    #[test]
    fn reg_file_structure() {
        let t = reg_text(r"C:\Users\me\AppData\Local\mmt\mmt.exe", false, &settings());
        assert!(t.starts_with("Windows Registry Editor Version 5.00\r\n"));
        assert!(t.contains(r"[HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.mp4\shell\mmt_01]"));
        assert!(t.contains("\"MUIVerb\"=\"Compress Video\""));
        assert!(t.contains(r#"@="\"C:\\Users\\me\\AppData\\Local\\mmt\\mmt.exe\" compress --batch \"%1\"""#));
        assert!(t.contains(r"SystemFileAssociations\.mp3\shell\mmt_01\shell\02_mp3\command]"));
        assert!(t.contains("convert-audio --yes --format mp3 --batch"));
        assert!(t.contains("\"SubCommands\"=\"\""));
        let off = reg_text("x.exe", true, &settings());
        assert!(off.contains(r"[-HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.mp4\shell\mmt_08]"));
        assert!(!off.contains("MUIVerb"));
    }

    #[test]
    fn reg_follows_the_settings() {
        let mut s = with_custom();
        s.menu.disabled = vec!["delete-sound".into()];
        let t = reg_text("x.exe", false, &s);
        assert!(!t.contains(r"\.mp4\shell\mmt_05]")); // numbering stays stable, the item is gone
        assert!(t.contains(r"\.mp4\shell\mmt_06]"));
        assert!(t.contains(r"\.mkv\shell\mmt_c_gif]") && t.contains("run gif --batch"));
        // the uninstall file still knows the switched-off item and the own one
        let off = reg_text("x.exe", true, &s);
        assert!(off.contains(r"[-HKEY_CURRENT_USER\Software\Classes\SystemFileAssociations\.mp4\shell\mmt_05]"));
        assert!(off.contains(r"\.mp4\shell\mmt_c_gif]"));
    }

    #[test]
    fn reg_is_utf16le_with_bom() {
        let b = reg_bytes("Aя");
        assert_eq!(b, vec![0xFF, 0xFE, b'A', 0, 0x4F, 0x04]);
    }

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
    fn powershell_scripts_quote_the_paths() {
        let add = ps_path_script(r"C:\Users\O'Neil\mmt", true);
        assert!(add.contains(r"$d='C:\Users\O''Neil\mmt'") && add.contains("SetEnvironmentVariable('Path'"));
        assert!(ps_path_script("C:\\mmt", false).contains("Where-Object"));
        assert!(ps_shortcut_script(r"C:\mmt\mmt.exe", true).contains("$s.Arguments='settings'"));
        assert!(ps_shortcut_script("x", false).starts_with("Remove-Item"));
    }

    #[test]
    fn launcher_entry_opens_the_settings() {
        let d = launcher_desktop("/home/u/.local/bin/mmt");
        assert!(d.contains("Exec=/home/u/.local/bin/mmt settings") && d.contains("Categories=AudioVideo;Settings;"));
    }
}
