//! The encoding jobs. A job runs on its own thread and reports through `Progress`
//! (the window reads it, or the console in `--cli` mode).

mod budget;
mod compress;
mod convert;
mod image;
mod output;
mod progress;
mod runner;

pub use crate::config::ConvertKind;
pub use image::{edit_filters, encoders, frame_sets, natural_cmp, FrameSet, SOURCE};
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

/// Turning, mirroring and cropping of Compress Image
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ImageEdit {
    /// quarter turns clockwise (0..=3)
    pub turns: u8,
    /// mirrored after the turn
    pub flip_h: bool,
    pub flip_v: bool,
    /// x, y, width, height as parts (0..1) of the turned picture; None = all of it
    pub crop: Option<[f64; 4]>,
}

/// What Compress Image aims at
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ImageTarget {
    /// 1..=100
    Quality(u32),
    /// the highest quality that fits into this many bytes
    Bytes(u64),
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
        /// mix the tracks that stay into one
        merge: bool,
    },
    Webp {
        size: u32,
        fps: u32,
        quality: u32,
        compression: u32,
    },
    CompressImage {
        /// key of [image_compress], or "source"
        format: String,
        target: ImageTarget,
        scale: ScaleMode,
        edit: ImageEdit,
    },
    Icon {
        sizes: Vec<u32>,
        /// the middle of a picture that is not square (else transparent borders)
        crop: bool,
    },
    /// One video per chosen folder, one of the chosen pictures
    FramesVideo {
        fps: f64,
        /// key of [compress]
        codec: String,
        level: usize,
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
            Request::CompressImage { .. } | Request::Icon { .. } | Request::FramesVideo { .. } => Kind::Image,
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
        Request::DeleteAudio { remove: None, .. } => name(&cfg.delete_audio.output, &[]),
        Request::DeleteAudio { merge: true, .. } => name(&cfg.delete_audio.merge_output, &[]),
        Request::DeleteAudio { remove: Some(_), .. } => name(&cfg.delete_audio.tracks_output, &[]),
        Request::Webp { .. } => name(&cfg.webp.output, &[]),
        Request::Custom { id } => name(&cfg.settings.custom(id)?.output, &[]),
        Request::CompressImage { format, .. } => short_name(&output::unique_path(image::compressed_path(cfg, format, file)?)),
        Request::Icon { .. } => name(&cfg.icon.output, &[]),
        Request::FramesVideo { .. } => {
            let folder = if file.is_dir() { file.to_path_buf() } else { file.parent()?.to_path_buf() };
            short_name(&output::unique_path(image::video_path(cfg, &folder, file.is_dir())))
        }
    })
}

/// The size of what Compress Image makes of `file` with a quality (the window shows it before Start)
pub fn estimate_image(cfg: Arc<Config>, ffmpeg: PathBuf, ffprobe: PathBuf, req: &Request, file: &Path) -> Result<u64, String> {
    let Request::CompressImage { format, target: ImageTarget::Quality(q), scale, edit } = req else { return Err("no estimate".into()) };
    let job = Job { cfg, ffmpeg, ffprobe, prog: Progress::new(Box::new(|| {}), false), delete_original: false };
    image::estimate(&job, file, format, *q, *scale, edit).map_err(|e| match e {
        Failure::Error(e) => e,
        Failure::Cancelled => String::new(),
    })
}

pub struct Job {
    pub cfg: Arc<Config>,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub prog: Arc<Progress>,
    /// delete every original that got a result (Settings: to the trash or for good)
    pub delete_original: bool,
}

/// What became of one file
pub struct Done {
    /// the line of the log
    text: String,
    /// the result (a file, or the folder of frames); None = nothing was made
    output: Option<PathBuf>,
}

impl Done {
    fn new(text: String, output: &Path) -> Done {
        Done { text, output: Some(output.to_path_buf()) }
    }

    /// Nothing to do for this file
    fn nothing(text: String) -> Done {
        Done { text, output: None }
    }
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
    if let Request::FramesVideo { fps, codec, level } = req {
        return frames_video(job, files, *fps, codec, *level);
    }
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
            Ok(done) => {
                p.result(done.text);
                let made = done.output.is_some_and(|o| o.exists() && o != **file);
                if job.delete_original && made {
                    remove_original(job, file);
                }
            }
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

/// Frames to Video: a video per set of pictures (the originals stay)
fn frames_video(job: &Job, files: &[PathBuf], fps: f64, codec: &str, level: usize) {
    let t = i18n::t();
    let p = &job.prog;
    let sets = image::frame_sets(files);
    if sets.is_empty() {
        p.error(t.fv_none.to_string());
    }
    for (i, set) in sets.iter().enumerate() {
        if p.is_cancelled() {
            break;
        }
        let name = image::set_name(set);
        p.begin_file(i, sets.len(), &name);
        match image::frames_video(job, set, fps, codec, level) {
            Ok(done) => p.result(done.text),
            Err(Failure::Cancelled) => break,
            Err(Failure::Error(e)) => p.error(format!("{} {name}\n{e}", t.failed)),
        }
    }
    if p.is_cancelled() {
        p.mark_cancelled();
        p.info(t.cancelled.to_string());
    }
    p.finish();
}

/// The original after a result was made: to the trash, or for good. When that fails it stays.
fn remove_original(job: &Job, file: &Path) {
    let t = i18n::t();
    let to_trash = job.cfg.settings.delete_mode != "permanent";
    let r = if to_trash { crate::trash::put(file) } else { std::fs::remove_file(file).map_err(|e| e.to_string()) };
    match r {
        Ok(()) => job.prog.info(format!("{}: {}", short_name(file), if to_trash { t.original_trashed } else { t.original_deleted })),
        Err(e) => job.prog.warn(format!("{}: {} {e}", short_name(file), t.original_kept)),
    }
}

fn process_one(job: &Job, req: &Request, file: &Path) -> Outcome<Done> {
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
        Request::DeleteAudio { remove, merge } => convert::delete_audio(job, file, &info, remove.as_deref(), *merge),
        Request::Webp { size, fps, quality, compression } => convert::webp(job, file, &info, *size, *fps, *quality, *compression),
        Request::Custom { id } => {
            let c = job.cfg.settings.custom(id).ok_or_else(|| unknown(id))?;
            convert::custom(job, file, &info, c)
        }
        Request::CompressImage { format, target, scale, edit } => image::compress(job, file, format, *target, *scale, edit),
        Request::Icon { sizes, crop } => image::icon(job, file, sizes, *crop),
        Request::FramesVideo { .. } => Err(Failure::Error("frames to video takes all files at once".into())),
    }
}
