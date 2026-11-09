//! Own default values of the fields of the option windows: [defaults] of settings.toml,
//! "field key" = "value". A value that does not fit the field (any more) is ignored.

use crate::config::{Config, ConvertKind};
use crate::scale::ScaleMode;
use crate::units;

pub const MODE: &str = "compress.mode";
pub const LEVEL: &str = "compress.level";
pub const CODEC: &str = "compress.codec";
pub const RESOLUTION: &str = "compress.resolution";
pub const AUDIO: &str = "compress.audio";
pub const MERGE: &str = "compress.merge";
pub const LIMIT_SIZE: &str = "limit.size";
pub const TWO_PASS: &str = "limit.two_pass";
pub const LIMIT_CODEC: &str = "limit.codec";
pub const LIMIT_RESOLUTION: &str = "limit.resolution";
pub const LIMIT_AUDIO: &str = "limit.audio";
pub const WEBP_SIZE: &str = "webp.size";
pub const WEBP_FPS: &str = "webp.fps";
pub const WEBP_QUALITY: &str = "webp.quality";
pub const WEBP_COMPRESSION: &str = "webp.compression";
pub const GIF_DITHER: &str = "gif.dither";
pub const DELETE_MERGE: &str = "delete_audio.merge";
pub const IMAGE_MODE: &str = "image.mode";
pub const IMAGE_FORMAT: &str = "image.format";
pub const IMAGE_QUALITY: &str = "image.quality";
pub const IMAGE_LIMIT: &str = "image.limit";
pub const IMAGE_RESOLUTION: &str = "image.resolution";
pub const ICON_SIZES: &str = "icon.sizes";
pub const ICON_SQUARE: &str = "icon.square";
pub const FRAMES_FPS: &str = "frames_video.fps";
pub const FRAMES_CODEC: &str = "frames_video.codec";
pub const FRAMES_LEVEL: &str = "frames_video.level";
/// "Delete the original when done", one value for every window
pub const DELETE_ORIGINAL: &str = "original.delete";

/// The values of the Compression field of Compress Video, in its order
pub const LEVELS: [&str; 3] = ["low", "medium", "high"];
/// The tabs of Compress Image
pub const IMAGE_MODES: [&str; 2] = ["quality", "limit"];
/// The dithers of a GIF of Make Anim, in the order of the window
pub const DITHERS: [&str; 4] = ["sierra2_4a", "floyd_steinberg", "bayer", "none"];
/// The shapes of Make Icon
pub const SQUARES: [&str; 2] = ["pad", "crop"];

/// What a field holds
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Kind {
    /// presets | limit: the tab of Compress Video
    Mode,
    Level,
    Codec {
        limit: bool,
    },
    Resolution {
        auto: bool,
    },
    Audio,
    Flag,
    Number {
        lo: u32,
        hi: u32,
    },
    Megabytes,
    Format(ConvertKind),
    /// one of a few words
    Choice(&'static [&'static str]),
    /// "source" or a key of [image_compress]
    ImageFormat,
    /// "500 KB", "2 MB"
    Size,
    /// "16,32,256": sizes of an icon
    Sizes,
    /// frames per second, 1 to 240, may have decimals
    Fps,
}

/// The key of the output format of a convert action
pub fn format_key(kind: ConvertKind) -> String {
    format!("format.{}", kind.section())
}

