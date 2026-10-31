//! Compress Video: the presets (fixed quality) and the size limit mode.

use super::budget::{self, bitrates, chunk_target, plan_chunks, AudioSize};
use super::output::{new_file, short_name};
use super::runner::{run_ffmpeg, run_template, value_after, TempDir, Track};
use super::{Failure, Job, Outcome};
use crate::cmdline::{self, Val};
use crate::config::{AudioCodec, Codec};
use crate::gpu;
use crate::i18n::{self, T};
use crate::media::Info;
use crate::scale::{self, ScaleMode};
use crate::units::{fmt_float, fmt_mb};
use std::path::{Path, PathBuf};

fn s(x: &str) -> String {
    x.to_string()
}

fn path_val(p: &Path) -> Val {
    Val::S(p.to_string_lossy().to_string())
}

fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

/// `{vf}` of the commands: "-vf <filters>", or nothing
fn vf_val(chain: &[String]) -> Val {
    if chain.is_empty() {
        Val::L(vec![])
    } else {
        Val::l([s("-vf"), chain.join(",")])
    }
}

/// `{scale}` of the commands: the filters with a trailing comma, for an own -vf chain
fn scale_val(chain: &[String]) -> Val {
    if chain.is_empty() {
        Val::s("")
    } else {
        Val::S(format!("{},", chain.join(",")))
    }
}

fn rate_val(bps: u64, max_pct: f64, buf_pct: f64) -> Val {
    let pct = |p: f64| ((bps as f64 * p / 100.0) as u64).to_string();
    Val::l([s("-b:v"), bps.to_string(), s("-maxrate"), pct(max_pct), s("-bufsize"), pct(buf_pct)])
}

/// VAAPI needs the render device of the GPU of that vendor; the other backends need nothing
fn render_node(t: &T, c: &Codec) -> Result<Option<String>, String> {
    if c.backend != "vaapi" {
        return Ok(None);
    }
    let name = if c.vendor == "amd" { "AMD" } else { "Intel" };
    let node = gpu::render_node(&c.vendor).ok_or_else(|| format!("{} {name}", t.no_gpu))?;
    if std::fs::OpenOptions::new().read(true).write(true).open(&node).is_err() {
        return Err(format!("{} {node}", t.gpu_access));
    }
    Ok(Some(node))
}

fn push_node(vars: &mut Vec<(&'static str, Val)>, node: &Option<String>) {
    if let Some(n) = node {
        vars.push(("render_node", Val::s(n.clone())));
    }
}

/// The software scaling filter for the chosen resolution (empty = untouched) and a note for the log
fn scale_chain(job: &Job, file: &Path, info: &Info, mode: ScaleMode, fps: f64, video_bps: f64, bpp_min: f64) -> (Vec<String>, String) {
    let t = i18n::t();
    if mode == ScaleMode::Orig || info.width == 0 || info.height == 0 {
        return (vec![], String::new());
    }
    let ch = scale::choose(mode, info.width, info.height, fps, video_bps, bpp_min);
    if ch.factor >= 1.0 {
        return (vec![], String::new());
    }
    let note = if bpp_min > 0.0 {
        format!(" | {} x{:.4} ({} {}px, {:.3} bit/px)", t.scale_word, ch.factor, t.shorter_side, ch.short_px, ch.bpp)
    } else {
        job.prog.info(format!("{}: {} x{:.4} ({} {}px)", short_name(file), t.scale_word, ch.factor, t.shorter_side, ch.short_px));
        String::new()
    };
    (vec![scale::filter(ch.factor)], note)
}

// ------------------------------------------------------------------ audio

/// Codecs a copied track may have in an MP4 / MOV that players understand
const MP4_AUDIO: &[&str] = &["aac", "mp3", "mp2", "ac3", "eac3", "opus", "flac", "alac", "dts"];

/// The audio entry for this file: the chosen one, or its fallback when a track cannot be copied
/// into the result
fn audio_entry<'a>(job: &'a Job, file: &Path, info: &Info, key: &str, output: &Path) -> Outcome<&'a AudioCodec> {
    let t = i18n::t();
    let find = |k: &str| job.cfg.audio_codec(k).ok_or_else(|| Failure::Error(format!("{} {k}", t.unknown_codec)));
    let chosen = find(key)?;
    let ext = output.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    if chosen.fallback.is_empty() || !matches!(ext.as_str(), "mp4" | "m4v" | "mov") {
        return Ok(chosen);
    }
    match info.audio_codecs.iter().find(|c| !MP4_AUDIO.contains(&c.as_str())) {
        Some(bad) => {
            let fallback = find(&chosen.fallback)?;
            job.prog.info(format!("{}: {bad} {} {}", short_name(file), t.audio_no_copy, fallback.label));
            Ok(fallback)
        }
        None => Ok(chosen),
    }
}

