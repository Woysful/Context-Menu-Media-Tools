//! The fonts of the system for the text on pictures: their names (read from the files, so it
//! works the same everywhere) and the widths of lines, so the text can be framed and wrapped
//! before ffmpeg draws it.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[derive(Clone, Debug, PartialEq)]
pub struct FontFile {
    pub family: String,
    pub style: String,
    pub path: PathBuf,
}

/// The families that are tried for text on a picture, then for the captions of a meme
const TEXT_FAMILIES: &[&str] =
    &["Arial", "Segoe UI", "Noto Sans", "DejaVu Sans", "Liberation Sans", "Open Sans", "Roboto", "Ubuntu", "Cantarell", "Nimbus Sans", "FreeSans"];
const CAPTION_FAMILIES: &[&str] = &["Impact", "Anton", "Oswald", "Bebas Neue", "League Gothic", "Haettenschweiler"];

fn font_dirs() -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = Vec::new();
    if cfg!(windows) {
        let win = std::env::var_os("WINDIR").map_or_else(|| PathBuf::from("C:\\Windows"), PathBuf::from);
        v.push(win.join("Fonts"));
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            v.push(PathBuf::from(local).join("Microsoft").join("Windows").join("Fonts"));
        }
    } else {
        let data = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
        for d in data.split(':').filter(|d| !d.is_empty()).chain(["/usr/local/share", "/usr/share"]) {
            v.push(Path::new(d).join("fonts"));
        }
        if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
            v.push(home.join(".local/share/fonts"));
            v.push(home.join(".fonts"));
        }
    }
    let mut seen = Vec::new();
    v.retain(|d| {
        let real = std::fs::canonicalize(d).unwrap_or_else(|_| d.clone());
        let new = d.is_dir() && !seen.contains(&real);
        seen.push(real);
        new
    });
    v
}

fn font_files(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth < 6 {
                font_files(&p, depth + 1, out);
            }
        } else if matches!(p.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref(), Some("ttf" | "otf" | "ttc" | "otc")) {
            out.push(p);
        }
    }
}

/// The text of a name record: UTF-16 (Unicode, Windows) or the Mac one byte encoding (ASCII
/// is all that matters for the names of fonts)
fn name_text(n: &ttf_parser::name::Name) -> Option<String> {
    if n.is_unicode() {
        n.to_string()
    } else if n.platform_id == ttf_parser::PlatformId::Macintosh && n.encoding_id == 0 {
        Some(n.name.iter().map(|b| if b.is_ascii() { *b as char } else { '?' }).collect())
    } else {
        None
    }
}

/// The family and the style of the first face of a font file. Only the table directory and
/// the name table are read: some files are tens of megabytes.
fn read_names(path: &Path) -> Option<(String, String)> {
    let mut f = std::fs::File::open(path).ok()?;
    let mut head = Vec::new();
    f.by_ref().take(64 * 1024).read_to_end(&mut head).ok()?;
    let raw = ttf_parser::RawFace::parse(&head, 0).ok()?;
    let rec = raw.table_records.into_iter().find(|r| r.tag == ttf_parser::Tag::from_bytes(b"name"))?;
    let mut table = vec![0u8; rec.length as usize];
    f.seek(SeekFrom::Start(rec.offset as u64)).ok()?;
    f.read_exact(&mut table).ok()?;
    let names = ttf_parser::name::Table::parse(&table)?.names;
    // the typographic family and style (16, 17) group the faces better than the old ones (1, 2)
    let pick = |ids: [u16; 2]| {
        for id in ids {
            let mut best: Option<(u8, String)> = None;
            for n in names.into_iter().filter(|n| n.name_id == id) {
                let Some(text) = name_text(&n).filter(|t| !t.trim().is_empty()) else { continue };
                let rank = match (n.platform_id, n.language_id) {
                    (ttf_parser::PlatformId::Windows, 0x409) => 0,
                    (ttf_parser::PlatformId::Macintosh, 0) => 1,
                    (ttf_parser::PlatformId::Unicode, _) => 2,
                    _ => 3,
                };
                if best.as_ref().is_none_or(|(r, _)| rank < *r) {
                    best = Some((rank, text.trim().to_string()));
                }
            }
            if let Some((_, t)) = best {
                return Some(t);
            }
        }
        None
    };
    let family = pick([16, 1])?;
    let style = pick([17, 2]).unwrap_or_else(|| "Regular".into());
    Some((family, style))
}

