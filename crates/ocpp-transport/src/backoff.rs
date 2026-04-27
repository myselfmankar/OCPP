use std::time::Duration;

use rand::Rng;

/// Exponential backoff with jitter (1 s → 5 min cap).
pub struct Backoff {
    attempt: u32,
}

impl Backoff {
    pub fn new() -> Self {
        Self { attempt: 0 }
    }

    pub fn reset(&mut self) {
        self.attempt = 0;
    }

    pub fn next_delay(&mut self) -> Duration {
        let base_ms = 1000u64.saturating_mul(2u64.saturating_pow(self.attempt.min(8)));
        let capped = base_ms.min(5 * 60 * 1000);
        let jitter = rand::thread_rng().gen_range(0..(capped / 4 + 1));
        self.attempt = self.attempt.saturating_add(1);
        Duration::from_millis(capped.saturating_add(jitter))
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new()
    }
}
