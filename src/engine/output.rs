//! The names of the results: from the `output` pattern of an entry, never an existing file.

use std::path::{Path, PathBuf};

pub fn short_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default()
}

/// The file without its extension, as text ("/a/b/clip.mp4" -> "/a/b/clip")
pub fn base_of(file: &Path) -> String {
    file.with_extension("").to_string_lossy().to_string()
}

/// The name of the result from the `output` pattern of an entry
pub fn named(pattern: &str, file: &Path, extra: &[(&str, String)]) -> PathBuf {
    let name = file.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let dir = file.parent().map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
    let ext = file.extension().map(|e| e.to_string_lossy().to_string()).unwrap_or_default();
    let mut text = pattern.replace("{base}", &base_of(file)).replace("{name}", &name).replace("{dir}", &dir).replace("{srcext}", &ext);
    for (k, v) in extra {
        text = text.replace(&format!("{{{k}}}"), v);
    }
    PathBuf::from(text)
}

/// The result for `file`: a name that is not taken yet, in a folder that exists
pub fn new_file(pattern: &str, file: &Path, extra: &[(&str, String)]) -> PathBuf {
    let out = unique_path(named(pattern, file, extra));
    ensure_parent(&out);
    out
}

/// Make the folder of the result if the pattern puts it into a new one
pub fn ensure_parent(p: &Path) {
    if let Some(dir) = p.parent().filter(|d| !d.as_os_str().is_empty() && !d.exists()) {
        let _ = std::fs::create_dir_all(dir);
    }
}

/// A path that does not exist yet: "name.ext", then "name (1).ext", "name (2).ext" ...
pub fn unique_path(wanted: PathBuf) -> PathBuf {
    if !wanted.exists() {
        return wanted;
    }
    let stem = wanted.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = wanted.extension().map(|e| format!(".{}", e.to_string_lossy())).unwrap_or_default();
    let dir = wanted.parent().map(Path::to_path_buf).unwrap_or_default();
    (1..).map(|n| dir.join(format!("{stem} ({n}){ext}"))).find(|c| !c.exists()).expect("an unused name exists")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_names_never_overwrite() {
        let dir = std::env::temp_dir().join(format!("cmmt-unique-{}", std::process::id()));
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
    fn patterns() {
        let f = Path::new("/v/my clip.mov");
        assert_eq!(named("{base}_compressed.mp4", f, &[]), PathBuf::from("/v/my clip_compressed.mp4"));
        assert_eq!(named("{dir}/out/{name}.{srcext}", f, &[]), PathBuf::from("/v/out/my clip.mov"));
        assert_eq!(named("{base}.{ext}", f, &[("ext", "mkv".into())]), PathBuf::from("/v/my clip.mkv"));
    }
}
