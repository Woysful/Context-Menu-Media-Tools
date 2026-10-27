//! Compress Video: the presets (fixed quality) and the size limit mode.

use super::budget::{self, bitrates, chunk_target, plan_chunks};
use super::output::{new_file, short_name};
use super::runner::{run_ffmpeg, run_template, value_after, TempDir, Track};
use super::{Job, Outcome};
use crate::cmdline::Val;
use crate::config::Codec;
use crate::gpu;
use crate::i18n::{self, T};
use crate::media::Info;
use crate::scale::{self, ScaleMode};
use crate::units::{fmt_float, fmt_mb};
use std::path::Path;

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

// ------------------------------------------------------------------ presets

pub(super) fn preset(job: &Job, file: &Path, info: &Info, level: usize, codec: &Codec, mode: ScaleMode) -> Outcome<String> {
    let node = render_node(i18n::t(), codec)?;
    let (chain, _) = scale_chain(job, file, info, mode, 30.0, 0.0, 0.0);
    let output = new_file(&codec.output, file, &[]);
    let mut vars = vec![
        ("input", path_val(file)),
        ("output", path_val(&output)),
        ("vf", vf_val(&chain)),
        ("scale", scale_val(&chain)),
        ("q", Val::S(Codec::pick(&codec.q, level))),
        ("p", Val::S(Codec::pick(&codec.p, level))),
    ];
    push_node(&mut vars, &node);
    run_template(job, &codec.command, &vars, !chain.is_empty(), Track::whole(info.duration), None)?;
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
    /// audio options of the result (empty without audio)
    audio: Vec<String>,
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
    audio: Vec<String>,
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
            ("audio", Val::L(step.audio)),
        ];
        push_node(&mut v, &self.node);
        v
    }
}

pub(super) fn limit(job: &Job, file: &Path, info: &Info, codec: &Codec, mb: f64, mode: ScaleMode, want_twopass: bool) -> Outcome<String> {
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
    let br = bitrates(mb, dur, fps, info.has_audio, aim, twopass || chunked);
    if br.tight {
        job.prog.warn(format!("\"{}\": {}", short_name(file), t.tight));
    }

    // Resolution; "auto" walks down a ladder until the picture gets enough bits per pixel
    let (chain, scale_note) = scale_chain(job, file, info, mode, fps, br.video as f64, codec.bpp_min);
    let audio = if info.has_audio { vec![s("-c:a"), s("aac"), s("-b:a"), br.audio.to_string()] } else { vec![] };
    let plan = Plan { file, codec, mb, node, chain, audio, dur, fps, has_audio: info.has_audio, aim };

    let mb_text = fmt_float(mb);
    let output = new_file(&codec.output, file, &[("mb", mb_text.clone())]);
    let mode_note = if chunked {
        " | chunked"
    } else if twopass {
        " | 2-pass"
    } else {
        ""
    };
    job.prog.info(format!(
        "{} | {dur:.1}s | video={}k audio={}k{mode_note}{scale_note} → {}",
        short_name(file),
        br.video / 1000,
        br.audio / 1000,
        short_name(&output)
    ));

    if chunked {
        encode_chunked(job, &output, &plan)?;
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
        let step = Step { output: path_val(output), overwrite: "-n", rate, seek: vec![], pass: vec![], audio: plan.audio.clone(), filters: &plan.chain };
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
    let null = Val::l([s("-f"), s("null"), s(if cfg!(windows) { "NUL" } else { "/dev/null" })]);
    job.prog.set_step(t.pass1);
    let vars = plan.vars(Step { output: null, overwrite: "-y", rate: rate.clone(), seek: vec![], pass: p1, audio: vec![s("-an")], filters: &plan.chain });
    run_template(job, &codec.command, &vars, hw_drop, Track::part(plan.dur, 0.0, 0.5), Some(dir.path()))?;
    job.prog.set_step(t.pass2);
    let vars = plan.vars(Step { output: path_val(output), overwrite: "-n", rate, seek: vec![], pass: p2, audio: plan.audio.clone(), filters: &plan.chain });
    run_template(job, &codec.command, &vars, hw_drop, Track::part(plan.dur, 0.5, 1.0), Some(dir.path()))
}

/// Closed-loop encode for hardware encoders (NVENC / VAAPI / AMF / QSV / V4L2). They only
/// approximate the requested bitrate, by an amount that depends on the GPU, the driver and the
/// footage, so a bitrate computed in advance can miss the size limit by several percent. Here
/// the video is cut into chunks and every chunk is encoded exactly once with the command of
/// the entry; after each one the real size is measured and the bitrate of the remaining chunks
/// is corrected by what the encoder actually did. The chunks are then joined without
/// re-encoding. The last chunk is short (its error is the one that stays in the result).
fn encode_chunked(job: &Job, output: &Path, plan: &Plan) -> Outcome<()> {
    let t = i18n::t();
    let work = TempDir::new()?;
    let (dur, file) = (plan.dur, plan.file);
    // Narrow rate window per chunk: a chunk may not run far over what was requested for it
    // (a smaller chunk is harmless, the unused bytes go to the next ones)
    let (rc_max, rc_buf) = (110.0, 100.0);

    // Audio is encoded once; afterwards its size is known exactly
    let audio_path = work.path().join("audio.m4a");
    let mut audio_bytes = 0.0;
    if plan.has_audio {
        job.prog.set_step(t.audio_track);
        let mut args = vec![s("-y"), s("-i"), file.to_string_lossy().to_string(), s("-vn")];
        args.extend(plan.audio.iter().cloned());
        args.push(audio_path.to_string_lossy().to_string());
        run_ffmpeg(job, &args, Track::at(0.0), None)?;
        audio_bytes = file_size(&audio_path) as f64;
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
            audio: vec![s("-an")],
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
