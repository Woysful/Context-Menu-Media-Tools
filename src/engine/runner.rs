//! Running ffmpeg (and the programs of own commands) and following its progress.

use super::{Failure, Job, Outcome};
use crate::cmdline::{expand, Val};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

/// A folder for the work files of one encode, removed when it is dropped
pub(super) struct TempDir(PathBuf);

impl TempDir {
    pub fn new() -> Result<TempDir, String> {
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.subsec_nanos()).unwrap_or(0);
        let path = std::env::temp_dir().join(format!("mmt-{}-{}", std::process::id(), nanos));
        std::fs::create_dir_all(&path).map_err(|e| format!("temp dir: {e}"))?;
        Ok(TempDir(path))
    }

    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Which part of the file's progress bar (`from`..`to`, 0..1) one run covers, and the
/// length of the output it writes (None = no progress can be shown)
#[derive(Clone, Copy, Debug)]
pub(super) struct Track {
    pub duration: Option<f64>,
    pub from: f64,
    pub to: f64,
}

impl Track {
    pub fn whole(duration: Option<f64>) -> Track {
        Track { duration, from: 0.0, to: 1.0 }
    }

    pub fn part(duration: f64, from: f64, to: f64) -> Track {
        Track { duration: Some(duration), from, to }
    }

    /// A step that is not shown on the progress bar
    pub fn at(fraction: f64) -> Track {
        Track { duration: None, from: fraction, to: fraction }
    }
}

/// "-hwaccel_output_format cuda" keeps the decoded frames on the GPU. A software filter
/// (scaling, trim) cannot work on them: decode on the GPU, but bring the frames to memory.
pub(super) fn drop_hw_output_format(args: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = args.iter();
    while let Some(a) = it.next() {
        if a == "-hwaccel_output_format" {
            it.next();
        } else {
            out.push(a.clone());
        }
    }
    out
}

/// The value that follows `flag` in an argument list ("-tag:v" -> "hvc1")
pub(super) fn value_after<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.iter().position(|a| a == flag).and_then(|i| args.get(i + 1)).map(String::as_str)
}

fn tool_name(word: &str) -> String {
    word.to_lowercase().trim_end_matches(".exe").to_string()
}

/// Run ffmpeg with ready arguments
pub(super) fn run_ffmpeg(job: &Job, args: &[String], track: Track, cwd: Option<&Path>) -> Outcome<()> {
    run(job, &job.ffmpeg, args, true, track, cwd)
}

/// Expand a command of presets.toml (or of an own item) and run it. `hw_drop`: a software
/// filter is in use, so frames may not stay on the GPU (see `drop_hw_output_format`).
pub(super) fn run_template(job: &Job, template: &str, vars: &[(&str, Val)], hw_drop: bool, track: Track, cwd: Option<&Path>) -> Outcome<()> {
    let mut tokens = expand(template, vars)?;
    if tokens.is_empty() {
        return Err(Failure::Error("the command is empty".into()));
    }
    let program = tokens.remove(0);
    if hw_drop {
        tokens = drop_hw_output_format(&tokens);
    }
    match tool_name(&program).as_str() {
        "ffmpeg" => run(job, &job.ffmpeg, &tokens, true, track, cwd),
        "ffprobe" => run(job, &job.ffprobe, &tokens, false, track, cwd),
        _ => run(job, Path::new(&program), &tokens, false, track, cwd),
    }
}

/// One line of `-progress pipe:1`
fn follow(prog: &super::Progress, line: &str, track: Track) {
    let Some((key, value)) = line.split_once('=') else { return };
    match key {
        // out_time_ms is in microseconds as well (an old misnomer of ffmpeg)
        "out_time_us" | "out_time_ms" => {
            if let (Ok(us), Some(d)) = (value.trim().parse::<f64>(), track.duration.filter(|d| *d > 0.0)) {
                let f = (us / 1e6 / d).clamp(0.0, 1.0);
                prog.set_fraction(track.from + (track.to - track.from) * f);
            }
        }
        "speed" => {
            if let Ok(speed) = value.trim().trim_end_matches('x').trim().parse::<f64>() {
                prog.set_speed(speed);
            }
        }
        _ => {}
    }
}

