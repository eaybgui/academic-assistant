use chrono::{DateTime, FixedOffset, Local, Utc};

use crate::domain::{RunId, RunProgress, RunStatus};

/// Source of the current time and the user's local UTC offset, so date resolution
/// (relative dates, "next monday", etc.) and timestamps are testable without relying
/// on the real system clock.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn local_offset(&self) -> FixedOffset;
}

/// [`Clock`] implementation backed by the real system clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }

    fn local_offset(&self) -> FixedOffset {
        *Local::now().offset()
    }
}

/// Receives progress updates while a check run is in flight.
pub trait RunObserver: Send + Sync {
    fn progress(&self, progress: &RunProgress);
    fn finished(&self, run_id: RunId, status: RunStatus);
}
