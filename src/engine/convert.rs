//! The actions that run one command per file: Convert *, Extract Audio, frames, Delete
//! sound, WebP animation and the own items.

use super::output::{base_of, ensure_parent, named, new_file, short_name, unique_path};
use super::runner::{run_template, Track};
use super::{ConvertKind, Job, Outcome};
use crate::cmdline::Val;
use crate::config::Format;
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

fn arrow(file: &Path, shown: String) -> String {
    format!("{} → {shown}", short_name(file))
}

pub(super) fn format(job: &Job, file: &Path, info: &Info, what: ConvertKind, key: &str, fmt: &Format) -> Outcome<String> {
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
    let shown = if what == ConvertKind::Frames { output.parent().map(short_name).unwrap_or_default() + "/" } else { short_name(&output) };
    Ok(arrow(file, shown))
}

pub(super) fn delete_sound(job: &Job, file: &Path, info: &Info) -> Outcome<String> {
    let d = &job.cfg.delete_sound;
    let output = new_file(&d.output, file, &[]);
    run(job, &d.command, file, &output, vec![], info.duration)?;
    Ok(arrow(file, short_name(&output)))
}

pub(super) fn webp(job: &Job, file: &Path, info: &Info, size: u32, fps: u32, quality: u32, compression: u32) -> Outcome<String> {
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
    Ok(arrow(file, format!("{} ({})", short_name(&output), fmt_mb(bytes))))
}

/// An own item. It has no resolution choice, so {vf} and {scale} are empty.
pub(super) fn custom(job: &Job, file: &Path, info: &Info, c: &Custom) -> Outcome<String> {
    let output = new_file(&c.output, file, &[]);
    run(job, &c.command, file, &output, vec![("vf", Val::L(vec![])), ("scale", Val::s(""))], info.duration)?;
    Ok(arrow(file, short_name(&output)))
}
