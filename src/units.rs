//! Numbers as text.

/// 20.0 -> "20", 20.5 -> "20.5" (at most four decimals)
pub fn fmt_float(v: f64) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn fmt_mb(bytes: u64) -> String {
    format!("{:.2} MB", bytes as f64 / 1_048_576.0)
}

/// 812 -> "812 B", 250_000 -> "244 KB", 3_000_000 -> "2.86 MB" (1 KB = 1024 bytes)
pub fn fmt_size(bytes: u64) -> String {
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if b < 1024.0 * 1024.0 {
        format!("{:.0} KB", b / 1024.0)
    } else {
        format!("{:.2} MB", b / 1_048_576.0)
    }
}

pub fn fmt_rate(bps: f64) -> String {
    if bps >= 1e6 {
        format!("{:.1} Mbit/s", bps / 1e6)
    } else {
        format!("{:.0} kbit/s", bps / 1e3)
    }
}

/// 6 -> "0:06", 3725 -> "1:02:05"
pub fn fmt_duration(sec: f64) -> String {
    let total = sec.max(0.0).round() as u64;
    let (h, m, s) = (total / 3600, (total / 60) % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

/// A number the user typed; a decimal comma is accepted
pub fn parse_decimal(text: &str) -> Option<f64> {
    text.trim().replace(',', ".").parse().ok()
}

/// A size the user typed: "500 KB", "1,5 MB", "500k", "2m" (a plain number is KB) -> bytes
pub fn parse_size(text: &str) -> Option<u64> {
    let t = text.trim().to_lowercase().replace(' ', "");
    let (num, unit) = match t.find(|c: char| c.is_ascii_alphabetic()) {
        Some(i) => t.split_at(i),
        None => (t.as_str(), "kb"),
    };
    let mul = match unit {
        "k" | "kb" | "kib" => 1024.0,
        "m" | "mb" | "mib" => 1024.0 * 1024.0,
        _ => return None,
    };
    let v = parse_decimal(num).filter(|v| *v > 0.0 && v.is_finite())?;
    Some((v * mul).round() as u64).filter(|b| *b > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers() {
        assert_eq!(fmt_float(20.0), "20");
        assert_eq!(fmt_float(20.5), "20.5");
        assert_eq!(fmt_float(0.1), "0.1");
        assert_eq!(fmt_duration(6.0), "0:06");
        assert_eq!(fmt_duration(3725.0), "1:02:05");
        assert_eq!(parse_decimal(" 2,5 "), Some(2.5));
        assert_eq!(parse_decimal("x"), None);
        assert_eq!(parse_size("500 KB"), Some(512_000));
        assert_eq!(parse_size("1,5mb"), Some(1_572_864));
        assert_eq!(parse_size("300"), Some(307_200));
        assert_eq!(parse_size("2m"), Some(2_097_152));
        assert_eq!(parse_size("0 KB"), None);
        assert_eq!(parse_size("5 GB"), None);
        assert_eq!(fmt_size(812), "812 B");
        assert_eq!(fmt_size(250_000), "244 KB");
        assert_eq!(fmt_size(3_000_000), "2.86 MB");
    }
}
