//! The actions that run one command per file: Convert *, Extract Audio, frames, Delete
//! Audio, WebP animation and the own items.

use super::compress::merge_maps;
use super::output::{base_of, ensure_parent, named, new_file, short_name, unique_path};
use super::runner::{run_template, Track};
use super::{ConvertKind, Done, Job, Outcome};
use crate::cmdline::{self, Val};
use crate::config::Format;
use crate::i18n;
use crate::media::Info;
use crate::settings::Custom;
use crate::units::fmt_mb;
use std::path::{Path, PathBuf};

fn path_val(p: &Path) -> Val {
    Val::S(p.to_string_lossy().to_string())
}

/// Run `command` with {input}, {output} and `extra`
fn run(job: &Job, command: &str, file: &Path, output: &Path, extra: Vec<(&'static str, Val)>, duration: Option<f64>) -> Outcome<()> {
    let mut vars = vec![("input", path_val(file)), ("output", path_val(output))];
    vars.extend(extra);
    run_template(job, command, &vars, false, Track::whole(duration), None)
}

/// "clip.mp4 → result"
fn arrow(file: &Path, output: &Path) -> Done {
    Done::new(format!("{} → {}", short_name(file), short_name(output)), output)
}

pub(super) fn format(job: &Job, file: &Path, info: &Info, what: ConvertKind, key: &str, fmt: &Format) -> Outcome<Done> {
    let ext = fmt.extension(key).to_string();
    let extra = [("ext", ext.clone()), ("fmt", key.to_string())];
    let output = if what == ConvertKind::Frames {
        // the frames go into a new folder: an existing one is never reused
        let dir = unique_path(PathBuf::from(base_of(file)));
        let out = named(&fmt.output.replacen("{base}", &dir.to_string_lossy(), 1), file, &extra);
        ensure_parent(&out);
        out
    } else {
        new_file(&fmt.output, file, &extra)
    };
    let duration = if what == ConvertKind::Image { None } else { info.duration };
    run(job, &fmt.command, file, &output, vec![("ext", Val::S(ext)), ("fmt", Val::s(key))], duration)?;
    if what == ConvertKind::Frames {
        let dir = output.parent().unwrap_or(&output);
        return Ok(Done::new(format!("{} → {}/", short_name(file), short_name(dir)), dir));
    }
    Ok(arrow(file, &output))
}

/// `remove`: the audio tracks to remove (None = all); a track the file does not have is left out.
/// `merge`: the tracks that stay are mixed into one (when there are two or more).
pub(super) fn delete_audio(job: &Job, file: &Path, info: &Info, remove: Option<&[usize]>, merge: bool) -> Outcome<Done> {
    let d = &job.cfg.delete_audio;
    let count = info.audio.len();
    let Some(list) = remove.map(|r| r.iter().copied().filter(|i| *i < count).collect::<Vec<usize>>()) else {
        let output = new_file(&d.output, file, &[]);
        run(job, &d.command, file, &output, vec![], info.duration)?;
        return Ok(arrow(file, &output));
    };
    let kept: Vec<usize> = (0..count).filter(|i| !list.contains(i)).collect();
    if merge && kept.len() > 1 {
        let output = new_file(&d.merge_output, file, &[]);
        let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
        let codec = if matches!(ext.as_str(), "webm" | "ogg" | "ogv") { &d.merge_codec_webm } else { &d.merge_codec };
        let encode = cmdline::expand(codec, &[])?;
        let extra = vec![("mix", Val::L(merge_maps(job, &kept))), ("encode", Val::L(encode))];
        run(job, &d.merge_command, file, &output, extra, info.duration)?;
        return Ok(arrow(file, &output));
    }
    if list.is_empty() {
        return Ok(Done::nothing(format!("{}: {}", short_name(file), i18n::t().del_nothing)));
    }
    let (command, output) = if kept.is_empty() { (&d.command, &d.output) } else { (&d.tracks_command, &d.tracks_output) };
    let output = new_file(output, file, &[]);
    let maps = list.iter().flat_map(|i| ["-map".to_string(), format!("-0:a:{i}")]).collect();
    run(job, command, file, &output, vec![("remove", Val::L(maps))], info.duration)?;
    Ok(arrow(file, &output))
}

pub(super) fn webp(job: &Job, file: &Path, info: &Info, size: u32, fps: u32, quality: u32, compression: u32) -> Outcome<Done> {
    let w = &job.cfg.webp;
    let output = new_file(&w.output, file, &[]);
    let extra = vec![
        ("size", Val::S(size.to_string())),
        ("fps", Val::S(fps.to_string())),
        ("quality", Val::S(quality.to_string())),
        ("compression", Val::S(compression.to_string())),
    ];
    run(job, &w.command, file, &output, extra, info.duration)?;
    let bytes = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    Ok(Done::new(format!("{} → {} ({})", short_name(file), short_name(&output), fmt_mb(bytes)), &output))
}

/// An own item. It has no resolution choice, so {vf} and {scale} are empty.
pub(super) fn custom(job: &Job, file: &Path, info: &Info, c: &Custom) -> Outcome<Done> {
    let output = new_file(&c.output, file, &[]);
    run(job, &c.command, file, &output, vec![("vf", Val::L(vec![])), ("scale", Val::s(""))], info.duration)?;
    Ok(arrow(file, &output))
}
