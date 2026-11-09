//! The actions that run one command per file: Convert *, Extract Audio, frames, Delete
//! Audio, Make Anim and the own items.

use super::compress::{merge_maps, scale_val};
use super::image::file_filters;
use super::output::{base_of, ensure_parent, named, new_file, short_name, unique_path};
use super::runner::{run_template, TempDir, Track};
use super::{Anim, AnimFormat, ConvertKind, Done, Job, Outcome};
use crate::cmdline::{self, Val};
use crate::config::Format;
use crate::i18n;
use crate::media::Info;
use crate::scale::ScaleMode;
use crate::settings::Custom;
use crate::units::{fmt_float, fmt_mb};
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

/// A command of an older presets.toml has no {edit}: the edits go after its fps filter
fn with_edit(command: &str) -> Option<String> {
    if command.contains("{edit}") {
        return Some(command.to_string());
    }
    command.contains("fps={fps},").then(|| command.replacen("fps={fps},", "fps={fps},{edit}", 1))
}

/// A command of an older presets.toml has no {speed}: the speed goes before its fps filter
fn with_speed(command: &str) -> Option<String> {
    if command.contains("{speed}") {
        return Some(command.to_string());
    }
    command.contains("fps={fps},").then(|| command.replacen("fps={fps},", "{speed}fps={fps},", 1))
}

/// {speed}: the filters of the speed with a trailing comma, nothing at the speed 1
fn speed_filters(speed: f64, smooth: bool, fps: u32) -> String {
    if (speed - 1.0).abs() < 1e-6 {
        return String::new();
    }
    let pts = format!("setpts=(PTS-STARTPTS)/{},", fmt_float(speed));
    if smooth && speed < 1.0 {
        format!("{pts}minterpolate=fps={fps}:mi_mode=mci,")
    } else {
        pts
    }
}

/// The colours of a GIF are picked at this frame rate at most
const PALETTE_FPS: u32 = 5;

/// Make Anim: an animated WebP, or a GIF in two runs (its palette, then the GIF)
pub(super) fn anim(job: &Job, file: &Path, info: &Info, a: &Anim) -> Outcome<Done> {
    let chain = file_filters(job, file, &a.edit, ScaleMode::Orig)?;
    let (section, command, output) = match a.format {
        AnimFormat::Webp => ("[webp_anim]", &job.cfg.webp.command, &job.cfg.webp.output),
        AnimFormat::Gif => ("[gif_anim]", &job.cfg.gif.command, &job.cfg.gif.output),
    };
    let t = i18n::t();
    let speed = speed_filters(a.speed, a.smooth, a.fps);
    let fixed = |command: &str| {
        let command = with_edit(command).unwrap_or_else(|| {
            if !chain.is_empty() {
                job.prog.warn(format!("{section}: {}", t.anim_no_edit));
            }
            command.to_string()
        });
        with_speed(&command).unwrap_or_else(|| {
            if !speed.is_empty() {
                job.prog.warn(format!("{section}: {}", t.anim_no_speed));
            }
            command
        })
    };
    let output = new_file(output, file, &[]);
    // the result is as long as the video at its new speed
    let length = info.duration.map(|d| d / a.speed);
    let mut extra = vec![
        ("size", Val::S(a.size.to_string())),
        ("fps", Val::S(a.fps.to_string())),
        ("quality", Val::S(a.quality.to_string())),
        ("compression", Val::S(a.compression.to_string())),
        ("dither", Val::S(a.dither.clone())),
        ("edit", scale_val(&chain)),
        ("speed", Val::S(speed.clone())),
    ];
    if a.format == AnimFormat::Gif {
        let work = TempDir::new()?;
        let palette = work.path().join("palette.png");
        extra.push(("palette", path_val(&palette)));
        extra.push(("palette_fps", Val::S(a.fps.min(PALETTE_FPS).to_string())));
        let mut vars = vec![("input", path_val(file)), ("output", path_val(&output))];
        vars.extend(extra);
        // ffmpeg tells nothing of its progress until the palette is written
        job.prog.set_step(t.anim_colours);
        job.prog.set_busy(true);
        let palette_command = with_edit(&job.cfg.gif.palette_command).unwrap_or_else(|| job.cfg.gif.palette_command.clone());
        run_template(job, &palette_command, &vars, false, Track::at(0.0), None)?;
        job.prog.set_busy(false);
        job.prog.set_step("");
        run_template(job, &fixed(command), &vars, false, Track::whole(length), None)?;
    } else {
        run(job, &fixed(command), file, &output, extra, length)?;
    }
    let bytes = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    Ok(Done::new(format!("{} → {} ({})", short_name(file), short_name(&output), fmt_mb(bytes)), &output))
}
/// An own item. It has no resolution choice, so {vf} and {scale} are empty.
pub(super) fn custom(job: &Job, file: &Path, info: &Info, c: &Custom) -> Outcome<Done> {
    let output = new_file(&c.output, file, &[]);
    run(job, &c.command, file, &output, vec![("vf", Val::L(vec![])), ("scale", Val::s(""))], info.duration)?;
    Ok(arrow(file, &output))
}

#[cfg(test)]
mod tests {
    use super::{with_edit, with_speed};

    #[test]
    fn old_commands_get_the_edits_after_the_fps() {
        assert_eq!(with_edit("ffmpeg -vf \"fps={fps},scale=1:1\" {output}").unwrap(), "ffmpeg -vf \"fps={fps},{edit}scale=1:1\" {output}");
        assert_eq!(with_edit("a {edit}b").unwrap(), "a {edit}b");
        assert_eq!(with_edit("ffmpeg -vf scale=1:1 {output}"), None);
    }

    #[test]
    fn old_commands_get_the_speed_before_the_fps() {
        let old = "ffmpeg -vf \"fps={fps},scale=1:1\" {output}";
        assert_eq!(with_speed(&with_edit(old).unwrap()).unwrap(), "ffmpeg -vf \"{speed}fps={fps},{edit}scale=1:1\" {output}");
        assert_eq!(with_speed("a {speed}b").unwrap(), "a {speed}b");
        assert_eq!(with_speed("ffmpeg -vf scale=1:1 {output}"), None);
    }

    #[test]
    fn the_speed_filters() {
        assert_eq!(super::speed_filters(1.0, true, 20), "");
        assert_eq!(super::speed_filters(2.0, true, 20), "setpts=(PTS-STARTPTS)/2,");
        assert_eq!(super::speed_filters(0.5, false, 20), "setpts=(PTS-STARTPTS)/0.5,");
        assert_eq!(super::speed_filters(0.25, true, 15), "setpts=(PTS-STARTPTS)/0.25,minterpolate=fps=15:mi_mode=mci,");
    }
}
