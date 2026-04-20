use lgs::state::AppState as LgsState;
use parking_lot::Mutex;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::oneshot;

pub struct LgsRunning {
    pub bound_addr: SocketAddr,
    pub math_dir: String,
    pub state: Arc<LgsState>,
    pub shutdown: oneshot::Sender<()>,
    pub join: tokio::task::JoinHandle<std::io::Result<()>>,
}

#[derive(Default)]
pub struct AppState {
    pub running: Mutex<Option<LgsRunning>>,
}

impl AppState {
    pub fn new() -> Self {
        Self::default()
    }
}
