//! The encoding jobs: everything that used to be shell + awk in compress_mp4.sh,
//! compress_limit.sh, convert_media.sh and convert_webp_anim.sh.
//!
//! A job runs on its own thread and reports through `Progress` (the window reads it,
//! or the console in `--cli` mode).

use crate::cmdline::{expand, Val};
use crate::config::{Codec, Config, Format};
use crate::gpu;
use crate::i18n::{self, T};
use crate::media::{self, Info, Kind};
use crate::scale::{self, ScaleMode};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// No console window flashes up when a GUI program starts ffmpeg on Windows
pub fn hide_console(cmd: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    #[cfg(not(windows))]
    {
        let _ = cmd;
    }
}

// ------------------------------------------------------------------ requests

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConvertKind {
    Video,
    VideoAudio,
    Extract,
    Frames,
    AudioFile,
    Image,
}

impl ConvertKind {
    pub fn required_kind(self) -> Kind {
        match self {
            ConvertKind::AudioFile => Kind::Audio,
            ConvertKind::Image => Kind::Image,
            _ => Kind::Video,
        }
    }
}

#[derive(Clone, Debug)]
pub enum Request {
    CompressPreset { level: usize, codec: String, scale: ScaleMode },
    CompressLimit { mb: f64, codec: String, scale: ScaleMode, twopass: bool },
    Convert { what: ConvertKind, format: String },
    DeleteSound,
    Webp { size: u32, fps: u32, quality: u32, compression: u32 },
    /// An own menu item (settings.toml)
    Custom { id: String },
}

impl Request {
    fn required_kind(&self, cfg: &Config) -> Kind {
        match self {
            Request::Convert { what, .. } => what.required_kind(),
            Request::Custom { id } => cfg.settings.custom.iter().find(|c| &c.id == id).map_or(Kind::Video, |c| c.media_kind()),
            _ => Kind::Video,
        }
    }
}

// ------------------------------------------------------------------ progress

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum LineKind {
    Info,
    Warn,
    Error,
    Result,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub kind: LineKind,
    pub text: String,
}

#[derive(Default)]
pub struct State {
    pub file_index: usize,
    pub file_count: usize,
    pub file_name: String,
    pub step: String,
    /// 0..1 inside the current file
    pub fraction: f64,
    pub speed: Option<f64>,
    pub file_started: Option<Instant>,
    pub lines: Vec<Line>,
    pub finished: bool,
    pub cancelled: bool,
    pub errors: usize,
    pub warnings: usize,
}

pub struct Progress {
    pub state: Mutex<State>,
    pub cancel: AtomicBool,
    repaint: Box<dyn Fn() + Send + Sync>,
    console: bool,
}

impl Progress {
    pub fn new(repaint: Box<dyn Fn() + Send + Sync>, console: bool) -> Arc<Progress> {
        Arc::new(Progress { state: Mutex::new(State::default()), cancel: AtomicBool::new(false), repaint, console })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
        (self.repaint)();
    }

    fn line(&self, kind: LineKind, text: String) {
        if self.console {
            match kind {
                LineKind::Warn | LineKind::Error => eprintln!("\n[!] {text}"),
                _ => println!("\n{text}"),
            }
        }
        {
            let mut s = self.lock();
            match kind {
                LineKind::Error => s.errors += 1,
                LineKind::Warn => s.warnings += 1,
                _ => {}
            }
            s.lines.push(Line { kind, text });
        }
        (self.repaint)();
    }

    pub fn info(&self, text: String) {
        self.line(LineKind::Info, text)
    }
    pub fn warn(&self, text: String) {
        self.line(LineKind::Warn, text)
    }
    pub fn error(&self, text: String) {
        self.line(LineKind::Error, text)
    }
    pub fn result(&self, text: String) {
        self.line(LineKind::Result, text)
    }

    fn begin_file(&self, index: usize, count: usize, name: &str) {
        {
            let mut s = self.lock();
            s.file_index = index;
            s.file_count = count;
            s.file_name = name.to_string();
            s.fraction = 0.0;
            s.speed = None;
            s.step.clear();
            s.file_started = Some(Instant::now());
        }
        (self.repaint)();
    }

    fn set_step(&self, step: &str) {
        self.lock().step = step.to_string();
        (self.repaint)();
    }

    fn set_fraction(&self, f: f64, speed: Option<f64>) {
        {
            let mut s = self.lock();
            s.fraction = f.clamp(0.0, 1.0);
            if speed.is_some() {
                s.speed = speed;
            }
        }
        if self.console {
            use std::io::Write;
            print!("\r   {:5.1}%  ", f.clamp(0.0, 1.0) * 100.0);
            let _ = std::io::stdout().flush();
        }
        (self.repaint)();
    }

    fn finish(&self) {
        self.lock().finished = true;
        (self.repaint)();
    }
}

