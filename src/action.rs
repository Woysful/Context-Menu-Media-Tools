//! The actions of the context menu and how they map to command line names.

use crate::i18n::T;

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
    pub fn from_cli(name: &str) -> Option<Action> {
        Some(match name {
            "compress" => Action::Compress,
            "convert-video" => Action::ConvertVideo,
            "convert-audio" => Action::ConvertAudio,
            "extract-audio" => Action::ExtractAudio,
            "delete-sound" => Action::DeleteSound,
            "frames" => Action::Frames,
            "webp" => Action::Webp,
            "convert-image" => Action::ConvertImage,
            "stats" => Action::Stats,
            _ => return None,
        })
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
