use std::sync::Mutex;

use academic_core::ports::Clock;
use chrono::{DateTime, FixedOffset, Utc};

/// A [`Clock`] whose `now()` is fixed at construction (mutable via [`FakeClock::set_now`]
/// for tests that care about elapsed time) and whose `local_offset()` is fixed, so
/// tests are deterministic regardless of the machine running them.
pub struct FakeClock {
    now: Mutex<DateTime<Utc>>,
    offset: FixedOffset,
}

impl FakeClock {
    pub fn new(now: DateTime<Utc>, offset: FixedOffset) -> Self {
        Self {
            now: Mutex::new(now),
            offset,
        }
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }

    fn local_offset(&self) -> FixedOffset {
        self.offset
    }
}
