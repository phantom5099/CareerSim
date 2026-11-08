//! 服务启动入口：装配配置、数据库与 AgentLoop；当前不提供交互界面。

use std::sync::Arc;
use std::time::Duration;

use careersim::agent::AgentLoop;
use careersim::agent::DefaultContextBuilder;
use careersim::agent::LoopConfig;
use careersim::agent::ToolRegistry;
use careersim::config::AppConfig;
use careersim::llm::OpenAiCompatibleClient;
use careersim::store::PostgresSessionStore;
use sqlx::postgres::PgPoolOptions;
use tracing_subscriber::EnvFilter;

const MODEL_TIMEOUT: Duration = Duration::from_secs(120);

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    match run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            // 错误文本可能来自 sqlx 或 reqwest，不原样输出连接串。
            tracing::error!(error = %redact_secrets(&err.to_string()), "startup failed");
            std::process::ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(config.db.max_connections)
        .acquire_timeout(config.db.acquire_timeout)
        .connect(&config.db.url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let http = reqwest::Client::builder().timeout(MODEL_TIMEOUT).build()?;

    let _agent = AgentLoop::new(
        Arc::new(OpenAiCompatibleClient::new(config.llm, http)),
        Arc::new(DefaultContextBuilder),
        Arc::new(ToolRegistry::new()),
        Arc::new(PostgresSessionStore::new(pool)),
        LoopConfig {
            max_steps: config.max_steps,
        },
    );

    tracing::info!("agent loop initialized; waiting for shutdown");
    tokio::signal::ctrl_c().await?;
    tracing::info!("shutdown signal received");
    Ok(())
}

/// 去掉错误信息中的连接串凭据，保留其余诊断内容。
fn redact_secrets(message: &str) -> String {
    const SCHEMES: [&str; 2] = ["postgres://", "postgresql://"];
    let mut result = String::with_capacity(message.len());
    let mut rest = message;

    while let Some((index, scheme)) = SCHEMES
        .iter()
        .filter_map(|scheme| rest.find(scheme).map(|index| (index, *scheme)))
        .min_by_key(|(index, _)| *index)
    {
        result.push_str(&rest[..index + scheme.len()]);
        let tail = &rest[index + scheme.len()..];
        let end = tail
            .find(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | ')' | ']' | '>'))
            .unwrap_or(tail.len());
        result.push_str("<redacted>");
        rest = &tail[end..];
    }

    result.push_str(rest);
    result
}
