//! XFCE (Thunar): `<action>`s with the unique id `cmmt-…` inside ~/.config/Thunar/uca.xml

use super::menu::{desktop_quote, flat_entries, thunar_type, xml_escape};
use crate::paths;
use crate::settings::Settings;
use std::fs;
use std::path::{Path, PathBuf};

/// The <action> blocks of Thunar's uca.xml
pub fn actions(exe: &str, s: &Settings) -> String {
    let exe = xml_escape(&desktop_quote(exe));
    let mut out = String::new();
    for e in flat_entries(s) {
        let mut types: Vec<&str> = e.kinds.iter().map(|k| thunar_type(*k)).collect();
        types.sort();
        let types: String = types.iter().map(|t| format!("\t<{t}/>\n")).collect();
        let name = xml_escape(&e.name);
        out.push_str(&format!(
            "<action>\n\t<icon>{}</icon>\n\t<name>{name}</name>\n\t<unique-id>cmmt-{}</unique-id>\n\t<command>{exe} {} %F</command>\n\t<description>{name}</description>\n\t<patterns>*</patterns>\n{types}</action>\n",
            e.icon,
            e.thunar_id,
            xml_escape(&e.args),
        ));
    }
    out
}

/// Insert our actions into the user's uca.xml (replacing the ones of an earlier install),
/// everything else in the file stays untouched.
pub fn merge(existing: Option<&str>, ours: &str) -> String {
    let base = match existing {
        Some(text) if text.contains("</actions>") => strip(text),
        _ => "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n</actions>\n".to_string(),
    };
    let pos = base.rfind("</actions>").expect("checked above");
    format!("{}{}{}", &base[..pos], ours, &base[pos..])
}

/// The text without the actions of cmmt
pub fn strip(text: &str) -> String {
    strip_named(text, "cmmt")
}

/// The text without the actions of the program called `name`
fn strip_named(text: &str, name: &str) -> String {
    let id = format!("<unique-id>{name}-");
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("<action>") {
        let Some(len) = rest[start..].find("</action>") else { break };
        let end = start + len + "</action>".len();
        if rest[start..end].contains(&id) {
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

fn file() -> PathBuf {
    paths::xdg_config().join("Thunar/uca.xml")
}

pub fn install(exe: &Path, st: &Settings) -> Result<Vec<String>, String> {
    let path = file();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let existing = fs::read_to_string(&path).ok();
    let merged = merge(existing.as_deref(), &actions(&exe.to_string_lossy(), st));
    fs::write(&path, merged).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(vec![format!("XFCE (Thunar): {}  (restart Thunar: thunar -q)", path.display())])
}

pub fn uninstall() -> Vec<String> {
    remove_named("cmmt")
}

/// Remove the actions of the program called `name`
pub fn remove_named(name: &str) -> Vec<String> {
    let path = file();
    match fs::read_to_string(&path) {
        Ok(text) if text.contains(&format!("<unique-id>{name}-")) && fs::write(&path, strip_named(&text, name)).is_ok() => {
            vec![format!("removed the {name} actions from {}", path.display())]
        }
        _ => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::with_custom;
    use super::*;
    use crate::action::Action;

    #[test]
    fn merge_keeps_foreign_actions_and_is_idempotent() {
        let foreign = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n<action>\n\t<name>Open Terminal</name>\n\t<unique-id>123-1</unique-id>\n\t<command>xterm</command>\n</action>\n</actions>\n";
        let ours = actions("/bin/cmmt", &Settings::default());
        let once = merge(Some(foreign), &ours);
        let twice = merge(Some(&once), &ours);
        assert_eq!(once, twice);
        assert!(once.contains("<unique-id>123-1</unique-id>"));
        assert_eq!(once.matches("<unique-id>cmmt-").count(), 9);
        assert!(once.contains("<command>/bin/cmmt compress %F</command>"));
        assert!(once.contains("<video-files/>"));
        assert!(once.contains("<unique-id>cmmt-convert-audio</unique-id>"));
        assert_eq!(strip(&once), foreign);
        assert!(merge(None, &ours).starts_with("<?xml"));
    }

    #[test]
    fn follow_the_settings() {
        let mut s = with_custom();
        s.set_enabled(Action::DeleteSound, false);
        let a = actions("/bin/cmmt", &s);
        assert!(!a.contains("cmmt-delete-sound"));
        assert!(a.contains("<unique-id>cmmt-custom-gif</unique-id>") && a.contains("<command>/bin/cmmt run gif %F</command>"));
        assert!(a.contains("<unique-id>cmmt-stats</unique-id>\n\t<command>/bin/cmmt stats %F</command>\n\t<description>Stats</description>\n\t<patterns>*</patterns>\n\t<audio-files/>\n\t<image-files/>\n\t<video-files/>\n"));
    }
}
