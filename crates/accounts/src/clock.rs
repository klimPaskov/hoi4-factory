//! Time source. Everything that expires reads time from a [`Clock`], so tests can move it.

use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};

/// Source of the current time.
pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> DateTime<Utc>;
}

/// The real wall clock.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// A clock that only moves when told to. Used by tests and by callers that need determinism.
#[derive(Debug)]
pub struct ManualClock {
    now: Mutex<DateTime<Utc>>,
}

impl ManualClock {
    pub fn new(start: DateTime<Utc>) -> Self {
        Self {
            now: Mutex::new(start),
        }
    }

    /// Starts at the current wall-clock time.
    pub fn starting_now() -> Self {
        Self::new(Utc::now())
    }

    pub fn advance(&self, by: Duration) {
        let mut guard = self.now.lock().unwrap_or_else(|e| e.into_inner());
        *guard += by;
    }

    pub fn set(&self, to: DateTime<Utc>) {
        *self.now.lock().unwrap_or_else(|e| e.into_inner()) = to;
    }
}

impl Clock for ManualClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap_or_else(|e| e.into_inner())
    }
}
