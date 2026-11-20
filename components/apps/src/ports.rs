//! What the app platform needs from the server: the shared state and its own execution limits.
use crate::service::limits::ServiceLimits;
use std::sync::Arc;
use veyndo_core::State;

/// The state of the app platform; it dereferences to the shared database and HTTP state.
#[derive(Clone)]
pub struct Platform {
    pub core: State,
    pub limits: Arc<ServiceLimits>,
}
impl std::ops::Deref for Platform {
    type Target = State;
    fn deref(&self) -> &State {
        &self.core
    }
}