/// Every field, in the order of the windows
pub fn fields() -> Vec<(String, Kind)> {
    let mut out: Vec<(String, Kind)> = [
        (MODE, Kind::Mode),
        (LEVEL, Kind::Level),
        (CODEC, Kind::Codec { limit: false }),
        (RESOLUTION, Kind::Resolution { auto: false }),
        (AUDIO, Kind::Audio),
        (MERGE, Kind::Flag),
        (LIMIT_SIZE, Kind::Megabytes),
        (TWO_PASS, Kind::Flag),
        (LIMIT_CODEC, Kind::Codec { limit: true }),
        (LIMIT_RESOLUTION, Kind::Resolution { auto: true }),
        (LIMIT_AUDIO, Kind::Audio),
    ]
    .into_iter()
    .map(|(k, kind)| (k.to_string(), kind))
    .collect();
    out.extend(ConvertKind::ALL.map(|k| (format_key(k), Kind::Format(k))));
    out.extend(
        [
            (WEBP_SIZE, Kind::Number { lo: 64, hi: 4320 }),
            (WEBP_FPS, Kind::Number { lo: 1, hi: 120 }),
            (WEBP_QUALITY, Kind::Number { lo: 0, hi: 100 }),
            (WEBP_COMPRESSION, Kind::Number { lo: 0, hi: 6 }),
            (GIF_DITHER, Kind::Choice(&DITHERS)),
            (DELETE_MERGE, Kind::Flag),
            (IMAGE_MODE, Kind::Choice(&IMAGE_MODES)),
            (IMAGE_FORMAT, Kind::ImageFormat),
            (IMAGE_QUALITY, Kind::Number { lo: 1, hi: 100 }),
            (IMAGE_LIMIT, Kind::Size),
            (IMAGE_RESOLUTION, Kind::Resolution { auto: false }),
            (ICON_SIZES, Kind::Sizes),
            (ICON_SQUARE, Kind::Choice(&SQUARES)),
            (FRAMES_FPS, Kind::Fps),
            (FRAMES_CODEC, Kind::Codec { limit: false }),
            (FRAMES_LEVEL, Kind::Level),
            (DELETE_ORIGINAL, Kind::Flag),
        ]
        .map(|(k, kind)| (k.to_string(), kind)),
    );
    out
}

pub fn kind_of(key: &str) -> Option<Kind> {
    fields().into_iter().find(|(k, _)| k == key).map(|(_, kind)| kind)
}

/// The value a field has without an own default
pub fn standard(cfg: &Config, key: &str) -> String {
    let s = |v: &str| v.to_string();
    match key {
        MODE => s("presets"),
        LEVEL => s("medium"),
        CODEC | LIMIT_CODEC | FRAMES_CODEC => s("h264_cpu"),
        RESOLUTION | LIMIT_RESOLUTION | IMAGE_RESOLUTION => s("orig"),
        FRAMES_LEVEL => s("medium"),
        IMAGE_MODE => s("quality"),
        IMAGE_FORMAT => s(crate::engine::SOURCE),
        IMAGE_QUALITY => s("80"),
        IMAGE_LIMIT => s("500 KB"),
        ICON_SIZES => sizes_text(&cfg.icon.sizes),
        ICON_SQUARE => s("pad"),
        FRAMES_FPS => units::fmt_float(cfg.frames_video.fps),
        AUDIO | LIMIT_AUDIO => s(cfg.default_audio()),
        MERGE | TWO_PASS | DELETE_MERGE | DELETE_ORIGINAL => s("false"),
        LIMIT_SIZE => s("20"),
        WEBP_SIZE => cfg.webp.size.to_string(),
        WEBP_FPS => cfg.webp.fps.to_string(),
        WEBP_QUALITY => cfg.webp.quality.to_string(),
        WEBP_COMPRESSION => cfg.webp.compression.to_string(),
        GIF_DITHER => s(&cfg.gif.dither),
        _ => match kind_of(key) {
            Some(Kind::Format(k)) => cfg.default_format(k).unwrap_or_default().to_string(),
            _ => String::new(),
        },
    }
}

