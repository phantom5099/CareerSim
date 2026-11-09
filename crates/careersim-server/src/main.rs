//! 服务入口：读取配置、装配认证与 HTTP 路由。

use std::process::ExitCode;

use careersim_server::auth::AuthService;
use careersim_server::auth::demo_user;
use careersim_server::build_router;
use careersim_server::config::AppMode;
use careersim_server::config::ServerConfig;
use careersim_server::state::AppState;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("careersim_server=info,tower_http=warn"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(error = %err, "server failed");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = ServerConfig::from_env()?;

    let auth = match (config.mode, config.demo_username, config.demo_password) {
        (AppMode::Demo, Some(username), Some(password)) => {
            tracing::warn!(
                "demo mode enabled: built-in demo account is available; never use it in production"
            );
            AuthService::demo(demo_user(&username), password, config.token_ttl)
        }
        _ => {
            tracing::warn!("live mode: account login is not configured yet");
            AuthService::disabled()
        }
    };

    let router = build_router(AppState::new(auth), &config.cors_origins);
    let listener = TcpListener::bind(config.http_addr).await?;

    tracing::info!(addr = %config.http_addr, "careersim-server listening");
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("server stopped");
    Ok(())
}

async fn shutdown_signal() {
    if let Err(err) = tokio::signal::ctrl_c().await {
        tracing::error!(error = %err, "failed to listen for shutdown signal");
    }
}
