//! Is there a newer version? GitHub is asked for the latest release (or, without releases, the
//! highest version tag) at most once a day. The request is made by `curl`, which Linux and
//! Windows 10+ have, so the program needs no TLS library of its own.

use crate::paths;
use crate::proc;
use serde::{Deserialize, Serialize};
use std::sync::mpsc::{self, Receiver};
use std::time::{SystemTime, UNIX_EPOCH};

const REPO: &str = "Woysful/Context-Menu-Media-Tools";
const INTERVAL: u64 = 24 * 3600;

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: String,
    pub url: String,
}

/// The answer of the last check, so the window knows it at once and GitHub is not asked every time
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Cache {
    checked: u64,
    version: String,
    url: String,
}

#[derive(Default)]
pub struct Checker {
    started: bool,
    rx: Option<Receiver<Option<Release>>>,
    found: Option<Release>,
}

impl Checker {
    /// Call every frame: starts the check once, `repaint` wakes the window when the answer comes.
    /// Returns the newer release, if there is one.
    pub fn poll(&mut self, enabled: bool, repaint: impl FnOnce() + Send + 'static) -> Option<&Release> {
        if !enabled {
            return None;
        }
        if !self.started {
            self.started = true;
            let cache = read_cache();
            self.found = newer(Release { version: cache.version, url: cache.url });
            let now = now();
            if now.saturating_sub(cache.checked) >= INTERVAL || cache.checked > now {
                let (tx, rx) = mpsc::channel();
                self.rx = Some(rx);
                std::thread::spawn(move || {
                    let latest = fetch();
                    if let Some(r) = &latest {
                        write_cache(&Cache { checked: now, version: r.version.clone(), url: r.url.clone() });
                    }
                    let _ = tx.send(latest);
                    repaint();
                });
            }
        }
        if let Some(Ok(latest)) = self.rx.as_ref().map(|rx| rx.try_recv()) {
            self.rx = None;
            if let Some(r) = latest {
                self.found = newer(r);
            }
        }
        self.found.as_ref()
    }
}

fn newer(r: Release) -> Option<Release> {
    let current = parse_version(env!("CARGO_PKG_VERSION"))?;
    (parse_version(&r.version)? > current).then_some(r)
}

fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn cache_file() -> std::path::PathBuf {
    paths::config_dir().join("update-check.toml")
}

fn read_cache() -> Cache {
    std::fs::read_to_string(cache_file()).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
}

fn write_cache(c: &Cache) {
    if let Ok(text) = toml::to_string(c) {
        let _ = std::fs::create_dir_all(paths::config_dir());
        let _ = std::fs::write(cache_file(), text);
    }
}

/// The latest release; a failed request is None. "No release yet" is an empty version, so it is cached, too.
fn fetch() -> Option<Release> {
    let api = format!("https://api.github.com/repos/{REPO}");
    if let Some(body) = get(&format!("{api}/releases/latest")) {
        if let Some(r) = parse_release(&body) {
            return Some(r);
        }
    }
    let body = get(&format!("{api}/tags?per_page=100"))?;
    Some(parse_tags(&body).unwrap_or(Release { version: String::new(), url: String::new() }))
}

fn get(url: &str) -> Option<String> {
    let agent = format!("cmmt/{}", env!("CARGO_PKG_VERSION"));
    proc::output_of("curl", &["-fsSL", "-m", "10", "-A", &agent, "-H", "Accept: application/vnd.github+json", url])
}

fn parse_release(body: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    if v["draft"].as_bool() == Some(true) || v["prerelease"].as_bool() == Some(true) {
        return None;
    }
    let tag = v["tag_name"].as_str()?;
    parse_version(tag)?;
    let url = v["html_url"].as_str().map(str::to_string).unwrap_or_else(|| tag_url(tag));
    Some(Release { version: clean(tag), url })
}

fn parse_tags(body: &str) -> Option<Release> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    let best = v.as_array()?.iter().filter_map(|t| t["name"].as_str()).filter_map(|n| Some((parse_version(n)?, n))).max()?;
    Some(Release { version: clean(best.1), url: tag_url(best.1) })
}

fn tag_url(tag: &str) -> String {
    format!("https://github.com/{REPO}/releases/tag/{tag}")
}

fn clean(tag: &str) -> String {
    tag.trim().trim_start_matches(['v', 'V']).to_string()
}

/// "v1.2" -> (1, 2, 0); pre-releases ("1.2.0-beta") and other tags are None
fn parse_version(s: &str) -> Option<(u64, u64, u64)> {
    let s = clean(s);
    let s = s.split('+').next()?;
    let mut parts = s.split('.').map(|p| p.parse::<u64>().ok());
    let major = parts.next()??;
    let minor = parts.next().unwrap_or(Some(0))?;
    let patch = parts.next().unwrap_or(Some(0))?;
    parts.next().is_none().then_some((major, minor, patch))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(parse_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("1.10"), Some((1, 10, 0)));
        assert_eq!(parse_version("2"), Some((2, 0, 0)));
        assert_eq!(parse_version("1.0.0+build5"), Some((1, 0, 0)));
        assert_eq!(parse_version("1.1.0-beta"), None);
        assert_eq!(parse_version("latest"), None);
        assert_eq!(parse_version("1.2.3.4"), None);
        assert!(parse_version("1.10.0") > parse_version("1.9.9"));
    }

    #[test]
    fn release_and_tags() {
        let r = parse_release(r#"{"tag_name":"v1.2.0","html_url":"https://x/r","draft":false,"prerelease":false}"#).unwrap();
        assert_eq!(r, Release { version: "1.2.0".into(), url: "https://x/r".into() });
        assert!(parse_release(r#"{"tag_name":"v2.0.0","prerelease":true}"#).is_none());
        let t = parse_tags(r#"[{"name":"v1.9.0"},{"name":"v1.10.0"},{"name":"v2.0.0-rc1"},{"name":"test"}]"#).unwrap();
        assert_eq!(t.version, "1.10.0");
        assert!(t.url.ends_with("/releases/tag/v1.10.0"));
        assert!(parse_tags("[]").is_none());
    }

    #[test]
    fn only_newer_versions_count() {
        let r = |v: &str| Release { version: v.into(), url: String::new() };
        assert!(newer(r("999.0.0")).is_some());
        assert!(newer(r(env!("CARGO_PKG_VERSION"))).is_none());
        assert!(newer(r("0.0.1")).is_none());
        assert!(newer(r("")).is_none());
    }
}