pub struct Job {
    pub cfg: Arc<Config>,
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub prog: Arc<Progress>,
}

// ------------------------------------------------------------------ helpers

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<TempDir, String> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("mmt-{}-{}", std::process::id(), nanos));
        std::fs::create_dir_all(&path).map_err(|e| format!("temp dir: {e}"))?;
        Ok(TempDir(path))
    }
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The file without its extension, as text ("/a/b/clip.mp4" -> "/a/b/clip")
fn base_of(file: &Path) -> String {
    file.with_extension("").to_string_lossy().to_string()
}

fn ext_of(file: &Path) -> String {
    file.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default()
}

/// A path that does not exist yet: "name.ext", then "name (1).ext", "name (2).ext" ...
/// (nothing is ever overwritten)
pub fn unique_path(wanted: PathBuf) -> PathBuf {
    if !wanted.exists() {
        return wanted;
    }
    let stem = wanted.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = wanted.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let dir = wanted.parent().map(Path::to_path_buf).unwrap_or_default();
    for n in 1.. {
        let candidate = dir.join(format!("{stem} ({n}){ext}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

fn short_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

pub fn fmt_float(v: f64) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn s(x: &str) -> String {
    x.to_string()
}

fn file_arg(p: &Path) -> String {
    p.to_string_lossy().to_string()
}

/// "-hwaccel_output_format cuda" keeps the decoded frames on the GPU. A software filter
/// (scaling, trim) cannot work on them: decode on the GPU, but bring the frames to memory.
fn drop_hw_output_format(pre: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut skip = false;
    for a in pre {
        if skip {
            skip = false;
        } else if a == "-hwaccel_output_format" {
            skip = true;
        } else {
            out.push(a.clone());
        }
    }
    out
}

/// Find the value that follows `flag` in an argument list ("-tag:v" -> "hvc1")
fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

/// Run ffmpeg and follow its progress. `range` is the part (0..1) of the file's progress bar
/// this run covers; `duration` is the length of the output it will write.
fn run_ffmpeg(job: &Job, args: &[String], duration: Option<f64>, range: (f64, f64), cwd: Option<&Path>) -> Result<(), String> {
    run_prog(job, &job.ffmpeg, args, duration, range, cwd)
}

fn run_prog(job: &Job, program: &Path, args: &[String], duration: Option<f64>, range: (f64, f64), cwd: Option<&Path>) -> Result<(), String> {
    let mut cmd = Command::new(program);
    cmd.args(["-hide_banner", "-nostdin", "-loglevel", "error", "-nostats", "-progress", "pipe:1"]);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    hide_console(&mut cmd);
    let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", program.display()))?;

    // stderr is read on a thread so a full pipe can never stall ffmpeg
    let mut stderr = child.stderr.take().expect("piped");
    let err_thread = std::thread::spawn(move || {
        let mut text = String::new();
        let mut raw = Vec::new();
        let _ = stderr.read_to_end(&mut raw);
        text.push_str(&String::from_utf8_lossy(&raw));
        text
    });

    let stdout = child.stdout.take().expect("piped");
    let mut cancelled = false;
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if job.prog.is_cancelled() {
            cancelled = true;
            let _ = child.kill();
            break;
        }
        if let Some((key, value)) = line.split_once('=') {
            match key {
                "out_time_us" | "out_time_ms" => {
                    if let (Ok(us), Some(d)) = (value.trim().parse::<f64>(), duration) {
                        if d > 0.0 {
                            let f = (us / 1e6 / d).clamp(0.0, 1.0);
                            job.prog.set_fraction(range.0 + (range.1 - range.0) * f, None);
                        }
                    }
                }
                "speed" => {
                    let speed = value.trim().trim_end_matches('x').trim().parse::<f64>().ok();
                    if speed.is_some() {
                        let cur = job.prog.lock().fraction;
                        job.prog.set_fraction(cur, speed);
                    }
                }
                _ => {}
            }
        }
    }
    if cancelled || job.prog.is_cancelled() {
        let _ = child.kill();
        let _ = child.wait();
        return Err("cancelled".into());
    }
    let status = child.wait().map_err(|e| e.to_string())?;
    let err_text = err_thread.join().unwrap_or_default();
    if status.success() {
        job.prog.set_fraction(range.1, None);
        Ok(())
    } else {
        let tail: Vec<&str> = err_text.lines().rev().take(8).collect();
        let tail: Vec<&str> = tail.into_iter().rev().collect();
        Err(if tail.is_empty() { format!("ffmpeg exited with {status}") } else { tail.join("\n") })
    }
}

fn file_size(p: &Path) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(0)
}

fn fmt_mb(bytes: u64) -> String {
    format!("{:.2} MB", bytes as f64 / 1_048_576.0)
}

// ------------------------------------------------------------------ the bitrate math (pure)

#[derive(Debug, PartialEq)]
pub struct Bitrates {
    pub video: u64,
    pub audio: u64,
    pub tight: bool,
}

/// Bitrates (bit/s) from the real budget in bytes:
///  * `aim`% of the limit is the target (a small reserve against rate-control error). One-pass
///    rate control needs a moment to settle, so its relative error grows on short videos:
///    the reserve gets an extra 30/duration percent (capped at 8%). Exact modes (two-pass,
///    chunked) see the real result and do not need that extra reserve.
///  * the MP4 container overhead is estimated from the frame counts
///    (measured: ~4 B per video frame, ~17 B per AAC frame, ~1 KB fixed; rounded up)
///  * the audio bitrate scales down for very tight budgets and is 0 without audio
pub fn bitrates(mb: f64, dur: f64, fps: f64, has_audio: bool, aim: f64, exact: bool) -> Bitrates {
    let extra = if exact { 0.0 } else { (30.0 / dur).min(8.0) };
    let budget = mb * 1_048_576.0 * (aim - extra) / 100.0;
    let overhead = 4096.0 + dur * fps * 12.0 + if has_audio { dur * 47.0 * 20.0 } else { 0.0 };
    let total = (budget - overhead) * 8.0 / dur;
    let audio = if !has_audio {
        0.0
    } else if total > 400_000.0 {
        128_000.0
    } else if total > 150_000.0 {
        96_000.0
    } else {
        64_000.0
    };
    let mut video = total - audio;
    let mut tight = false;
    if video < 50_000.0 {
        video = 50_000.0;
        tight = true;
    }
    Bitrates { video: video as u64, audio: audio as u64, tight }
}

/// Chunk plan: equal chunks of about 6 s (3 to 16 of them) and a short last one.
/// Returns (number of chunks including the last, length of a normal chunk).
pub fn plan_chunks(d: f64) -> (usize, f64) {
    let last = (d / 12.0).clamp(1.0, 3.0);
    let m = (((d - last) / 6.0) + 0.5).floor().clamp(3.0, 16.0);
    (m as usize + 1, (d - last) / m)
}

/// Bitrate and requested size of one chunk. The fair share of what is left, divided by how much
/// more than requested the encoder has produced so far. Running over the limit is worse than
/// falling short, so an encoder that tends to produce less than requested is only compensated
/// a little (not at all in the last chunk, which can no longer be corrected), while one that
/// produces more is compensated fully.
pub fn chunk_target(remaining_bytes: f64, remaining_dur: f64, d: f64, sum_actual: f64, sum_req: f64, last: bool) -> (u64, f64) {
    let mut ratio = if sum_req > 0.0 { sum_actual / sum_req } else { 1.0 };
    let lowest = if last { 1.0 } else { 0.85 };
    ratio = ratio.clamp(lowest, 1.6).max(lowest);
    let mut bps = (remaining_bytes * d / remaining_dur) * 8.0 / d / ratio;
    if bps < 50_000.0 {
        bps = 50_000.0;
    }
    let bps = bps.round();
    (bps as u64, bps * d / 8.0)
}

// ------------------------------------------------------------------ commands

fn is_ffmpeg_name(word: &str) -> bool {
    let w = word.to_lowercase();
    w == "ffmpeg" || w == "ffmpeg.exe"
}

/// Expand the command of presets.toml and run it. `hw_drop`: a software filter is in use,
/// so frames may not stay on the GPU (see `drop_hw_output_format`).
fn run_template(
    job: &Job,
    template: &str,
    vars: &[(&str, Val)],
    hw_drop: bool,
    duration: Option<f64>,
    range: (f64, f64),
    cwd: Option<&Path>,
) -> Result<(), String> {
    let mut tokens = expand(template, vars)?;
    if tokens.is_empty() {
        return Err("the command is empty".into());
    }
    let program = tokens.remove(0);
    if hw_drop {
        tokens = drop_hw_output_format(&tokens);
    }
    let prog = if is_ffmpeg_name(&program) { job.ffmpeg.clone() } else { PathBuf::from(program) };
    run_prog(job, &prog, &tokens, duration, range, cwd)
}

/// The name of the result from the `output` pattern of an entry
fn output_for(pattern: &str, file: &Path, extra: &[(&str, String)]) -> PathBuf {
    let name = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let dir = file.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let mut text = pattern
        .replace("{base}", &base_of(file))
        .replace("{name}", &name)
        .replace("{dir}", &dir)
        .replace("{srcext}", &ext_of(file));
    for (k, v) in extra {
        text = text.replace(&format!("{{{k}}}"), v);
    }
    PathBuf::from(text)
}

/// Make the folder of the result if the pattern puts it into a new one
fn ensure_parent(p: &Path) {
    if let Some(dir) = p.parent() {
        if !dir.as_os_str().is_empty() && !dir.exists() {
            let _ = std::fs::create_dir_all(dir);
        }
    }
}

fn path_val(p: &Path) -> Val {
    Val::S(file_arg(p))
}

/// `{vf}` and `{scale}` of the commands, from the list of software filters
fn vf_val(chain: &[String]) -> Val {
    if chain.is_empty() {
        Val::L(vec![])
    } else {
        Val::l([s("-vf"), chain.join(",")])
    }
}

fn scale_val(chain: &[String]) -> Val {
    if chain.is_empty() {
        Val::s("")
    } else {
        Val::S(format!("{},", chain.join(",")))
    }
}

/// VAAPI needs the render device of the GPU of that vendor; the other backends need nothing
fn prepare_codec(t: &T, c: &Codec) -> Result<Option<String>, String> {
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

/// The software scaling filter for the chosen resolution (empty = untouched)
fn scale_chain(job: &Job, file: &Path, info: &Info, mode: ScaleMode, fps: f64, video_bps: f64, bpp_min: f64) -> (Vec<String>, String) {
    let t = i18n::t();
    if mode != ScaleMode::Orig && info.width > 0 && info.height > 0 {
        let ch = scale::choose(mode, info.width, info.height, fps, video_bps, bpp_min);
        if ch.factor < 1.0 {
            let note = if bpp_min > 0.0 {
                format!(" | {} x{:.4} ({} {}px, {:.3} bit/px)", t.scale_word, ch.factor, t.shorter_side, ch.short_px, ch.bpp)
            } else {
                job.prog.info(format!("{}: {} x{:.4} ({} {}px)", short_name(file), t.scale_word, ch.factor, t.shorter_side, ch.short_px));
                String::new()
            };
            return (vec![scale::filter(ch.factor)], note);
        }
    }
    (vec![], String::new())
}

// ------------------------------------------------------------------ Compress Video: presets

fn compress_preset(job: &Job, file: &Path, info: &Info, level: usize, codec: &Codec, mode: ScaleMode) -> Result<String, String> {
    let t = i18n::t();
    let node = prepare_codec(t, codec)?;
    let (chain, _) = scale_chain(job, file, info, mode, 30.0, 0.0, 0.0);
    let output = unique_path(output_for(&codec.output, file, &[]));
    ensure_parent(&output);
    let mut vars = vec![
        ("input", path_val(file)),
        ("output", path_val(&output)),
        ("vf", vf_val(&chain)),
        ("scale", scale_val(&chain)),
        ("q", Val::S(Codec::pick(&codec.q, level))),
        ("p", Val::S(Codec::pick(&codec.p, level))),
    ];
    push_node(&mut vars, &node);
    run_template(job, &codec.command, &vars, !chain.is_empty(), info.duration, (0.0, 1.0), None)?;
    Ok(format!("{} → {} ({})", short_name(file), short_name(&output), fmt_mb(file_size(&output))))
}

// ------------------------------------------------------------------ Compress Video: size limit

struct LimitPlan<'a> {
    codec: &'a Codec,
    mb: f64,
    chunked: bool,
    twopass: bool,
    node: Option<String>,
}

fn compress_limit(job: &Job, file: &Path, info: &Info, codec: &Codec, mb: f64, mode: ScaleMode, want_twopass: bool) -> Result<String, String> {
    let t = i18n::t();
    if !(mb > 0.0) {
        return Err(format!("{} {mb}", t.invalid_limit));
    }
    let node = prepare_codec(t, codec)?;

    // The budget math needs the exact duration, the frame rate (container overhead grows
    // with the number of frames) and whether there is an audio track at all.
    let dur = info.duration.filter(|d| *d > 0.0).unwrap_or(1.0);
    let fps = info.fps.unwrap_or(30.0);
    let has_audio = info.has_audio;

    let mut chunked = codec.chunked;
    match std::env::var("MMT_CHUNKED").as_deref() {
        Ok("1") => chunked = true,
        Ok("0") => chunked = false,
        _ => {}
    }
    let twopass = want_twopass && !codec.twopass.is_empty() && !chunked;
    // Two-pass and chunked encoding see the real result, so neither needs the extra reserve
    // for short videos; chunked mode replaces two-pass
    let aim_eff = if chunked { 99.0 } else { codec.aim };
    let plan = LimitPlan { codec, mb, chunked, twopass, node };

    let br = bitrates(mb, dur, fps, has_audio, aim_eff, twopass || chunked);
    if br.tight {
        job.prog.warn(format!("\"{}\": {}", short_name(file), t.tight));
    }

    // Resolution; "auto" walks down a ladder until the picture gets enough bits per pixel
    let (chain, scale_info) = scale_chain(job, file, info, mode, fps, br.video as f64, codec.bpp_min);

    let maxrate = (br.video as f64 * codec.rc_max / 100.0) as u64;
    let bufsize = (br.video as f64 * codec.rc_buf / 100.0) as u64;
    let audio: Vec<String> = if has_audio { vec![s("-c:a"), s("aac"), s("-b:a"), br.audio.to_string()] } else { vec![] };

    let mb_text = fmt_float(mb);
    let output = unique_path(output_for(&codec.output, file, &[("mb", mb_text.clone())]));
    ensure_parent(&output);

    let note = if chunked {
        " | chunked"
    } else if twopass {
        " | 2-pass"
    } else {
        ""
    };
    job.prog.info(format!(
        "{} | {:.1}s | video={}k audio={}k{}{} → {}",
        short_name(file),
        dur,
        br.video / 1000,
        br.audio / 1000,
        note,
        scale_info,
        short_name(&output)
    ));

    if plan.chunked {
        encode_chunked(job, file, &output, &plan, &chain, &audio, dur, fps, has_audio, aim_eff)?;
    } else {
        let rate = Val::l([s("-b:v"), br.video.to_string(), s("-maxrate"), maxrate.to_string(), s("-bufsize"), bufsize.to_string()]);
        let mk = |overwrite: &str, pass: Vec<String>, audio: Vec<String>, out: Val| -> Vec<(&'static str, Val)> {
            let mut v = vec![
                ("input", path_val(file)),
                ("output", out),
                ("vf", vf_val(&chain)),
                ("scale", scale_val(&chain)),
                ("overwrite", Val::s(overwrite)),
                ("seek", Val::L(vec![])),
                ("rate", rate.clone()),
                ("pass", Val::L(pass)),
                ("audio", Val::L(audio)),
            ];
            push_node(&mut v, &plan.node);
            v
        };
        let mut guard = None;
        let mut range = (0.0, 1.0);
        let mut pass2: Vec<String> = Vec::new();
        if plan.twopass {
            // Pass 1 only analyses the video and writes no output; pass 2 makes the single real
            // encode with the whole video known in advance, which makes the size predictable.
            let dir = TempDir::new()?;
            let (p1, p2): (Vec<String>, Vec<String>) = if codec.twopass == "x265" {
                (vec![s("-x265-params"), s("pass=1:stats=x265.log")], vec![s("-x265-params"), s("pass=2:stats=x265.log")])
            } else {
                (vec![s("-pass"), s("1"), s("-passlogfile"), s("pass")], vec![s("-pass"), s("2"), s("-passlogfile"), s("pass")])
            };
            let null = Val::l([s("-f"), s("null"), s(if cfg!(windows) { "NUL" } else { "/dev/null" })]);
            job.prog.set_step(t.pass1);
            run_template(job, &codec.command, &mk("-y", p1, vec![s("-an")], null), !chain.is_empty(), Some(dur), (0.0, 0.5), Some(dir.path()))?;
            job.prog.set_step(t.pass2);
            range = (0.5, 1.0);
            pass2 = p2;
            guard = Some(dir);
        }
        run_template(
            job,
            &codec.command,
            &mk("-n", pass2, audio.clone(), path_val(&output)),
            !chain.is_empty(),
            Some(dur),
            range,
            guard.as_ref().map(|d| d.path()),
        )?;
    }

    // Exact size in bytes
    let bytes = file_size(&output);
    let limit_bytes = mb * 1_048_576.0;
    let mut text = format!(
        "{} → {} — {} ({:.2} MB / decimal) | {} {} MB = {:.1}%",
        short_name(file),
        short_name(&output),
        fmt_mb(bytes),
        bytes as f64 / 1e6,
        t.target,
        mb_text,
        bytes as f64 / limit_bytes * 100.0
    );
    if bytes as f64 > limit_bytes {
        let over = format!("{} {:.2} MB", t.over_limit, (bytes as f64 - limit_bytes) / 1_048_576.0);
        job.prog.warn(over.clone());
        text.push_str(&format!("\n⚠ {over}"));
    }
    Ok(text)
}

/// Closed-loop encode for hardware encoders (NVENC / VAAPI / AMF / QSV / V4L2). They only
/// approximate the requested bitrate, by an amount that depends on the GPU, the driver and the
/// footage, so a bitrate computed in advance can miss the size limit by several percent. Here
/// the video is cut into chunks and every chunk is encoded exactly once with the command of
/// the entry; after each one the real size is measured and the bitrate of the remaining chunks
/// is corrected by what the encoder actually did. The chunks are then joined without
/// re-encoding. The last chunk is short (its error is the one that stays in the result).
#[allow(clippy::too_many_arguments)]
fn encode_chunked(
    job: &Job,
    file: &Path,
    output: &Path,
    plan: &LimitPlan,
    chain: &[String],
    audio_args: &[String],
    dur: f64,
    fps: f64,
    has_audio: bool,
    aim: f64,
) -> Result<(), String> {
    let t = i18n::t();
    let work = TempDir::new()?;
    // Narrow rate window per chunk: a chunk may not run far over what was requested for it
    // (a smaller chunk is harmless, the unused bytes go to the next ones)
    let (rc_max, rc_buf) = (110.0, 100.0);

    // Audio is encoded once; afterwards its size is known exactly
    let audio_path = work.path().join("audio.m4a");
    let mut audio_bytes = 0.0;
    if has_audio {
        job.prog.set_step(t.audio_track);
        let mut args = vec![s("-y"), s("-i"), file_arg(file), s("-vn")];
        args.extend(audio_args.iter().cloned());
        args.push(file_arg(&audio_path));
        run_ffmpeg(job, &args, Some(dur), (0.0, 0.0), None)?;
        audio_bytes = file_size(&audio_path) as f64;
    }

    // What is left for the video: the limit (times the aim), minus the container overhead
    // (estimated from the frame counts) and the finished audio track
    let mut remaining_bytes = plan.mb * 1_048_576.0 * aim / 100.0
        - (4096.0 + dur * fps * 12.0 + if has_audio { dur * 47.0 * 20.0 } else { 0.0 })
        - audio_bytes;
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
        let (dur_i, mut filters, seek): (f64, Vec<String>, Vec<String>) = if !last {
            (len, vec![format!("trim=end={len:.6}"), s("setpts=PTS-STARTPTS")], vec![s("-ss"), format!("{start:.6}"), s("-t"), format!("{:.6}", len + 1.0)])
        } else {
            (remaining_dur, vec![s("setpts=PTS-STARTPTS")], vec![s("-ss"), format!("{start:.6}")])
        };
        filters.extend(chain.iter().cloned());
        let (bps, req_bytes) = chunk_target(remaining_bytes, remaining_dur, dur_i, sum_actual, sum_req, last);
        let chunk_path = work.path().join(format!("chunk_{i}.mkv"));

        job.prog.set_step(&format!("{} {}/{}", t.chunk, i + 1, n));
        let mut vars: Vec<(&str, Val)> = vec![
            ("input", path_val(file)),
            ("output", path_val(&chunk_path)),
            ("vf", vf_val(&filters)),
            ("scale", scale_val(&filters)),
            ("overwrite", Val::s("-y")),
            ("seek", Val::L(seek)),
            (
                "rate",
                Val::l([
                    s("-b:v"),
                    bps.to_string(),
                    s("-maxrate"),
                    ((bps as f64 * rc_max / 100.0) as u64).to_string(),
                    s("-bufsize"),
                    ((bps as f64 * rc_buf / 100.0) as u64).to_string(),
                ]),
            ),
            ("pass", Val::L(vec![])),
            ("audio", Val::l(["-an"])),
        ];
        push_node(&mut vars, &plan.node);
        // frames stay in normal memory here (trim / setpts are software filters)
        run_template(job, &plan.codec.command, &vars, true, Some(dur_i), (i as f64 / n as f64, (i + 1) as f64 / n as f64), None)?;

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
    let mut args = vec![s("-y"), s("-f"), s("concat"), s("-safe"), s("0"), s("-i"), file_arg(&list_path)];
    if has_audio {
        args.extend([s("-i"), file_arg(&audio_path)]);
    }
    args.extend([s("-map"), s("0:v")]);
    if has_audio {
        args.extend([s("-map"), s("1:a")]);
    }
    args.extend([s("-c"), s("copy")]);
    let tokens = crate::cmdline::split(&plan.codec.command).unwrap_or_default();
    if let Some(tag) = value_after(&tokens, "-tag:v") {
        args.extend([s("-tag:v"), tag.to_string()]);
    }
    args.push(file_arg(output));
    run_ffmpeg(job, &args, None, (1.0, 1.0), None)
}

// ------------------------------------------------------------------ the other actions

fn find_format<'a>(list: &'a [(String, Format)], key: &str) -> Option<&'a (String, Format)> {
    list.iter().find(|(k, _)| k == key)
}