/// Run a program and wait for it, killing it as soon as the job is cancelled.
/// `ffmpeg`: the program is ffmpeg, so it is asked for machine-readable progress.
fn run(job: &Job, program: &Path, args: &[String], ffmpeg: bool, track: Track, cwd: Option<&Path>) -> Outcome<()> {
    let mut cmd = crate::proc::command(program);
    if ffmpeg {
        cmd.args(["-hide_banner", "-nostdin", "-loglevel", "error", "-nostats", "-progress", "pipe:1"]);
    }
    cmd.args(args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let mut child = cmd.spawn().map_err(|e| format!("{}: {e}", program.display()))?;

    // both pipes are read on threads, so a full pipe can never stall the program
    let mut stderr = child.stderr.take().expect("piped");
    let err_thread = std::thread::spawn(move || {
        let mut raw = Vec::new();
        let _ = stderr.read_to_end(&mut raw);
        String::from_utf8_lossy(&raw).into_owned()
    });
    let stdout = child.stdout.take().expect("piped");
    let prog = Arc::clone(&job.prog);
    let out_thread = std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if ffmpeg {
                follow(&prog, &line, track);
            }
        }
    });

    let status = loop {
        if job.prog.is_cancelled() {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Failure::Cancelled);
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(Failure::Error(e.to_string())),
        }
    };
    let _ = out_thread.join();
    let err_text = err_thread.join().unwrap_or_default();
    if status.success() {
        job.prog.set_fraction(track.to);
        return Ok(());
    }
    let lines: Vec<&str> = err_text.lines().filter(|l| !l.trim().is_empty()).collect();
    let tail = lines[lines.len().saturating_sub(8)..].join("\n");
    Err(Failure::Error(if tail.is_empty() { format!("{} exited with {status}", program.display()) } else { tail }))
}

/// `ffprobe -hide_banner -i file` as text (the Stats window)
pub fn stats_text(ffprobe: &Path, file: &Path) -> String {
    match crate::proc::command(ffprobe).args(["-hide_banner", "-i"]).arg(file).output() {
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
    use crate::config::Config;
    use crate::engine::Progress;

    fn strings(list: &[&str]) -> Vec<String> {
        list.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn hw_output_format_is_dropped() {
        assert_eq!(drop_hw_output_format(&strings(&["-hwaccel", "cuda", "-hwaccel_output_format", "cuda", "-i", "x"])), ["-hwaccel", "cuda", "-i", "x"]);
    }

    #[test]
    fn tool_names() {
        assert_eq!(tool_name("FFmpeg.exe"), "ffmpeg");
        assert_eq!(tool_name("ffprobe"), "ffprobe");
        assert_eq!(value_after(&strings(&["-c:v", "x", "-tag:v", "hvc1"]), "-tag:v"), Some("hvc1"));
    }

    fn job() -> Job {
        Job {
            cfg: Arc::new(Config::defaults()),
            ffmpeg: PathBuf::from("ffmpeg"),
            ffprobe: PathBuf::from("ffprobe"),
            prog: Progress::new(Box::new(|| {}), false),
        }
    }

    #[cfg(unix)]
    #[test]
    fn other_programs_get_no_ffmpeg_options() {
        let j = job();
        let dir = TempDir::new().unwrap();
        let out = dir.path().join("args.txt");
        let vars = [("input", Val::s("in put")), ("output", Val::S(out.to_string_lossy().into()))];
        run_template(&j, "sh -c 'printf %s \"$1\" > \"$2\"' sh {input} {output}", &vars, false, Track::whole(None), None).unwrap();
        assert_eq!(std::fs::read_to_string(&out).unwrap(), "in put");
        let err = run_template(&j, "sh -c 'echo broken >&2; exit 3'", &[], false, Track::whole(None), None).unwrap_err();
        assert!(matches!(err, Failure::Error(e) if e == "broken"));
    }

    #[cfg(unix)]
    #[test]
    fn cancel_stops_a_quiet_program_at_once() {
        let j = job();
        let prog = Arc::clone(&j.prog);
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            prog.request_cancel();
        });
        let started = std::time::Instant::now();
        let r = run_template(&j, "sleep 20", &[], false, Track::whole(None), None);
        assert!(matches!(r, Err(Failure::Cancelled)));
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