/// The value fits the field
pub fn valid(cfg: &Config, kind: Kind, value: &str) -> bool {
    match kind {
        Kind::Mode => matches!(value, "presets" | "limit"),
        Kind::Level => LEVELS.contains(&value),
        Kind::Codec { limit: false } => cfg.compress_codec(value).is_some(),
        Kind::Codec { limit: true } => cfg.limit_codec(value).is_some(),
        Kind::Resolution { auto } => match ScaleMode::from_code(value) {
            Some(ScaleMode::Auto) => auto,
            Some(ScaleMode::Height(h)) => h > 0,
            Some(ScaleMode::Factor(f)) => f > 0.0 && f <= 1.0,
            Some(ScaleMode::Orig) => true,
            None => false,
        },
        Kind::Audio => cfg.audio_codec(value).is_some(),
        Kind::Flag => matches!(value, "true" | "false"),
        Kind::Number { lo, hi } => value.trim().parse::<u32>().is_ok_and(|v| (lo..=hi).contains(&v)),
        Kind::Megabytes => units::parse_decimal(value).is_some_and(|v| v > 0.0),
        Kind::Format(k) => cfg.format(k, value).is_some(),
        Kind::Choice(words) => words.contains(&value),
        Kind::ImageFormat => value == crate::engine::SOURCE || cfg.image_format(value).is_some(),
        Kind::Size => units::parse_size(value).is_some(),
        Kind::Sizes => parse_sizes(value).is_some_and(|v| !v.is_empty()),
        Kind::Fps => parse_fps(value).is_some(),
    }
}

/// "16,32,256" -> [16, 32, 256]; None when a size is not 1..=256
pub fn parse_sizes(text: &str) -> Option<Vec<u32>> {
    text.split(',').filter(|s| !s.trim().is_empty()).map(|s| s.trim().parse::<u32>().ok().filter(|v| (1..=256).contains(v))).collect()
}

pub fn sizes_text(sizes: &[u32]) -> String {
    sizes.iter().map(u32::to_string).collect::<Vec<_>>().join(",")
}

/// Frames per second the user typed: 1 to 240
pub fn parse_fps(text: &str) -> Option<f64> {
    units::parse_decimal(text).filter(|v| (1.0..=240.0).contains(v))
}

/// The value a field starts with: the own default when it fits, else the standard one
pub fn value(cfg: &Config, key: &str) -> String {
    match (cfg.settings.defaults.get(key), kind_of(key)) {
        (Some(v), Some(kind)) if valid(cfg, kind, v) => v.clone(),
        _ => standard(cfg, key),
    }
}

pub fn flag(cfg: &Config, key: &str) -> bool {
    value(cfg, key) == "true"
}

pub fn level_index(value: &str) -> usize {
    LEVELS.iter().position(|l| *l == value).unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn standard_values_fit_their_fields() {
        let cfg = Config::defaults();
        for (key, kind) in fields() {
            assert!(valid(&cfg, kind, &standard(&cfg, &key)), "{key}");
        }
        assert_eq!(standard(&cfg, &format_key(ConvertKind::Video)), "mp4");
        assert_eq!(standard(&cfg, WEBP_SIZE), "720");
        assert_eq!(standard(&cfg, ICON_SIZES), "16,24,32,48,64,256");
        assert_eq!(parse_sizes("16, 32,256"), Some(vec![16, 32, 256]));
        assert_eq!(parse_sizes("16,512"), None);
        assert_eq!(parse_fps("23,976"), Some(23.976));
        assert!(valid(&cfg, Kind::Choice(&SQUARES), "crop") && !valid(&cfg, Kind::Choice(&SQUARES), "round"));
    }

    #[test]
    fn own_values_are_used_when_they_fit() {
        let mut s = Settings::default();
        for (k, v) in [(CODEC, "h265_cpu"), (LEVEL, "loud"), (RESOLUTION, "auto"), (LIMIT_RESOLUTION, "auto"), (WEBP_FPS, "500"), (LIMIT_SIZE, "9,5")] {
            s.defaults.insert(k.into(), v.into());
        }
        let cfg = Config::from_sources(crate::config::DEFAULT_PRESETS, None, s);
        assert_eq!(value(&cfg, CODEC), "h265_cpu");
        assert_eq!(value(&cfg, LEVEL), "medium");
        // auto exists only in the size limit tab
        assert_eq!(value(&cfg, RESOLUTION), "orig");
        assert_eq!(value(&cfg, LIMIT_RESOLUTION), "auto");
        assert_eq!(value(&cfg, WEBP_FPS), "20");
        assert_eq!(value(&cfg, LIMIT_SIZE), "9,5");
        assert!(!flag(&cfg, MERGE));
        assert_eq!(level_index("high"), 2);
    }
}
