//! In-memory fakes for every port `core::pipeline::CheckRunner` depends on, used
//! by `tests/pipeline.rs`. Every fake uses interior mutability (`Mutex`) so it can
//! be shared behind an `Arc<dyn Trait>` while still recording the calls it
//! received, for tests to assert on afterwards.

mod clock;
mod fixtures;
mod model;
mod observer;
mod provider;
mod store;

pub use clock::FakeClock;
pub use fixtures::*;
pub use model::{FakeModelError, FakeModelProvider, FakeStructuredModel};
pub use observer::FakeObserver;
pub use provider::{FakeEmailProviderFactory, FakeProviderError};
pub use store::TestStore;
