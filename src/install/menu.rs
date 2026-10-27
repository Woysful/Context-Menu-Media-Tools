//! What goes into every menu: the standard items in their order, the format submenus,
//! and the quoting rules of the files that are written.

use crate::action::Action;
use crate::i18n;
use crate::media::Kind;
use crate::settings::Settings;

pub struct Item {
    /// stable id of the item in the files (KDE action names, file names)
    pub id: &'static str,
    pub action: Action,
    /// KDE / freedesktop icon
    pub icon: &'static str,
}

impl Item {
    pub fn name(&self) -> &'static str {
        self.action.title(i18n::t())
    }
}

const fn item(id: &'static str, action: Action, icon: &'static str) -> Item {
    Item { id, action, icon }
}

/// The video items, in menu order
pub const VIDEO: &[Item] = &[
    item("01_compressVideo", Action::Compress, "video-compress"),
    item("02_convertVideo", Action::ConvertVideo, "video-mp4"),
    item("03_convertAudio", Action::ConvertAudio, "audio-x-generic"),
    item("04_extractAudio", Action::ExtractAudio, "audio-x-generic"),
    item("05_deleteSound", Action::DeleteSound, "media-track-remove-amarok"),
    item("06_makeFrames", Action::Frames, "image-x-generic"),
    item("07_convertWebPAnim", Action::Webp, "image-webp"),
    item("08_stats", Action::Stats, "documentinfo"),
];

pub const CONVERT_AUDIO: Item = item("convertAudio", Action::ConvertAudio, "audio-x-generic");
pub const CONVERT_IMAGE: Item = item("convertImage", Action::ConvertImage, "image-x-generic");
pub const STATS: Item = item("stats", Action::Stats, "documentinfo");

/// One entry of a format submenu: (format key, text, KDE icon)
pub type SubFormat = (&'static str, &'static str, &'static str);

const AUDIO_FORMATS: &[SubFormat] = &[
    ("wav", "WAV", "audio-wav"),
    ("mp3", "MP3", "audio-mpeg"),
    ("flac", "FLAC", "audio-flac"),
    ("aac", "AAC", "audio-aac"),
    ("ogg", "OGG", "audio-ogg"),
    ("wma", "WMA", "audio-x-ms-wma"),
    ("m4a", "M4A", "audio-mp4"),
    ("opus", "Opus", "audio-opus"),
];
const IMAGE_FORMATS: &[SubFormat] = &[
    ("jpg", "JPG", "image-jpeg"),
    ("png", "PNG", "image-png"),
    ("bmp", "BMP", "image-bmp"),
    ("tiff", "TIFF", "image-tiff"),
    ("gif", "GIF", "image-gif"),
    ("webp", "WebP", "image-webp"),
    ("tga", "TGA", "image-x-tga"),
];

/// Audio and image files get a submenu with one entry per format (no dialog) and Stats
pub struct Submenu {
    pub kind: Kind,
    pub name: &'static str,
    pub action: Action,
    pub formats: &'static [SubFormat],
}

pub const SUBMENUS: [Submenu; 2] = [
    Submenu { kind: Kind::Audio, name: "Convert Audio", action: Action::ConvertAudio, formats: AUDIO_FORMATS },
    Submenu { kind: Kind::Image, name: "Convert Image", action: Action::ConvertImage, formats: IMAGE_FORMATS },
];

/// One entry of a flat menu (Thunar, PCManFM): no submenus, every item at the top
pub struct Flat {
    /// the name of the action in Thunar's uca.xml (after "mmt-")
    pub thunar_id: String,
    /// the name of the PCManFM file (after "mmt-")
    pub file_id: String,
    pub name: String,
    pub icon: &'static str,
    pub kinds: Vec<Kind>,
    /// the arguments of mmt, before the files
    pub args: String,
}

/// The items of a flat menu: the video items, Convert Audio (audio files and videos),
/// Convert Image, Stats (every kind) and the own items
pub fn flat_entries(s: &Settings) -> Vec<Flat> {
    let mut out = Vec::new();
    let mut add = |it: &Item, kinds: Vec<Kind>| {
        if s.enabled(it.action) {
            let action = it.action.cli_name();
            out.push(Flat { thunar_id: action.into(), file_id: it.id.into(), name: it.name().into(), icon: it.icon, kinds, args: action.into() });
        }
    };
    for it in VIDEO.iter().filter(|i| ![Action::ConvertAudio, Action::Stats].contains(&i.action)) {
        add(it, vec![Kind::Video]);
    }
    add(&CONVERT_AUDIO, vec![Kind::Audio, Kind::Video]);
    add(&CONVERT_IMAGE, vec![Kind::Image]);
    add(&STATS, vec![Kind::Video, Kind::Audio, Kind::Image]);
    for c in s.menu_customs() {
        let kind = c.media_kind();
        let id = format!("custom-{}", c.id);
        out.push(Flat { thunar_id: id.clone(), file_id: id, name: c.name.clone(), icon: kind.icon(), kinds: vec![kind], args: format!("run {}", c.id) });
    }
    out
}
/// The file manager group of Thunar ("video-files", ...)
pub fn thunar_type(kind: Kind) -> &'static str {
    match kind {
        Kind::Video => "video-files",
        Kind::Audio => "audio-files",
        Kind::Image => "image-files",
    }
}

/// Exec= field of a desktop file: quoted when the path needs it
pub fn desktop_quote(arg: &str) -> String {
    if arg.chars().any(|c| c.is_whitespace() || "\"'\\<>~|&;$*?#()`".contains(c)) {
        format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\"").replace('$', "\\$").replace('`', "\\`"))
    } else {
        arg.to_string()
    }
}

pub fn sh_quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', "'\\''"))
}

pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(desktop_quote("/home/my user/mmt"), "\"/home/my user/mmt\"");
        assert_eq!(desktop_quote("/usr/bin/mmt"), "/usr/bin/mmt");
        assert_eq!(sh_quote("it's"), "'it'\\''s'");
        assert_eq!(xml_escape("a&<\">"), "a&amp;&lt;&quot;&gt;");
    }
}
