use crate::math_engine::MathEngine;
use crate::session::SessionStore;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForcedEvent {
    pub mode: String,
    #[serde(rename = "eventId")]
    pub event_id: u32,
}

pub struct AppState {
    pub sessions: Arc<SessionStore>,
    pub engine: Arc<MathEngine>,
    /// When set, `/play` calls with matching `mode` bypass the RNG and return
    /// this exact event. Cleared via the `/api/devtool/force-event` endpoint.
    pub forced_event: Mutex<Option<ForcedEvent>>,
}

impl AppState {
    pub fn new(engine: MathEngine) -> Self {
        Self {
            sessions: Arc::new(SessionStore::new()),
            engine: Arc::new(engine),
            forced_event: Mutex::new(None),
        }
    }

    pub fn from_parts(sessions: Arc<SessionStore>, engine: Arc<MathEngine>) -> Self {
        Self {
            sessions,
            engine,
            forced_event: Mutex::new(None),
        }
    }
}
