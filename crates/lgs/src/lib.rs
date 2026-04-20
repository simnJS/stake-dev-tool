pub mod admin;
pub mod config;
pub mod error;
pub mod math_engine;
pub mod routes;
pub mod session;
pub mod settings;
pub mod state;
pub mod tls;
pub mod types;

use crate::config::ServerConfig;
use crate::math_engine::MathEngine;
use crate::state::AppState;
use axum::http::{HeaderName, Method};
use axum_server::tls_rustls::RustlsConfig;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::Once;
use std::time::Duration;
use tokio::sync::oneshot;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::trace::TraceLayer;

pub struct ServerHandle {
    pub bound_addr: SocketAddr,
    pub shutdown: oneshot::Sender<()>,
    pub join: tokio::task::JoinHandle<std::io::Result<()>>,
}

pub fn build_router(state: Arc<AppState>, ui_dir: Option<std::path::PathBuf>) -> axum::Router {
    let cors = CorsLayer::new()
        .allow_origin(AllowOrigin::mirror_request())
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers([
            HeaderName::from_static("content-type"),
            HeaderName::from_static("authorization"),
            HeaderName::from_static("x-requested-with"),
        ])
        .allow_credentials(true)
        .allow_private_network(true)
        .expose_headers([HeaderName::from_static("content-type")]);

    let mut router = routes::router(state.clone()).merge(admin::router(state));

    if let Some(dir) = ui_dir {
        use tower_http::services::{ServeDir, ServeFile};
        let fallback = ServeFile::new(dir.join("index.html"));
        router = router.fallback_service(ServeDir::new(dir).fallback(fallback));
    }

    router.layer(cors).layer(TraceLayer::new_for_http())
}

static CRYPTO_INIT: Once = Once::new();

fn init_crypto_provider() {
    CRYPTO_INIT.call_once(|| {
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

async fn make_local_ca_tls_config() -> anyhow::Result<RustlsConfig> {
    let ca = tls::LocalCa::load_or_create().await?;
    let leaf = ca.leaf_bundle();
    let config =
        RustlsConfig::from_pem(leaf.cert_pem.into_bytes(), leaf.key_pem.into_bytes()).await?;
    Ok(config)
}

pub async fn start_server(cfg: ServerConfig) -> anyhow::Result<ServerHandle> {
    let bind = cfg.bind_addr.clone();
    let ui_dir = cfg.ui_dir.clone();
    let engine = MathEngine::new(cfg);
    let app_state = Arc::new(AppState::new(engine));
    start_server_with_state(app_state, bind, ui_dir).await
}

pub async fn start_server_with_state(
    app_state: Arc<AppState>,
    bind: String,
    ui_dir: Option<std::path::PathBuf>,
) -> anyhow::Result<ServerHandle> {
    init_crypto_provider();

    let app = build_router(app_state, ui_dir);

    let tls_config = make_local_ca_tls_config().await?;

    let std_listener = std::net::TcpListener::bind(&bind)?;
    let bound_addr = std_listener.local_addr()?;

    let handle = axum_server::Handle::new();
    let handle_for_shutdown = handle.clone();

    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
    tokio::spawn(async move {
        let _ = shutdown_rx.await;
        handle_for_shutdown.graceful_shutdown(Some(Duration::from_secs(5)));
    });

    let join = tokio::spawn(async move {
        axum_server::from_tcp_rustls(std_listener, tls_config)
            .handle(handle)
            .serve(app.into_make_service())
            .await
    });

    Ok(ServerHandle {
        bound_addr,
        shutdown: shutdown_tx,
        join,
    })
}
