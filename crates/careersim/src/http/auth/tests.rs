//! 认证相关测试：Cookie 属性拼装与 Origin 校验矩阵（plan/01 §7.2）。

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

use crate::account::session::SESSION_COOKIE;
use crate::http::auth::{clear_session_cookie_header, session_cookie_header};
use crate::http::testing::{body_json, empty_request, json_request, send};

// ---------- Cookie 属性 ----------

#[test]
fn session_cookie_header_carries_security_attributes() {
    let header = session_cookie_header("abc123", false);
    assert!(header.starts_with(&format!("{SESSION_COOKIE}=abc123")));
    assert!(header.contains("Path=/"));
    assert!(header.contains("HttpOnly"));
    assert!(header.contains("SameSite=Lax"));
    assert!(header.contains("Max-Age="));
    assert!(!header.contains("Secure"));

    let secure = session_cookie_header("abc123", true);
    assert!(secure.ends_with("; Secure"));
}

#[test]
fn clear_cookie_header_expires_immediately() {
    let header = clear_session_cookie_header(false);
    assert!(header.starts_with(&format!("{SESSION_COOKIE}=;")));
    assert!(header.contains("Max-Age=0"));

    let secure = clear_session_cookie_header(true);
    assert!(secure.ends_with("; Secure"));
}

// ---------- Origin 校验矩阵 ----------

async fn login_without_origin() -> axum::response::Response {
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "a", "password": "b"}).to_string(),
        ))
        .unwrap();
    send(crate::http::testing::app(), request).await
}

#[tokio::test]
async fn post_without_origin_is_forbidden() {
    let response = login_without_origin().await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "forbidden");
}

#[tokio::test]
async fn post_with_unknown_origin_is_forbidden() {
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/login")
        .header(axum::http::header::ORIGIN, "http://evil.example")
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({"username": "a", "password": "b"}).to_string(),
        ))
        .unwrap();
    let response = send(crate::http::testing::app(), request).await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn post_with_allowed_origin_passes_to_handler() {
    // 合法 Origin + 未注册用户：通过 Origin 检查，落到 401 而非 403。
    let response = send(
        crate::http::testing::app(),
        json_request(
            "POST",
            "/api/v1/auth/login",
            json!({"username": "nobody", "password": "password123"}),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn get_without_origin_is_allowed() {
    let response = send(
        crate::http::testing::app(),
        empty_request("GET", "/healthz"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
}
