//! Stepping clock for deterministic runs. Wall and monotonic readings advance
//! together and only when the scenario says so (never from the host clock), the
//! same discipline as `g9_evaluation::Harness`.

use g9_domain::{DurationMs, UtcMillis};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

/// Initial monotonic reading (a process that has been up for one second).
pub const MONOTONIC_ORIGIN_MS: u64 = 1_000;

/// Shared stepping clock; clones observe the same readings.
#[derive(Debug, Clone)]
pub struct SimClock {
    wall: Arc<AtomicI64>,
    mono: Arc<AtomicU64>,
}

impl SimClock {
    /// Clock starting at `origin`.
    pub fn new(origin: UtcMillis) -> Self {
        Self {
            wall: Arc::new(AtomicI64::new(origin.as_millis())),
            mono: Arc::new(AtomicU64::new(MONOTONIC_ORIGIN_MS)),
        }
    }

    /// Current simulated instant.
    pub fn now(&self) -> UtcMillis {
        UtcMillis::from_millis(self.wall.load(Ordering::SeqCst))
    }

    /// Current simulated monotonic reading.
    pub fn mono(&self) -> u64 {
        self.mono.load(Ordering::SeqCst)
    }

    /// Advance both readings by `d`.
    pub fn advance(&self, d: DurationMs) {
        let ms = d.as_millis();
        self.wall.fetch_add(ms, Ordering::SeqCst);
        self.mono
            .fetch_add(u64::try_from(ms).unwrap_or(0), Ordering::SeqCst);
    }

    /// Advance by whole minutes.
    pub fn advance_minutes(&self, minutes: u32) {
        self.advance(DurationMs::from_minutes_u32(minutes));
    }

    /// Move the wall clock to `at` (never backwards); the monotonic reading
    /// advances by the same positive delta.
    pub fn jump_to(&self, at: UtcMillis) {
        let now = self.now();
        if at > now {
            if let Some(delta) = at.duration_since(now) {
                self.advance(delta);
            }
        }
    }

    /// Wall-clock closure for `AppContextBuilder::clock`.
    pub fn wall_fn(&self) -> Box<dyn Fn() -> UtcMillis + Send + Sync> {
        let wall = self.wall.clone();
        Box::new(move || UtcMillis::from_millis(wall.load(Ordering::SeqCst)))
    }

    /// Monotonic closure for `AppContextBuilder::monotonic`.
    pub fn mono_fn(&self) -> Box<dyn Fn() -> u64 + Send + Sync> {
        let mono = self.mono.clone();
        Box::new(move || mono.load(Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advances_wall_and_monotonic_together_and_never_backwards() {
        let c = SimClock::new(UtcMillis::from_millis(1_000));
        c.advance_minutes(2);
        assert_eq!(c.now(), UtcMillis::from_millis(121_000));
        assert_eq!(c.mono(), MONOTONIC_ORIGIN_MS + 120_000);
        c.jump_to(UtcMillis::from_millis(5_000));
        assert_eq!(
            c.now(),
            UtcMillis::from_millis(121_000),
            "no backwards jump"
        );
        c.jump_to(UtcMillis::from_millis(200_000));
        assert_eq!(c.now(), UtcMillis::from_millis(200_000));
        assert_eq!(c.mono(), MONOTONIC_ORIGIN_MS + 199_000);
        let f = c.wall_fn();
        assert_eq!(f(), c.now());
    }
}
