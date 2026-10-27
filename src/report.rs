//! What the Stats window shows about a file: the JSON of ffprobe turned into a few numbers
//! and named sections (the streams, the tags), in words a person reads.

use crate::i18n::T;
use crate::units::{fmt_duration, fmt_float, fmt_rate};
use serde_json::{Map, Value};

pub struct Section {
    pub title: String,
    pub rows: Vec<(String, String)>,
}

pub struct Report {
    /// (caption, value): duration, size, bitrate, container
    pub metrics: Vec<(String, String)>,
    pub sections: Vec<Section>,
}

fn text(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        _ => None,
    }
}

fn num(v: Option<&Value>) -> Option<f64> {
    text(v)?.parse().ok()
}

/// The usual name of a codec ("h264" -> "H.264")
fn codec_name(name: &str) -> String {
    let known = [
        ("h264", "H.264"),
        ("hevc", "HEVC"),
        ("av1", "AV1"),
        ("vp9", "VP9"),
        ("vp8", "VP8"),
        ("mpeg4", "MPEG-4"),
        ("mpeg2video", "MPEG-2"),
        ("prores", "ProRes"),
        ("mjpeg", "JPEG"),
        ("aac", "AAC"),
        ("mp3", "MP3"),
        ("opus", "Opus"),
        ("vorbis", "Vorbis"),
        ("flac", "FLAC"),
        ("alac", "ALAC"),
        ("ac3", "AC-3"),
        ("eac3", "E-AC-3"),
        ("dts", "DTS"),
        ("wmav2", "WMA"),
        ("subrip", "SubRip (SRT)"),
        ("ass", "ASS"),
        ("mov_text", "MP4 text"),
        ("webvtt", "WebVTT"),
        ("hdmv_pgs_subtitle", "PGS"),
        ("dvd_subtitle", "VobSub"),
        ("webp", "WebP"),
    ];
    if let Some((_, nice)) = known.iter().find(|(k, _)| *k == name) {
        return nice.to_string();
    }
    if let Some(pcm) = name.strip_prefix("pcm_") {
        return format!("PCM ({pcm})");
    }
    name.to_uppercase()
}

/// "30000/1001" -> "29.97 fps"
fn frame_rate(v: Option<&Value>) -> Option<String> {
    let (a, b) = text(v)?.split_once('/').map(|(a, b)| (a.parse::<f64>().ok(), b.parse::<f64>().ok()))?;
    let (a, b) = (a?, b?);
    (a > 0.0 && b > 0.0).then(|| format!("{} fps", fmt_float((a / b * 100.0).round() / 100.0)))
}

fn size_text(bytes: f64) -> String {
    if bytes >= 1_073_741_824.0 {
        format!("{:.2} GB", bytes / 1_073_741_824.0)
    } else if bytes >= 1_048_576.0 {
        format!("{:.1} MB", bytes / 1_048_576.0)
    } else {
        format!("{:.0} KB", (bytes / 1024.0).ceil())
    }
}

