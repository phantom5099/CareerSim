//! multipart 解析与下载头构造的单元测试。

use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, HeaderValue};

use super::parse_multipart_upload;
use super::percent_encode_file_name;

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
