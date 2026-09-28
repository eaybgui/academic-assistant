//! Enforces "only one check run in progress at a time" and exposes which run (if
//! any) is currently running, without ever panicking on a poisoned mutex.

use std::sync::{Arc, Mutex, MutexGuard};

use crate::domain::RunId;

/// Shared, cloneable state: whether a run is currently in progress and, once
/// known, its [`RunId`].
#[derive(Default)]
struct LockState {
    busy: bool,
    run_id: Option<RunId>,
}

/// The single-run lock owned by [`super::CheckRunner`]. Cloning shares the same
/// underlying state (cheap: an `Arc<Mutex<_>>` handle).
#[derive(Clone, Default)]
pub(super) struct RunLock {
    state: Arc<Mutex<LockState>>,
}

impl RunLock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reserves the lock. Returns `None` if a run is already in progress.
    ///
    /// The returned guard reports no run id (see [`RunLock::running_run`]) until
    /// [`RunLockGuard::set_run_id`] is called, and releases the lock as soon as it
    /// is dropped — including on every early-return path, since the guard is a
    /// plain value that is dropped when it goes out of scope.
    pub fn try_acquire(&self) -> Option<RunLockGuard> {
        let mut state = lock(&self.state);
        if state.busy {
            return None;
        }
        state.busy = true;
        drop(state);
        Some(RunLockGuard {
            state: self.state.clone(),
        })
    }

    /// The id of the run currently in progress, if any. `None` both when idle and
    /// during the brief window between reserving the lock and the run row being
    /// created (see [`RunLockGuard::set_run_id`]).
    pub fn running_run(&self) -> Option<RunId> {
        lock(&self.state).run_id
    }
}

/// RAII handle for a reserved [`RunLock`]. Releases the lock on drop, whether the
/// holder finishes normally, returns early, or the future is dropped.
pub(super) struct RunLockGuard {
    state: Arc<Mutex<LockState>>,
}

impl RunLockGuard {
    /// Records the id of the run row this reservation is for, so
    /// [`RunLock::running_run`] reports it while the run is in progress.
    pub fn set_run_id(&self, run_id: RunId) {
        lock(&self.state).run_id = Some(run_id);
    }
}

impl Drop for RunLockGuard {
    fn drop(&mut self) {
        let mut state = lock(&self.state);
        state.busy = false;
        state.run_id = None;
    }
}

/// Locks `mutex`, recovering the inner value rather than panicking if a previous
/// holder panicked while holding it (a poisoned lock's data is otherwise still
/// consistent for our purposes: plain `Copy` fields).
fn lock(mutex: &Mutex<LockState>) -> MutexGuard<'_, LockState> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_acquire_fails_while_first_guard_is_held() {
        let lock = RunLock::new();
        let _first = lock.try_acquire().expect("first acquire succeeds");
        assert!(lock.try_acquire().is_none());
    }

    #[test]
    fn dropping_the_guard_releases_the_lock() {
        let lock = RunLock::new();
        {
            let _guard = lock.try_acquire().expect("first acquire succeeds");
        }
        assert!(lock.try_acquire().is_some());
    }

    #[test]
    fn running_run_reports_the_id_set_on_the_guard_and_clears_it_on_drop() {
        let lock = RunLock::new();
        assert_eq!(lock.running_run(), None);
        {
            let guard = lock.try_acquire().expect("acquire succeeds");
            assert_eq!(lock.running_run(), None);
            guard.set_run_id(42);
            assert_eq!(lock.running_run(), Some(42));
        }
        assert_eq!(lock.running_run(), None);
    }
}