/// "major_brand" -> "Major brand"
fn key_text(key: &str) -> String {
    let words = key.replace('_', " ");
    let mut chars = words.chars();
    chars.next().map(|f| f.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

fn tag(s: &Value, key: &str) -> Option<String> {
    let tags = s.get("tags")?.as_object()?;
    tags.iter().find(|(k, _)| k.eq_ignore_ascii_case(key)).and_then(|(_, v)| text(Some(v)))
}

fn video_rows(s: &Value, t: &T, still: bool) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut push = |label: &str, value: Option<String>| {
        if let Some(v) = value {
            rows.push((label.to_string(), v));
        }
    };
    let codec = text(s.get("codec_name")).map(|c| codec_name(&c));
    push(t.st_codec, codec.map(|c| text(s.get("profile")).map_or(c.clone(), |p| format!("{c} · {p}"))));
    let (w, h) = (num(s.get("width")).unwrap_or(0.0), num(s.get("height")).unwrap_or(0.0));
    if w > 0.0 {
        let aspect = text(s.get("display_aspect_ratio")).filter(|a| a != "0:1" && a != "1:1");
        push(t.st_resolution, Some(aspect.map_or(format!("{w}×{h}"), |a| format!("{w}×{h} · {a}"))));
    }
    if !still {
        push(t.st_fps, frame_rate(s.get("avg_frame_rate")).or_else(|| frame_rate(s.get("r_frame_rate"))));
        push(t.st_bitrate, num(s.get("bit_rate")).map(fmt_rate));
    }
    let depth = text(s.get("bits_per_raw_sample")).map(|b| format!("{b} bit"));
    let scan = text(s.get("field_order")).filter(|f| f != "unknown");
    let pixels: Vec<String> = [text(s.get("pix_fmt")), depth, scan].into_iter().flatten().collect();
    push(t.st_pixels, (!pixels.is_empty()).then(|| pixels.join(" · ")));
    let color = match text(s.get("color_transfer")).as_deref() {
        Some("smpte2084") => Some("HDR10 (PQ)".to_string()),
        Some("arib-std-b67") => Some("HDR (HLG)".to_string()),
        _ => text(s.get("color_space")),
    };
    if !still {
        push(t.st_color, color);
        push(t.st_frames, text(s.get("nb_frames")));
    }
    rows
}

fn audio_rows(s: &Value, t: &T) -> Vec<(String, String)> {
    let mut rows = Vec::new();
    let mut push = |label: &str, value: Option<String>| {
        if let Some(v) = value {
            rows.push((label.to_string(), v));
        }
    };
    let codec = text(s.get("codec_name")).map(|c| codec_name(&c));
    push(t.st_codec, codec.map(|c| text(s.get("profile")).map_or(c.clone(), |p| format!("{c} · {p}"))));
    push(t.st_sample_rate, num(s.get("sample_rate")).map(|r| format!("{} kHz", fmt_float(r / 1000.0))));
    let channels = num(s.get("channels")).unwrap_or(0.0) as u32;
    let layout = text(s.get("channel_layout")).or(match channels {
        0 => None,
        1 => Some("mono".into()),
        2 => Some("stereo".into()),
        n => Some(format!("{n} ch")),
    });
    push(t.st_channels, layout);
    push(t.st_bitrate, num(s.get("bit_rate")).map(fmt_rate));
    rows
}

/// The JSON of `ffprobe -print_format json -show_format -show_streams` as a report
pub fn build(json: &str, t: &T) -> Option<Report> {
    let data: Value = serde_json::from_str(json).ok()?;
    let streams = data.get("streams")?.as_array()?;
    let empty = Value::Object(Map::new());
    let fmt = data.get("format").unwrap_or(&empty);
    let format_name = text(fmt.get("format_name")).unwrap_or_default();
    // a single picture: no duration, no frame rate
    let still = format_name.ends_with("_pipe") || format_name == "image2";

    let mut metrics = Vec::new();
    if !still {
        if let Some(d) = num(fmt.get("duration")).filter(|d| *d > 0.0) {
            metrics.push((t.st_duration.to_string(), fmt_duration(d)));
        }
    }
    if let Some(b) = num(fmt.get("size")) {
        metrics.push((t.st_size.to_string(), size_text(b)));
    }
    if !still {
        if let Some(b) = num(fmt.get("bit_rate")) {
            metrics.push((t.st_bitrate.to_string(), fmt_rate(b)));
        }
    }
    let container = if still {
        streams.first().and_then(|s| text(s.get("codec_name"))).map(|c| codec_name(&c))
    } else {
        // "WAV / WAVE (Waveform Audio)" -> "WAV / WAVE"
        text(fmt.get("format_long_name")).map(|n| n.split(" (").next().unwrap_or(&n).trim().to_string())
    };
    if let Some(c) = container {
        metrics.push((t.st_container.to_string(), c));
    }

    let count = |kind: &str| streams.iter().filter(|s| s["codec_type"] == kind).count();
    let mut seen = std::collections::HashMap::new();
    let mut sections = Vec::new();
    for s in streams {
        let kind = s["codec_type"].as_str().unwrap_or("data");
        let cover = s.get("disposition").and_then(|d| num(d.get("attached_pic"))).unwrap_or(0.0) != 0.0;
        let (name, mut rows) = match kind {
            "video" if cover => (t.st_cover, video_rows(s, t, true)),
            "video" if still => (t.st_image, video_rows(s, t, true)),
            "video" => (t.st_video, video_rows(s, t, false)),
            "audio" => (t.st_audio, audio_rows(s, t)),
            "subtitle" => (t.st_subtitle, vec![(t.st_codec.to_string(), codec_name(&text(s.get("codec_name")).unwrap_or_default()))]),
            _ => (t.st_other, text(s.get("codec_name")).map(|c| vec![(t.st_codec.to_string(), c)]).unwrap_or_default()),
        };
        if let Some(lang) = tag(s, "language").filter(|l| l != "und") {
            rows.push((t.st_language.to_string(), lang));
        }
        if let Some(title) = tag(s, "title") {
            rows.push((t.st_title.to_string(), title));
        }
        let n = seen.entry(name).or_insert(0);
        *n += 1;
        let title = if count(kind) > 1 && !cover { format!("{name} {n}") } else { name.to_string() };
        sections.push(Section { title, rows });
    }
    let tags: Vec<(String, String)> =
        fmt.get("tags").and_then(Value::as_object).map(|m| m.iter().filter_map(|(k, v)| Some((key_text(k), text(Some(v))?))).collect()).unwrap_or_default();
    if !tags.is_empty() {
        sections.push(Section { title: t.st_tags.to_string(), rows: tags });
    }
    Some(Report { metrics, sections })
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLIP: &str = r#"{"streams":[
        {"codec_type":"video","codec_name":"h264","profile":"High","width":1920,"height":1080,"display_aspect_ratio":"16:9",
         "pix_fmt":"yuv420p","field_order":"progressive","bits_per_raw_sample":"8","avg_frame_rate":"30000/1001","bit_rate":"6112762","nb_frames":"240"},
        {"codec_type":"audio","codec_name":"aac","profile":"LC","sample_rate":"44100","channels":1,"channel_layout":"mono","bit_rate":"69318",
         "tags":{"language":"eng"}},
        {"codec_type":"audio","codec_name":"pcm_s16le","sample_rate":"48000","channels":2}],
        "format":{"format_name":"mov,mp4","format_long_name":"QuickTime / MOV","duration":"8.0","size":"6192204","bit_rate":"6192204",
         "tags":{"major_brand":"isom","encoder":"Lavf60.16.100"}}}"#;

    #[test]
    fn a_video_becomes_numbers_and_sections() {
        let r = build(CLIP, crate::i18n::t()).unwrap();
        let m: Vec<&str> = r.metrics.iter().map(|(_, v)| v.as_str()).collect();
        assert_eq!(m, ["0:08", "5.9 MB", "6.2 Mbit/s", "QuickTime / MOV"]);
        let titles: Vec<&str> = r.sections.iter().map(|s| s.title.as_str()).collect();
        assert_eq!(titles, ["Video", "Audio 1", "Audio 2", "Tags"]);
        let video = &r.sections[0].rows;
        assert!(video.contains(&("Codec".into(), "H.264 · High".into())));
        assert!(video.contains(&("Resolution".into(), "1920×1080 · 16:9".into())));
        assert!(video.contains(&("Frame rate".into(), "29.97 fps".into())));
        assert!(video.contains(&("Pixels".into(), "yuv420p · 8 bit · progressive".into())));
        let audio = &r.sections[1].rows;
        assert!(audio.contains(&("Sample rate".into(), "44.1 kHz".into())) && audio.contains(&("Language".into(), "eng".into())));
        assert!(r.sections[2].rows.contains(&("Channels".into(), "stereo".into())));
        assert!(r.sections[2].rows.contains(&("Codec".into(), "PCM (s16le)".into())));
        assert_eq!(r.sections[3].rows[0], ("Major brand".into(), "isom".into()));
    }

    #[test]
    fn a_picture_has_no_duration() {
        let json = r#"{"streams":[{"codec_type":"video","codec_name":"png","width":640,"height":480,"pix_fmt":"rgb24","avg_frame_rate":"25/1"}],
            "format":{"format_name":"png_pipe","format_long_name":"piped png sequence","size":"2048"}}"#;
        let r = build(json, crate::i18n::t()).unwrap();
        assert_eq!(r.metrics, [("Size".into(), "2 KB".into()), ("Container".into(), "PNG".into())]);
        assert_eq!(r.sections[0].title, "Image");
        assert!(!r.sections[0].rows.iter().any(|(k, _)| k == "Frame rate"));
    }
}
