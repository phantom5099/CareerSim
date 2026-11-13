//! multipart 解析、下载头构造与路由级 HTTP 测试。

use axum::Router;
use axum::body::Body;
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode};
use serde_json::{Value, json};

use super::parse_multipart_upload;
use super::percent_encode_file_name;
use crate::testing::{ALLOWED_ORIGIN, body_json, empty_request, registered_cookie, send};

const BOUNDARY: &str = "careersim-boundary-1234";

fn multipart_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_str(&format!("multipart/form-data; boundary={BOUNDARY}")).unwrap(),
    );
    headers
}

fn build_multipart(title: &str, file_name: &str, file_bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(b"Content-Disposition: form-data; name=\"title\"\r\n\r\n");
    body.extend_from_slice(title.as_bytes());
    body.extend_from_slice(format!("\r\n--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
             Content-Type: application/pdf\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(file_bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

/// 只带 `file` 字段的 multipart（给已有简历传附件）。
fn build_file_multipart(file_name: &str, file_bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{file_name}\"\r\n\
             Content-Type: application/pdf\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(file_bytes);
    body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
    body
}

#[test]
fn parses_title_and_file_fields() {
    let body = build_multipart("  后端简历 ", "resume.pdf", b"%PDF-bytes");
    let (title, file_name, file_bytes) =
        parse_multipart_upload(&multipart_headers(), &body).unwrap();
    assert_eq!(title, "  后端简历 ");
    assert_eq!(file_name, "resume.pdf");
    assert_eq!(file_bytes, b"%PDF-bytes");
}

#[test]
fn boundary_may_be_quoted_and_missing_boundary_is_rejected() {
    let boundary = "quoted-boundary";
    let body = {
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"title\"\r\n\r\nt");
        body.extend_from_slice(format!("\r\n--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"a.pdf\"\r\n\r\n",
        );
        body.extend_from_slice(b"pdf");
        body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
        body
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("multipart/form-data; boundary=\"quoted-boundary\""),
    );
    assert!(parse_multipart_upload(&headers, &body).is_ok());

    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    assert!(parse_multipart_upload(&headers, &body).is_err());
}

#[test]
fn missing_fields_are_rejected() {
    let title_only = {
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(b"Content-Disposition: form-data; name=\"title\"\r\n\r\nt");
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
        body
    };
    assert!(parse_multipart_upload(&multipart_headers(), &title_only).is_err());

    let file_only = {
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            "Content-Disposition: form-data; name=\"file\"; filename=\"a.pdf\"\r\n\r\n".as_bytes(),
        );
        body.extend_from_slice(b"pdf");
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
        body
    };
    assert!(parse_multipart_upload(&multipart_headers(), &file_only).is_err());
}

#[test]
fn file_name_percent_encoding_is_ascii_safe() {
    assert_eq!(percent_encode_file_name("resume.pdf"), "resume.pdf");
    assert_eq!(
        percent_encode_file_name("我的 简历.pdf"),
        "%E6%88%91%E7%9A%84%20%E7%AE%80%E5%8E%86.pdf"
    );
    let encoded = percent_encode_file_name("bad\"name\r\n.pdf");
    assert!(encoded.is_ascii());
    assert!(!encoded.contains('"'));
    assert!(!encoded.contains('\r'));
    assert!(!encoded.contains('\n'));
}

// ---------- 路由级 HTTP 测试 ----------

/// 携带 Cookie 与 Origin 的 JSON 请求。
fn authed_json(method: &str, uri: &str, cookie: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// 携带 Cookie 与 Origin 的无体请求。
fn authed_empty(method: &str, uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap()
}

fn resume_payload(title: &str) -> Value {
    json!({
        "title": title,
        "payload": { "name": "Alice", "targetRole": "后端工程师", "city": "上海" },
    })
}

/// 创建一份在线简历，返回 (状态码, 响应体)。
async fn create_resume(app: &axum::Router, cookie: &str, title: &str) -> (StatusCode, Value) {
    let response = send(
        app.clone(),
        authed_json("POST", "/api/v1/resumes", cookie, resume_payload(title)),
    )
    .await;
    let status = response.status();
    (status, body_json(response).await)
}

#[tokio::test]
async fn resume_routes_without_cookie_are_unauthenticated() {
    let response = send(
        crate::testing::app(),
        empty_request("GET", "/api/v1/resumes"),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "unauthenticated");
}

#[tokio::test]
async fn create_online_resume_returns_created_as_default() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let (status, body) = create_resume(&app, &cookie, "我的简历").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["title"], "我的简历");
    assert_eq!(body["isDefault"], true);
    assert_eq!(body["parseStatus"], "not_applicable");
    assert_eq!(body["payload"]["name"], "Alice");
    assert_eq!(body["payload"]["targetRole"], "后端工程师");
}

#[tokio::test]
async fn create_resume_with_blank_title_maps_to_validation() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let response = send(
        app,
        authed_json(
            "POST",
            "/api/v1/resumes",
            &cookie,
            json!({ "title": "   ", "payload": {} }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "validation_failed");
}

#[tokio::test]
async fn fourth_resume_maps_to_conflict() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    for index in 0..3 {
        let (status, _) = create_resume(&app, &cookie, &format!("简历{index}")).await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (status, body) = create_resume(&app, &cookie, "第四份").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "conflict");
}

#[tokio::test]
async fn resume_crud_roundtrip_preserves_unpatched_content() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let (_, created) = create_resume(&app, &cookie, "我的简历").await;
    let resume_id = created["id"].as_str().unwrap().to_owned();

    // 只改标题，payload 保持原值。
    let response = send(
        app.clone(),
        authed_json(
            "PATCH",
            &format!("/api/v1/resumes/{resume_id}"),
            &cookie,
            json!({ "title": "改过的简历" }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["title"], "改过的简历");
    assert_eq!(body["payload"]["targetRole"], "后端工程师");

    // 删除后读取 404。
    let response = send(
        app.clone(),
        authed_empty("DELETE", &format!("/api/v1/resumes/{resume_id}"), &cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = send(
        app,
        authed_empty("GET", &format!("/api/v1/resumes/{resume_id}"), &cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn set_default_switches_flag_and_list_orders_default_first() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let (_, first) = create_resume(&app, &cookie, "第一份").await;
    let (_, second) = create_resume(&app, &cookie, "第二份").await;
    assert_eq!(first["isDefault"], true);
    assert_eq!(second["isDefault"], false);
    let second_id = second["id"].as_str().unwrap().to_owned();

    let response = send(
        app.clone(),
        authed_empty(
            "PUT",
            &format!("/api/v1/resumes/{second_id}/default"),
            &cookie,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(body_json(response).await["isDefault"], true);

    // 列表：默认在前。
    let response = send(app, authed_empty("GET", "/api/v1/resumes", &cookie)).await;
    assert_eq!(response.status(), StatusCode::OK);
    let items = body_json(response).await.as_array().unwrap().to_vec();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["title"], "第二份");
    assert_eq!(items[0]["isDefault"], true);
    assert_eq!(items[1]["isDefault"], false);
}

#[tokio::test]
async fn patch_resume_of_other_owner_maps_to_not_found() {
    let app = crate::testing::app();
    let alice = registered_cookie(&app, "alice").await;
    let bob = registered_cookie(&app, "bob").await;
    let (_, created) = create_resume(&app, &alice, "Alice 的简历").await;
    let resume_id = created["id"].as_str().unwrap();

    let response = send(
        app,
        authed_json(
            "PATCH",
            &format!("/api/v1/resumes/{resume_id}"),
            &bob,
            json!({ "title": "劫持" }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------- 附件上传 ----------

/// 最小合法 PDF：扩展名 + magic bytes 双重校验都能通过。
const PDF_BYTES: &[u8] = b"%PDF-1.4\nfake-pdf-content";

#[tokio::test]
async fn upload_attachment_creates_ready_draft_resume() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/resumes/attachment")
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_str(&format!("multipart/form-data; boundary={BOUNDARY}")).unwrap(),
        )
        .body(Body::from(build_multipart(
            "附件简历",
            "resume.pdf",
            PDF_BYTES,
        )))
        .unwrap();
    let response = send(app, request).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body_json(response).await;
    assert_eq!(body["parseStatus"], "ready");
    // StaticTextExtractor 的固定文本没有可识别段头，兜底进入摘要。
    assert_eq!(
        body["payload"]["summary"],
        "提取到的简历正文：五年 Rust 后端经验"
    );
    assert_eq!(body["attachment"]["fileName"], "resume.pdf");
}

#[tokio::test]
async fn upload_with_unsupported_type_maps_to_415() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/resumes/attachment")
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_str(&format!("multipart/form-data; boundary={BOUNDARY}")).unwrap(),
        )
        .body(Body::from(build_multipart(
            "文本简历",
            "resume.txt",
            b"plain text",
        )))
        .unwrap();
    let response = send(app, request).await;
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "unsupported_media_type");
}

#[tokio::test]
async fn upload_with_magic_extension_mismatch_maps_to_415() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    // 扩展名 .pdf 但内容不是 PDF magic → 415。
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/resumes/attachment")
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_str(&format!("multipart/form-data; boundary={BOUNDARY}")).unwrap(),
        )
        .body(Body::from(build_multipart(
            "假 PDF",
            "resume.pdf",
            b"not a pdf at all",
        )))
        .unwrap();
    let response = send(app, request).await;
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
}

#[tokio::test]
async fn attach_to_existing_resume_backfills_and_keeps_filled_fields() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let (_, created) = create_resume(&app, &cookie, "我的简历").await;
    let resume_id = created["id"].as_str().unwrap().to_owned();

    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/resumes/{resume_id}/attachment"))
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, &cookie)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_str(&format!("multipart/form-data; boundary={BOUNDARY}")).unwrap(),
        )
        .body(Body::from(build_file_multipart("resume.pdf", PDF_BYTES)))
        .unwrap();
    let response = send(app, request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["id"], resume_id.as_str());
    assert_eq!(body["parseStatus"], "ready");
    assert_eq!(body["attachment"]["fileName"], "resume.pdf");
    // 已填字段不被覆盖。
    assert_eq!(body["payload"]["name"], "Alice");
    // 空字段由解析回填（兜底摘要）。
    assert_eq!(
        body["payload"]["summary"],
        "提取到的简历正文：五年 Rust 后端经验"
    );
}

#[tokio::test]
async fn remove_attachment_resets_parse_status_and_keeps_content() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let created = upload_pdf_attachment(&app, &cookie).await;
    let resume_id = created["id"].as_str().unwrap().to_owned();

    let response = send(
        app,
        authed_empty(
            "DELETE",
            &format!("/api/v1/resumes/{resume_id}/attachment"),
            &cookie,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["attachment"], Value::Null);
    assert_eq!(body["parseStatus"], "not_applicable");
    // 内容保留。
    assert_eq!(
        body["payload"]["summary"],
        "提取到的简历正文：五年 Rust 后端经验"
    );
}

// ---------- 附件下载 ----------

/// 上传一份合法 PDF 附件，返回创建出的简历 JSON。
async fn upload_pdf_attachment(app: &Router, cookie: &str) -> Value {
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/resumes/attachment")
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_str(&format!("multipart/form-data; boundary={BOUNDARY}")).unwrap(),
        )
        .body(Body::from(build_multipart(
            "附件简历",
            "resume.pdf",
            PDF_BYTES,
        )))
        .unwrap();
    let response = send(app.clone(), request).await;
    assert_eq!(response.status(), StatusCode::CREATED);
    body_json(response).await
}

#[tokio::test]
async fn download_attachment_streams_bytes_with_headers() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let created = upload_pdf_attachment(&app, &cookie).await;
    let resume_id = created["id"].as_str().unwrap();

    let response = send(
        app,
        authed_empty(
            "GET",
            &format!("/api/v1/resumes/{resume_id}/attachment"),
            &cookie,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/pdf")
    );
    let disposition = response
        .headers()
        .get(CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .unwrap();
    assert!(disposition.starts_with("attachment; filename*=UTF-8''resume.pdf"));

    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(&bytes[..], PDF_BYTES);
}

#[tokio::test]
async fn download_attachment_of_online_resume_maps_to_404() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let (status, body) = create_resume(&app, &cookie, "在线简历").await;
    assert_eq!(status, StatusCode::CREATED);
    let resume_id = body["id"].as_str().unwrap();

    // 在线简历没有附件 → 404，与简历本身存在与否不区分。
    let response = send(
        app,
        authed_empty(
            "GET",
            &format!("/api/v1/resumes/{resume_id}/attachment"),
            &cookie,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn download_attachment_of_other_owner_maps_to_404() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let created = upload_pdf_attachment(&app, &cookie).await;
    let resume_id = created["id"].as_str().unwrap();

    let other = registered_cookie(&app, "bob").await;
    let response = send(
        app,
        authed_empty(
            "GET",
            &format!("/api/v1/resumes/{resume_id}/attachment"),
            &other,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn upload_with_missing_title_maps_to_validation() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let file_only = {
        let mut body = Vec::new();
        body.extend_from_slice(format!("--{BOUNDARY}\r\n").as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"file\"; filename=\"a.pdf\"\r\n\r\n",
        );
        body.extend_from_slice(PDF_BYTES);
        body.extend_from_slice(format!("\r\n--{BOUNDARY}--\r\n").as_bytes());
        body
    };
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/resumes/attachment")
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .header(
            CONTENT_TYPE,
            HeaderValue::from_str(&format!("multipart/form-data; boundary={BOUNDARY}")).unwrap(),
        )
        .body(Body::from(file_only))
        .unwrap();
    let response = send(app, request).await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "validation_failed");
}