/// The options of the audio tracks, {bitrate} filled in
fn audio_args(a: &AudioCodec, bitrate: &str) -> Result<Vec<String>, String> {
    cmdline::expand(&a.args, &[("bitrate", Val::s(bitrate))])
}

/// {audio} of a command: the video and every audio track, with the options of the audio entry
/// (nothing for a file without audio)
fn audio_val(has_audio: bool, args: &[String]) -> Val {
    if !has_audio {
        return Val::L(vec![]);
    }
    let mut v = vec![s("-map"), s("0:v:0"), s("-map"), s("0:a")];
    v.extend(args.iter().cloned());
    Val::L(v)
}

/// A command of an older presets.toml has no {audio}: it goes in front of the last {output}
fn with_audio(command: &str) -> String {
    let has = cmdline::placeholders(command).is_ok_and(|p| p.iter().any(|x| x == "audio"));
    match command.rfind("{output}") {
        Some(i) if !has => {
            let quoted = i > 0 && matches!(command.as_bytes()[i - 1], b'"' | b'\'');
            let at = if quoted { i - 1 } else { i };
            format!("{}{{audio}} {}", &command[..at], &command[at..])
        }
        _ => command.to_string(),
    }
}

/// Only the audio tracks of the file, with `args`, into `out`; returns its size
fn encode_audio(job: &Job, file: &Path, args: &[String], out: &Path) -> Outcome<f64> {
    job.prog.set_step(i18n::t().audio_track);
    let mut all = vec![s("-y"), s("-i"), file.to_string_lossy().to_string(), s("-vn"), s("-map"), s("0:a")];
    all.extend(args.iter().cloned());
    all.push(out.to_string_lossy().to_string());
    run_ffmpeg(job, &all, Track::at(0.0), None)?;
    Ok(file_size(out) as f64)
}

// ------------------------------------------------------------------ presets

pub(super) fn preset(job: &Job, file: &Path, info: &Info, level: usize, codec: &Codec, mode: ScaleMode, audio: &str) -> Outcome<String> {
    let node = render_node(i18n::t(), codec)?;
    let (chain, _) = scale_chain(job, file, info, mode, 30.0, 0.0, 0.0);
    let output = new_file(&codec.output, file, &[]);
    let a = audio_entry(job, file, info, audio, &output)?;
    let audio_opts = audio_args(a, &a.bitrate)?;
    let mut vars = vec![
        ("input", path_val(file)),
        ("output", path_val(&output)),
        ("vf", vf_val(&chain)),
        ("scale", scale_val(&chain)),
        ("q", Val::S(Codec::pick(&codec.q, level))),
        ("p", Val::S(Codec::pick(&codec.p, level))),
        ("audio", audio_val(info.has_audio, &audio_opts)),
    ];
    push_node(&mut vars, &node);
    run_template(job, &with_audio(&codec.command), &vars, !chain.is_empty(), Track::whole(info.duration), None)?;
    Ok(format!("{} → {} ({})", short_name(file), short_name(&output), fmt_mb(file_size(&output))))
}

// ------------------------------------------------------------------ size limit

/// Everything an encode in the size limit mode needs
struct Plan<'a> {
    file: &'a Path,
    codec: &'a Codec,
    mb: f64,
    node: Option<String>,
    /// software filters (scaling)
    chain: Vec<String>,
    /// options of the audio tracks (codec, bitrate)
    audio: Vec<String>,
    /// the audio, already made (copied / lossless tracks, whose size must be known first)
    audio_file: Option<PathBuf>,
    dur: f64,
    fps: f64,
    has_audio: bool,
    aim: f64,
}

/// What differs between the encodes of one plan (passes, chunks)
struct Step<'a> {
    output: Val,
    overwrite: &'static str,
    rate: Val,
    seek: Vec<String>,
    pass: Vec<String>,
    audio: Val,
    filters: &'a [String],
}