fn convert(job: &Job, file: &Path, info: &Info, what: ConvertKind, key: &str) -> Result<String, String> {
    let cfg = &job.cfg;
    let list = match what {
        ConvertKind::Video => &cfg.video_convert,
        ConvertKind::VideoAudio => &cfg.video_audio,
        ConvertKind::Extract => &cfg.extract,
        ConvertKind::Frames => &cfg.frames,
        ConvertKind::AudioFile => &cfg.audio_file,
        ConvertKind::Image => &cfg.image,
    };
    let (_, fmt) = find_format(list, key).ok_or_else(|| format!("{} {key}", i18n::t().unknown_codec))?;
    let ext = if fmt.ext.is_empty() { key.to_string() } else { fmt.ext.clone() };
    let extra = [("ext", ext), ("fmt", key.to_string())];

    let output = if what == ConvertKind::Frames {
        // the frames go into a new folder: an existing one is never reused
        let dir = unique_path(PathBuf::from(base_of(file)));
        let pattern = fmt.output.replacen("{base}", &dir.to_string_lossy(), 1);
        let out = output_for(&pattern, file, &extra);
        ensure_parent(&out);
        out
    } else {
        let out = unique_path(output_for(&fmt.output, file, &extra));
        ensure_parent(&out);
        out
    };
    let vars = vec![("input", path_val(file)), ("output", path_val(&output)), ("ext", Val::S(extra[0].1.clone())), ("fmt", Val::s(key))];
    let duration = if matches!(what, ConvertKind::Image) { None } else { info.duration };
    run_template(job, &fmt.command, &vars, false, duration, (0.0, 1.0), None)?;
    let shown = if what == ConvertKind::Frames { output.parent().map(short_name).unwrap_or_default() + "/" } else { short_name(&output) };
    Ok(format!("{} → {}", short_name(file), shown))
}

