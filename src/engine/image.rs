//! Compress Image, Make Icon and Frames to Video.

use super::compress::{file_size, push_node, render_node, scale_val, vf_val};
use super::output::{base_of, ensure_parent, new_file, short_name, unique_path};
use super::runner::{run_ffmpeg, run_template, TempDir, Track};
use super::{Done, Failure, ImageEdit, ImageTarget, Job, Outcome};
use crate::cmdline::{self, Val};
use crate::config::{Codec, Config, ImageFormat};
use crate::i18n;
use crate::media::{self, Kind};
use crate::scale::ScaleMode;
use crate::units::{fmt_float, fmt_size};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// The format key of "As source"
pub const SOURCE: &str = "source";

fn path_val(p: &Path) -> Val {
    Val::S(p.to_string_lossy().to_string())
}

/// The filters of the edits, then the size. The crop is a part of the turned picture and the
/// size is that of the result, so only areas and text need to know the size of the picture
/// (`size`: as ffmpeg decodes it, turned by its EXIF orientation).
pub fn edit_filters(edit: &ImageEdit, scale: ScaleMode, size: Option<(u32, u32)>) -> Result<Vec<String>, String> {
    let mut f = super::edit::build(edit, size, super::edit::Upto::All)?.filters;
    match scale {
        ScaleMode::Height(px) => f.push(format!("scale=w='if(lte(iw,ih),min(iw,{px}),-2)':h='if(lte(iw,ih),-2,min(ih,{px}))':flags=lanczos")),
        ScaleMode::Factor(v) if v < 0.999 => f.push(format!("scale=w='max(1,trunc(iw*{v:.6}))':h='max(1,trunc(ih*{v:.6}))':flags=lanczos")),
        _ => {}
    }
    Ok(f)
}

/// The filters of `edit` for `file`; the size of the picture is read only when they need it
pub(super) fn file_filters(job: &Job, file: &Path, edit: &ImageEdit, scale: ScaleMode) -> Outcome<Vec<String>> {
    let size = if super::edit::needs_size(edit) {
        Some(media::picture_size(&job.ffprobe, file).ok_or_else(|| Failure::Error(format!("ffprobe: {}", short_name(file))))?)
    } else {
        None
    };
    Ok(edit_filters(edit, scale, size)?)
}

/// The format of the result for `file` ("source" = the format of the file, as far as there is one)
pub fn format_for<'a>(cfg: &'a Config, key: &'a str, file: &Path) -> Option<(&'a str, &'a ImageFormat)> {
    if key == SOURCE {
        cfg.image_source_format(file)
    } else {
        cfg.image_format(key).map(|f| (key, f))
    }
}

/// The result of Compress Image for `file` (not made unique yet)
pub fn compressed_path(cfg: &Config, key: &str, file: &Path) -> Option<PathBuf> {
    let (key, f) = format_for(cfg, key, file)?;
    Some(super::output::named(&f.output, file, &[("ext", f.extension(key).to_string()), ("fmt", key.to_string())]))
}

/// The encoders of this ffmpeg (`ffmpeg -encoders`)
pub fn encoders(ffmpeg: &Path) -> Vec<String> {
    let Ok(out) = crate::proc::command(ffmpeg).args(["-hide_banner", "-encoders"]).output() else { return Vec::new() };
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .skip_while(|l| !l.trim_start().starts_with("------"))
        .skip(1)
        .filter_map(|l| {
            let mut words = l.split_whitespace();
            let flags = words.next()?;
            (flags.len() == 6).then(|| words.next().map(str::to_string)).flatten()
        })
        .collect()
}

/// One encode at `quality` into `out`; returns its size
fn encode(job: &Job, file: &Path, fmt: &ImageFormat, chain: &[String], quality: u32, out: &Path, track: Track) -> Outcome<u64> {
    let vars =
        vec![("input", path_val(file)), ("output", path_val(out)), ("vf", vf_val(chain)), ("chain", scale_val(chain)), ("q", Val::S(fmt.q_value(quality)))];
    run_template(job, fmt.command_for(quality), &vars, false, track, None)?;
    Ok(file_size(out))
}

