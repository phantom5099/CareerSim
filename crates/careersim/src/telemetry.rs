//! 统一初始化结构化诊断日志。

use std::sync::OnceLock;

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// 安装全局 tracing subscriber；`RUST_LOG` 无效或缺失时使用可诊断的默认级别。
pub fn init() -> Result<(), String> {
    let handle = PrometheusBuilder::new()
        .install_recorder()
        .map_err(|error| format!("install metrics recorder: {error}"))?;
    PROMETHEUS_HANDLE
        .set(handle)
        .map_err(|_| "metrics recorder was already initialized".to_owned())?;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("careersim=info,tower_http=info"));

    tracing_subscriber::registry()
        .with(filter)
        .with(
            fmt::layer()
                .json()
                .with_current_span(true)
                .with_span_list(true),
        )
        .init();
    Ok(())
}

/// 渲染 Prometheus 文本格式；测试环境未初始化 recorder 时返回 `None`。
pub fn render_metrics() -> Option<String> {
    PROMETHEUS_HANDLE.get().map(PrometheusHandle::render)
}