fn delete_sound(job: &Job, file: &Path, info: &Info) -> Result<String, String> {
    let d = &job.cfg.delete_sound;
    let output = unique_path(output_for(&d.output, file, &[]));
    ensure_parent(&output);
    let vars = vec![("input", path_val(file)), ("output", path_val(&output))];
    run_template(job, &d.command, &vars, false, info.duration, (0.0, 1.0), None)?;
    Ok(format!("{} → {}", short_name(file), short_name(&output)))
}

fn webp_anim(job: &Job, file: &Path, info: &Info, size: u32, fps: u32, quality: u32, compression: u32) -> Result<String, String> {
    let w = &job.cfg.webp;
    let output = unique_path(output_for(&w.output, file, &[]));
    ensure_parent(&output);
    let vars = vec![
        ("input", path_val(file)),
        ("output", path_val(&output)),
        ("size", Val::S(size.to_string())),
        ("fps", Val::S(fps.to_string())),
        ("quality", Val::S(quality.to_string())),
        ("compression", Val::S(compression.to_string())),
    ];
    run_template(job, &w.command, &vars, false, info.duration, (0.0, 1.0), None)?;
    Ok(format!("{} → {} ({})", short_name(file), short_name(&output), fmt_mb(file_size(&output))))
}

fn custom_run(job: &Job, file: &Path, info: &Info, id: &str) -> Result<String, String> {
    let c = job.cfg.settings.custom.iter().find(|c| c.id == id).ok_or_else(|| format!("{} {id}", i18n::t().unknown_codec))?;
    let output = unique_path(output_for(&c.output, file, &[]));
    ensure_parent(&output);
    let vars = vec![("input", path_val(file)), ("output", path_val(&output))];
    run_template(job, &c.command, &vars, false, info.duration, (0.0, 1.0), None)?;
    Ok(format!("{} → {}", short_name(file), short_name(&output)))
}