/// The size the window shows before Start: the first file made with the chosen quality, in a
/// folder of its own
pub(super) fn estimate(job: &Job, file: &Path, key: &str, quality: u32, scale: ScaleMode, edit: &ImageEdit) -> Outcome<u64> {
    let (key, fmt) = format_for(&job.cfg, key, file).ok_or_else(|| Failure::Error(format!("{} {key}", i18n::t().unknown_codec)))?;
    let work = TempDir::new()?;
    let out = work.path().join(format!("estimate.{}", fmt.extension(key)));
    encode(job, file, fmt, &file_filters(job, file, edit, scale)?, quality, &out, Track::at(0.0))
}

pub(super) fn compress(job: &Job, file: &Path, key: &str, target: ImageTarget, scale: ScaleMode, edit: &ImageEdit) -> Outcome<Done> {
    let t = i18n::t();
    let (key, fmt) = format_for(&job.cfg, key, file).ok_or_else(|| Failure::Error(format!("{} {key}", t.unknown_codec)))?;
    let ext = fmt.extension(key).to_string();
    let output = new_file(&fmt.output, file, &[("ext", ext.clone()), ("fmt", key.to_string())]);
    let chain = file_filters(job, file, edit, scale)?;
    let before = file_size(file);
    let (size, quality) = match target {
        ImageTarget::Quality(q) => (encode(job, file, fmt, &chain, q, &output, Track::whole(None))?, q),
        ImageTarget::Bytes(limit) => {
            let work = TempDir::new()?;
            let (path, size, q) = search(job, file, fmt, &chain, limit, &ext, work.path())?;
            if size > limit {
                job.prog.warn(format!("\"{}\": {} {}. {}", short_name(file), t.over_limit, fmt_size(size - limit), t.img_smaller));
            }
            if std::fs::rename(&path, &output).is_err() {
                std::fs::copy(&path, &output).map_err(|e| format!("{}: {e}", output.display()))?;
            }
            (size, q)
        }
    };
    let pct = if before > 0 { format!(", {:+.0}%", (size as f64 / before as f64 - 1.0) * 100.0) } else { String::new() };
    Ok(Done::new(
        format!("{} → {} ({} → {}{pct}, {} {quality})", short_name(file), short_name(&output), fmt_size(before), fmt_size(size), t.img_quality_word),
        &output,
    ))
}

/// The highest quality whose result fits into `limit` bytes (a binary search: 7 encodes).
/// Nothing fits: the result of quality 1.
fn search(job: &Job, file: &Path, fmt: &ImageFormat, chain: &[String], limit: u64, ext: &str, work: &Path) -> Outcome<(PathBuf, u64, u32)> {
    let t = i18n::t();
    let (mut lo, mut hi) = (1u32, 100u32);
    let mut best: Option<(PathBuf, u64, u32)> = None;
    let mut smallest: Option<(PathBuf, u64, u32)> = None;
    let mut step = 0;
    while lo <= hi {
        let q = (lo + hi).div_ceil(2);
        job.prog.set_step(&t.img_trying.replace("{}", &q.to_string()));
        let out = work.join(format!("q{q}.{ext}"));
        let size = encode(job, file, fmt, chain, q, &out, Track::at(step as f64 / 7.0))?;
        step += 1;
        if size <= limit {
            best = Some((out, size, q));
            lo = q + 1;
        } else {
            if smallest.as_ref().is_none_or(|s| size < s.1) {
                smallest = Some((out, size, q));
            }
            if q == 1 {
                break;
            }
            hi = q - 1;
        }
    }
    best.or(smallest).ok_or_else(|| Failure::Error("no result".into()))
}

// ------------------------------------------------------------------ Make Icon

/// The sizes of an icon that can be made: 1 to 256 px, each once, from small to large
pub fn icon_sizes(sizes: &[u32]) -> Vec<u32> {
    let mut v: Vec<u32> = sizes.iter().copied().filter(|s| (1..=256).contains(s)).collect();
    v.sort_unstable();
    v.dedup();
    v
}

