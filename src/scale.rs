//! Output resolution: parsing of what the user typed, and the scale factor of a file.
//! A factor is always relative to the SHORTER side, so portrait videos behave like
//! landscape ones; nothing is ever scaled up.

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScaleMode {
    Orig,
    /// Largest size that still gets enough bits per pixel (size limit mode only)
    Auto,
    /// Shorter side in pixels
    Height(u32),
    Factor(f64),
}

impl ScaleMode {
    /// Canonical text of the mode (the same codes the shell scripts used: orig, auto, h720, s0.5)
    #[allow(dead_code)]
    pub fn code(&self) -> String {
        match self {
            ScaleMode::Orig => "orig".into(),
            ScaleMode::Auto => "auto".into(),
            ScaleMode::Height(h) => format!("h{h}"),
            ScaleMode::Factor(f) => format!("s{}", trim_float(*f)),
        }
    }

    pub fn from_code(code: &str) -> Option<ScaleMode> {
        match code {
            "orig" => Some(ScaleMode::Orig),
            "auto" => Some(ScaleMode::Auto),
            _ => {
                let (head, rest) = code.split_at(code.chars().next()?.len_utf8());
                match head {
                    "h" => rest.parse().ok().map(ScaleMode::Height),
                    "s" => rest.parse().ok().map(ScaleMode::Factor),
                    _ => None,
                }
            }
        }
    }
}

