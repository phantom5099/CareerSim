//! CLI 薄壳：启动服务并提供一个最小的终端对话入口。

use std::io::Write;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use careersim::agent::AgentLoop;
use careersim::agent::DefaultContextBuilder;
use careersim::agent::LoopConfig;
use careersim::agent::SessionId;
use careersim::agent::Stage;
use careersim::agent::ToolRegistry;
use careersim::config::AppConfig;
use careersim::llm::OpenAiCompatibleClient;
use careersim::store::PostgresSessionStore;
use sqlx::postgres::PgPoolOptions;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio_util::sync::CancellationToken;
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

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = AppConfig::from_env()?;

    let pool = PgPoolOptions::new()
        .max_connections(config.db.max_connections)
        .acquire_timeout(config.db.acquire_timeout)
        .connect(&config.db.url)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    let http = reqwest::Client::builder().timeout(MODEL_TIMEOUT).build()?;

    let agent = AgentLoop::new(
        Arc::new(OpenAiCompatibleClient::new(config.llm, http)),
        Arc::new(DefaultContextBuilder),
        Arc::new(ToolRegistry::new()),
        Arc::new(PostgresSessionStore::new(pool)),
        LoopConfig {
            max_steps: config.max_steps,
        },
    );

    repl(&agent).await
}

async fn repl(agent: &AgentLoop) -> Result<(), Box<dyn std::error::Error>> {
    let stdin = tokio::io::stdin();
    let mut lines = BufReader::new(stdin).lines();
    let mut current: Option<SessionId> = None;

    println!("CareerSim AgentLoop。输入 /help 查看命令。");

    loop {
        print!("careersim> ");
        std::io::stdout().flush()?;

        let Some(line) = lines.next_line().await? else {
            break;
        };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match line {
            "/quit" => break,
            "/help" => {
                println!("/new [stage]  创建会话，默认 hr_screen");
                println!("/use <id>     切换到已有会话");
                println!("/finish       结束当前阶段并推进");
                println!("/quit         退出");
            }
            "/finish" => {
                let Some(id) = current else {
                    println!("没有活动会话，先用 /new 创建。");
                    continue;
                };
                match agent.finish_stage(id).await {
                    Ok(completion) => println!("stage advanced to {}", completion.stage),
                    Err(err) => println!("error: {err}"),
                }
            }
            _ if line.starts_with("/new") => {
                let stage = line["/new".len()..].trim();
                let stage = if stage.is_empty() {
                    Stage::HrScreen
                } else {
                    match Stage::from_str(stage) {
                        Ok(stage) => stage,
                        Err(err) => {
                            println!("error: {err}");
                            continue;
                        }
                    }
                };
                match agent.create_session(stage).await {
                    Ok(session) => {
                        println!("session {} (stage {})", session.id, session.stage);
                        current = Some(session.id);
                    }
                    Err(err) => println!("error: {err}"),
                }
            }
            _ if line.starts_with("/use") => {
                let id: SessionId = match line["/use".len()..].trim().parse() {
                    Ok(id) => id,
                    Err(err) => {
                        println!("error: {err}");
                        continue;
                    }
                };
                match agent.load_session(id).await {
                    Ok(session) => {
                        println!("session {} (stage {})", session.id, session.stage);
                        current = Some(session.id);
                    }
                    Err(err) => println!("error: {err}"),
                }
            }
            _ => {
                let Some(id) = current else {
                    println!("没有活动会话，先用 /new 创建。");
                    continue;
                };
                match agent.run_turn(id, line, CancellationToken::new()).await {
                    Ok(result) => {
                        println!("{}", result.output);
                        if let Some(err) = result.error {
                            println!("error: {err}");
                        }
                    }
                    Err(err) => println!("error: {err}"),
                }
            }
        }
    }

    Ok(())
}
