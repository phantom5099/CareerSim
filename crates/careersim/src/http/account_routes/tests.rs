//! 账户路由测试：状态码与冻结错误体映射（plan/00 §6.2、plan/01 §6.1）。
//!
//! 通过完整路由 + oneshot 发请求，验证 AuthContext 提取、
//! `From<AccountError>` 分类与 `ApiError::status()` 映射。

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::json;

use crate::http::testing::{
    ALLOWED_ORIGIN, body_json, empty_request, first_cookie_pair, json_request, logged_in_cookie,
    send,
};

// ---------- 注册 / 登录 ----------

#[tokio::test]
async fn register_returns_created_with_user_body() {
    let response = send(
        crate::http::testing::app(),
        json_request(
            "POST",
            "/api/v1/auth/register",
            json!({ "username": "alice", "password": "password123", "displayName": "Alice" }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    assert!(first_cookie_pair(&response).is_some());
    let body = body_json(response).await;
    assert_eq!(body["username"], "alice");
    assert_eq!(body["displayName"], "Alice");
}

#[tokio::test]
async fn register_duplicate_username_maps_to_conflict() {
    let app = crate::http::testing::app();
    let payload = json!({ "username": "alice", "password": "password123", "displayName": "A" });
    send(
        app.clone(),
        json_request("POST", "/api/v1/auth/register", payload.clone()),
    )
    .await;
    let response = send(app, json_request("POST", "/api/v1/auth/register", payload)).await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "conflict");
}

#[tokio::test]
async fn login_with_wrong_password_maps_to_invalid_credentials() {
    let app = crate::http::testing::app();
    send(
        app.clone(),
        json_request(
            "POST",
            "/api/v1/auth/register",
            json!({ "username": "alice", "password": "password123", "displayName": "Alice" }),
        ),
    )
    .await;
    let response = send(
        app,
        json_request(
            "POST",
            "/api/v1/auth/login",
            json!({ "username": "alice", "password": "wrong-password" }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "invalid_credentials");
}

// ---------- 认证提取与会话路由 ----------

#[tokio::test]
async fn me_without_cookie_is_unauthenticated() {
    let response = send(
        crate::http::testing::app(),
        empty_request("GET", "/api/v1/auth/me"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "unauthenticated");
}

#[tokio::test]
async fn me_with_session_cookie_returns_current_user() {
    let app = crate::http::testing::app();
    let cookie = logged_in_cookie(&app).await;
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/auth/me")
        .header(axum::http::header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    let response = send(app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["username"], "alice");
}

#[tokio::test]
async fn me_with_garbage_cookie_is_unauthenticated() {
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/auth/me")
        .header(axum::http::header::COOKIE, "careersim_session=not-a-token")
        .body(Body::empty())
        .unwrap();
    let response = send(crate::http::testing::app(), request).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn patch_profile_updates_display_name() {
    let app = crate::http::testing::app();
    let cookie = logged_in_cookie(&app).await;
    let request = Request::builder()
        .method("PATCH")
        .uri("/api/v1/me")
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "displayName": "Alice Chen" }).to_string(),
        ))
        .unwrap();
    let response = send(app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["displayName"], "Alice Chen");
}

// ---------- 改密 / 注销 ----------

#[tokio::test]
async fn change_password_with_wrong_current_maps_to_400() {
    let app = crate::http::testing::app();
    let cookie = logged_in_cookie(&app).await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/me/password")
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "currentPassword": "wrong-password", "newPassword": "newpassword1" })
                .to_string(),
        ))
        .unwrap();
    let response = send(app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "current_password_mismatch");
}

#[tokio::test]
async fn change_password_returns_rotated_cookie() {
    let app = crate::http::testing::app();
    let cookie = logged_in_cookie(&app).await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/me/password")
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "currentPassword": "password123", "newPassword": "newpassword1" }).to_string(),
        ))
        .unwrap();
    let response = send(app, request).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(first_cookie_pair(&response).is_some());
}

#[tokio::test]
async fn logout_clears_cookie_and_revokes_session() {
    let app = crate::http::testing::app();
    let cookie = logged_in_cookie(&app).await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/auth/logout")
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie.clone())
        .body(Body::empty())
        .unwrap();
    let response = send(app.clone(), request).await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let cleared = response
        .headers()
        .get(axum::http::header::SET_COOKIE)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(cleared.starts_with("careersim_session=;"));
    assert!(cleared.contains("Max-Age=0"));

    // 注销后同一 Cookie 不再可用。
    let request = Request::builder()
        .method("GET")
        .uri("/api/v1/auth/me")
        .header(axum::http::header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    let response = send(app, request).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
