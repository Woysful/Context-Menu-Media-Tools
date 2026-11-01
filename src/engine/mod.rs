//! The encoding jobs. A job runs on its own thread and reports through `Progress`
//! (the window reads it, or the console in `--cli` mode).

mod budget;
mod compress;
mod convert;
mod output;
mod progress;
mod runner;

pub use crate::config::ConvertKind;
pub use progress::{Line, LineKind, Progress, State};
pub use runner::stats_text;

use crate::config::Config;
use crate::i18n;
use crate::media::{self, Info, Kind};
use crate::scale::ScaleMode;
use crate::units::fmt_float;
use output::short_name;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The audio tracks of Compress Video that go into the result
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracks {
    /// numbers of the tracks to keep, from 0 (None = all of them)
    pub keep: Option<Vec<usize>>,
    /// mix the kept tracks into one
    pub merge: bool,
}

#[derive(Clone, Debug)]
pub enum Request {
    CompressPreset {
        level: usize,
        codec: String,
        scale: ScaleMode,
        /// key of [compress_audio]
        audio: String,
        tracks: Tracks,
    },
    CompressLimit {
        mb: f64,
        codec: String,
        scale: ScaleMode,
        twopass: bool,
        audio: String,
        tracks: Tracks,
    },
    Convert {
        what: ConvertKind,
        format: String,
    },
    DeleteAudio {
        /// numbers of the audio tracks to remove, from 0 (None = all of them)
        remove: Option<Vec<usize>>,
    },
    Webp {
        size: u32,
        fps: u32,
        quality: u32,
        compression: u32,
    },
    /// An own menu item (settings.toml)
    Custom {
        id: String,
    },
}

impl Request {
    /// The kind of files the request works on; the others are skipped
    pub fn input_kind(&self, cfg: &Config) -> Kind {
        match self {
            Request::Convert { what, .. } => what.input_kind(),
            Request::Custom { id } => cfg.settings.custom(id).map_or(Kind::Video, |c| c.media_kind()),
            _ => Kind::Video,
        }
    }
}

/// The name of what `req` would make of `file` now (a taken name gets a number, as in the job).
/// Frames go into a folder: "clip/".
pub fn result_name(cfg: &Config, req: &Request, file: &Path) -> Option<String> {
    let named = |pattern: &str, extra: &[(&str, String)], ext: &str| short_name(&output::unique_path(output::named_ext(pattern, file, extra, ext)));
    let name = |pattern: &str, extra: &[(&str, String)]| named(pattern, extra, "");
    let audio_ext = |key: &str| cfg.audio_codec(key).map_or(String::new(), |a| a.ext.clone());
    Some(match req {
        Request::CompressPreset { codec, audio, .. } => named(&cfg.compress_codec(codec)?.output, &[], &audio_ext(audio)),
        Request::CompressLimit { mb, codec, audio, .. } => named(&cfg.limit_codec(codec)?.output, &[("mb", fmt_float(*mb))], &audio_ext(audio)),
        Request::Convert { what: ConvertKind::Frames, .. } => format!("{}/", short_name(&output::unique_path(PathBuf::from(output::base_of(file))))),
        Request::Convert { what, format } => {
            let f = cfg.format(*what, format)?;
            name(&f.output, &[("ext", f.extension(format).to_string()), ("fmt", format.clone())])
        }
        Request::DeleteAudio { remove: None } => name(&cfg.delete_audio.output, &[]),
        Request::DeleteAudio { remove: Some(_) } => name(&cfg.delete_audio.tracks_output, &[]),
        Request::Webp { .. } => name(&cfg.webp.output, &[]),
        Request::Custom { id } => name(&cfg.settings.custom(id)?.output, &[]),
    })
}

pub struct Job {
    pub cfg: Arc<Config>,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub prog: Arc<Progress>,
}

/// Why a file has no result
#[derive(Debug)]
pub enum Failure {
    Cancelled,
    Error(String),
}

impl From<String> for Failure {
    fn from(e: String) -> Self {
        Failure::Error(e)
    }
}

type Outcome<T> = Result<T, Failure>;

/// Process every file. The worker thread calls this; it never panics on bad input.
pub fn run(job: &Job, req: &Request, files: &[PathBuf]) {
    let t = i18n::t();
    let p = &job.prog;
    let wanted = req.input_kind(&job.cfg);

    let mut eligible: Vec<&PathBuf> = Vec::new();
    for f in files {
        if f.is_file() && Kind::of(f) == Some(wanted) {
            eligible.push(f);
        } else {
            p.warn(format!("{} {}", t.skipped, short_name(f)));
        }
    }
    if eligible.is_empty() {
        p.error(t.no_files.to_string());
        p.finish();
        return;
    }

    for (i, file) in eligible.iter().enumerate() {
        if p.is_cancelled() {
            break;
        }
        p.begin_file(i, eligible.len(), &short_name(file));
        match process_one(job, req, file) {
            Ok(text) => p.result(text),
            Err(Failure::Cancelled) => break,
            Err(Failure::Error(e)) => p.error(format!("{} {}\n{}", t.failed, short_name(file), e)),
        }
    }
    if p.is_cancelled() {
        p.mark_cancelled();
        p.info(t.cancelled.to_string());
    }
    p.finish();
}

fn process_one(job: &Job, req: &Request, file: &Path) -> Outcome<String> {
    let t = i18n::t();
    let info = media::probe(&job.ffprobe, file).unwrap_or_else(|| Info { name: short_name(file), ..Default::default() });
    let unknown = |key: &str| Failure::Error(format!("{} {key}", t.unknown_codec));
    match req {
        Request::CompressPreset { level, codec, scale, audio, tracks } => {
            let c = job.cfg.compress_codec(codec).ok_or_else(|| unknown(codec))?;
            // "auto" only exists in the size limit mode
            let mode = if *scale == ScaleMode::Auto { ScaleMode::Orig } else { *scale };
            compress::preset(job, file, &info, *level, c, mode, audio, tracks)
        }
        Request::CompressLimit { mb, codec, scale, twopass, audio, tracks } => {
            let c = job.cfg.limit_codec(codec).ok_or_else(|| unknown(codec))?;
            compress::limit(job, file, &info, c, *mb, *scale, *twopass, audio, tracks)
        }
        Request::Convert { what, format } => {
            let f = job.cfg.format(*what, format).ok_or_else(|| unknown(format))?;
            convert::format(job, file, &info, *what, format, f)
        }
        Request::DeleteAudio { remove } => convert::delete_audio(job, file, &info, remove.as_deref()),
        Request::Webp { size, fps, quality, compression } => convert::webp(job, file, &info, *size, *fps, *quality, *compression),
        Request::Custom { id } => {
            let c = job.cfg.settings.custom(id).ok_or_else(|| unknown(id))?;
            convert::custom(job, file, &info, c)
        }
    }
}
