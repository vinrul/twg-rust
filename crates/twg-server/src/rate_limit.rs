use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::RateLimitConfig;

#[derive(Clone)]
pub struct RateLimiter {
    inner: Arc<Mutex<HashMap<String, Window>>>,
    attempts: u32,
    window: Duration,
}

#[derive(Debug, Clone, Copy)]
struct Window {
    started: Instant,
    count: u32,
}

impl RateLimiter {
    pub fn new(config: &RateLimitConfig) -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            attempts: config.attempts,
            window: config.window,
        }
    }

    pub fn disabled() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
            attempts: 0,
            window: Duration::from_secs(60),
        }
    }

    /// Returns the retry delay when the key exceeded its window budget.
    pub fn check(&self, key: &str) -> Result<(), Duration> {
        self.check_at(key, Instant::now())
    }

    fn check_at(&self, key: &str, now: Instant) -> Result<(), Duration> {
        if self.attempts == 0 {
            return Ok(());
        }
        let mut inner = self.inner.lock().expect("rate limiter poisoned");
        if inner.len() > 4_096 {
            inner.retain(|_, window| now.duration_since(window.started) < self.window);
        }
        let window = inner.entry(key.to_owned()).or_insert(Window {
            started: now,
            count: 0,
        });
        let elapsed = now.duration_since(window.started);
        if elapsed >= self.window {
            window.started = now;
            window.count = 0;
        }
        if window.count >= self.attempts {
            return Err(self.window.saturating_sub(elapsed));
        }
        window.count += 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::RateLimiter;
    use crate::config::RateLimitConfig;

    fn limiter(attempts: u32, seconds: u64) -> RateLimiter {
        RateLimiter::new(&RateLimitConfig {
            attempts,
            window: Duration::from_secs(seconds),
        })
    }

    #[test]
    fn blocks_after_the_budget_is_spent() {
        let limiter = limiter(2, 60);
        let now = Instant::now();
        assert!(limiter.check_at("ip", now).is_ok());
        assert!(limiter.check_at("ip", now).is_ok());
        assert!(limiter.check_at("ip", now).is_err());
        assert!(limiter.check_at("other", now).is_ok());
    }

    #[test]
    fn resets_after_the_window() {
        let limiter = limiter(1, 60);
        let now = Instant::now();
        assert!(limiter.check_at("ip", now).is_ok());
        assert!(limiter.check_at("ip", now).is_err());
        assert!(
            limiter
                .check_at("ip", now + Duration::from_secs(61))
                .is_ok()
        );
    }

    #[test]
    fn zero_attempts_disables_the_limiter() {
        let limiter = limiter(0, 60);
        for _ in 0..100 {
            assert!(limiter.check("ip").is_ok());
        }
    }
}
