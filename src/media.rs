//! ffprobe: what is in a file. Also the kind of a file by its extension.

use serde_json::Value;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Video,
    Audio,
    Image,
}

pub const VIDEO_EXT: &[&str] = &["mp4", "avi", "mov", "mkv", "webm", "m4v"];
pub const AUDIO_EXT: &[&str] = &["wav", "mp3", "flac", "aac", "ogg", "wma", "m4a", "weba", "opus"];
pub const IMAGE_EXT: &[&str] = &["jpg", "jpeg", "png", "bmp", "tiff", "tif", "gif", "webp", "tga", "psd"];

pub fn kind_of(path: &Path) -> Option<Kind> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    let ext = ext.as_str();
    if VIDEO_EXT.contains(&ext) {
        Some(Kind::Video)
    } else if AUDIO_EXT.contains(&ext) {
        Some(Kind::Audio)
    } else if IMAGE_EXT.contains(&ext) {
        Some(Kind::Image)
    } else {
        None
    }
}

#[derive(Clone, Debug, Default)]
#[allow(dead_code)]
pub struct Info {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps: Option<f64>,
    pub duration: Option<f64>,
    pub size: Option<f64>,
    pub video_bps: Option<f64>,
    pub bitrate_estimated: bool,
    pub audio_bps: Option<f64>,
    pub has_audio: bool,
    pub has_video: bool,
}

fn num(v: Option<&Value>) -> Option<f64> {
    match v? {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Parse the JSON of `ffprobe -print_format json -show_format -show_streams`
pub fn parse_probe(name: &str, json: &str) -> Option<Info> {
    let data: Value = serde_json::from_str(json).ok()?;
    let streams = data.get("streams")?.as_array()?;
    let fmt = data.get("format");
    let video = streams.iter().find(|s| {
        s["codec_type"] == "video" && s.get("disposition").map_or(true, |d| num(d.get("attached_pic")).unwrap_or(0.0) == 0.0)
    });
    let audio = streams.iter().find(|s| s["codec_type"] == "audio");
    let mut info = Info { name: name.to_string(), has_audio: audio.is_some(), has_video: video.is_some(), ..Default::default() };
    info.duration = num(fmt.and_then(|f| f.get("duration"))).or_else(|| num(video.and_then(|v| v.get("duration"))));
    info.size = num(fmt.and_then(|f| f.get("size")));
    info.audio_bps = audio.and_then(|a| num(a.get("bit_rate")));

    if let Some(v) = video {
        let mut w = num(v.get("width")).unwrap_or(0.0) as u32;
        let mut h = num(v.get("height")).unwrap_or(0.0) as u32;
        // Phone videos are often stored sideways and rotated by metadata: use the picture as played
        let mut rotation = 0i64;
        if let Some(list) = v.get("side_data_list").and_then(|l| l.as_array()) {
            for side in list {
                if let Some(r) = num(side.get("rotation")) {
                    rotation = r.round() as i64;
                }
            }
        }
        if rotation == 0 {
            rotation = num(v.get("tags").and_then(|t| t.get("rotate"))).unwrap_or(0.0) as i64;
        }
        if rotation.abs() % 180 == 90 {
            std::mem::swap(&mut w, &mut h);
        }
        info.width = w;
        info.height = h;
        for key in ["avg_frame_rate", "r_frame_rate"] {
            if let Some(s) = v.get(key).and_then(|x| x.as_str()) {
                if let Some((a, b)) = s.split_once('/') {
                    if let (Ok(a), Ok(b)) = (a.parse::<f64>(), b.parse::<f64>()) {
                        if a > 0.0 && b > 0.0 {
                            info.fps = Some(a / b);
                            break;
                        }
                    }
                }
            }
        }
        info.video_bps = num(v.get("bit_rate"));
        if info.video_bps.is_none() {
            if let (Some(d), Some(s)) = (info.duration, info.size) {
                // MKV / WebM store no per-stream bitrates: derive it from the file size
                if d > 0.0 {
                    info.video_bps = Some((s * 8.0 / d - info.audio_bps.unwrap_or(0.0)).max(0.0));
                    info.bitrate_estimated = true;
                }
            }
        }
    }
    Some(info)
}

pub fn probe(ffprobe: &Path, file: &Path) -> Option<Info> {
    let mut cmd = std::process::Command::new(ffprobe);
    cmd.args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams"]).arg(file);
    crate::engine::hide_console(&mut cmd);
    let out = cmd.output().ok()?;
    let name = file.file_name()?.to_string_lossy().to_string();
    parse_probe(&name, &String::from_utf8_lossy(&out.stdout))
}

pub fn fmt_rate(bps: f64) -> String {
    if bps >= 1e6 {
        format!("{:.1} Mbit/s", bps / 1e6)
    } else {
        format!("{:.0} kbit/s", bps / 1e3)
    }
}

pub fn fmt_duration(sec: f64) -> String {
    let total = sec.round() as u64;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
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
    fn kinds() {
        assert_eq!(kind_of(Path::new("a.MKV")), Some(Kind::Video));
        assert_eq!(kind_of(Path::new("a.flac")), Some(Kind::Audio));
        assert_eq!(kind_of(Path::new("a.psd")), Some(Kind::Image));
        assert_eq!(kind_of(Path::new("a.txt")), None);
    }

    #[test]
    fn duration_format() {
        assert_eq!(fmt_duration(6.0), "0:06");
        assert_eq!(fmt_duration(3725.0), "1:02:05");
    }
}
