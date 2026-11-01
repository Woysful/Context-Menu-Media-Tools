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
pub const DELETE_MERGE: &str = "delete_audio.merge";
/// "Delete the original when done", one value for every window
pub const DELETE_ORIGINAL: &str = "original.delete";

/// The values of the Compression field of Compress Video, in its order
pub const LEVELS: [&str; 3] = ["low", "medium", "high"];

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
            (DELETE_MERGE, Kind::Flag),
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
        CODEC | LIMIT_CODEC => s("h264_cpu"),
        RESOLUTION | LIMIT_RESOLUTION => s("orig"),
        AUDIO | LIMIT_AUDIO => s(cfg.default_audio()),
        MERGE | TWO_PASS | DELETE_MERGE | DELETE_ORIGINAL => s("false"),
        LIMIT_SIZE => s("20"),
        WEBP_SIZE => cfg.webp.size.to_string(),
        WEBP_FPS => cfg.webp.fps.to_string(),
        WEBP_QUALITY => cfg.webp.quality.to_string(),
        WEBP_COMPRESSION => cfg.webp.compression.to_string(),
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
    }
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
