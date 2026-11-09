use axum::Router;
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::HeaderMap;
use axum::http::Method;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header;
use careersim_server::auth::AuthService;
use careersim_server::auth::demo_user;
use careersim_server::build_router;
use careersim_server::state::AppState;
use chrono::Duration;
use serde_json::Value;
use serde_json::json;
use tower::ServiceExt;

fn app() -> Router {
    build_router(
        AppState::new(AuthService::demo(
            demo_user("admin"),
            "123456",
            Duration::days(1),
        )),
        &[],
    )
}

fn disabled_app() -> Router {
    build_router(AppState::new(AuthService::disabled()), &[])
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    bytes: Vec<u8>,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_slice(&self.bytes).expect("response should be JSON")
    }
}

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    token: Option<&str>,
    body: Option<Value>,
) -> Reply {
    let mut builder = Request::builder().method(method).uri(uri);
    if body.is_some() {
        builder = builder.header(header::CONTENT_TYPE, "application/json");
    }
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }

    let request = builder
        .body(body.map_or_else(Body::empty, |value| Body::from(value.to_string())))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();

    Reply {
        status,
        headers,
        bytes: bytes.to_vec(),
    }
}

async fn login(app: &Router) -> Value {
    let reply = send(
        app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({ "username": "admin", "password": "123456" })),
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json());
    reply.json()
}

#[tokio::test]
async fn login_returns_token_and_profile() {
    let app = app();
    let reply = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({ "username": "admin", "password": "123456" })),
    )
    .await;

    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.json();
    assert!(body["token"].as_str().unwrap().starts_with("cst_"));
    assert_eq!(body["user"]["username"], "admin");
    assert_eq!(body["user"]["displayName"], "林同学");
    assert!(body["expiresAt"].as_str().unwrap().ends_with('Z'));
    assert!(!reply.bytes.windows(6).any(|window| window == b"123456"));
    assert!(reply.headers.contains_key("x-request-id"));
}

#[tokio::test]
async fn login_rejects_wrong_password() {
    let app = app();
    let reply = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({ "username": "admin", "password": "wrong" })),
    )
    .await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    let body = reply.json();
    assert_eq!(body["error"]["code"], "INVALID_CREDENTIALS");
    assert_eq!(
        body["error"]["requestId"],
        reply.headers["x-request-id"].to_str().unwrap()
    );
}

#[tokio::test]
async fn login_validates_empty_fields() {
    let app = app();
    let reply = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({ "username": "  ", "password": "123456" })),
    )
    .await;

    assert_eq!(reply.status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(reply.json()["error"]["code"], "VALIDATION_FAILED");
}

#[tokio::test]
async fn login_maps_malformed_json_to_validation_error() {
    let app = app();
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{"))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["error"]["code"], "VALIDATION_FAILED");
}

#[tokio::test]
async fn me_requires_bearer_token() {
    let app = app();
    let reply = send(&app, Method::GET, "/api/v1/auth/me", None, None).await;

    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(reply.json()["error"]["code"], "UNAUTHORIZED");
}

#[tokio::test]
async fn me_returns_profile_for_valid_token() {
    let app = app();
    let token = login(&app).await["token"].as_str().unwrap().to_owned();

    let reply = send(&app, Method::GET, "/api/v1/auth/me", Some(&token), None).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json()["username"], "admin");
}

#[tokio::test]
async fn logout_invalidates_token() {
    let app = app();
    let token = login(&app).await["token"].as_str().unwrap().to_owned();

    let reply = send(
        &app,
        Method::POST,
        "/api/v1/auth/logout",
        Some(&token),
        None,
    )
    .await;
    assert_eq!(reply.status, StatusCode::NO_CONTENT);

    let reply = send(&app, Method::GET, "/api/v1/auth/me", Some(&token), None).await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn live_mode_disables_demo_login() {
    let app = disabled_app();
    let reply = send(
        &app,
        Method::POST,
        "/api/v1/auth/login",
        None,
        Some(json!({ "username": "admin", "password": "123456" })),
    )
    .await;

    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reply.json()["error"]["code"], "AUTH_UNAVAILABLE");
}

#[tokio::test]
async fn unknown_route_uses_error_envelope() {
    let app = app();
    let reply = send(&app, Method::GET, "/api/v1/nope", None, None).await;

    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.json()["error"]["code"], "NOT_FOUND");
}

#[tokio::test]
async fn health_endpoints_are_public() {
    let app = app();
    let reply = send(&app, Method::GET, "/healthz", None, None).await;

    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.json()["status"], "ok");
}
