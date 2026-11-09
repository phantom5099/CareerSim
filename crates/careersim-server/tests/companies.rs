use axum::Router;
use axum::body::Body;
use axum::body::to_bytes;
use axum::http::Request;
use axum::http::StatusCode;
use careersim_server::auth::AuthService;
use careersim_server::build_router;
use careersim_server::state::AppState;
use serde_json::Value;
use tower::ServiceExt;

fn app() -> Router {
    build_router(AppState::new(AuthService::disabled()), &[])
}

#[tokio::test]
async fn industries_lists_supported_tags() {
    let response = app()
        .oneshot(
            Request::builder()
                .uri("/api/v1/companies/industries")
                .body(Body::empty())
                .expect("request should build"),
        )
        .await
        .expect("router should respond");

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should collect");
    let payload: Value = serde_json::from_slice(&bytes).expect("response should be JSON");
    let industries = payload
        .get("industries")
        .and_then(Value::as_array)
        .expect("industries should be an array");

    assert!(industries.iter().any(|tag| tag == "人工智能"));
    assert!(industries.len() > 5);

    let mut unique: Vec<&str> = industries
        .iter()
        .map(|tag| tag.as_str().expect("tags should be strings"))
        .collect();
    let total = unique.len();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), total, "industry tags should stay unique");
}