// ------------------------------------------------------------------ the job

/// Process every file. The worker thread calls this; it never panics on bad input.
pub fn run(job: &Job, req: &Request, files: &[PathBuf]) {
    let t = i18n::t();
    let p = &job.prog;
    let wanted = req.required_kind(&job.cfg);

    let mut eligible: Vec<&PathBuf> = Vec::new();
    for f in files {
        if f.is_file() && media::kind_of(f) == Some(wanted) {
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
            Err(e) if e == "cancelled" => {
                p.lock().cancelled = true;
                p.info(t.cancelled.to_string());
                break;
            }
            Err(e) => p.error(format!("{} {}\n{}", t.failed, short_name(file), e)),
        }
    }
    if p.is_cancelled() {
        p.lock().cancelled = true;
    }
    p.finish();
}

fn process_one(job: &Job, req: &Request, file: &Path) -> Result<String, String> {
    let t = i18n::t();
    let info = media::probe(&job.ffprobe, file).unwrap_or_else(|| Info { name: short_name(file), ..Default::default() });
    match req {
        Request::CompressPreset { level, codec, scale } => {
            let c = job.cfg.compress_codec(codec).ok_or_else(|| format!("{} {codec}", t.unknown_codec))?;
            // "auto" only exists in the size limit mode
            let mode = if *scale == ScaleMode::Auto { ScaleMode::Orig } else { *scale };
            compress_preset(job, file, &info, *level, c, mode)
        }
        Request::CompressLimit { mb, codec, scale, twopass } => {
            let c = job.cfg.limit_codec(codec).ok_or_else(|| format!("{} {codec}", t.unknown_codec))?;
            compress_limit(job, file, &info, c, *mb, *scale, *twopass)
        }
        Request::Convert { what, format } => convert(job, file, &info, *what, format),
        Request::DeleteSound => delete_sound(job, file, &info),
        Request::Webp { size, fps, quality, compression } => webp_anim(job, file, &info, *size, *fps, *quality, *compression),
        Request::Custom { id } => custom_run(job, file, &info, id),
    }
}

