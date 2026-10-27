//! Numbers as text.

/// 20.0 -> "20", 20.5 -> "20.5" (at most four decimals)
pub fn fmt_float(v: f64) -> String {
    let s = format!("{v:.4}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub fn fmt_mb(bytes: u64) -> String {
    format!("{:.2} MB", bytes as f64 / 1_048_576.0)
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
    }
}
