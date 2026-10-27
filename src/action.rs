//! The actions of the context menu and how they map to command line names.

use crate::config::ConvertKind;
use crate::i18n::T;
use crate::media::Kind;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    Compress,
    ConvertVideo,
    /// Audio files: convert the file. Video files: re-encode the audio tracks.
    ConvertAudio,
    ExtractAudio,
    DeleteSound,
    Frames,
    Webp,
    ConvertImage,
    Stats,
}

impl Action {
    /// Every standard item, in menu order
    pub const ALL: [Action; 9] = [
        Action::Compress,
        Action::ConvertVideo,
        Action::ConvertAudio,
        Action::ExtractAudio,
        Action::DeleteSound,
        Action::Frames,
        Action::Webp,
        Action::ConvertImage,
        Action::Stats,
    ];

    pub fn from_cli(name: &str) -> Option<Action> {
        Action::ALL.into_iter().find(|a| a.cli_name() == name)
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Action::Compress => "compress",
            Action::ConvertVideo => "convert-video",
            Action::ConvertAudio => "convert-audio",
            Action::ExtractAudio => "extract-audio",
            Action::DeleteSound => "delete-sound",
            Action::Frames => "frames",
            Action::Webp => "webp",
            Action::ConvertImage => "convert-image",
            Action::Stats => "stats",
        }
    }

    /// The format list of an action that only asks for an output format. `first` is the kind of
    /// the first file: Convert Audio of a video re-encodes its audio tracks.
    pub fn convert_kind(self, first: Option<Kind>) -> Option<ConvertKind> {
        Some(match self {
            Action::ConvertVideo => ConvertKind::Video,
            Action::ExtractAudio => ConvertKind::Extract,
            Action::Frames => ConvertKind::Frames,
            Action::ConvertImage => ConvertKind::Image,
            Action::ConvertAudio if first == Some(Kind::Video) => ConvertKind::VideoAudio,
            Action::ConvertAudio => ConvertKind::AudioFile,
            _ => return None,
        })
    }

    /// The name in menus and window titles
    pub fn title(self, t: &T) -> &'static str {
        match self {
            Action::Compress => t.t_compress,
            Action::ConvertVideo => t.t_convert_video,
            Action::ConvertAudio => t.t_convert_audio,
            Action::ExtractAudio => t.t_extract_audio,
            Action::DeleteSound => t.t_delete_sound,
            Action::Frames => t.t_frames,
            Action::Webp => t.t_webp,
            Action::ConvertImage => t.t_convert_image,
            Action::Stats => t.t_stats,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_names_roundtrip() {
        for a in Action::ALL {
            assert_eq!(Action::from_cli(a.cli_name()), Some(a));
        }
        assert_eq!(Action::from_cli("nope"), None);
    }
}
