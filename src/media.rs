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
    pub audio: Vec<AudioTrack>,
    /// the number of frames of the video stream, where the file says it (an animated GIF)
    pub frames: Option<u64>,
    /// codec_name of the video stream ("mjpeg" for a JPEG)
    pub video_codec: String,
    /// bytes of the file
    pub size: Option<u64>,
}

/// One audio track of a file
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AudioTrack {
    /// codec_name of ffprobe ("aac")
    pub codec: String,
    pub channels: u32,
    pub layout: String,
    pub language: String,
    pub title: String,
}

impl AudioTrack {
    /// "AAC · stereo · eng · Commentary"
    pub fn describe(&self) -> String {
        let channels = match (self.layout.as_str(), self.channels) {
            ("", 0) => String::new(),
            ("", 1) => "mono".into(),
            ("", 2) => "stereo".into(),
            ("", n) => format!("{n} ch"),
            (l, _) => l.to_string(),
        };
        let language = if self.language == "und" { "" } else { &self.language };
        [crate::report::codec_name(&self.codec), channels, language.to_string(), self.title.clone()]
            .into_iter()
            .filter(|p| !p.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

fn tag(s: &Value, key: &str) -> String {
    let found = s.get("tags").and_then(|t| t.as_object()).and_then(|t| t.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)));
    found.and_then(|(_, v)| v.as_str()).unwrap_or_default().to_string()
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
    let mut info = Info { name: name.to_string(), ..Default::default() };
    info.duration = num(fmt.and_then(|f| f.get("duration"))).or_else(|| num(video.and_then(|v| v.get("duration"))));
    info.audio_bps = audio.and_then(|a| num(a.get("bit_rate")));
    info.size = num(fmt.and_then(|f| f.get("size"))).map(|s| s as u64);
    info.audio = streams
        .iter()
        .filter(|s| s["codec_type"] == "audio")
        .map(|s| AudioTrack {
            codec: s["codec_name"].as_str().unwrap_or_default().to_string(),
            channels: num(s.get("channels")).unwrap_or(0.0) as u32,
            layout: s["channel_layout"].as_str().unwrap_or_default().to_string(),
            language: tag(s, "language"),
            title: tag(s, "title"),
        })
        .collect();

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
        info.frames = num(v.get("nb_frames")).map(|n| n as u64);
        info.video_codec = v["codec_name"].as_str().unwrap_or_default().to_string();
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

/// The number of audio tracks of a file (a short ffprobe call, for many files)
pub fn audio_count(ffprobe: &Path, file: &Path) -> Option<usize> {
    let out = crate::proc::command(ffprobe)
        .args(["-v", "error", "-select_streams", "a", "-show_entries", "stream=index", "-of", "csv=p=0"])
        .arg(file)
        .output()
        .ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).lines().filter(|l| !l.trim().is_empty()).count())
}

/// The audio tracks of every file (those ffprobe cannot read are left out), a few files at a time
pub fn audio_counts(ffprobe: &Path, files: &[std::path::PathBuf]) -> Vec<usize> {
    let files = &files[..files.len().min(500)];
    let per_thread = files.len().div_ceil(8).max(1);
    std::thread::scope(|s| {
        let jobs: Vec<_> =
            files.chunks(per_thread).map(|part| s.spawn(move || part.iter().filter_map(|f| audio_count(ffprobe, f)).collect::<Vec<_>>())).collect();
        jobs.into_iter().flat_map(|j| j.join().unwrap_or_default()).collect()
    })
}

/// The size of a picture as ffmpeg decodes it: turned by its EXIF orientation (which a
/// stream of ffprobe does not show)
pub fn picture_size(ffprobe: &Path, file: &Path) -> Option<(u32, u32)> {
    let out = crate::proc::command(ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-read_intervals",
            "%+#1",
            "-show_frames",
            "-show_entries",
            "frame=width,height:side_data=rotation",
            "-of",
            "json",
        ])
        .arg(file)
        .output()
        .ok()?;
    parse_picture_size(&String::from_utf8_lossy(&out.stdout))
}