fn style_rank(style: &str) -> (u8, String) {
    let s = style.to_lowercase();
    let r = match s.as_str() {
        "regular" | "book" | "normal" | "roman" => 0,
        "bold" => 1,
        "italic" | "oblique" => 2,
        "bold italic" | "bold oblique" => 3,
        _ => 4,
    };
    (r, s)
}

/// Every font of the system, by family, the plain style of a family first. Read once.
pub fn system() -> Arc<Vec<FontFile>> {
    static LIST: OnceLock<Arc<Vec<FontFile>>> = OnceLock::new();
    LIST.get_or_init(|| {
        let mut files = Vec::new();
        for d in font_dirs() {
            font_files(&d, 0, &mut files);
        }
        files.sort();
        let mut list: Vec<FontFile> = Vec::new();
        for path in files {
            let Some((family, style)) = read_names(&path) else { continue };
            if family.starts_with('.') || list.iter().any(|f| f.family == family && f.style == style) {
                continue;
            }
            list.push(FontFile { family, style, path });
        }
        list.sort_by(|a, b| a.family.to_lowercase().cmp(&b.family.to_lowercase()).then_with(|| style_rank(&a.style).cmp(&style_rank(&b.style))));
        Arc::new(list)
    })
    .clone()
}

/// The first of `families` that is there, in the first of `styles` it has (or its plain one)
fn pick(list: &[FontFile], families: &[&str], styles: &[&str]) -> Option<PathBuf> {
    for fam in families {
        let faces: Vec<&FontFile> = list.iter().filter(|f| f.family.eq_ignore_ascii_case(fam)).collect();
        if faces.is_empty() {
            continue;
        }
        for st in styles {
            if let Some(f) = faces.iter().find(|f| f.style.eq_ignore_ascii_case(st)) {
                return Some(f.path.clone());
            }
        }
        return Some(faces[0].path.clone());
    }
    None
}

/// The font of a new text: a bold sans-serif one
pub fn default_text(list: &[FontFile]) -> Option<PathBuf> {
    pick(list, TEXT_FAMILIES, &["Bold", "Regular"]).or_else(|| list.first().map(|f| f.path.clone()))
}

/// The font of the captions of a meme: Impact or one like it, else a bold sans-serif one
pub fn default_caption(list: &[FontFile]) -> Option<PathBuf> {
    pick(list, CAPTION_FAMILIES, &["Regular", "Bold"]).or_else(|| pick(list, TEXT_FAMILIES, &["Black", "Bold"])).or_else(|| default_text(list))
}

/// A font named by its family ("DejaVu Sans"), its family and style ("DejaVu Sans:Bold") or
/// its file
pub fn find(list: &[FontFile], name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_file() {
        return Some(p.to_path_buf());
    }
    let (family, style) = name.split_once(':').map_or((name, None), |(f, s)| (f, Some(s.trim())));
    let styles: Vec<&str> = style.map_or_else(|| vec!["Regular"], |s| vec![s]);
    pick(list, &[family.trim()], &styles)
}

/// The family and style of a font file, for the window
pub fn describe(list: &[FontFile], path: &Path) -> Option<(String, String)> {
    list.iter().find(|f| f.path == path).map(|f| (f.family.clone(), f.style.clone()))
}

fn data(path: &Path) -> Result<Arc<Vec<u8>>, String> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, Arc<Vec<u8>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(d) = cache.lock().unwrap_or_else(|e| e.into_inner()).get(path) {
        return Ok(d.clone());
    }
    let d = Arc::new(std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?);
    cache.lock().unwrap_or_else(|e| e.into_inner()).insert(path.to_path_buf(), d.clone());
    Ok(d)
}

