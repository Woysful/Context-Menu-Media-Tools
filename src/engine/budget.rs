//! The bitrate math of the size limit mode (pure functions).

/// The container overhead in bytes of an MP4, estimated from the frame counts
/// (measured: ~4 B per video frame, ~17 B per AAC frame, ~1 KB fixed; rounded up)
pub fn overhead(dur: f64, fps: f64, has_audio: bool) -> f64 {
    4096.0 + dur * fps * 12.0 + if has_audio { dur * 47.0 * 20.0 } else { 0.0 }
}

#[derive(Debug, PartialEq)]
pub struct Bitrates {
    pub video: u64,
    pub audio: u64,
    pub tight: bool,
}

/// Bitrates (bit/s) from the real budget in bytes:
///  * `aim`% of the limit is the target (a small reserve against rate-control error). One-pass
///    rate control needs a moment to settle, so its relative error grows on short videos:
///    the reserve gets an extra 30/duration percent (capped at 8%). Exact modes (two-pass,
///    chunked) see the real result and do not need that extra reserve.
///  * the audio bitrate scales down for very tight budgets and is 0 without audio
pub fn bitrates(mb: f64, dur: f64, fps: f64, has_audio: bool, aim: f64, exact: bool) -> Bitrates {
    let extra = if exact { 0.0 } else { (30.0 / dur).min(8.0) };
    let budget = mb * 1_048_576.0 * (aim - extra) / 100.0;
    let total = (budget - overhead(dur, fps, has_audio)) * 8.0 / dur;
    let audio = match (has_audio, total) {
        (false, _) => 0.0,
        (true, t) if t > 400_000.0 => 128_000.0,
        (true, t) if t > 150_000.0 => 96_000.0,
        _ => 64_000.0,
    };
    let video = total - audio;
    let tight = video < 50_000.0;
    Bitrates { video: video.max(50_000.0) as u64, audio: audio as u64, tight }
}

/// Chunk plan: equal chunks of about 6 s (3 to 16 of them) and a short last one.
/// Returns (number of chunks including the last, length of a normal chunk).
pub fn plan_chunks(d: f64) -> (usize, f64) {
    let last = (d / 12.0).clamp(1.0, 3.0);
    let m = (((d - last) / 6.0) + 0.5).floor().clamp(3.0, 16.0);
    (m as usize + 1, (d - last) / m)
}

/// Bitrate and requested size of one chunk. The fair share of what is left, divided by how much
/// more than requested the encoder has produced so far. Running over the limit is worse than
/// falling short, so an encoder that tends to produce less than requested is only compensated
/// a little (not at all in the last chunk, which can no longer be corrected), while one that
/// produces more is compensated fully.
pub fn chunk_target(remaining_bytes: f64, remaining_dur: f64, d: f64, sum_actual: f64, sum_req: f64, last: bool) -> (u64, f64) {
    let ratio = if sum_req > 0.0 { sum_actual / sum_req } else { 1.0 };
    let lowest = if last { 1.0 } else { 0.85 };
    let ratio = ratio.clamp(lowest, 1.6);
    let bps = (remaining_bytes / remaining_dur * 8.0 / ratio).max(50_000.0).round();
    (bps as u64, bps * d / 8.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrates_match_the_old_awk_formula() {
        // 20 MB, 60 s, 30 fps, audio, aim 99, one pass: extra reserve = 30/60 = 0.5%
        let b = bitrates(20.0, 60.0, 30.0, true, 99.0, false);
        // overhead = 4096 + 60*30*12 + 60*47*20 = 82096
        let budget = 20.0 * 1_048_576.0 * 98.5 / 100.0;
        let total = (budget - 82_096.0) * 8.0 / 60.0;
        assert_eq!(b.audio, 128_000);
        assert_eq!(b.video, (total - 128_000.0) as u64);
        assert!(!b.tight);
    }

    #[test]
    fn no_audio_no_audio_bitrate() {
        let b = bitrates(5.0, 30.0, 25.0, false, 99.0, true);
        assert_eq!(b.audio, 0);
        assert!(b.video > 1_000_000);
    }

    #[test]
    fn tight_budget_is_flagged() {
        let b = bitrates(0.1, 600.0, 30.0, true, 99.0, false);
        assert!(b.tight);
        assert_eq!(b.video, 50_000);
        assert_eq!(b.audio, 64_000);
    }

    #[test]
    fn chunk_plan() {
        // 60 s: last = 3 s, m = round(57/6) = 10 -> 11 chunks of 5.7 s + the last
        let (n, len) = plan_chunks(60.0);
        assert_eq!(n, 11);
        assert!((len - 5.7).abs() < 1e-9);
        // very short video: at least 3 + 1 chunks
        assert_eq!(plan_chunks(6.0).0, 4);
        // very long: at most 16 + 1
        assert_eq!(plan_chunks(10_000.0).0, 17);
    }

    #[test]
    fn chunk_target_compensates_an_encoder_that_overshoots() {
        // first chunk: no history
        let (bps0, req0) = chunk_target(10_000_000.0, 60.0, 6.0, 0.0, 0.0, false);
        assert_eq!(bps0, (10_000_000.0f64 / 60.0 * 8.0).round() as u64);
        assert!((req0 - bps0 as f64 * 6.0 / 8.0).abs() < 1e-6);
        // the encoder produced 20% too much so far: ask for less
        let (bps1, _) = chunk_target(9_000_000.0, 54.0, 6.0, 1_200_000.0, 1_000_000.0, false);
        let (plain, _) = chunk_target(9_000_000.0, 54.0, 6.0, 1_000_000.0, 1_000_000.0, false);
        assert!(bps1 < plain);
        // an encoder that undershoots is compensated only a little, never in the last chunk
        let (under, _) = chunk_target(9_000_000.0, 54.0, 6.0, 500_000.0, 1_000_000.0, false);
        let (under_last, _) = chunk_target(9_000_000.0, 54.0, 6.0, 500_000.0, 1_000_000.0, true);
        assert!(under > plain && under_last == plain);
    }
}
