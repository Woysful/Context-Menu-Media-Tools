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
    DeleteAudio,
    Frames,
    /// A video into an animated WebP or GIF
    Anim,
    ConvertImage,
    CompressImage,
    MakeIcon,
    /// A folder of pictures (or several pictures) into a video
    FramesVideo,
    Stats,
}

impl Action {
    /// Every standard item, in menu order
    pub const ALL: [Action; 12] = [
        Action::Compress,
        Action::ConvertVideo,
        Action::ConvertAudio,
        Action::ExtractAudio,
        Action::DeleteAudio,
        Action::Frames,
        Action::Anim,
        Action::ConvertImage,
        Action::CompressImage,
        Action::MakeIcon,
        Action::FramesVideo,
        Action::Stats,
    ];

    pub fn from_cli(name: &str) -> Option<Action> {
        // the menus of an install before the rename call the old name until the next install
        Action::ALL.into_iter().find(|a| a.cli_name() == name || a.old_cli_name() == Some(name))
    }

    /// The name before a rename (menus and settings of older versions use it)
    pub fn old_cli_name(self) -> Option<&'static str> {
        match self {
            Action::DeleteAudio => Some("delete-sound"),
            Action::Anim => Some("webp"),
            _ => None,
        }
    }

    pub fn cli_name(self) -> &'static str {
        match self {
            Action::Compress => "compress",
            Action::ConvertVideo => "convert-video",
            Action::ConvertAudio => "convert-audio",
            Action::ExtractAudio => "extract-audio",
            Action::DeleteAudio => "delete-audio",
            Action::Frames => "frames",
            Action::Anim => "make-anim",
            Action::ConvertImage => "convert-image",
            Action::CompressImage => "compress-image",
            Action::MakeIcon => "make-icon",
            Action::FramesVideo => "frames-to-video",
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
            Action::DeleteAudio => t.t_delete_audio,
            Action::Frames => t.t_frames,
            Action::Anim => t.t_anim,
            Action::ConvertImage => t.t_convert_image,
            Action::CompressImage => t.t_compress_image,
            Action::MakeIcon => t.t_make_icon,
            Action::FramesVideo => t.t_frames_video,
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
        assert_eq!(Action::from_cli("delete-sound"), Some(Action::DeleteAudio));
        assert_eq!(Action::from_cli("webp"), Some(Action::Anim));
    }
}