/// Sizes of a font at `px` pixels to the em
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// from the top of a line to its baseline
    pub ascent: f64,
    /// the height of a line of text
    pub height: f64,
    /// from one baseline to the next
    pub step: f64,
}

/// A font that can be measured
pub struct Font {
    data: Arc<Vec<u8>>,
}

impl Font {
    pub fn load(path: &Path) -> Result<Font, String> {
        let data = data(path)?;
        ttf_parser::Face::parse(&data, 0).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(Font { data })
    }

    fn face(&self) -> Option<ttf_parser::Face<'_>> {
        ttf_parser::Face::parse(&self.data, 0).ok()
    }

    pub fn metrics(&self, px: f64) -> Metrics {
        let Some(f) = self.face() else { return Metrics { ascent: px * 0.9, height: px * 1.15, step: px * 1.2 } };
        let k = px / f.units_per_em().max(1) as f64;
        let (a, d, g) = (f.ascender() as f64 * k, -f.descender() as f64 * k, f.line_gap() as f64 * k);
        Metrics { ascent: a, height: a + d, step: a + d + g }
    }

    /// The width of a line (the advances of its letters, without kerning)
    pub fn width(&self, text: &str, px: f64) -> f64 {
        let Some(f) = self.face() else { return text.chars().count() as f64 * px * 0.55 };
        let k = px / f.units_per_em().max(1) as f64;
        text.chars().filter(|c| !c.is_control()).map(|c| f.glyph_hor_advance(f.glyph_index(c).unwrap_or(ttf_parser::GlyphId(0))).unwrap_or(0) as f64 * k).sum()
    }

    /// The lines of `text` no wider than `max_w`: its own lines, broken between words (a word
    /// that is wider stays on a line of its own)
    pub fn wrap(&self, text: &str, px: f64, max_w: f64) -> Vec<String> {
        let mut out = Vec::new();
        for line in text.lines() {
            let mut cur = String::new();
            for word in line.split_whitespace() {
                let joined = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
                if cur.is_empty() || self.width(&joined, px) <= max_w {
                    cur = joined;
                } else {
                    out.push(std::mem::replace(&mut cur, word.to_string()));
                }
            }
            if !cur.is_empty() {
                out.push(cur);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fonts_are_found_by_name() {
        let f = |fam: &str, st: &str| FontFile { family: fam.into(), style: st.into(), path: PathBuf::from(format!("/f/{fam}-{st}.ttf")) };
        let list = vec![f("DejaVu Sans", "Regular"), f("DejaVu Sans", "Bold"), f("Impact", "Regular"), f("Zed", "Light")];
        assert_eq!(find(&list, "dejavu sans"), Some(PathBuf::from("/f/DejaVu Sans-Regular.ttf")));
        assert_eq!(find(&list, "DejaVu Sans:bold"), Some(PathBuf::from("/f/DejaVu Sans-Bold.ttf")));
        // a style it does not have: the first one of the family
        assert_eq!(find(&list, "Zed"), Some(PathBuf::from("/f/Zed-Light.ttf")));
        assert_eq!(find(&list, "Nope"), None);
        assert_eq!(default_text(&list), Some(PathBuf::from("/f/DejaVu Sans-Bold.ttf")));
        assert_eq!(default_caption(&list), Some(PathBuf::from("/f/Impact-Regular.ttf")));
    }

    #[test]
    fn a_font_of_the_system_is_measured() {
        let list = system();
        let Some(path) = default_text(&list) else { return };
        let font = Font::load(&path).unwrap();
        let m = font.metrics(100.0);
        assert!(m.ascent > 50.0 && m.height > m.ascent && m.step >= m.height);
        assert!(font.width("WWWW", 100.0) > font.width("iiii", 100.0));
        let lines = font.wrap("one two three four five six", 100.0, font.width("one two", 100.0) + 1.0);
        assert_eq!(lines[0], "one two");
        assert_eq!(font.wrap("a\n\nb", 10.0, 1000.0), ["a", "b"]);
    }
}