fn parse_picture_size(json: &str) -> Option<(u32, u32)> {
    let data: Value = serde_json::from_str(json).ok()?;
    let frame = data.get("packets_and_frames").or_else(|| data.get("frames"))?.as_array()?.iter().find(|f| f.get("width").is_some())?;
    let (w, h) = (num(frame.get("width"))? as u32, num(frame.get("height"))? as u32);
    let rotation = frame.get("side_data_list").and_then(|l| l.as_array()).and_then(|l| l.iter().find_map(|s| num(s.get("rotation")))).unwrap_or(0.0);
    Some(if (rotation.round() as i64).abs() % 180 == 90 { (h, w) } else { (w, h) })
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
    // a picture: "1600×1067 · JPEG · 214 KB"
    if Kind::of(Path::new(&info.name)) == Some(Kind::Image) {
        if !info.video_codec.is_empty() {
            parts.push(picture_format(&info.video_codec));
        }
        if let Some(n) = info.frames.filter(|n| *n > 1) {
            parts.push(format!("{n} frames"));
        }
        if let Some(s) = info.size {
            parts.push(crate::units::fmt_size(s));
        }
        return parts.join(" · ");
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

/// The name of a picture format by its ffmpeg codec
fn picture_format(codec: &str) -> String {
    match codec {
        "mjpeg" | "jpegls" => "JPEG".into(),
        "png" | "apng" => "PNG".into(),
        "webp" => "WebP".into(),
        "av1" => "AVIF".into(),
        "jpegxl" => "JPEG XL".into(),
        "gif" => "GIF".into(),
        "bmp" => "BMP".into(),
        "tiff" => "TIFF".into(),
        "targa" => "TGA".into(),
        other => other.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{"streams":[
        {"codec_type":"video","width":1080,"height":1920,"avg_frame_rate":"30/1","bit_rate":"6100000",
         "side_data_list":[{"rotation":-90}]},
        {"codec_type":"audio","codec_name":"aac","bit_rate":"69000","channels":2,"tags":{"language":"eng","title":"Commentary"}}],
        "format":{"duration":"6.0","size":"5000000"}}"#;

    #[test]
    fn parses_probe_and_swaps_rotated_sides() {
        let i = parse_probe("a.mp4", SAMPLE).unwrap();
        assert_eq!((i.width, i.height), (1920, 1080));
        assert_eq!(i.fps, Some(30.0));
        assert_eq!(i.duration, Some(6.0));
        assert!(!i.audio.is_empty());
        assert_eq!(i.audio.len(), 1);
        assert_eq!(i.audio[0].describe(), "AAC · stereo · eng · Commentary");
        assert_eq!(AudioTrack { codec: "pcm_s16le".into(), channels: 6, language: "und".into(), ..Default::default() }.describe(), "PCM (s16le) · 6 ch");
        assert_eq!(describe(&i, "audio"), "1920×1080 · 30 fps · 6.1 Mbit/s · audio 69 kbit/s · 0:06");
    }

    #[test]
    fn bitrate_is_derived_when_missing() {
        let json = r#"{"streams":[{"codec_type":"video","width":640,"height":360,"avg_frame_rate":"25/1"}],
            "format":{"duration":"10","size":"1250000"}}"#;
        let i = parse_probe("a.mkv", json).unwrap();
        assert!(i.bitrate_estimated);
        assert_eq!(i.video_bps, Some(1_000_000.0));
        assert!(i.audio.is_empty());
    }

    #[test]
    fn cover_art_is_not_the_video() {
        let json = r#"{"streams":[{"codec_type":"audio","bit_rate":"320000"},
            {"codec_type":"video","width":500,"height":500,"disposition":{"attached_pic":1}}],"format":{"duration":"10"}}"#;
        let i = parse_probe("a.mp3", json).unwrap();
        assert_eq!(i.width, 0);
        assert!(!i.audio.is_empty());
    }

    #[test]
    fn picture_size_follows_the_orientation() {
        let turned = r#"{"packets_and_frames":[{"type":"packet"},{"type":"frame","width":640,"height":480,"side_data_list":[{"rotation":-90}]}]}"#;
        assert_eq!(parse_picture_size(turned), Some((480, 640)));
        let plain = r#"{"packets_and_frames":[{"type":"frame","width":640,"height":480}]}"#;
        assert_eq!(parse_picture_size(plain), Some((640, 480)));
        assert_eq!(parse_picture_size(""), None);
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