/// `ffprobe -hide_banner -i file` as text (the Stats window)
pub fn stats_text(ffprobe: &Path, file: &Path) -> String {
    let mut cmd = Command::new(ffprobe);
    cmd.args(["-hide_banner", "-i"]).arg(file);
    hide_console(&mut cmd);
    match cmd.output() {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stderr).to_string();
            if text.trim().is_empty() {
                text = String::from_utf8_lossy(&out.stdout).to_string();
            }
            text.trim_end().to_string()
        }
        Err(e) => format!("{}: {e}", ffprobe.display()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrates_match_the_old_awk_formula() {
        // 20 MB, 60 s, 30 fps, audio, aim 99, one pass: extra reserve = 30/60 = 0.5%
        let b = bitrates(20.0, 60.0, 30.0, true, 99.0, false);
        // budget = 20*1048576*98.5/100 = 20656...; overhead = 4096 + 60*30*12 + 60*47*20 = 82096
        let budget = 20.0 * 1_048_576.0 * 98.5 / 100.0;
        let total = (budget - 82_096.0) * 8.0 / 60.0;
        assert_eq!(b.audio, 128_000);
        assert_eq!(b.video, (total - 128_000.0) as u64);
        assert!(!b.tight);
    }

    #[test]
    fn no_audio_no_audio_bitrate() {
        let b = bitrates(5.0, 30.0, 25.0, false, 99.0, true);
        assert_eq!(b.audio, 0);
        assert!(b.video > 1_000_000);
    }

    #[test]
    fn tight_budget_is_flagged() {
        let b = bitrates(0.1, 600.0, 30.0, true, 99.0, false);
        assert!(b.tight);
        assert_eq!(b.video, 50_000);
        assert_eq!(b.audio, 64_000);
    }

    #[test]
    fn chunk_plan() {
        // 60 s: last = 3 s, m = round(57/6) = 10 -> 11 chunks of 5.7 s + the last
        let (n, len) = plan_chunks(60.0);
        assert_eq!(n, 11);
        assert!((len - 5.7).abs() < 1e-9);
        // very short video: at least 3 + 1 chunks
        assert_eq!(plan_chunks(6.0).0, 4);
        // very long: at most 16 + 1
        assert_eq!(plan_chunks(10_000.0).0, 17);
    }

    #[test]
    fn chunk_target_compensates_an_encoder_that_overshoots() {
        // first chunk: no history
        let (bps0, req0) = chunk_target(10_000_000.0, 60.0, 6.0, 0.0, 0.0, false);
        assert_eq!(bps0, (10_000_000.0f64 / 60.0 * 8.0).round() as u64);
        assert!((req0 - bps0 as f64 * 6.0 / 8.0).abs() < 1e-6);
        // the encoder produced 20% too much so far: ask for less
        let (bps1, _) = chunk_target(9_000_000.0, 54.0, 6.0, 1_200_000.0, 1_000_000.0, false);
        let (plain, _) = chunk_target(9_000_000.0, 54.0, 6.0, 1_000_000.0, 1_000_000.0, false);
        assert!(bps1 < plain);
        // an encoder that undershoots is compensated only a little, never in the last chunk
        let (under, _) = chunk_target(9_000_000.0, 54.0, 6.0, 500_000.0, 1_000_000.0, false);
        let (under_last, _) = chunk_target(9_000_000.0, 54.0, 6.0, 500_000.0, 1_000_000.0, true);
        assert!(under > plain && under_last == plain);
    }

    #[test]
    fn hw_output_format_is_dropped() {
        let pre: Vec<String> = ["-hwaccel", "cuda", "-hwaccel_output_format", "cuda"].iter().map(|x| x.to_string()).collect();
        assert_eq!(drop_hw_output_format(&pre), ["-hwaccel", "cuda"]);
    }

    #[test]
    fn unique_names_never_overwrite() {
        let dir = std::env::temp_dir().join(format!("mmt-unique-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("clip.mp4");
        assert_eq!(unique_path(a.clone()), a);
        std::fs::write(&a, b"x").unwrap();
        assert_eq!(unique_path(a.clone()), dir.join("clip (1).mp4"));
        std::fs::write(dir.join("clip (1).mp4"), b"x").unwrap();
        assert_eq!(unique_path(a), dir.join("clip (2).mp4"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn float_text() {
        assert_eq!(fmt_float(20.0), "20");
        assert_eq!(fmt_float(20.5), "20.5");
        assert_eq!(fmt_float(0.1), "0.1");
    }
}
