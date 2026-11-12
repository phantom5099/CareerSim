//! HTTP 层测试共用装配：内存账户服务 + 完整路由 + oneshot 请求。

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::account::doubles::{FakeClock, FakeHasher, InMemorySessionStore, InMemoryUserStore};
use crate::account::service::AccountService;
use crate::http::router::{AppState, router};

pub(crate) const ALLOWED_ORIGIN: &str = "http://localhost:5173";

/// 组装带内存账户服务的完整应用路由。
pub(crate) fn app() -> Router {
    let service = AccountService::new(
        Arc::new(InMemoryUserStore::new()),
        Arc::new(InMemorySessionStore::new()),
        Arc::new(FakeHasher::new()),
        Arc::new(FakeClock::new()),
    );
    let state = AppState {
        account: Arc::new(service),
        allowed_origins: Arc::from([ALLOWED_ORIGIN.to_owned()]),
        cookie_secure: false,
    };
    router(state)
}

/// JSON 请求；自动携带允许的 Origin 与 Content-Type。
pub(crate) fn json_request(method: &str, uri: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// 无请求体的请求；不携带 Origin 与 Cookie。
pub(crate) fn empty_request(method: &str, uri: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

pub(crate) async fn send(app: Router, request: Request<Body>) -> Response {
    app.oneshot(request).await.unwrap()
}

pub(crate) async fn body_json(response: Response) -> Value {
    let bytes = axum::body::to_bytes(response.into_body(), 64 * 1024)
        .await
        .unwrap();
    serde_json::from_slice(&bytes).unwrap_or(Value::Null)
}

/// 取 Set-Cookie 中第一个 `名=值` 对（不含属性）；空值对（清除 Cookie）返回 None。
pub(crate) fn first_cookie_pair(response: &Response) -> Option<String> {
    response
        .headers()
        .get(axum::http::header::SET_COOKIE)?
        .to_str()
        .ok()
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|pair| !pair.ends_with('='))
        .map(str::to_owned)
}

/// 注册并登录，返回可复用的 Cookie 对（`careersim_session=...`）。
pub(crate) async fn logged_in_cookie(app: &Router) -> String {
    let response = send(
        app.clone(),
        json_request(
            "POST",
            "/api/v1/auth/register",
            json!({ "username": "alice", "password": "password123", "displayName": "Alice" }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    first_cookie_pair(&response).expect("register sets session cookie")
}
