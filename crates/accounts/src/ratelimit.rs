//! Rate limiting behind a trait, with an in-memory token bucket implementation.
//!
//! A multi-instance deployment replaces [`InMemoryRateLimiter`] with a shared implementation of
//! [`RateLimiter`]. Each instance of this one counts separately.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::clock::Clock;

pub const SIGNIN_IP: &str = "signin_ip";
pub const SIGNIN_ACCOUNT: &str = "signin_account";
pub const SIGNUP_IP: &str = "signup_ip";
pub const SIGNUP_EMAIL: &str = "signup_email";
pub const RESEND_ACCOUNT: &str = "resend_account";
pub const RESET_IP: &str = "reset_ip";
pub const RESET_EMAIL: &str = "reset_email";

/// Admission control keyed by a rule name and a subject (an IP address, an email address).
pub trait RateLimiter: Send + Sync + 'static {
    /// Consumes one unit for `key` under `rule`. `Err` carries the time until a retry can succeed.
    fn check(&self, rule: &str, key: &str) -> Result<(), Duration>;
}

/// A bucket holds up to `capacity` units and regains one every `refill_every`.
#[derive(Debug, Clone, Copy)]
pub struct Rule {
    pub capacity: u32,
    pub refill_every: Duration,
}

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub rules: HashMap<String, Rule>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        let rule = |capacity, secs| Rule {
            capacity,
            refill_every: Duration::from_secs(secs),
        };
        let rules = [
            // Per source address: a burst of 20, then one attempt every 3 seconds.
            (SIGNIN_IP, rule(20, 3)),
            // Per target account: a burst of 8, then one attempt every 20 seconds.
            (SIGNIN_ACCOUNT, rule(8, 20)),
            (SIGNUP_IP, rule(5, 60)),
            (SIGNUP_EMAIL, rule(3, 600)),
            (RESEND_ACCOUNT, rule(3, 300)),
            (RESET_IP, rule(5, 60)),
            (RESET_EMAIL, rule(3, 600)),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        Self { rules }
    }
}

#[derive(Debug)]
struct Bucket {
    tokens: f64,
    updated: DateTime<Utc>,
}

pub struct InMemoryRateLimiter {
    clock: Arc<dyn Clock>,
    rules: HashMap<String, Rule>,
    buckets: Mutex<HashMap<String, Bucket>>,
}

const MAX_TRACKED_KEYS: usize = 50_000;

impl InMemoryRateLimiter {
    pub fn new(clock: Arc<dyn Clock>, config: &RateLimitConfig) -> Self {
        Self {
            clock,
            rules: config.rules.clone(),
            buckets: Mutex::new(HashMap::new()),
        }
    }

    fn refilled(rule: &Rule, bucket: &Bucket, now: DateTime<Utc>) -> f64 {
        let elapsed = (now - bucket.updated).num_milliseconds().max(0) as f64 / 1000.0;
        let per_token = rule.refill_every.as_secs_f64().max(f64::EPSILON);
        (bucket.tokens + elapsed / per_token).min(f64::from(rule.capacity))
    }
}

impl RateLimiter for InMemoryRateLimiter {
    fn check(&self, rule_name: &str, key: &str) -> Result<(), Duration> {
        let Some(rule) = self.rules.get(rule_name) else {
            return Ok(());
        };
        let now = self.clock.now();
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        if buckets.len() >= MAX_TRACKED_KEYS {
            // Forget buckets that have refilled completely; they carry no information.
            buckets.retain(|k, b| {
                let rule_name = k.split('\u{0}').next().unwrap_or_default();
                match self.rules.get(rule_name) {
                    Some(r) => Self::refilled(r, b, now) < f64::from(r.capacity),
                    None => false,
                }
            });
        }
        let bucket = buckets
            .entry(format!("{rule_name}\u{0}{key}"))
            .or_insert_with(|| Bucket {
                tokens: f64::from(rule.capacity),
                updated: now,
            });
        let available = Self::refilled(rule, bucket, now);
        bucket.updated = now;
        if available >= 1.0 {
            bucket.tokens = available - 1.0;
            Ok(())
        } else {
            bucket.tokens = available;
            let missing = 1.0 - available;
            Err(Duration::from_secs_f64(
                missing * rule.refill_every.as_secs_f64(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::ManualClock;

    fn limiter(clock: Arc<ManualClock>) -> InMemoryRateLimiter {
        let mut cfg = RateLimitConfig::default();
        cfg.rules.insert(
            "t".to_owned(),
            Rule {
                capacity: 2,
                refill_every: Duration::from_secs(10),
            },
        );
        InMemoryRateLimiter::new(clock, &cfg)
    }

    #[test]
    fn bucket_empties_refills_and_isolates_keys() {
        let clock = Arc::new(ManualClock::starting_now());
        let l = limiter(clock.clone());
        assert!(l.check("t", "a").is_ok());
        assert!(l.check("t", "a").is_ok());
        let wait = l.check("t", "a").unwrap_err();
        assert!(
            wait > Duration::from_secs(9) && wait <= Duration::from_secs(10),
            "{wait:?}"
        );
        // Another key and another rule are unaffected.
        assert!(l.check("t", "b").is_ok());
        assert!(l.check(SIGNIN_IP, "a").is_ok());
        // Half a refill is not enough, a full one is.
        clock.advance(chrono::Duration::seconds(5));
        assert!(l.check("t", "a").is_err());
        clock.advance(chrono::Duration::seconds(5));
        assert!(l.check("t", "a").is_ok());
        assert!(l.check("t", "a").is_err());
    }

    #[test]
    fn capacity_is_capped_after_a_long_pause() {
        let clock = Arc::new(ManualClock::starting_now());
        let l = limiter(clock.clone());
        clock.advance(chrono::Duration::hours(5));
        assert!(l.check("t", "a").is_ok());
        assert!(l.check("t", "a").is_ok());
        assert!(l.check("t", "a").is_err());
    }

    #[test]
    fn unknown_rules_do_not_limit() {
        let clock = Arc::new(ManualClock::starting_now());
        let l = limiter(clock);
        for _ in 0..100 {
            assert!(l.check("nope", "a").is_ok());
        }
    }
}
