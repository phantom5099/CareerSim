//! 进程级错误与错误链格式化。

/// 配置读取失败。错误信息只包含变量名或非法值，不包含 API Key。
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("missing required environment variable `{0}`")]
    Missing(&'static str),
    #[error("invalid value for `{name}`: {value}")]
    Invalid { name: &'static str, value: String },
}

/// 把错误及其 `source()` 链拼成一行，用于日志和事件日志。
pub fn error_chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut rendered = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        rendered.push_str(": ");
        rendered.push_str(&cause.to_string());
        source = cause.source();
    }
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, thiserror::Error)]
    #[error("root cause")]
    struct Root;

    #[derive(Debug, thiserror::Error)]
    #[error("transport failed")]
    struct Transport {
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    }

    #[test]
    fn error_chain_walks_optional_sources() {
        let error = Transport {
            source: Some(Box::new(Root)),
        };
        assert_eq!(error_chain(&error), "transport failed: root cause");
    }

    #[test]
    fn error_chain_without_source_is_just_display() {
        assert_eq!(error_chain(&Root), "root cause");
    }
}