/// {graph} of the icon: the picture made square, split and scaled to every size
pub fn icon_graph(square: &str, sizes: &[u32]) -> Vec<String> {
    let labels: String = (0..sizes.len()).map(|i| format!("[i{i}]")).collect();
    let mut graph = format!("[0:v]{square},split={}{labels}", sizes.len());
    for (i, s) in sizes.iter().enumerate() {
        graph.push_str(&format!(";[i{i}]scale={s}:{s}:flags=lanczos[o{i}]"));
    }
    let mut args = vec!["-filter_complex".to_string(), graph];
    for i in 0..sizes.len() {
        args.push("-map".into());
        args.push(format!("[o{i}]"));
    }
    args
}

pub(super) fn icon(job: &Job, file: &Path, sizes: &[u32], crop: bool) -> Outcome<Done> {
    let ic = &job.cfg.icon;
    let sizes = icon_sizes(sizes);
    if sizes.is_empty() {
        return Err(Failure::Error(i18n::t().icon_no_sizes.into()));
    }
    let output = new_file(&ic.output, file, &[]);
    let square = if crop { &ic.crop } else { &ic.pad };
    let vars = vec![("input", path_val(file)), ("output", path_val(&output)), ("graph", Val::L(icon_graph(square, &sizes)))];
    run_template(job, &ic.command, &vars, false, Track::whole(None), None)?;
    let list: Vec<String> = sizes.iter().map(u32::to_string).collect();
    Ok(Done::new(format!("{} → {} ({} px)", short_name(file), short_name(&output), list.join(", ")), &output))
}

// ------------------------------------------------------------------ Frames to Video

/// "frame2" before "frame10": runs of digits compare as numbers, the rest without case
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut x, mut y) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (x.peek().copied(), y.peek().copied()) {
            (None, None) => return a.cmp(b),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(c), Some(d)) if c.is_ascii_digit() && d.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (m, n) = (take(&mut x), take(&mut y));
                let (m, n) = (m.trim_start_matches('0'), n.trim_start_matches('0'));
                let o = m.len().cmp(&n.len()).then_with(|| m.cmp(n));
                if o != Ordering::Equal {
                    return o;
                }
            }
            (Some(c), Some(d)) => {
                let o = c.to_lowercase().cmp(d.to_lowercase());
                if o != Ordering::Equal {
                    return o;
                }
                x.next();
                y.next();
            }
        }
    }
}

fn sort_frames(list: &mut [PathBuf]) {
    list.sort_by(|a, b| natural_cmp(&short_name(a), &short_name(b)).then_with(|| a.cmp(b)));
}

/// The pictures of one video
#[derive(Clone, Debug, PartialEq)]
pub struct FrameSet {
    pub frames: Vec<PathBuf>,
    /// the folder the video is named after
    pub folder: PathBuf,
    /// a folder was chosen (the video goes next to it), not pictures (the video goes among them)
    pub chosen: bool,
}

/// The pictures of a folder, in order
fn pictures_in(dir: &Path) -> Vec<PathBuf> {
    let mut list: Vec<PathBuf> =
        std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path()).filter(|p| p.is_file() && Kind::of(p) == Some(Kind::Image)).collect();
    sort_frames(&mut list);
    list
}

/// A video per chosen folder, and one of all the chosen pictures
pub fn frame_sets(files: &[PathBuf]) -> Vec<FrameSet> {
    let mut sets = Vec::new();
    let mut loose: Vec<PathBuf> = Vec::new();
    for f in files {
        if f.is_dir() {
            sets.push(FrameSet { frames: pictures_in(f), folder: f.clone(), chosen: true });
        } else if f.is_file() && Kind::of(f) == Some(Kind::Image) {
            loose.push(f.clone());
        }
    }
    if !loose.is_empty() {
        sort_frames(&mut loose);
        let folder = loose[0].parent().map(Path::to_path_buf).unwrap_or_default();
        sets.push(FrameSet { frames: loose, folder, chosen: false });
    }
    sets
}

/// Where the video of a folder goes (not made unique yet). `chosen`: the folder itself was chosen.
pub fn video_path(cfg: &Config, folder: &Path, chosen: bool) -> PathBuf {
    let name = folder.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_else(|| "video".into());
    let dir = if chosen { folder.parent().unwrap_or(folder) } else { folder };
    PathBuf::from(cfg.frames_video.output.replace("{dir}", &dir.to_string_lossy()).replace("{name}", &name))
}

