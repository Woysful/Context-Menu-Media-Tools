//! The kinds of files (by extension) and what ffprobe says is in a file.

use crate::units::{fmt_duration, fmt_rate};
use serde_json::Value;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Video,
    Audio,
    Image,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::Video, Kind::Audio, Kind::Image];

    /// The name used in settings.toml ("video" | "audio" | "image"); anything else is video
    pub fn from_name(name: &str) -> Kind {
        match name {
            "audio" => Kind::Audio,
            "image" => Kind::Image,
            _ => Kind::Video,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Kind::Video => "video",
            Kind::Audio => "audio",
            Kind::Image => "image",
        }
    }

    pub fn extensions(self) -> &'static [&'static str] {
        match self {
            Kind::Video => &["mp4", "avi", "mov", "mkv", "webm", "m4v"],
            Kind::Audio => &["wav", "mp3", "flac", "aac", "ogg", "wma", "m4a", "weba", "opus"],
            Kind::Image => &["jpg", "jpeg", "png", "bmp", "tiff", "tif", "gif", "webp", "tga", "psd"],
        }
    }

    /// MIME types of the file managers (`;`-separated, like desktop files want them)
    pub fn mime(self) -> &'static str {
        match self {
            Kind::Video => "video/mp4;video/x-msvideo;video/quicktime;video/x-matroska;video/webm",
            Kind::Audio => "audio/wav;audio/mpeg;audio/flac;audio/aac;audio/ogg;audio/x-ms-wma;audio/mp4;audio/webm;audio/opus",
            Kind::Image => "image/jpeg;image/png;image/bmp;image/tiff;image/gif;image/webp;image/x-tga;image/vnd.adobe.photoshop",
        }
    }

    /// Icon of the freedesktop icon theme
    pub fn icon(self) -> &'static str {
        match self {
            Kind::Video => "video-x-generic",
            Kind::Audio => "audio-x-generic",
            Kind::Image => "image-x-generic",
        }
    }

    pub fn of(path: &Path) -> Option<Kind> {
        let ext = path.extension()?.to_string_lossy().to_lowercase();
        Kind::ALL.into_iter().find(|k| k.extensions().contains(&ext.as_str()))
    }
}

#[derive(Clone, Debug, Default)]
pub struct Info {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f64>,
    pub duration: Option<f64>,
    pub video_bps: Option<f64>,
    /// the video bitrate was derived from the file size (MKV / WebM store none)
    pub bitrate_estimated: bool,
    pub audio_bps: Option<f64>,
    pub has_audio: bool,
}

fn num(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// "30000/1001" -> 29.97
fn ratio(text: &str) -> Option<f64> {
    let (a, b) = text.split_once('/')?;
    let (a, b) = (a.parse::<f64>().ok()?, b.parse::<f64>().ok()?);
    (a > 0.0 && b > 0.0).then(|| a / b)
}

/// Parse the JSON of `ffprobe -print_format json -show_format -show_streams`
pub fn parse_probe(name: &str, json: &str) -> Option<Info> {
    let data: Value = serde_json::from_str(json).ok()?;
    let streams = data.get("streams")?.as_array()?;
    let fmt = data.get("format");
    let is_cover = |s: &Value| s.get("disposition").is_some_and(|d| num(d.get("attached_pic")).unwrap_or(0.0) != 0.0);
    let video = streams.iter().find(|s| s["codec_type"] == "video" && !is_cover(s));
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let mut info = Info { name: name.to_string(), has_audio: audio.is_some(), ..Default::default() };
    info.duration = num(fmt.and_then(|f| f.get("duration"))).or_else(|| num(video.and_then(|v| v.get("duration"))));
    info.audio_bps = audio.and_then(|a| num(a.get("bit_rate")));

    if let Some(v) = video {
        let mut w = num(v.get("width")).unwrap_or(0.0) as u32;
        let mut h = num(v.get("height")).unwrap_or(0.0) as u32;
        // Phone videos are often stored sideways and rotated by metadata: use the picture as played
        let side = v.get("side_data_list").and_then(|l| l.as_array()).and_then(|list| list.iter().rev().find_map(|s| num(s.get("rotation"))));
        let rotation = side.or_else(|| num(v.get("tags").and_then(|t| t.get("rotate")))).unwrap_or(0.0).round() as i64;
        if rotation.abs() % 180 == 90 {
            std::mem::swap(&mut w, &mut h);
        }
        info.width = w;
        info.height = h;
        info.fps = ["avg_frame_rate", "r_frame_rate"].iter().find_map(|k| v.get(*k).and_then(|x| x.as_str()).and_then(ratio));
        info.video_bps = num(v.get("bit_rate"));
        if info.video_bps.is_none() {
            if let (Some(d), Some(size)) = (info.duration.filter(|d| *d > 0.0), num(fmt.and_then(|f| f.get("size")))) {
                info.video_bps = Some((size * 8.0 / d - info.audio_bps.unwrap_or(0.0)).max(0.0));
                info.bitrate_estimated = true;
            }
        }
    }
    Some(info)
}

/// What ffprobe says about the file, as JSON (empty when it cannot be run)
pub fn probe_json(ffprobe: &Path, file: &Path) -> String {
    match crate::proc::command(ffprobe).args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"]).arg(file).output() {
        Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
        Err(_) => String::new(),
    }
}

