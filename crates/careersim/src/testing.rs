//! crate 级测试装配：内存账户端口替身、完整路由与请求辅助。
//!
//! 仅在 `cfg(test)` 下编译；account 与 http 的就近单测共用，
//! 不实现任何生产语义（如真实哈希耗时）。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::response::Response;
use chrono::{DateTime, Duration, Utc};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::account::model::{
    CredentialUser, NewUser, Password, PasswordHash, ProfilePatch, User, UserId,
};
use crate::account::password::{PasswordError, PasswordHasher};
use crate::account::service::{AccountService, Clock};
use crate::account::session::{LoginSession, NewLoginSession, SessionId, SessionTokenHash};
use crate::account::store::{SessionStore, SessionStoreError, UserStore, UserStoreError};
use crate::http::router::{AppState, router};
use crate::resume::model::{
    NewResume, NewResumeAttachment, ParseStatus, RESUME_LIMIT, Resume, ResumeAttachment, ResumeId,
    ResumePatch,
};
use crate::resume::parse::{AttachmentKind, ParseError, ResumeTextExtractor};
use crate::resume::storage::{AttachmentStore, AttachmentStoreError};
use crate::resume::store::{ResumeStore, ResumeStoreError};
use crate::resume::{ResumeContent, ResumeService};

// ---------- 账户端口替身 ----------

#[derive(Clone)]
struct StoredUser {
    user: User,
    username_key: String,
    password_hash: PasswordHash,
}

pub(crate) struct InMemoryUserStore {
    users: Mutex<HashMap<UserId, StoredUser>>,
}

