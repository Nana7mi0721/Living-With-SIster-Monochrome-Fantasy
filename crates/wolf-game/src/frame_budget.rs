//! Frame-budget timing for a real Game::frame loop.
//!
//! The packaging task asks for a p50/p95/max microsecond measurement of the
//! game loop. The logic lives here, in wolf-game, rather than in the CLI so the
//! same measurement can be reused by any front end without adding a clock to
//! Game itself. Nothing in Game reads this module; callers opt in by handing a
//! live Game to measure_game.

use std::time::Instant;

use crate::game::Game;

/// One frame-loop benchmark result. All times are microseconds.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FrameBudgetReport {
    /// Number of measured frames.
    pub frames: usize,
    /// Sum of every measured frame.
    pub total_us: f64,
    /// Arithmetic mean.
    pub mean_us: f64,
    /// Median (nearest-rank 50th percentile).
    pub p50_us: f64,
    /// 95th percentile (nearest-rank).
    pub p95_us: f64,
    /// Slowest measured frame.
    pub max_us: f64,
}

impl FrameBudgetReport {
    /// Build a report from nanosecond samples. Public so a caller that already
    /// owns a timing loop can share the percentile code.
    pub fn from_nanos(mut samples: Vec<u64>) -> Self {
        let frames = samples.len();
        if frames == 0 {
            return Self::default();
        }
        samples.sort_unstable();
        let total_ns: u64 = samples.iter().sum();
        let total_us = total_ns as f64 / 1_000.0;
        Self {
            frames,
            total_us,
            mean_us: total_us / frames as f64,
            p50_us: percentile_us(&samples, 50),
            p95_us: percentile_us(&samples, 95),
            max_us: *samples.last().unwrap() as f64 / 1_000.0,
        }
    }

    /// One stable, easy-to-parse line for logs and docs.
    pub fn summary_line(&self) -> String {
        format!(
            "frame-budget: frames={} p50_us={:.1} p95_us={:.1} max_us={:.1} mean_us={:.1} total_us={:.1}",
            self.frames, self.p50_us, self.p95_us, self.max_us, self.mean_us, self.total_us
        )
    }
}

/// Nearest-rank percentile over an ascending sample slice.
fn percentile_us(sorted: &[u64], percent: usize) -> f64 {
    debug_assert!(!sorted.is_empty());
    let rank = (percent * sorted.len()).div_ceil(100);
    let index = rank.saturating_sub(1).min(sorted.len() - 1);
    sorted[index] as f64 / 1_000.0
}

/// Time a caller-supplied step closure for the requested number of frames.
/// Warm-up is intentionally the caller's job so the measured count is exactly
/// the frames reported.
pub fn measure<F: FnMut()>(frames: usize, mut step: F) -> FrameBudgetReport {
    let mut samples = Vec::with_capacity(frames);
    for _ in 0..frames {
        let start = Instant::now();
        step();
        samples.push(start.elapsed().as_nanos() as u64);
    }
    FrameBudgetReport::from_nanos(samples)
}

/// Time a real Game::frame loop. The caller owns all setup (save load, event
/// start, auto-input) so this function stays a pure timer around Game::frame.
pub fn measure_game(game: &mut Game, frames: usize) -> FrameBudgetReport {
    measure(frames, || {
        let _ = game.frame();
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_are_monotonic_and_keep_the_sample_count() {
        let report = FrameBudgetReport::from_nanos(vec![5_000, 1_000, 3_000, 2_000, 4_000]);
        assert_eq!(report.frames, 5);
        assert_eq!(report.p50_us, 3.0);
        assert_eq!(report.p95_us, 5.0);
        assert_eq!(report.max_us, 5.0);
        assert!(report.p50_us <= report.p95_us);
        assert!(report.p95_us <= report.max_us);
    }

    #[test]
    fn empty_input_is_all_zero() {
        let report = FrameBudgetReport::from_nanos(Vec::new());
        assert_eq!(report.frames, 0);
        assert_eq!(report.max_us, 0.0);
    }
}
