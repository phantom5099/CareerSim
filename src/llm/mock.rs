//! 测试用模型客户端：按预设顺序返回响应，并可模拟等待取消。

use std::collections::VecDeque;
use std::sync::Mutex;

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

use super::client::ChatRequest;
use super::client::ChatResponse;
use super::client::LlmClient;
use super::client::LlmError;

enum Mode {
    Scripted,
    /// 前 `remaining_waits` 次调用等待取消，之后按脚本返回；`api_error` 决定收尾错误类型。
    WaitForCancel {
        remaining_waits: usize,
        api_error: bool,
    },
}

struct State {
    mode: Mode,
    responses: VecDeque<Result<ChatResponse, LlmError>>,
    requests: Vec<ChatRequest>,
}

pub struct MockLlmClient {
    state: Mutex<State>,
}

impl MockLlmClient {
    pub fn new(responses: Vec<ChatResponse>) -> Self {
        Self::from_results(responses.into_iter().map(Ok).collect())
    }

    pub fn from_results(responses: Vec<Result<ChatResponse, LlmError>>) -> Self {
        Self {
            state: Mutex::new(State {
                mode: Mode::Scripted,
                responses: responses.into(),
                requests: Vec::new(),
            }),
        }
    }

    pub fn wait_for_cancel() -> Self {
        Self::wait_for_cancel_with(usize::MAX, Vec::new(), false)
    }

    /// 只为下一会话锁住模型调用，之后返回脚本响应；用于验证不同会话可以并行。
    pub fn wait_once_then(responses: Vec<ChatResponse>) -> Self {
        Self::wait_for_cancel_with(1, responses, false)
    }

    /// 等待取消后返回 API 错误，模拟把客户端中止报告为请求失败的供应商。
    pub fn wait_for_cancel_api() -> Self {
        Self::wait_for_cancel_with(usize::MAX, Vec::new(), true)
    }

    fn wait_for_cancel_with(waits: usize, responses: Vec<ChatResponse>, api_error: bool) -> Self {
        let client = Self::new(responses);
        client.state.lock().expect("mock state").mode = Mode::WaitForCancel {
            remaining_waits: waits,
            api_error,
        };
        client
    }

    pub fn requests(&self) -> Vec<ChatRequest> {
        self.state.lock().expect("mock state").requests.clone()
    }
}

#[async_trait]
impl LlmClient for MockLlmClient {
    async fn complete(
        &self,
        request: ChatRequest,
        cancel: &CancellationToken,
    ) -> Result<ChatResponse, LlmError> {
        let wait = {
            let mut state = self.state.lock().expect("mock state");
            state.requests.push(request);
            match &mut state.mode {
                Mode::Scripted => None,
                Mode::WaitForCancel {
                    remaining_waits,
                    api_error,
                } => {
                    if *remaining_waits == 0 {
                        None
                    } else {
                        *remaining_waits -= 1;
                        Some(*api_error)
                    }
                }
            }
        };

        if let Some(api_error) = wait {
            cancel.cancelled().await;
            return Err(if api_error {
                LlmError::Api("request cancelled".to_owned())
            } else {
                LlmError::Cancelled
            });
        }

        let mut state = self.state.lock().expect("mock state");
        state
            .responses
            .pop_front()
            .unwrap_or_else(|| Err(LlmError::Api("no scripted response left".to_owned())))
    }
}
