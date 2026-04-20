use lgs::config::ServerConfig;
use lgs::start_server;
use tracing_subscriber::{EnvFilter, fmt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info")),
        )
        .init();

    let cfg = ServerConfig::from_env();
    tracing::info!(bind = %cfg.bind_addr, math_dir = %cfg.math_dir, "starting LGS");

    let handle = start_server(cfg).await?;
    tracing::info!(addr = ?handle.bound_addr, "LGS listening");

    handle.join.await??;
    Ok(())
}
