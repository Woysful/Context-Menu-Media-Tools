//! The edits of Compress Image as ffmpeg filters: turn and mirror, colours, sharpness, blurred
//! areas, text, crop, vignette and the captions of a meme, in this order. Areas and text are
//! placed in parts of the turned picture (before the crop), so they stay on what they cover
//! when the crop changes.

use super::ImageEdit;
use crate::fonts::{self, Font};
use std::path::{Path, PathBuf};

/// Colours: every value 0 = as it is, -100..=100 (hue: degrees, -180..=180)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Colours {
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
    pub gamma: i32,
    /// warmer above 0
    pub temperature: i32,
    pub hue: i32,
    pub vibrance: i32,
    /// red, green, blue of the dark and of the light parts
    pub shadows: [i32; 3],
    pub highlights: [i32; 3],
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum BlurKind {
    #[default]
    Gauss,
    Box,
    Pixels,
}

impl BlurKind {
    pub const ALL: [BlurKind; 3] = [BlurKind::Gauss, BlurKind::Box, BlurKind::Pixels];

    pub fn key(self) -> &'static str {
        match self {
            BlurKind::Gauss => "gauss",
            BlurKind::Box => "box",
            BlurKind::Pixels => "pixels",
        }
    }
}

/// A blurred area
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BlurArea {
    /// x, y, width, height as parts of the turned picture
    pub rect: [f64; 4],
    pub kind: BlurKind,
    /// 1..=100: how blurred, as a part of the area (the same at any size of the picture)
    pub strength: u32,
}

/// A text on the picture
#[derive(Clone, Debug, PartialEq)]
pub struct TextMark {
    /// may have several lines; each is centred
    pub text: String,
    /// the middle of the text, as parts of the turned picture
    pub at: [f64; 2],
    /// the size of the letters as a part of the longer side of the picture
    pub size: f64,
    /// empty: a bold sans-serif font of the system
    pub font: PathBuf,
    pub color: [u8; 3],
    /// the outline as a percentage of the size of the letters (0 = none)
    pub outline: u32,
    pub outline_color: [u8; 3],
    pub boxed: bool,
    pub box_color: [u8; 3],
    /// 0..=100
    pub box_opacity: u32,
}

impl Default for TextMark {
    fn default() -> Self {
        TextMark {
            text: String::new(),
            at: [0.5, 0.5],
            size: 0.06,
            font: PathBuf::new(),
            color: [255, 255, 255],
            outline: 8,
            outline_color: [0, 0, 0],
            boxed: false,
            box_color: [0, 0, 0],
            box_opacity: 60,
        }
    }
}

/// The captions of a meme over and under the picture
#[derive(Clone, Debug, PartialEq)]
pub struct Caption {
    pub top: String,
    pub bottom: String,
    /// white strips added over and under the picture with black text; otherwise white text
    /// with a black outline on the picture
    pub strip: bool,
    /// the size of the letters as a percentage of the width of the picture
    pub size: u32,
    /// empty: Impact or one like it
    pub font: PathBuf,
}

impl Default for Caption {
    fn default() -> Self {
        Caption { top: String::new(), bottom: String::new(), strip: true, size: 9, font: PathBuf::new() }
    }
}

impl Caption {
    pub fn is_empty(&self) -> bool {
        self.top.trim().is_empty() && self.bottom.trim().is_empty()
    }
}

/// How far the filters go
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Upto {
    /// what is before the crop: the picture the crop is chosen on
    Crop,
    /// all of it (without the size of the result)
    All,
}

/// The filters and the size of the picture they make (when the size going in is known)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Built {
    pub filters: Vec<String>,
    pub size: Option<(u32, u32)>,
}

