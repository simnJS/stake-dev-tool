use crate::math_engine::MathEngine;
use crate::session::SessionStore;
use std::sync::Arc;

pub struct AppState {
    pub sessions: Arc<SessionStore>,
    pub engine: Arc<MathEngine>,
}

impl AppState {
    pub fn new(engine: MathEngine) -> Self {
        Self {
            sessions: Arc::new(SessionStore::new()),
            engine: Arc::new(engine),
        }
    }

    pub fn from_parts(sessions: Arc<SessionStore>, engine: Arc<MathEngine>) -> Self {
        Self { sessions, engine }
    }
}