#[allow(dead_code)]
fn trim_float(v: f64) -> String {
    let s = format!("{:.4}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// What the user typed into the resolution field: "50%", "x0.5", "0.5x", "540p", "540", "0.5".
pub fn parse_typed(text: &str) -> Option<ScaleMode> {
    let t: String = text
        .trim()
        .to_lowercase()
        .replace(',', ".")
        .replace('×', "x")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if t.is_empty() {
        return None;
    }
    let num = |s: &str| -> Option<f64> {
        if s.is_empty() || !s.chars().all(|c| c.is_ascii_digit() || c == '.') || s.matches('.').count() > 1 {
            return None;
        }
        s.parse().ok()
    };
    if let Some(p) = t.strip_suffix('%') {
        let v = num(p)?;
        return (5.0..=100.0).contains(&v).then(|| ScaleMode::Factor(v / 100.0));
    }
    for rest in [t.strip_prefix('x'), t.strip_prefix('*')].into_iter().flatten() {
        let v = num(rest)?;
        return (0.05..=1.0).contains(&v).then_some(ScaleMode::Factor(v));
    }
    for rest in [t.strip_suffix('x'), t.strip_suffix('*')].into_iter().flatten() {
        let v = num(rest)?;
        return (0.05..=1.0).contains(&v).then_some(ScaleMode::Factor(v));
    }
    if let Some(p) = t.strip_suffix('p') {
        let v: u32 = p.parse().ok()?;
        return (64..=4320).contains(&v).then_some(ScaleMode::Height(v));
    }
    let v = num(&t)?;
    if v <= 1.0 {
        return (v >= 0.05).then_some(ScaleMode::Factor(v));
    }
    if t.chars().all(|c| c.is_ascii_digit()) {
        let n: u32 = t.parse().ok()?;
        if (64..=4320).contains(&n) {
            return Some(ScaleMode::Height(n));
        }
    }
    None
}

/// Size of the picture after scaling; None = unchanged (same rule as the encoder:
/// one factor for both sides, rounded to even numbers, never larger than the source).
pub fn target_size(width: u32, height: u32, mode: ScaleMode) -> Option<(u32, u32)> {
    let short = width.min(height) as f64;
    let factor = match mode {
        ScaleMode::Factor(f) => f,
        ScaleMode::Height(h) => h as f64 / short,
        _ => return None,
    };
    if factor >= 0.999 {
        return None;
    }
    let even = |v: f64| ((v / 2.0 + 0.5).floor() as u32 * 2).max(2);
    Some((even(width as f64 * factor), even(height as f64 * factor)))
}

#[derive(Clone, Copy, Debug)]
pub struct Choice {
    pub factor: f64,
    pub short_px: u32,
    pub bpp: f64,
}

/// The factor for one file. "Auto" walks down a ladder of sizes and takes the largest one
/// that still gets `bpp_min` bits per pixel at the bitrate `video_bps` (frame rates above
/// 30 count only half, because fast video has more in common between frames).
pub fn choose(mode: ScaleMode, w: u32, h: u32, fps: f64, video_bps: f64, bpp_min: f64) -> Choice {
    let (wf, hf) = (w as f64, h as f64);
    let short = wf.min(hf);
    let fe = if fps > 30.0 { 30.0 + (fps - 30.0) * 0.5 } else { fps };
    let mut f = 1.0;
    match mode {
        ScaleMode::Factor(v) => f = v,
        ScaleMode::Height(px) => f = px as f64 / short,
        ScaleMode::Auto => {
            if video_bps / (wf * hf * fe) < bpp_min {
                f = 0.0;
                for step in [2160.0, 1440.0, 1080.0, 720.0, 540.0, 480.0, 360.0, 240.0] {
                    if step >= short {
                        continue;
                    }
                    let cf = step / short;
                    if video_bps / (wf * hf * cf * cf * fe) >= bpp_min {
                        f = cf;
                        break;
                    }
                }
                if f == 0.0 {
                    // nothing is good enough: the smallest step
                    f = if short > 240.0 { 240.0 / short } else { 1.0 };
                }
            }
        }
        ScaleMode::Orig => {}
    }
    if f > 1.0 || f >= 0.999 {
        f = 1.0;
    }
    Choice {
        factor: f,
        short_px: (2.0 * (short * f / 2.0 + 0.5).floor()) as u32,
        bpp: video_bps / (wf * hf * f * f * fe),
    }
}

/// The ffmpeg filter for a factor below 1
pub fn filter(factor: f64) -> String {
    format!("scale=2*round(iw*{factor:.6}/2):2*round(ih*{factor:.6}/2):flags=lanczos")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_values() {
        assert_eq!(parse_typed("50%"), Some(ScaleMode::Factor(0.5)));
        assert_eq!(parse_typed(" x0.5 "), Some(ScaleMode::Factor(0.5)));
        assert_eq!(parse_typed("0,25x"), Some(ScaleMode::Factor(0.25)));
        assert_eq!(parse_typed("540p"), Some(ScaleMode::Height(540)));
        assert_eq!(parse_typed("720"), Some(ScaleMode::Height(720)));
        assert_eq!(parse_typed("0.5"), Some(ScaleMode::Factor(0.5)));
        assert_eq!(parse_typed("150%"), None);
        assert_eq!(parse_typed("x2"), None);
        assert_eq!(parse_typed("10p"), None);
        assert_eq!(parse_typed("garbage"), None);
        assert_eq!(parse_typed(""), None);
    }

    #[test]
    fn codes_roundtrip() {
        for m in [ScaleMode::Orig, ScaleMode::Auto, ScaleMode::Height(720), ScaleMode::Factor(0.5)] {
            assert_eq!(ScaleMode::from_code(&m.code()), Some(m));
        }
        assert_eq!(ScaleMode::Factor(0.33).code(), "s0.33");
    }

    #[test]
    fn sizes() {
        assert_eq!(target_size(1920, 1080, ScaleMode::Height(720)), Some((1280, 720)));
        assert_eq!(target_size(1080, 1920, ScaleMode::Height(720)), Some((720, 1280)));
        assert_eq!(target_size(1920, 1080, ScaleMode::Height(1080)), None);
        assert_eq!(target_size(1920, 1080, ScaleMode::Height(2160)), None);
        assert_eq!(target_size(1920, 1080, ScaleMode::Factor(0.25)), Some((480, 270)));
    }

    #[test]
    fn auto_walks_down_the_ladder() {
        // 1080p30 at 1 Mbit/s: 1e6 / (1920*1080*30) = 0.016 bit/px, far below 0.07
        let c = choose(ScaleMode::Auto, 1920, 1080, 30.0, 1_000_000.0, 0.07);
        assert!(c.factor < 1.0);
        assert!(c.bpp >= 0.07 || c.short_px == 240);
        // plenty of bits: untouched
        let c = choose(ScaleMode::Auto, 1280, 720, 30.0, 8_000_000.0, 0.07);
        assert_eq!(c.factor, 1.0);
    }
}