fn same_kind_of_file(frames: &[PathBuf]) -> bool {
    let ext = |p: &PathBuf| match p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default().as_str() {
        "jpeg" => "jpg".to_string(),
        "tif" => "tiff".to_string(),
        e => e.to_string(),
    };
    frames.windows(2).all(|w| ext(&w[0]) == ext(&w[1]))
}

/// The list of the concat demuxer
fn concat_list(frames: &[PathBuf]) -> String {
    let mut s = String::from("ffconcat version 1.0\n");
    for f in frames {
        s.push_str(&format!("file '{}'\n", f.to_string_lossy().replace('\'', "'\\''")));
    }
    s
}

/// Pictures are decoded on the CPU: the options of the codec entry that decode on the GPU go
fn without_hw_decoding(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "-hwaccel" || a == "-hwaccel_output_format" {
            it.next();
        } else {
            out.push(a.clone());
        }
    }
    out
}

pub(super) fn frames_video(job: &Job, set: &FrameSet, fps: f64, codec_key: &str, level: usize) -> Outcome<Done> {
    let t = i18n::t();
    let codec = job.cfg.compress_codec(codec_key).ok_or_else(|| Failure::Error(format!("{} {codec_key}", t.unknown_codec)))?;
    if set.frames.len() < 2 {
        return Err(Failure::Error(t.fv_too_few.to_string()));
    }
    let node = render_node(t, codec)?;
    let (w, h) = media::picture_size(&job.ffprobe, &set.frames[0]).ok_or_else(|| Failure::Error(format!("ffprobe: {}", short_name(&set.frames[0]))))?;
    let (w, h) = ((w / 2 * 2).max(2), (h / 2 * 2).max(2));

    let work = TempDir::new()?;
    let count = set.frames.len();
    // the concat demuxer decodes every picture with the decoder of the first one
    let (frames, from) = if same_kind_of_file(&set.frames) {
        (set.frames.clone(), 0.0)
    } else {
        job.prog.info(format!("{}: {}", short_name(&set.folder), t.fv_mixed));
        let mut out = Vec::new();
        for (i, f) in set.frames.iter().enumerate() {
            let png = work.path().join(format!("{i:06}.png"));
            let args: Vec<String> = ["-y", "-i"]
                .iter()
                .map(|s| s.to_string())
                .chain([f.to_string_lossy().to_string(), "-frames:v".into(), "1".into(), png.to_string_lossy().to_string()])
                .collect();
            run_ffmpeg(job, &args, Track::at(0.3 * i as f64 / count as f64), None)?;
            out.push(png);
        }
        (out, 0.3)
    };
    let list = work.path().join("frames.ffconcat");
    std::fs::write(&list, concat_list(&frames)).map_err(|e| format!("{}: {e}", list.display()))?;

    let output = unique_path(video_path(&job.cfg, &set.folder, set.chosen));
    ensure_parent(&output);
    let chain = vec![job.cfg.frames_video.fit.replace("{w}", &w.to_string()).replace("{h}", &h.to_string())];
    let mut vars = vec![
        ("input", path_val(&list)),
        ("output", path_val(&output)),
        ("vf", vf_val(&chain)),
        ("scale", scale_val(&chain)),
        ("q", Val::S(Codec::pick(&codec.q, level))),
        ("p", Val::S(Codec::pick(&codec.p, level))),
        ("audio", Val::l(["-an"])),
    ];
    push_node(&mut vars, &node);
    let mut args = cmdline::expand(&codec.command, &vars)?;
    if args.is_empty() {
        return Err(Failure::Error("the command is empty".into()));
    }
    args.remove(0);
    let mut args = without_hw_decoding(&args);
    let list_text = list.to_string_lossy().to_string();
    let at =
        args.windows(2).position(|p| p[0] == "-i" && p[1] == list_text).ok_or_else(|| Failure::Error(format!("[compress.{codec_key}]: no -i {{input}}")))?;
    let fps_text = fmt_float(fps);
    args.splice(at..at, ["-r", fps_text.as_str(), "-f", "concat", "-safe", "0"].map(String::from));
    run_ffmpeg(job, &args, Track::part(count as f64 / fps, from, 1.0), None)?;
    Ok(Done::new(format!("{} ({count} {}) → {} ({})", short_name(&set.folder), t.fv_frames_word, short_name(&output), fmt_size(file_size(&output))), &output))
}