impl Plan<'_> {
    fn vars(&self, step: Step) -> Vec<(&'static str, Val)> {
        let mut v = vec![
            ("input", path_val(self.file)),
            ("output", step.output),
            ("vf", vf_val(step.filters)),
            ("scale", scale_val(step.filters)),
            ("overwrite", Val::s(step.overwrite)),
            ("seek", Val::L(step.seek)),
            ("rate", step.rate),
            ("pass", Val::L(step.pass)),
            ("audio", step.audio),
        ];
        push_node(&mut v, &self.node);
        v
    }

    fn audio_val(&self) -> Val {
        audio_val(self.has_audio, &self.audio)
    }
}

fn no_audio() -> Val {
    Val::l(["-an"])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn limit(job: &Job, file: &Path, info: &Info, codec: &Codec, mb: f64, mode: ScaleMode, want_twopass: bool, audio: &str) -> Outcome<String> {
    let t = i18n::t();
    if mb.is_nan() || mb <= 0.0 {
        return Err(format!("{} {mb}", t.invalid_limit).into());
    }
    let node = render_node(t, codec)?;

    // The budget math needs the exact duration, the frame rate (container overhead grows
    // with the number of frames) and whether there is an audio track at all.
    let dur = info.duration.filter(|d| *d > 0.0).unwrap_or(1.0);
    let fps = info.fps.unwrap_or(30.0);

    let chunked = match std::env::var("CMMT_CHUNKED").as_deref() {
        Ok("1") => true,
        Ok("0") => false,
        _ => codec.chunked,
    };
    // chunked mode replaces two-pass; both see the real result, so neither needs the extra
    // reserve of a one-pass encode on a short video
    let twopass = want_twopass && !codec.twopass.is_empty() && !chunked;
    let aim = if chunked { 99.0 } else { codec.aim };

    let mb_text = fmt_float(mb);
    let output = new_file(&codec.output, file, &[("mb", mb_text.clone())]);
    // Audio without a bitrate of its own (copied, lossless) is made first: the video gets what is left
    let a = audio_entry(job, file, info, audio, &output)?;
    let work = TempDir::new()?;
    let (audio_file, audio_size) = if !info.has_audio {
        (None, AudioSize::None)
    } else if a.has_bitrate() {
        (None, AudioSize::Rate)
    } else {
        let path = work.path().join("audio.mka");
        let bytes = encode_audio(job, file, &audio_args(a, "")?, &path)?;
        (Some(path), AudioSize::Bytes(bytes))
    };
    let br = bitrates(mb, dur, fps, audio_size, aim, twopass || chunked);
    if br.tight {
        job.prog.warn(format!("\"{}\": {}", short_name(file), t.tight));
    }

    // Resolution; "auto" walks down a ladder until the picture gets enough bits per pixel
    let (chain, scale_note) = scale_chain(job, file, info, mode, fps, br.video as f64, codec.bpp_min);
    // the audio bitrate is shared by the tracks
    let tracks = info.audio_codecs.len().max(1) as u64;
    let audio_opts = audio_args(a, &(br.audio / tracks).to_string())?;
    let plan = Plan { file, codec, mb, node, chain, audio: audio_opts, audio_file, dur, fps, has_audio: info.has_audio, aim };

    let mode_note = if chunked {
        " | chunked"
    } else if twopass {
        " | 2-pass"
    } else {
        ""
    };
    job.prog.info(format!(
        "{} | {dur:.1}s | video={}k audio={} {}k{mode_note}{scale_note} → {}",
        short_name(file),
        br.video / 1000,
        a.label,
        br.audio / 1000,
        short_name(&output)
    ));

    if chunked {
        encode_chunked(job, &output, &plan, &work)?;
    } else {
        encode_whole(job, &output, &plan, br.video, twopass)?;
    }

    let bytes = file_size(&output);
    let limit_bytes = mb * 1_048_576.0;
    let mut text = format!(
        "{} → {} — {} ({:.2} MB / decimal) | {} {mb_text} MB = {:.1}%",
        short_name(file),
        short_name(&output),
        fmt_mb(bytes),
        bytes as f64 / 1e6,
        t.target,
        bytes as f64 / limit_bytes * 100.0
    );
    if bytes as f64 > limit_bytes {
        let over = format!("{} {:.2} MB", t.over_limit, (bytes as f64 - limit_bytes) / 1_048_576.0);
        job.prog.warn(over.clone());
        text.push_str(&format!("\n⚠ {over}"));
    }
    Ok(text)
}

/// One pass at a bitrate computed in advance, or two passes: pass 1 only analyses the video
/// and writes nothing, pass 2 makes the single real encode with the whole video known in
/// advance, which makes the size predictable.
fn encode_whole(job: &Job, output: &Path, plan: &Plan, video_bps: u64, twopass: bool) -> Outcome<()> {
    let t = i18n::t();
    let codec = plan.codec;
    let rate = rate_val(video_bps, codec.rc_max, codec.rc_buf);
    let hw_drop = !plan.chain.is_empty();
    if !twopass {
        let step = Step { output: path_val(output), overwrite: "-n", rate, seek: vec![], pass: vec![], audio: plan.audio_val(), filters: &plan.chain };
        let vars = plan.vars(step);
        return run_template(job, &codec.command, &vars, hw_drop, Track::part(plan.dur, 0.0, 1.0), None);
    }
    // the pass logs are written to the working folder
    let dir = TempDir::new()?;
    let (p1, p2): (Vec<String>, Vec<String>) = if codec.twopass == "x265" {
        (vec![s("-x265-params"), s("pass=1:stats=x265.log")], vec![s("-x265-params"), s("pass=2:stats=x265.log")])
    } else {
        (vec![s("-pass"), s("1"), s("-passlogfile"), s("pass")], vec![s("-pass"), s("2"), s("-passlogfile"), s("pass")])
    };
    // Pass 1 writes a real file of the same container: the null muxer keeps the frame times of
    // the source, MP4 makes the frame rate constant, so from an MKV (millisecond times) pass 2
    // would get a duplicated frame more than pass 1 and x264 stops with "Incomplete MB-tree stats"
    let ext = output.extension().map_or_else(|| s("mp4"), |e| e.to_string_lossy().to_string());
    let scratch = path_val(&dir.path().join(format!("pass1.{ext}")));
    job.prog.set_step(t.pass1);
    let vars = plan.vars(Step { output: scratch, overwrite: "-y", rate: rate.clone(), seek: vec![], pass: p1, audio: no_audio(), filters: &plan.chain });
    run_template(job, &codec.command, &vars, hw_drop, Track::part(plan.dur, 0.0, 0.5), Some(dir.path()))?;
    job.prog.set_step(t.pass2);
    let vars = plan.vars(Step { output: path_val(output), overwrite: "-n", rate, seek: vec![], pass: p2, audio: plan.audio_val(), filters: &plan.chain });
    run_template(job, &codec.command, &vars, hw_drop, Track::part(plan.dur, 0.5, 1.0), Some(dir.path()))
}

/// Closed-loop encode for hardware encoders (NVENC / VAAPI / AMF / QSV / V4L2). They only
/// approximate the requested bitrate, by an amount that depends on the GPU, the driver and the
/// footage, so a bitrate computed in advance can miss the size limit by several percent. Here
/// the video is cut into chunks and every chunk is encoded exactly once with the command of
/// the entry; after each one the real size is measured and the bitrate of the remaining chunks
/// is corrected by what the encoder actually did. The chunks are then joined without
/// re-encoding. The last chunk is short (its error is the one that stays in the result).
fn encode_chunked(job: &Job, output: &Path, plan: &Plan, work: &TempDir) -> Outcome<()> {
    let t = i18n::t();
    let (dur, file) = (plan.dur, plan.file);
    // Narrow rate window per chunk: a chunk may not run far over what was requested for it
    // (a smaller chunk is harmless, the unused bytes go to the next ones)
    let (rc_max, rc_buf) = (110.0, 100.0);

    // Audio is made once (unless that was done already); afterwards its size is known exactly
    let audio_path = plan.audio_file.clone().unwrap_or_else(|| work.path().join("audio.mka"));
    let mut audio_bytes = 0.0;
    if plan.has_audio {
        audio_bytes = match &plan.audio_file {
            Some(p) => file_size(p) as f64,
            None => encode_audio(job, file, &plan.audio, &audio_path)?,
        };
    }

    // What is left for the video: the limit (times the aim), minus the container overhead
    // and the finished audio track
    let mut remaining_bytes = plan.mb * 1_048_576.0 * plan.aim / 100.0 - budget::overhead(dur, plan.fps, plan.has_audio) - audio_bytes;
    let mut remaining_dur = dur;
    let (n, len) = plan_chunks(dur);
    let (mut sum_req, mut sum_actual) = (0.0, 0.0);
    let mut list = String::new();

    for i in 0..n {
        let last = i == n - 1;
        let start = len * i as f64;
        // Cut by absolute time with the trim filter (every frame lands in exactly one chunk)
        // and restart the time stamps at 0 so the chunks join seamlessly. -t only stops
        // reading soon after the cut, it is not the cut itself.
        let (dur_i, mut filters, seek) = if last {
            (remaining_dur, vec![s("setpts=PTS-STARTPTS")], vec![s("-ss"), format!("{start:.6}")])
        } else {
            (len, vec![format!("trim=end={len:.6}"), s("setpts=PTS-STARTPTS")], vec![s("-ss"), format!("{start:.6}"), s("-t"), format!("{:.6}", len + 1.0)])
        };
        filters.extend(plan.chain.iter().cloned());
        let (bps, req_bytes) = chunk_target(remaining_bytes, remaining_dur, dur_i, sum_actual, sum_req, last);
        let chunk_path = work.path().join(format!("chunk_{i}.mkv"));

        job.prog.set_step(&format!("{} {}/{}", t.chunk, i + 1, n));
        let step = Step {
            output: path_val(&chunk_path),
            overwrite: "-y",
            rate: rate_val(bps, rc_max, rc_buf),
            seek,
            pass: vec![],
            audio: no_audio(),
            filters: &filters,
        };
        let vars = plan.vars(step);
        // frames stay in normal memory here (trim / setpts are software filters)
        let track = Track::part(dur_i, i as f64 / n as f64, (i + 1) as f64 / n as f64);
        run_template(job, &plan.codec.command, &vars, true, track, None)?;

        let actual = file_size(&chunk_path) as f64;
        job.prog.info(format!("   {} {}/{}: {} B (x{:.2})", t.chunk, i + 1, n, actual as u64, actual / req_bytes));
        sum_actual += actual;
        sum_req += req_bytes;
        remaining_bytes -= actual;
        remaining_dur -= dur_i;
        list.push_str(&format!("file '{}'\n", chunk_path.to_string_lossy().replace('\\', "/").replace('\'', "'\\''")));
    }

    // Join without re-encoding; the HEVC tag of the encoder settings is kept
    job.prog.set_step(t.joining);
    let list_path = work.path().join("chunks.txt");
    std::fs::write(&list_path, list).map_err(|e| e.to_string())?;
    let mut args = vec![s("-y"), s("-f"), s("concat"), s("-safe"), s("0"), s("-i"), list_path.to_string_lossy().to_string()];
    if plan.has_audio {
        args.extend([s("-i"), audio_path.to_string_lossy().to_string()]);
    }
    args.extend([s("-map"), s("0:v")]);
    if plan.has_audio {
        args.extend([s("-map"), s("1:a")]);
    }
    args.extend([s("-c"), s("copy")]);
    let tokens = crate::cmdline::split(&plan.codec.command).unwrap_or_default();
    if let Some(tag) = value_after(&tokens, "-tag:v") {
        args.extend([s("-tag:v"), tag.to_string()]);
    }
    args.push(output.to_string_lossy().to_string());
    run_ffmpeg(job, &args, Track::at(1.0), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_old_command_gets_the_audio_in_front_of_the_output() {
        assert_eq!(with_audio("ffmpeg -n -i {input} -crf 20 {output}"), "ffmpeg -n -i {input} -crf 20 {audio} {output}");
        assert_eq!(with_audio("ffmpeg -i {input} \"{output}\""), "ffmpeg -i {input} {audio} \"{output}\"");
        let new = "ffmpeg -i {input} {audio} -movflags +faststart {output}";
        assert_eq!(with_audio(new), new);
    }

    #[test]
    fn audio_maps_every_track() {
        assert_eq!(audio_val(true, &[s("-c:a"), s("copy")]), Val::l(["-map", "0:v:0", "-map", "0:a", "-c:a", "copy"]));
        assert_eq!(audio_val(false, &[s("-c:a"), s("copy")]), Val::L(vec![]));
    }
}
