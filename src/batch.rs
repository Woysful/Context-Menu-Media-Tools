//! Windows starts the program once per selected file ("%1" in the registry command), but
//! the dialog must be shown once for the whole selection. `--batch` makes the launches
//! gather: every process appends its file to a queue; the first one becomes the leader,
//! waits until nobody adds anything for a moment, and then processes all of them. The
//! others exit at once.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const QUIET: Duration = Duration::from_millis(700);
const STALE_LOCK: Duration = Duration::from_secs(60);

fn spool_dir(action: &str) -> PathBuf {
    std::env::temp_dir().join(format!("mmt-batch-{action}"))
}

/// Returns Some(all files) in the leader process, None in the followers.
pub fn gather(action: &str, files: &[PathBuf]) -> Option<Vec<PathBuf>> {
    gather_in(&spool_dir(action), files, QUIET)
}

pub fn gather_in(dir: &Path, files: &[PathBuf], quiet: Duration) -> Option<Vec<PathBuf>> {
    let _ = fs::create_dir_all(dir);
    let queue = dir.join("queue.txt");
    let lock = dir.join("leader.lock");

    // 1. put the own files into the queue (one write = one append, so lines never mix)
    let mut text = String::new();
    for f in files {
        text.push_str(&f.to_string_lossy());
        text.push('\n');
    }
    if let Ok(mut q) = OpenOptions::new().create(true).append(true).open(&queue) {
        let _ = q.write_all(text.as_bytes());
    }

    // 2. try to become the leader
    let mut got = OpenOptions::new().write(true).create_new(true).open(&lock).is_ok();
    if !got {
        let stale = fs::metadata(&lock)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .map_or(false, |age| age > STALE_LOCK);
        if stale {
            let _ = fs::remove_file(&lock);
            got = OpenOptions::new().write(true).create_new(true).open(&lock).is_ok();
        }
    }
    if !got {
        return None;
    }

    // 3. wait until the queue stops growing
    let mut last_len = 0u64;
    let mut last_change = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(100));
        let len = fs::metadata(&queue).map(|m| m.len()).unwrap_or(0);
        if len != last_len {
            last_len = len;
            last_change = Instant::now();
        } else if last_change.elapsed() >= quiet {
            break;
        }
    }

    // 4. release the lock first, then take the queue (a late launch becomes a new leader)
    let _ = fs::remove_file(&lock);
    let taken = dir.join(format!("taken-{}.txt", std::process::id()));
    let content = if fs::rename(&queue, &taken).is_ok() {
        let c = fs::read_to_string(&taken).unwrap_or_default();
        let _ = fs::remove_file(&taken);
        c
    } else {
        String::new()
    };

    let mut out: Vec<PathBuf> = Vec::new();
    for line in content.lines() {
        let p = PathBuf::from(line);
        if !line.is_empty() && !out.contains(&p) {
            out.push(p);
        }
    }
    out.sort();
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_one_process_wins_and_it_gets_every_file() {
        let dir = std::env::temp_dir().join(format!("mmt-batch-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut handles = Vec::new();
        for i in 0..6 {
            let d = dir.clone();
            handles.push(std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(i * 30));
                gather_in(&d, &[PathBuf::from(format!("/x/file{i}.mp4"))], Duration::from_millis(400))
            }));
        }
        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        let leaders: Vec<_> = results.iter().flatten().collect();
        assert_eq!(leaders.len(), 1, "{results:?}");
        assert_eq!(leaders[0].len(), 6);
        let _ = fs::remove_dir_all(&dir);
    }
}