/// A folder name for the log when there is no folder ("clip" of "/a/clip.png")
pub fn set_name(set: &FrameSet) -> String {
    let n = short_name(&set.folder);
    if n.is_empty() {
        short_name(Path::new(&base_of(&set.frames[0])))
    } else {
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = ["f10.png", "f2.png", "F1.png", "f02b.png", "a.png", "f2a.png"].map(String::from).to_vec();
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["a.png", "F1.png", "f2.png", "f2a.png", "f02b.png", "f10.png"]);
        assert_eq!(natural_cmp("img_0009", "img_0010"), Ordering::Less);
    }

    #[test]
    fn filters_of_an_edit() {
        let e = ImageEdit { turns: 1, flip_h: true, crop: Some([0.1, 0.2, 0.5, 0.4]), ..Default::default() };
        let f = edit_filters(&e, ScaleMode::Height(720), None).unwrap();
        assert_eq!(f[0], "transpose=clock");
        assert_eq!(f[1], "hflip");
        assert_eq!(f[2], "crop=w=iw*0.500000:h=ih*0.400000:x=iw*0.100000:y=ih*0.200000:exact=1");
        assert!(f[3].starts_with("scale=w='if(lte(iw,ih),min(iw,720),-2)'"));
        assert!(edit_filters(&ImageEdit::default(), ScaleMode::Orig, None).unwrap().is_empty());
        assert_eq!(edit_filters(&ImageEdit { turns: 2, ..Default::default() }, ScaleMode::Factor(1.0), None).unwrap(), ["hflip,vflip"]);
    }

    #[test]
    fn icon_graph_has_every_size() {
        assert_eq!(icon_sizes(&[256, 16, 16, 0, 512, 32]), [16, 32, 256]);
        let g = icon_graph("SQ", &[16, 32]);
        assert_eq!(g[1], "[0:v]SQ,split=2[i0][i1];[i0]scale=16:16:flags=lanczos[o0];[i1]scale=32:32:flags=lanczos[o1]");
        assert_eq!(&g[2..], ["-map", "[o0]", "-map", "[o1]"]);
    }

    #[test]
    fn concat_list_quotes() {
        assert_eq!(concat_list(&[PathBuf::from("/a/it's.png")]), "ffconcat version 1.0\nfile '/a/it'\\''s.png'\n");
        assert!(same_kind_of_file(&[PathBuf::from("a.JPG"), PathBuf::from("b.jpeg")]));
        assert!(!same_kind_of_file(&[PathBuf::from("a.png"), PathBuf::from("b.jpg")]));
        assert_eq!(without_hw_decoding(&["-hwaccel", "cuda", "-hwaccel_output_format", "cuda", "-i", "x"].map(String::from)), ["-i", "x"]);
    }

    #[test]
    fn sets_and_names() {
        let dir = std::env::temp_dir().join(format!("cmmt-frames-{}", std::process::id()));
        let sub = dir.join("shots");
        std::fs::create_dir_all(&sub).unwrap();
        for n in ["10.png", "9.png", "notes.txt"] {
            std::fs::write(sub.join(n), b"x").unwrap();
        }
        let sets = frame_sets(&[sub.clone(), sub.join("9.png"), sub.join("10.png")]);
        assert_eq!(sets.len(), 2);
        assert_eq!(sets[0].frames, [sub.join("9.png"), sub.join("10.png")]);
        assert!(sets[0].chosen && !sets[1].chosen);
        let cfg = Config::defaults();
        assert_eq!(video_path(&cfg, &sub, true), dir.join("shots.mp4"));
        assert_eq!(video_path(&cfg, &sub, false), sub.join("shots.mp4"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