impl InMemoryUserStore {
    pub(crate) fn new() -> Self {
        Self {
            users: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl UserStore for InMemoryUserStore {
    async fn insert(&self, new: NewUser) -> Result<User, UserStoreError> {
        let mut users = self.users.lock().unwrap();
        if users
            .values()
            .any(|row| row.username_key == new.username_key.as_str())
        {
            return Err(UserStoreError::Conflict);
        }
        let row = StoredUser {
            user: User {
                id: new.id,
                username: new.username,
                display_name: new.display_name,
                school: None,
                major: None,
                graduation_year: None,
                target_roles: Vec::new(),
                target_cities: Vec::new(),
                created_at: new.registered_at,
                updated_at: new.registered_at,
            },
            username_key: new.username_key.as_str().to_owned(),
            password_hash: new.password_hash,
        };
        let user = row.user.clone();
        users.insert(row.user.id, row);
        Ok(user)
    }

    async fn find_credential(
        &self,
        username_key: &str,
    ) -> Result<Option<CredentialUser>, UserStoreError> {
        let users = self.users.lock().unwrap();
        Ok(users
            .values()
            .find(|row| row.username_key == username_key)
            .map(|row| CredentialUser {
                id: row.user.id,
                password_hash: row.password_hash.clone(),
            }))
    }

    async fn find_by_id(&self, id: UserId) -> Result<Option<User>, UserStoreError> {
        Ok(self
            .users
            .lock()
            .unwrap()
            .get(&id)
            .map(|row| row.user.clone()))
    }

    async fn update_profile(
        &self,
        id: UserId,
        patch: &ProfilePatch,
        now: DateTime<Utc>,
    ) -> Result<User, UserStoreError> {
        let mut users = self.users.lock().unwrap();
        let Some(row) = users.get_mut(&id) else {
            return Err(UserStoreError::NotFound);
        };
        apply_patch(&mut row.user, patch, now);
        Ok(row.user.clone())
    }

    async fn update_password(
        &self,
        id: UserId,
        hash: &PasswordHash,
        now: DateTime<Utc>,
    ) -> Result<(), UserStoreError> {
        let mut users = self.users.lock().unwrap();
        let Some(row) = users.get_mut(&id) else {
            return Err(UserStoreError::NotFound);
        };
        row.password_hash = hash.clone();
        row.user.updated_at = now;
        Ok(())
    }
}

/// PATCH 双层 Option 语义的参考实现；PG 适配器行为须与其一致。
fn apply_patch(user: &mut User, patch: &ProfilePatch, now: DateTime<Utc>) {
    if let Some(name) = &patch.display_name {
        user.display_name = name.clone();
    }
    if let Some(school) = &patch.school {
        user.school = school.clone();
    }
    if let Some(major) = &patch.major {
        user.major = major.clone();
    }
    if let Some(year) = &patch.graduation_year {
        user.graduation_year = *year;
    }
    if let Some(roles) = &patch.target_roles {
        user.target_roles = roles.clone();
    }
    if let Some(cities) = &patch.target_cities {
        user.target_cities = cities.clone();
    }
    user.updated_at = now;
}

/// 内存会话存储：会话表 + 令牌哈希索引。
pub(crate) struct InMemorySessionStore {
    sessions: Mutex<HashMap<SessionId, LoginSession>>,
    by_hash: Mutex<HashMap<[u8; 32], SessionId>>,
}

impl InMemorySessionStore {
    pub(crate) fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            by_hash: Mutex::new(HashMap::new()),
        }
    }

    fn login_session(new: NewLoginSession) -> LoginSession {
        LoginSession {
            id: new.id,
            user_id: new.user_id,
            created_at: new.created_at,
            last_seen_at: new.created_at,
            idle_expires_at: new.idle_expires_at,
            absolute_expires_at: new.absolute_expires_at,
            revoked_at: None,
        }
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn insert(&self, session: NewLoginSession) -> Result<(), SessionStoreError> {
        self.by_hash
            .lock()
            .unwrap()
            .insert(*session.token_hash.as_array(), session.id);
        self.sessions
            .lock()
            .unwrap()
            .insert(session.id, Self::login_session(session));
        Ok(())
    }

    async fn find_by_token_hash(
        &self,
        hash: &SessionTokenHash,
    ) -> Result<Option<LoginSession>, SessionStoreError> {
        let id = self.by_hash.lock().unwrap().get(hash.as_array()).copied();
        Ok(id.and_then(|id| self.sessions.lock().unwrap().get(&id).cloned()))
    }

    async fn touch(
        &self,
        id: SessionId,
        last_seen_at: DateTime<Utc>,
        idle_expires_at: DateTime<Utc>,
    ) -> Result<(), SessionStoreError> {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(session) = sessions.get_mut(&id) else {
            return Err(SessionStoreError::NotFound);
        };
        session.last_seen_at = last_seen_at;
        session.idle_expires_at = idle_expires_at;
        Ok(())
    }

    async fn revoke(&self, id: SessionId, now: DateTime<Utc>) -> Result<(), SessionStoreError> {
        if let Some(session) = self.sessions.lock().unwrap().get_mut(&id) {
            session.revoked_at.get_or_insert(now);
        }
        Ok(())
    }

    async fn revoke_others(
        &self,
        user: UserId,
        keep: SessionId,
        now: DateTime<Utc>,
    ) -> Result<(), SessionStoreError> {
        let mut sessions = self.sessions.lock().unwrap();
        for session in sessions.values_mut() {
            if session.user_id == user && session.id != keep && session.revoked_at.is_none() {
                session.revoked_at = Some(now);
            }
        }
        Ok(())
    }

    async fn rotate(
        &self,
        old: SessionId,
        new: NewLoginSession,
        now: DateTime<Utc>,
    ) -> Result<(), SessionStoreError> {
        let mut sessions = self.sessions.lock().unwrap();
        let Some(old_session) = sessions.get_mut(&old) else {
            return Err(SessionStoreError::NotFound);
        };
        old_session.revoked_at = Some(now);
        self.by_hash
            .lock()
            .unwrap()
            .insert(*new.token_hash.as_array(), new.id);
        sessions.insert(new.id, Self::login_session(new));
        Ok(())
    }
}

pub(crate) struct FakeClock {
    now: Mutex<DateTime<Utc>>,
}

impl FakeClock {
    pub(crate) fn new() -> Self {
        Self {
            now: Mutex::new(Utc::now()),
        }
    }

    pub(crate) fn advance(&self, by: Duration) {
        *self.now.lock().unwrap() += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
}

/// 可逆的假哈希器；`dummy_verify` 计数用于断言防枚举路径。
pub(crate) struct FakeHasher {
    pub(crate) dummy_calls: Mutex<usize>,
}

impl FakeHasher {
    pub(crate) fn new() -> Self {
        Self {
            dummy_calls: Mutex::new(0),
        }
    }
}

impl PasswordHasher for FakeHasher {
    fn hash(&self, password: &Password) -> Result<PasswordHash, PasswordError> {
        Ok(PasswordHash::new(format!("fake${}", password.as_str())))
    }

    fn verify(&self, password: &Password, hash: &PasswordHash) -> Result<bool, PasswordError> {
        Ok(hash.as_str() == format!("fake${}", password.as_str()))
    }

    fn dummy_verify(&self, _password: &Password) {
        *self.dummy_calls.lock().unwrap() += 1;
    }
}

// ---------- HTTP 测试装配 ----------

pub(crate) const ALLOWED_ORIGIN: &str = "http://localhost:5173";

// ---------- resume 内存替身 ----------

struct ResumeRow {
    resume: Resume,
    deleted: bool,
}

/// resume 内存替身：镜像 PG 实现的每用户上限、首份默认与软删除提升语义。
pub(crate) struct InMemoryResumeStore {
    data: Mutex<Vec<ResumeRow>>,
}

impl InMemoryResumeStore {
    pub(crate) fn new() -> Self {
        Self {
            data: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait]
impl ResumeStore for InMemoryResumeStore {
    async fn list(&self, owner: UserId) -> Result<Vec<Resume>, ResumeStoreError> {
        let data = self.data.lock().unwrap();
        let mut resumes: Vec<Resume> = data
            .iter()
            .filter(|row| !row.deleted && row.resume.owner == owner)
            .map(|row| row.resume.clone())
            .collect();
        // 默认在前，其余按更新时间倒序。
        resumes.sort_by(|a, b| {
            b.is_default
                .cmp(&a.is_default)
                .then_with(|| b.updated_at.cmp(&a.updated_at))
        });
        Ok(resumes)
    }

    async fn get(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeStoreError> {
        let data = self.data.lock().unwrap();
        data.iter()
            .find(|row| !row.deleted && row.resume.owner == owner && row.resume.id == id)
            .map(|row| row.resume.clone())
            .ok_or(ResumeStoreError::NotFound)
    }

    async fn insert(&self, owner: UserId, resume: NewResume) -> Result<Resume, ResumeStoreError> {
        let mut data = self.data.lock().unwrap();
        let live_count = data
            .iter()
            .filter(|row| !row.deleted && row.resume.owner == owner)
            .count();
        if live_count >= RESUME_LIMIT {
            return Err(ResumeStoreError::LimitReached);
        }
        let is_default = !data
            .iter()
            .any(|row| !row.deleted && row.resume.owner == owner && row.resume.is_default);
        let content: ResumeContent = serde_json::from_value(resume.content).map_err(|err| {
            ResumeStoreError::backend(format!("stored content is not valid: {err}"))
        })?;
        let now = Utc::now();
        let resume = Resume {
            id: resume.id,
            owner,
            title: resume.title,
            is_default,
            content,
            attachment: resume.attachment.map(|att| {
                let mime = super::resume::model::mime_from_file_name(&att.file_name)
                    .unwrap_or_default()
                    .to_owned();
                ResumeAttachment {
                    storage_key: att.storage_key,
                    file_name: att.file_name,
                    mime,
                }
            }),
            parse_status: resume.parse_status,
            created_at: now,
            updated_at: now,
        };
        data.push(ResumeRow {
            resume: resume.clone(),
            deleted: false,
        });
        Ok(resume)
    }

    async fn update(
        &self,
        owner: UserId,
        id: ResumeId,
        patch: ResumePatch,
    ) -> Result<Resume, ResumeStoreError> {
        let mut data = self.data.lock().unwrap();
        let Some(row) = data
            .iter_mut()
            .find(|row| !row.deleted && row.resume.owner == owner && row.resume.id == id)
        else {
            return Err(ResumeStoreError::NotFound);
        };
        if let Some(title) = patch.title {
            row.resume.title = title;
        }
        if let Some(content) = patch.content {
            row.resume.content = serde_json::from_value(content).map_err(|err| {
                ResumeStoreError::backend(format!("stored content is not valid: {err}"))
            })?;
        }
        row.resume.updated_at = Utc::now();
        Ok(row.resume.clone())
    }

    async fn set_default(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeStoreError> {
        let mut data = self.data.lock().unwrap();
        if !data
            .iter()
            .any(|row| !row.deleted && row.resume.owner == owner && row.resume.id == id)
        {
            return Err(ResumeStoreError::NotFound);
        }
        for row in data
            .iter_mut()
            .filter(|row| !row.deleted && row.resume.owner == owner)
        {
            row.resume.is_default = row.resume.id == id;
        }
        let row = data
            .iter()
            .find(|row| !row.deleted && row.resume.owner == owner && row.resume.id == id)
            .expect("checked above");
        Ok(row.resume.clone())
    }

    async fn soft_delete(&self, owner: UserId, id: ResumeId) -> Result<(), ResumeStoreError> {
        let mut data = self.data.lock().unwrap();
        let Some(row) = data
            .iter_mut()
            .find(|row| !row.deleted && row.resume.owner == owner && row.resume.id == id)
        else {
            return Err(ResumeStoreError::NotFound);
        };
        row.deleted = true;
        let was_default = row.resume.is_default;
        if was_default {
            // 提升最近更新的剩余简历为默认。
            let mut remaining: Vec<usize> = data
                .iter()
                .enumerate()
                .filter(|(_, row)| !row.deleted && row.resume.owner == owner)
                .map(|(index, _)| index)
                .collect();
            remaining.sort_by(|a, b| data[*b].resume.updated_at.cmp(&data[*a].resume.updated_at));
            if let Some(index) = remaining.first() {
                data[*index].resume.is_default = true;
            }
        }
        Ok(())
    }

    async fn attach(
        &self,
        owner: UserId,
        id: ResumeId,
        attachment: NewResumeAttachment,
        parse_status: ParseStatus,
        content: serde_json::Value,
    ) -> Result<Resume, ResumeStoreError> {
        let mut data = self.data.lock().unwrap();
        let Some(row) = data
            .iter_mut()
            .find(|row| !row.deleted && row.resume.owner == owner && row.resume.id == id)
        else {
            return Err(ResumeStoreError::NotFound);
        };
        let mime = super::resume::model::mime_from_file_name(&attachment.file_name)
            .unwrap_or_default()
            .to_owned();
        row.resume.attachment = Some(ResumeAttachment {
            storage_key: attachment.storage_key,
            file_name: attachment.file_name,
            mime,
        });
        row.resume.parse_status = parse_status;
        row.resume.content = serde_json::from_value(content).map_err(|err| {
            ResumeStoreError::backend(format!("stored content is not valid: {err}"))
        })?;
        row.resume.updated_at = Utc::now();
        Ok(row.resume.clone())
    }

    async fn detach(&self, owner: UserId, id: ResumeId) -> Result<Resume, ResumeStoreError> {
        let mut data = self.data.lock().unwrap();
        let Some(row) = data
            .iter_mut()
            .find(|row| !row.deleted && row.resume.owner == owner && row.resume.id == id)
        else {
            return Err(ResumeStoreError::NotFound);
        };
        row.resume.attachment = None;
        row.resume.parse_status = ParseStatus::NotApplicable;
        row.resume.updated_at = Utc::now();
        Ok(row.resume.clone())
    }
}

/// 附件存储内存替身：键值对字节表；语义对齐 LocalFsAttachmentStore。
#[derive(Default)]
pub(crate) struct InMemoryAttachmentStore {
    files: Mutex<HashMap<String, Vec<u8>>>,
}

impl InMemoryAttachmentStore {
    pub(crate) fn new() -> Self {
        Self {
            files: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait]
impl AttachmentStore for InMemoryAttachmentStore {
    async fn put(&self, key: &str, bytes: &[u8]) -> Result<(), AttachmentStoreError> {
        self.files
            .lock()
            .unwrap()
            .insert(key.to_owned(), bytes.to_vec());
        Ok(())
    }

    async fn open(
        &self,
        key: &str,
    ) -> Result<Box<dyn tokio::io::AsyncRead + Send + Unpin>, AttachmentStoreError> {
        self.files
            .lock()
            .unwrap()
            .get(key)
            .map(|bytes| Box::new(std::io::Cursor::new(bytes.clone())) as _)
            .ok_or(AttachmentStoreError::NotFound)
    }

    async fn delete(&self, key: &str) -> Result<(), AttachmentStoreError> {
        self.files.lock().unwrap().remove(key);
        Ok(())
    }
}

/// 固定文本提取器：跳过真实 PDF/DOCX 解析，让上传链路确定性走到 ready。
struct StaticTextExtractor;

impl ResumeTextExtractor for StaticTextExtractor {
    fn supports(&self, _kind: AttachmentKind) -> bool {
        true
    }

    fn extract(&self, _kind: AttachmentKind, _bytes: &[u8]) -> Result<String, ParseError> {
        Ok("提取到的简历正文：五年 Rust 后端经验".to_owned())
    }
}

/// 组装带内存账户服务的完整应用路由。
pub(crate) fn app() -> Router {
    let service = AccountService::new(
        Arc::new(InMemoryUserStore::new()),
        Arc::new(InMemorySessionStore::new()),
        Arc::new(FakeHasher::new()),
        Arc::new(FakeClock::new()),
    );
    let resume = ResumeService::new(
        Arc::new(InMemoryResumeStore::new()),
        Arc::new(InMemoryAttachmentStore::new()),
        Arc::new(StaticTextExtractor),
    );
    let state = AppState {
        account: Arc::new(service),
        resume: Arc::new(resume),
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

/// 以指定用户名注册并登录，返回会话 Cookie 对（`careersim_session=...`）。
pub(crate) async fn registered_cookie(app: &Router, username: &str) -> String {
    let response = send(
        app.clone(),
        json_request(
            "POST",
            "/api/v1/auth/register",
            json!({
                "username": username,
                "password": "password123",
                "displayName": username,
            }),
        ),
    )
    .await;
    assert_eq!(
        response.status(),
        StatusCode::CREATED,
        "register {username}"
    );
    first_cookie_pair(&response).expect("register sets session cookie")
}

/// 注册并登录，返回可复用的 Cookie 对（`careersim_session=...`）。
pub(crate) async fn logged_in_cookie(app: &Router) -> String {
    registered_cookie(app, "alice").await
}
