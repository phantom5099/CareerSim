//! 服务启动入口：初始化日志后交给组合根 `app::run`。

use careersim::error::error_chain;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> std::process::ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    match careersim::app::run().await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            // 错误文本可能来自 sqlx 或 reqwest，不原样输出连接串。
            tracing::error!(
                error = %redact_secrets(&error_chain(err.as_ref())),
                "startup failed"
            );
            std::process::ExitCode::FAILURE
        }
    }
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