/// A value inside the options of a filter in a filter graph: escaped for the options, then
/// for the graph (the two levels of ffmpeg)
pub fn escape(s: &str) -> String {
    let mut opt = String::new();
    for c in s.chars() {
        if matches!(c, '\\' | '\'' | ':') {
            opt.push('\\');
        }
        opt.push(c);
    }
    let mut out = String::new();
    for c in opt.chars() {
        if matches!(c, '\\' | '\'' | '[' | ']' | ',' | ';') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

fn hex(c: [u8; 3]) -> String {
    format!("0x{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn colour_filters(c: &Colours, f: &mut Vec<String>) {
    let k = |v: i32| v as f64 / 100.0;
    if c.brightness != 0 || c.contrast != 0 || c.saturation != 0 || c.gamma != 0 {
        f.push(format!(
            "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}:gamma={:.3}",
            k(c.brightness) * 0.5,
            1.0 + k(c.contrast),
            1.0 + k(c.saturation),
            2f64.powf(k(c.gamma) * 1.5)
        ));
    }
    if c.temperature != 0 {
        // a lower temperature of the light makes the picture warmer
        f.push(format!("colortemperature=temperature={}", 6500 - c.temperature * 35));
    }
    if c.hue != 0 {
        f.push(format!("hue=h={}", c.hue));
    }
    if c.vibrance != 0 {
        f.push(format!("vibrance=intensity={:.3}", k(c.vibrance)));
    }
    if c.shadows != [0; 3] || c.highlights != [0; 3] {
        let [rs, gs, bs] = c.shadows.map(|v| k(v) * 0.5);
        let [rh, gh, bh] = c.highlights.map(|v| k(v) * 0.5);
        f.push(format!("colorbalance=rs={rs:.3}:gs={gs:.3}:bs={bs:.3}:rh={rh:.3}:gh={gh:.3}:bh={bh:.3}"));
    }
}

/// The area in whole pixels of a picture of `size`, at least one pixel
fn area_px(r: [f64; 4], (w, h): (u32, u32)) -> (u32, u32, u32, u32) {
    let x = ((r[0] * w as f64).round() as u32).min(w.saturating_sub(1));
    let y = ((r[1] * h as f64).round() as u32).min(h.saturating_sub(1));
    let aw = ((r[2] * w as f64).round() as u32).clamp(1, w - x);
    let ah = ((r[3] * h as f64).round() as u32).clamp(1, h - y);
    (x, y, aw, ah)
}

fn blur_filter(b: &BlurArea, size: (u32, u32), label: &mut usize) -> String {
    let (x, y, w, h) = area_px(b.rect, size);
    let s = b.strength.clamp(1, 100) as f64 / 100.0;
    let short = w.min(h) as f64;
    let inner = match b.kind {
        BlurKind::Gauss => format!("gblur=sigma={:.2}", (s * short / 2.0).max(0.5)),
        BlurKind::Box => {
            let r = ((s * short / 2.0).round() as u32).max(1);
            // the colours of a JPEG are half as large: their radius has to fit too
            format!("boxblur=lr={r}:lp=2:cr='min({r},min(cw,ch)/2)':cp=2")
        }
        BlurKind::Pixels => {
            let block = (s * short / 3.0).round().max(2.0);
            let (sw, sh) = (((w as f64 / block).round() as u32).max(1), ((h as f64 / block).round() as u32).max(1));
            format!("scale={sw}:{sh}:flags=area,scale={w}:{h}:flags=neighbor")
        }
    };
    let (a, b, c) = (*label, *label + 1, *label + 2);
    *label += 3;
    format!("split[cmmt{a}][cmmt{b}];[cmmt{b}]crop={w}:{h}:{x}:{y}:exact=1,{inner}[cmmt{c}];[cmmt{a}][cmmt{c}]overlay={x}:{y}:format=auto")
}

/// The font of a text, or the one that is used when none is chosen
fn font_or(path: &Path, default: fn(&[fonts::FontFile]) -> Option<PathBuf>) -> Result<PathBuf, String> {
    if !path.as_os_str().is_empty() {
        return Ok(path.to_path_buf());
    }
    default(&fonts::system()).ok_or_else(|| "no font found on this system".to_string())
}

/// One line of text drawn by ffmpeg: centred on `cx`, its baseline at `baseline`
fn line_filter(font: &Path, text: &str, px: f64, color: [u8; 3], border: Option<(u32, [u8; 3])>, cx: f64, baseline: f64) -> String {
    let mut s = format!("drawtext=fontfile={}:expansion=none:text={}:fontsize={}:fontcolor={}", escape(&font.to_string_lossy()), escape(text), px, hex(color));
    if let Some((w, c)) = border.filter(|(w, _)| *w > 0) {
        s.push_str(&format!(":borderw={w}:bordercolor={}", hex(c)));
    }
    // max_glyph_a: from the top of what is drawn to the baseline, so every line sits on its own
    s.push_str(&format!(":x={cx:.1}-text_w/2:y={baseline:.1}-max_glyph_a"));
    s
}

/// Where a text goes on a picture of `size` (turned): the frame around it (with the room of
/// its background) and the baselines of its lines, in pixels
#[derive(Clone, Debug, PartialEq)]
pub struct TextBlock {
    pub rect: [f64; 4],
    pub px: f64,
    pub lines: Vec<(String, f64)>,
    pub font: PathBuf,
}

pub fn text_block(t: &TextMark, (w, h): (u32, u32)) -> Result<TextBlock, String> {
    let font_path = font_or(&t.font, fonts::default_text)?;
    let font = Font::load(&font_path)?;
    let px = (t.size * w.max(h) as f64).round().max(1.0);
    let m = font.metrics(px);
    let mut lines: Vec<&str> = t.text.lines().collect();
    if lines.is_empty() {
        lines.push("");
    }
    let width = lines.iter().map(|l| font.width(l, px)).fold(0.0, f64::max).max(px);
    let height = (lines.len() - 1) as f64 * m.step + m.height;
    let (cx, cy) = (t.at[0] * w as f64, t.at[1] * h as f64);
    let top = cy - height / 2.0;
    let pad = (px * 0.25).round();
    let rect = [cx - width / 2.0 - pad, top - pad, width + 2.0 * pad, height + 2.0 * pad];
    let lines = lines.iter().enumerate().map(|(i, l)| (l.to_string(), top + i as f64 * m.step + m.ascent)).collect();
    Ok(TextBlock { rect, px, lines, font: font_path })
}

fn text_filters(t: &TextMark, size: (u32, u32), f: &mut Vec<String>) -> Result<(), String> {
    if t.text.trim().is_empty() {
        return Ok(());
    }
    let b = text_block(t, size)?;
    if t.boxed {
        let [x, y, w, h] = b.rect.map(f64::round);
        f.push(format!("drawbox=x={x}:y={y}:w={w}:h={h}:color={}@{:.2}:t=fill", hex(t.box_color), t.box_opacity.min(100) as f64 / 100.0));
    }
    let border = (t.outline > 0).then(|| (((b.px * t.outline.min(50) as f64 / 100.0).round() as u32).max(1), t.outline_color));
    let cx = b.rect[0] + b.rect[2] / 2.0;
    for (line, baseline) in b.lines.iter().filter(|(l, _)| !l.trim().is_empty()) {
        f.push(line_filter(&b.font, line, b.px, t.color, border, cx, *baseline));
    }
    Ok(())
}

/// The captions of a meme on a picture of `size`; returns the size with the strips
fn caption_filters(c: &Caption, (w, h): (u32, u32), f: &mut Vec<String>) -> Result<(u32, u32), String> {
    let path = font_or(&c.font, fonts::default_caption)?;
    let font = Font::load(&path)?;
    let px = (c.size.clamp(1, 50) as f64 / 100.0 * w as f64).round().max(4.0);
    let m = font.metrics(px);
    let room = w as f64 * 0.94;
    let (top, bottom) = (font.wrap(&c.top, px, room), font.wrap(&c.bottom, px, room));
    let block = |n: usize| if n == 0 { 0.0 } else { (n - 1) as f64 * m.step + m.height };
    let pad = (px * 0.35).round();
    let cx = w as f64 / 2.0;
    let mut lines: Vec<(&String, f64)> = Vec::new();
    let (size, color, border) = if c.strip {
        let strip = |n: usize| if n == 0 { 0 } else { (block(n) + 2.0 * pad).ceil() as u32 };
        let (st, sb) = (strip(top.len()), strip(bottom.len()));
        // pad rounds sizes and places down to the half-size colours of yuv420p (WebP, AVIF,
        // after sharpen or vignette): an odd picture or strip would not fit in it
        f.push("format=yuv444p|yuva444p|rgb24|rgba".into());
        f.push(format!("pad=w=iw:h=ih+{}:x=0:y={st}:color=white", st + sb));
        lines.extend(top.iter().enumerate().map(|(i, l)| (l, pad + i as f64 * m.step + m.ascent)));
        let below = (st + h) as f64 + pad;
        lines.extend(bottom.iter().enumerate().map(|(i, l)| (l, below + i as f64 * m.step + m.ascent)));
        ((w, h + st + sb), [0, 0, 0], None)
    } else {
        lines.extend(top.iter().enumerate().map(|(i, l)| (l, pad + i as f64 * m.step + m.ascent)));
        let start = h as f64 - pad - block(bottom.len());
        lines.extend(bottom.iter().enumerate().map(|(i, l)| (l, start + i as f64 * m.step + m.ascent)));
        ((w, h), [255, 255, 255], Some((((px * 0.07).round() as u32).max(1), [0, 0, 0])))
    };
    for (line, baseline) in lines {
        f.push(line_filter(&path, line, px, color, border, cx, baseline));
    }
    Ok(size)
}

fn odd(v: u32) -> u32 {
    v | 1
}

/// The filters of `edit`; `size`: of the picture going in (turned by its EXIF orientation).
/// Areas, text and captions need it.
pub fn build(edit: &ImageEdit, size: Option<(u32, u32)>, upto: Upto) -> Result<Built, String> {
    let mut f: Vec<String> = Vec::new();
    match edit.turns % 4 {
        1 => f.push("transpose=clock".into()),
        2 => f.push("hflip,vflip".into()),
        3 => f.push("transpose=cclock".into()),
        _ => {}
    }
    if edit.flip_h {
        f.push("hflip".into());
    }
    if edit.flip_v {
        f.push("vflip".into());
    }
    let mut size = size.map(|(w, h)| if edit.turns % 2 == 1 { (h, w) } else { (w, h) });
    let known = |s: Option<(u32, u32)>| s.filter(|(w, h)| *w > 0 && *h > 0).ok_or_else(|| "the size of the picture is not known".to_string());
    colour_filters(&edit.colours, &mut f);
    if edit.sharpen > 0 {
        // the same sharpness on the preview as on the large picture
        let m = size.map_or(5, |(w, h)| odd(w.max(h) / 320).clamp(3, 23));
        f.push(format!("unsharp=lx={m}:ly={m}:la={:.3}:cx={m}:cy={m}:ca=0", edit.sharpen.min(100) as f64 / 100.0 * 1.5));
    }
    let mut label = 0;
    for b in &edit.blurs {
        f.push(blur_filter(b, known(size)?, &mut label));
    }
    for t in &edit.texts {
        text_filters(t, known(size)?, &mut f)?;
    }
    if upto == Upto::Crop {
        return Ok(Built { filters: f, size });
    }
    // exact: x and y are not moved to even pixels for the subsampled colours of a JPEG
    match (edit.crop, size) {
        // in whole pixels: the size that comes out is known whatever ffmpeg rounds to
        (Some(r), Some(s)) => {
            let (x, y, w, h) = area_px(r, s);
            f.push(format!("crop={w}:{h}:{x}:{y}:exact=1"));
            size = Some((w, h));
        }
        (Some([x, y, w, h]), None) => f.push(format!("crop=w=iw*{w:.6}:h=ih*{h:.6}:x=iw*{x:.6}:y=ih*{y:.6}:exact=1")),
        (None, _) => {}
    }
    if edit.vignette > 0 {
        f.push(format!("vignette=angle={:.4}", edit.vignette.min(100) as f64 / 100.0 * 1.2));
    }
    if let Some(c) = edit.caption.as_ref().filter(|c| !c.is_empty()) {
        size = Some(caption_filters(c, known(size)?, &mut f)?);
    }
    Ok(Built { filters: f, size })
}

/// The edit needs the size of the picture
pub fn needs_size(edit: &ImageEdit) -> bool {
    !edit.blurs.is_empty() || edit.texts.iter().any(|t| !t.text.trim().is_empty()) || edit.caption.as_ref().is_some_and(|c| !c.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_escaped_twice() {
        assert_eq!(escape("a:b"), r"a\\:b");
        assert_eq!(escape("it's"), r"it\\\'s");
        assert_eq!(escape("x,y;[z]"), r"x\,y\;\[z\]");
        assert_eq!(escape(r"C:\Fonts\a.ttf"), r"C\\:\\\\Fonts\\\\a.ttf");
    }

    #[test]
    fn colours_and_areas() {
        let edit = ImageEdit {
            colours: Colours { brightness: 20, saturation: -100, temperature: 10, ..Default::default() },
            blurs: vec![BlurArea { rect: [0.1, 0.2, 0.5, 0.25], kind: BlurKind::Pixels, strength: 30 }],
            ..Default::default()
        };
        let b = build(&edit, Some((1000, 400)), Upto::All).unwrap();
        assert_eq!(b.filters[0], "eq=brightness=0.100:contrast=1.000:saturation=0.000:gamma=1.000");
        assert_eq!(b.filters[1], "colortemperature=temperature=6150");
        // 500 x 100 px at 100, 80; blocks of 10 px
        assert_eq!(
            b.filters[2],
            "split[cmmt0][cmmt1];[cmmt1]crop=500:100:100:80:exact=1,scale=50:10:flags=area,scale=500:100:flags=neighbor[cmmt2];[cmmt0][cmmt2]overlay=100:80:format=auto"
        );
        assert_eq!(b.size, Some((1000, 400)));
        // the areas need the size
        assert!(build(&edit, None, Upto::All).is_err());
        assert!(needs_size(&edit) && !needs_size(&ImageEdit::default()));
    }

    #[test]
    fn the_crop_and_the_turn_give_the_size() {
        let edit = ImageEdit { turns: 1, crop: Some([0.0, 0.0, 0.5, 1.0 / 3.0]), vignette: 50, ..Default::default() };
        let b = build(&edit, Some((1067, 1601)), Upto::All).unwrap();
        // turned: 1601 x 1067; 1601 * 0.5 = 800.5, 1067 / 3 = 355.67
        assert_eq!(b.size, Some((801, 356)));
        assert_eq!(b.filters[1], "crop=801:356:0:0:exact=1");
        assert_eq!(b.filters.last().unwrap(), "vignette=angle=0.6000");
        // without the size: parts of it
        let parts = build(&edit, None, Upto::All).unwrap();
        assert_eq!(parts.filters[1], "crop=w=iw*0.500000:h=ih*0.333333:x=iw*0.000000:y=ih*0.000000:exact=1");
        let before = build(&edit, Some((1067, 1601)), Upto::Crop).unwrap();
        assert_eq!((before.filters, before.size), (vec!["transpose=clock".to_string()], Some((1601, 1067))));
    }

    #[test]
    fn text_and_captions() {
        let Some(font) = fonts::default_text(&fonts::system()) else { return };
        let mark = TextMark { text: "Hello\nworld: 'x'".into(), at: [0.5, 0.25], size: 0.05, font: font.clone(), boxed: true, ..Default::default() };
        let b = text_block(&mark, (1000, 800)).unwrap();
        assert_eq!(b.px, 50.0);
        assert_eq!(b.lines.len(), 2);
        // the middle of the frame is the point of the text
        assert!((b.rect[0] + b.rect[2] / 2.0 - 500.0).abs() < 1e-6 && (b.rect[1] + b.rect[3] / 2.0 - 200.0).abs() < 1e-6);
        let edit = ImageEdit {
            texts: vec![mark],
            caption: Some(Caption { top: "top text".into(), bottom: String::new(), strip: true, size: 10, font }),
            ..Default::default()
        };
        let built = build(&edit, Some((1000, 800)), Upto::All).unwrap();
        assert!(built.filters[0].starts_with("drawbox=x="));
        assert!(built.filters[1].contains(":text=Hello:fontsize=50:fontcolor=0xFFFFFF:borderw=4:bordercolor=0x000000:x=500.0-text_w/2"));
        assert!(built.filters[2].contains(r":text=world\\: \\\'x\\\':"));
        assert!(built.filters[3].starts_with("format=yuv444p"));
        assert!(built.filters[4].starts_with("pad=w=iw:h=ih+"));
        let strip = built.size.unwrap().1 - 800;
        assert!(strip > 100 && strip < 200, "{strip}");
    }
}