pub fn probe(ffprobe: &Path, file: &Path) -> Option<Info> {
    let name = file.file_name()?.to_string_lossy().to_string();
    parse_probe(&name, &probe_json(ffprobe, file))
}

/// "1920×1080 · 30 fps · 6.1 Mbit/s · audio 69 kbit/s · 0:06"
pub fn describe(info: &Info, audio_word: &str) -> String {
    let mut parts = Vec::new();
    if info.width > 0 {
        parts.push(format!("{}×{}", info.width, info.height));
    }
    if let Some(f) = info.fps {
        let s = format!("{f:.2}");
        parts.push(format!("{} fps", s.trim_end_matches('0').trim_end_matches('.')));
    }
    if let Some(b) = info.video_bps {
        parts.push(format!("{}{}", if info.bitrate_estimated { "≈" } else { "" }, fmt_rate(b)));
    }
    if let Some(a) = info.audio_bps {
        parts.push(format!("{audio_word} {}", fmt_rate(a)));
    }
    if let Some(d) = info.duration {
        parts.push(fmt_duration(d));
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"streams":[
        {"codec_type":"video","width":1080,"height":1920,"avg_frame_rate":"30/1","bit_rate":"6100000",
         "side_data_list":[{"rotation":-90}]},
        {"codec_type":"audio","bit_rate":"69000"}],
        "format":{"duration":"6.0","size":"5000000"}}"#;

    #[test]
    fn parses_probe_and_swaps_rotated_sides() {
        let i = parse_probe("a.mp4", SAMPLE).unwrap();
        assert_eq!((i.width, i.height), (1920, 1080));
        assert_eq!(i.fps, Some(30.0));
        assert_eq!(i.duration, Some(6.0));
        assert!(i.has_audio);
        assert_eq!(describe(&i, "audio"), "1920×1080 · 30 fps · 6.1 Mbit/s · audio 69 kbit/s · 0:06");
    }

    #[test]
    fn bitrate_is_derived_when_missing() {
        let json = r#"{"streams":[{"codec_type":"video","width":640,"height":360,"avg_frame_rate":"25/1"}],
            "format":{"duration":"10","size":"1250000"}}"#;
        let i = parse_probe("a.mkv", json).unwrap();
        assert!(i.bitrate_estimated);
        assert_eq!(i.video_bps, Some(1_000_000.0));
        assert!(!i.has_audio);
    }

    #[test]
    fn cover_art_is_not_the_video() {
        let json = r#"{"streams":[{"codec_type":"audio","bit_rate":"320000"},
            {"codec_type":"video","width":500,"height":500,"disposition":{"attached_pic":1}}],"format":{"duration":"10"}}"#;
        let i = parse_probe("a.mp3", json).unwrap();
        assert_eq!(i.width, 0);
        assert!(i.has_audio);
    }

    #[test]
    fn kinds() {
        assert_eq!(Kind::of(Path::new("a.MKV")), Some(Kind::Video));
        assert_eq!(Kind::of(Path::new("a.flac")), Some(Kind::Audio));
        assert_eq!(Kind::of(Path::new("a.psd")), Some(Kind::Image));
        assert_eq!(Kind::of(Path::new("a.txt")), None);
        assert_eq!(Kind::from_name("audio"), Kind::Audio);
        assert_eq!(Kind::from_name("whatever"), Kind::Video);
    }
}
