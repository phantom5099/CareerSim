//! 组合根：读配置、建连接池、装配适配器与路由，启动 HTTP 服务。

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use tokio::net::TcpListener;

use crate::account::password::Argon2idHasher;
use crate::account::postgres_account_service;
use crate::agent::{AgentLoop, DefaultContextBuilder, LoopConfig, ToolRegistry};
use crate::config::AppConfig;
use crate::http::router::{AppState, router};
use crate::llm::OpenAiCompatibleClient;
use crate::postgres_session_service;

const MODEL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

/// 启动服务；成功时阻塞到收到 Ctrl-C。
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(config.db.max_connections)
        .acquire_timeout(config.db.acquire_timeout)
        .connect(&config.db.url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let http = reqwest::Client::builder().timeout(MODEL_TIMEOUT).build()?;

    // AgentLoop 先按原装配保留；投递编排接入属 02/04 方案。
    let _agent = AgentLoop::new(
        Arc::new(OpenAiCompatibleClient::new(config.llm, http)),
        Arc::new(DefaultContextBuilder),
        Arc::new(ToolRegistry::new()),
        Arc::new(postgres_session_service(pool.clone())),
        LoopConfig {
            max_steps: config.max_steps,
        },
    );

    let hasher = Argon2idHasher::with_params(
        config.argon2.m_cost_kib,
        config.argon2.t_cost,
        config.argon2.p_cost,
    )?;
    let account = postgres_account_service(pool, Arc::new(hasher));

    let state = AppState {
        account: Arc::new(account),
        allowed_origins: config.http.allowed_origins.clone().into(),
        cookie_secure: config.http.cookie_secure,
    };
    let app = router(state);

    let listener = TcpListener::bind(&config.http.bind).await?;
    tracing::info!(bind = %config.http.bind, "http server listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

/// Ctrl-C 或 SIGTERM 后开始优雅停机。
async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(err) => {
                tracing::warn!(error = %err, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }
    tracing::info!("shutdown signal received");
}
