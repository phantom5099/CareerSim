//! 请求共享状态。后续的模拟业务仓储同样从这里注入。

use crate::auth::AuthService;

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthService,
}

impl AppState {
    pub fn new(auth: AuthService) -> Self {
        Self { auth }
    }
}
