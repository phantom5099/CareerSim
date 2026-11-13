//! 简历路由处理器：在线简历 CRUD、附件上传/下载（plan/01 §6.3、§8）。

use axum::Json;
use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};
use tower_http::limit::RequestBodyLimitLayer;

use crate::resume::content::ResumeContent;
use crate::resume::model::{ParseStatus, Resume, ResumeError, ResumeId};
use crate::resume::service::OpenedAttachment;

use super::auth::AuthContext;
use super::error::ApiError;
use super::router::AppState;

/// JSON 请求体上限：与全局一致（1 MiB）。
const JSON_BODY_LIMIT_BYTES: usize = 1024 * 1024;
/// 上传请求体上限：10 MiB 附件 + multipart 开销余量（plan/01 §8.1）。
const UPLOAD_BODY_LIMIT_BYTES: usize = 12 * 1024 * 1024;

// ---------- DTO ----------

/// 附件展示信息；MIME 由文件名后缀决定，无需对外暴露（plan/01 §6.3）。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentDto {
    pub file_name: String,
    pub file_size: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeDto {
    pub id: String,
    pub title: String,
    pub is_default: bool,
    pub parse_status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error_code: Option<&'static str>,
    /// 对外统一叫 `payload`，与创建/更新请求的 `payload` 字段对称。
    #[serde(rename = "payload")]
    pub content: ResumeContent,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub attachment: Option<AttachmentDto>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Resume> for ResumeDto {
    fn from(resume: Resume) -> Self {
        let Resume {
            id,
            title,
            is_default,
            content,
            attachment,
            parse_status,
            created_at,
            updated_at,
            ..
        } = resume;
        let parse_error_code = match &parse_status {
            ParseStatus::Failed { code } => Some(code.as_str()),
            _ => None,
        };
        Self {
            id: id.into_uuid().to_string(),
            title,
            is_default,
            parse_status: parse_status.as_str(),
            parse_error_code,
            content,
            attachment: attachment.map(|att| AttachmentDto {
                file_name: att.file_name,
                file_size: att.size,
            }),
            created_at: created_at.to_rfc3339_opts(SecondsFormat::Millis, true),
            updated_at: updated_at.to_rfc3339_opts(SecondsFormat::Millis, true),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateResumeRequest {
    pub title: String,
    pub payload: ResumeContent,
}

/// PATCH 语义：字段缺省表示不修改。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateResumeRequest {
    pub title: Option<String>,
    pub payload: Option<ResumeContent>,
}

/// `ResumeError` → 统一错误体；后端错误细节只进日志。
impl From<ResumeError> for ApiError {
    fn from(err: ResumeError) -> Self {
        match err {
            ResumeError::InvalidTitle { .. }
            | ResumeError::InvalidContent { .. }
            | ResumeError::InvalidFileName => ApiError::validation_failed(err.to_string()),
            ResumeError::ContentTooLarge => {
                ApiError::validation_failed("resume content is too large")
            }
            ResumeError::NotFound => ApiError::not_found("resume not found"),
            // plan/01 §9 的 `resume_limit_reached` 码未纳入 A0 冻结的错误码表，
            // 因此按通用冲突码 409 处理（偏差已在交付报告说明）。
            ResumeError::LimitReached => ApiError::conflict("resume limit reached"),
            ResumeError::AttachmentTooLarge => {
                ApiError::payload_too_large("attachment exceeds the 10 MiB size limit")
            }
            ResumeError::UnsupportedAttachment => {
                ApiError::unsupported_media_type("attachment must be a PDF or DOCX file")
            }
            ResumeError::Backend { message, source } => {
                tracing::error!(message = %message, source = ?source, "resume backend error");
                ApiError::internal_error()
            }
        }
    }
}

// ---------- 路由装配 ----------

/// 简历路由；JSON 路由 1 MiB、上传路由 12 MiB 各自限流。
pub fn router() -> Router<AppState> {
    let json_routes = Router::new()
        .route("/api/v1/resumes", get(list_resumes).post(create_resume))
        .route(
            "/api/v1/resumes/{id}",
            get(get_resume).patch(update_resume).delete(delete_resume),
        )
        .route("/api/v1/resumes/{id}/default", put(set_default_resume))
        .route(
            "/api/v1/resumes/{id}/attachment",
            get(download_attachment).delete(remove_attachment),
        )
        .layer(RequestBodyLimitLayer::new(JSON_BODY_LIMIT_BYTES));
    let upload_routes = Router::new()
        .route("/api/v1/resumes/attachment", post(upload_attachment))
        .route("/api/v1/resumes/{id}/attachment", post(attach_to_resume))
        .layer(RequestBodyLimitLayer::new(UPLOAD_BODY_LIMIT_BYTES));
    json_routes.merge(upload_routes)
}

// ---------- 处理器 ----------

pub async fn list_resumes(
    State(state): State<AppState>,
    auth: AuthContext,
) -> Result<Json<Vec<ResumeDto>>, ApiError> {
    let resumes = state.resume.list(auth.user_id).await?;
    Ok(Json(resumes.into_iter().map(ResumeDto::from).collect()))
}

pub async fn create_resume(
    State(state): State<AppState>,
    auth: AuthContext,
    Json(request): Json<CreateResumeRequest>,
) -> Result<Response, ApiError> {
    let resume = state
        .resume
        .create_online(auth.user_id, &request.title, request.payload)
        .await?;
    Ok((StatusCode::CREATED, Json(ResumeDto::from(resume))).into_response())
}

pub async fn upload_attachment(
    State(state): State<AppState>,
    auth: AuthContext,
    request: Request,
) -> Result<Response, ApiError> {
    let headers = request.headers().clone();
    let body = axum::body::to_bytes(request.into_body(), UPLOAD_BODY_LIMIT_BYTES)
        .await
        .map_err(|_| ApiError::payload_too_large("multipart body exceeds the upload limit"))?;
    let (title, file_name, file_bytes) = parse_multipart_upload(&headers, &body)?;
    let resume = state
        .resume
        .upload_attachment(auth.user_id, &title, &file_name, &file_bytes)
        .await?;
    Ok((StatusCode::CREATED, Json(ResumeDto::from(resume))).into_response())
}

/// 给已有简历上传附件：解析结构化后回填字段（只补空字段、列表追加去重）。
pub async fn attach_to_resume(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(raw_id): Path<String>,
    request: Request,
) -> Result<Json<ResumeDto>, ApiError> {
    let id = parse_resume_id(&raw_id)?;
    let headers = request.headers().clone();
    let body = axum::body::to_bytes(request.into_body(), UPLOAD_BODY_LIMIT_BYTES)
        .await
        .map_err(|_| ApiError::payload_too_large("multipart body exceeds the upload limit"))?;
    let (file_name, file_bytes) = parse_multipart_file(&headers, &body)?;
    let resume = state
        .resume
        .attach_to_resume(auth.user_id, id, &file_name, &file_bytes)
        .await?;
    Ok(Json(ResumeDto::from(resume)))
}

/// 移除简历附件；内容保持不变。
pub async fn remove_attachment(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(raw_id): Path<String>,
) -> Result<Json<ResumeDto>, ApiError> {
    let id = parse_resume_id(&raw_id)?;
    let resume = state.resume.remove_attachment(auth.user_id, id).await?;
    Ok(Json(ResumeDto::from(resume)))
}

pub async fn get_resume(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(raw_id): Path<String>,
) -> Result<Json<ResumeDto>, ApiError> {
    let id = parse_resume_id(&raw_id)?;
    let resume = state.resume.get(auth.user_id, id).await?;
    Ok(Json(ResumeDto::from(resume)))
}

pub async fn update_resume(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(raw_id): Path<String>,
    Json(request): Json<UpdateResumeRequest>,
) -> Result<Json<ResumeDto>, ApiError> {
    let id = parse_resume_id(&raw_id)?;
    let resume = state
        .resume
        .update(auth.user_id, id, request.title.as_deref(), request.payload)
        .await?;
    Ok(Json(ResumeDto::from(resume)))
}

pub async fn set_default_resume(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(raw_id): Path<String>,
) -> Result<Json<ResumeDto>, ApiError> {
    let id = parse_resume_id(&raw_id)?;
    let resume = state.resume.set_default(auth.user_id, id).await?;
    Ok(Json(ResumeDto::from(resume)))
}

pub async fn delete_resume(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(raw_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let id = parse_resume_id(&raw_id)?;
    state.resume.delete(auth.user_id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn download_attachment(
    State(state): State<AppState>,
    auth: AuthContext,
    Path(raw_id): Path<String>,
) -> Result<Response, ApiError> {
    let id = parse_resume_id(&raw_id)?;
    let opened: OpenedAttachment = state.resume.open_attachment(auth.user_id, id).await?;

    // 文件名做 RFC 5987 百分号编码（CRLF/引号/非 ASCII 全部转义），防止头注入。
    let encoded = percent_encode_file_name(&opened.file_name);
    let disposition = format!("attachment; filename*=UTF-8''{encoded}");
    let disposition = HeaderValue::from_str(&disposition).map_err(|_| {
        tracing::error!("content-disposition header construction failed");
        ApiError::internal_error()
    })?;

    // 流式转发：附件不整体读入内存（plan/01 §8.3）。
    let stream = tokio_util::io::ReaderStream::new(opened.stream);
    let response = Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, opened.mime.as_str())
        .header(CONTENT_LENGTH, opened.size)
        .header(CONTENT_DISPOSITION, disposition)
        .header(HeaderName::from_static("x-content-type-options"), "nosniff")
        .header(CACHE_CONTROL, "private, no-store")
        .body(Body::from_stream(stream))
        .map_err(|_| ApiError::internal_error())?;
    Ok(response)
}

// ---------- 辅助 ----------

/// 路径参数 → `ResumeId`；非法 UUID 一律 404，不区分格式错误。
fn parse_resume_id(raw: &str) -> Result<ResumeId, ApiError> {
    uuid::Uuid::parse_str(raw)
        .map(ResumeId::from_uuid)
        .map_err(|_| ApiError::not_found("resume not found"))
}

/// RFC 5987 `filename*` 的百分号编码；输出保证为可见 ASCII。
fn percent_encode_file_name(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for byte in name.as_bytes() {
        let ch = *byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '.' | '_' | '~') {
            out.push(ch);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// 手写 multipart/form-data 解析（crate 未启用 axum `multipart` feature）。
/// 只接受 `title` 文本字段与 `file` 文件字段，其余字段忽略。
fn parse_multipart_upload(
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(String, String, Vec<u8>), ApiError> {
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::validation_failed("content-type header is required"))?;
    let boundary = extract_boundary(content_type)
        .ok_or_else(|| ApiError::validation_failed("multipart boundary is missing"))?;
    let fields = parse_multipart_body(&boundary, body)?;

    let mut title: Option<String> = None;
    let mut file_name: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    for field in fields {
        match field.name.as_str() {
            "title" if field.file_name.is_none() && title.is_none() => {
                title =
                    Some(String::from_utf8(field.content).map_err(|_| {
                        ApiError::validation_failed("title field must be UTF-8 text")
                    })?);
            }
            "file" if file_name.is_none() && file_bytes.is_none() => {
                file_name = field.file_name;
                file_bytes = Some(field.content);
            }
            _ => {}
        }
    }
    let Some(title) = title else {
        return Err(ApiError::validation_failed(
            "multipart field `title` is required",
        ));
    };
    let Some(file_name) = file_name else {
        return Err(ApiError::validation_failed(
            "multipart field `file` is required",
        ));
    };
    let Some(file_bytes) = file_bytes else {
        return Err(ApiError::validation_failed(
            "multipart field `file` is required",
        ));
    };
    Ok((title, file_name, file_bytes))
}

fn validation_error(message: &'static str) -> ApiError {
    ApiError::validation_failed(message)
}

/// 只接受 `file` 文件字段的 multipart 解析（给已有简历传附件场景）。
fn parse_multipart_file(
    headers: &HeaderMap,
    body: &[u8],
) -> Result<(String, Vec<u8>), ApiError> {
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| ApiError::validation_failed("content-type header is required"))?;
    let boundary = extract_boundary(content_type)
        .ok_or_else(|| ApiError::validation_failed("multipart boundary is missing"))?;
    let fields = parse_multipart_body(&boundary, body)?;

    let mut file_name: Option<String> = None;
    let mut file_bytes: Option<Vec<u8>> = None;
    for field in fields {
        if field.name == "file" && file_bytes.is_none() {
            file_name = field.file_name;
            file_bytes = Some(field.content);
        }
    }
    let Some(file_name) = file_name else {
        return Err(ApiError::validation_failed(
            "multipart field `file` is required",
        ));
    };
    let Some(file_bytes) = file_bytes else {
        return Err(ApiError::validation_failed(
            "multipart field `file` is required",
        ));
    };
    Ok((file_name, file_bytes))
}

struct MultipartField {
    name: String,
    file_name: Option<String>,
    content: Vec<u8>,
}

fn extract_boundary(content_type: &str) -> Option<String> {
    let lower = content_type.to_ascii_lowercase();
    let position = lower.find("boundary=")?;
    let raw = content_type[position + "boundary=".len()..].trim();
    // boundary 值到下一个参数为止；可能被双引号包裹（引号内的 `;` 属于值）。
    let raw = split_disposition_params(raw)
        .first()
        .copied()
        .unwrap_or(raw)
        .trim();
    let raw = raw.trim_matches('"');
    // 合法 boundary 为可见 ASCII 且 ≤70 字符；防止病态输入拖垮扫描。
    if raw.is_empty() || raw.len() > 70 || !raw.chars().all(|ch| (0x20..0x7f).contains(&(ch as u8)))
    {
        return None;
    }
    Some(raw.to_owned())
}

fn parse_multipart_body(boundary: &str, body: &[u8]) -> Result<Vec<MultipartField>, ApiError> {
    let dash_boundary = format!("--{boundary}");
    let needle = format!("\r\n{dash_boundary}").into_bytes();

    let mut fields = Vec::new();
    // 跳过 preamble，定位第一个 boundary。
    let Some(start) = find_subslice(body, dash_boundary.as_bytes()) else {
        return Err(validation_error("malformed multipart body"));
    };
    let mut cursor = start + dash_boundary.len();
    loop {
        let rest = body
            .get(cursor..)
            .ok_or_else(|| validation_error("malformed multipart body"))?;
        if rest.starts_with(b"--") {
            break; // 结束 boundary。
        }
        let rest = rest
            .strip_prefix(b"\r\n")
            .ok_or_else(|| validation_error("malformed multipart body"))?;
        let Some(relative_end) = find_subslice(rest, &needle) else {
            return Err(validation_error("malformed multipart body"));
        };
        fields.push(parse_multipart_part(&rest[..relative_end])?);
        cursor += 2 + relative_end + needle.len();
    }
    Ok(fields)
}

fn parse_multipart_part(part: &[u8]) -> Result<MultipartField, ApiError> {
    let Some(header_end) = find_subslice(part, b"\r\n\r\n") else {
        return Err(validation_error("malformed multipart part"));
    };
    let headers_raw = std::str::from_utf8(&part[..header_end])
        .map_err(|_| validation_error("multipart headers must be UTF-8"))?;
    let content = part[header_end + 4..].to_vec();

    let mut name = None;
    let mut file_name = None;
    for line in headers_raw.split("\r\n") {
        let Some((header, value)) = line.split_once(':') else {
            continue;
        };
        if !header.trim().eq_ignore_ascii_case("content-disposition") {
            continue;
        }
        if let Some(value) = disposition_param(value, "name") {
            name = Some(value);
        }
        if let Some(value) = disposition_param(value, "filename") {
            file_name = Some(value);
        }
    }
    let Some(name) = name else {
        return Err(validation_error("multipart part misses `name`"));
    };
    Ok(MultipartField {
        name,
        file_name,
        content,
    })
}

/// 读取 `Content-Disposition` 中的参数；支持双引号与 `\` 转义，
/// 引号内的 `;` 不作为参数分隔符（如 `filename="a;b.pdf"`）。
fn disposition_param(disposition: &str, key: &str) -> Option<String> {
    for segment in split_disposition_params(disposition) {
        let segment = segment.trim();
        let Some((k, v)) = segment.split_once('=') else {
            continue;
        };
        if !k.trim().eq_ignore_ascii_case(key) {
            continue;
        }
        let v = v.trim();
        let v = v
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or(v);
        return Some(unescape_disposition_value(v));
    }
    None
}

/// 按 `;` 切分参数段，但双引号内的 `;` 视为字面值；`\` 转义保留原样，
/// 由 `unescape_disposition_value` 统一处理。
fn split_disposition_params(disposition: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut in_quotes = false;
    let mut escaped = false;
    let mut start = 0;
    for (index, ch) in disposition.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match ch {
            '\\' => escaped = true,
            '"' => in_quotes = !in_quotes,
            ';' if !in_quotes => {
                segments.push(&disposition[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    segments.push(&disposition[start..]);
    segments
}

fn unescape_disposition_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                out.push(next);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests;
