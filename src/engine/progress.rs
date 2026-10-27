//! What a running job reports: the file, the step, the fraction done and a log.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

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

#[derive(Clone, Default)]
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

impl State {
    /// 0..1 over all files
    pub fn overall(&self) -> f64 {
        ((self.file_index as f64 + self.fraction) / self.file_count.max(1) as f64).clamp(0.0, 1.0)
    }

    /// Seconds left for the current file, once there is enough progress to guess
    pub fn remaining(&self) -> Option<f64> {
        let started = self.file_started?;
        (self.fraction > 0.03).then(|| started.elapsed().as_secs_f64() * (1.0 - self.fraction) / self.fraction)
    }

    pub fn clean(&self) -> bool {
        self.errors == 0 && self.warnings == 0 && !self.cancelled
    }
}

pub struct Progress {
    state: Mutex<State>,
    cancel: AtomicBool,
    repaint: Box<dyn Fn() + Send + Sync>,
    console: bool,
}

impl Progress {
    /// `repaint` is called after every change; `console` also prints the progress
    pub fn new(repaint: Box<dyn Fn() + Send + Sync>, console: bool) -> Arc<Progress> {
        Arc::new(Progress { state: Mutex::new(State::default()), cancel: AtomicBool::new(false), repaint, console })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn changed(&self, f: impl FnOnce(&mut State)) {
        f(&mut self.lock());
        (self.repaint)();
    }

    pub fn snapshot(&self) -> State {
        self.lock().clone()
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
        self.changed(|s| {
            match kind {
                LineKind::Error => s.errors += 1,
                LineKind::Warn => s.warnings += 1,
                _ => {}
            }
            s.lines.push(Line { kind, text });
        });
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

    pub(super) fn begin_file(&self, index: usize, count: usize, name: &str) {
        self.changed(|s| {
            s.file_index = index;
            s.file_count = count;
            s.file_name = name.to_string();
            s.fraction = 0.0;
            s.speed = None;
            s.step.clear();
            s.file_started = Some(Instant::now());
        });
    }

    pub(super) fn set_step(&self, step: &str) {
        self.changed(|s| s.step = step.to_string());
    }

    pub(super) fn set_fraction(&self, f: f64) {
        let f = f.clamp(0.0, 1.0);
        self.changed(|s| s.fraction = f);
        if self.console {
            use std::io::Write;
            print!("\r   {:5.1}%  ", f * 100.0);
            let _ = std::io::stdout().flush();
        }
    }

    pub(super) fn set_speed(&self, speed: f64) {
        self.changed(|s| s.speed = Some(speed));
    }

    pub(super) fn mark_cancelled(&self) {
        self.changed(|s| s.cancelled = true);
    }

    pub(super) fn finish(&self) {
        self.changed(|s| s.finished = true);
    }
}
