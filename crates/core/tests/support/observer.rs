use std::sync::Mutex;

use academic_core::domain::{RunId, RunProgress, RunStatus};
use academic_core::ports::RunObserver;

/// Records every progress/finished call it receives, in order.
#[derive(Default)]
pub struct FakeObserver {
    progress_calls: Mutex<Vec<RunProgress>>,
    finished_calls: Mutex<Vec<(RunId, RunStatus)>>,
}

impl FakeObserver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn progress_calls(&self) -> Vec<RunProgress> {
        self.progress_calls.lock().unwrap().clone()
    }

    pub fn finished_calls(&self) -> Vec<(RunId, RunStatus)> {
        self.finished_calls.lock().unwrap().clone()
    }
}

impl RunObserver for FakeObserver {
    fn progress(&self, progress: &RunProgress) {
        self.progress_calls.lock().unwrap().push(progress.clone());
    }

    fn finished(&self, run_id: RunId, status: RunStatus) {
        self.finished_calls.lock().unwrap().push((run_id, status));
    }
}
