//! 账户服务单元测试：内存端口替身 + FakeClock/FakeHasher。

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};

use crate::account::model::{
    AccountError, CredentialUser, NewUser, Password, PasswordHash, ProfilePatch, User, UserId,
    Username,
};
use crate::account::password::{PasswordError, PasswordHasher};
use crate::account::service::{AccountService, Clock};
use crate::account::session::{LoginSession, NewLoginSession, SessionId, SessionTokenHash};
use crate::account::store::{SessionStore, SessionStoreError, UserStore, UserStoreError};

// ---------- 测试替身 ----------

#[derive(Clone)]
struct StoredUser {
    user: User,
    username_key: String,
    password_hash: PasswordHash,
}

struct InMemoryUserStore {
    users: Mutex<HashMap<UserId, StoredUser>>,
}

impl InMemoryUserStore {
    fn new() -> Self {
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
struct InMemorySessionStore {
    sessions: Mutex<HashMap<SessionId, LoginSession>>,
    by_hash: Mutex<HashMap<[u8; 32], SessionId>>,
}

impl InMemorySessionStore {
    fn new() -> Self {
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
    ) -> Result<u64, SessionStoreError> {
        let mut sessions = self.sessions.lock().unwrap();
        let mut revoked = 0_u64;
        for session in sessions.values_mut() {
            if session.user_id == user && session.id != keep && session.revoked_at.is_none() {
                session.revoked_at = Some(now);
                revoked += 1;
            }
        }
        Ok(revoked)
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

struct FakeClock {
    now: Mutex<DateTime<Utc>>,
}

impl FakeClock {
    fn advance(&self, by: Duration) {
        *self.now.lock().unwrap() += by;
    }
}

impl Clock for FakeClock {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().unwrap()
    }
}

/// 可逆的假哈希器；`dummy_verify` 计数用于断言防枚举路径。
struct FakeHasher {
    dummy_calls: Mutex<usize>,
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

struct Harness {
    service: AccountService,
    clock: Arc<FakeClock>,
    hasher: Arc<FakeHasher>,
}

fn harness() -> Harness {
    let clock = Arc::new(FakeClock {
        now: Mutex::new(Utc::now()),
    });
    let hasher = Arc::new(FakeHasher {
        dummy_calls: Mutex::new(0),
    });
    let service = AccountService::new(
        Arc::new(InMemoryUserStore::new()),
        Arc::new(InMemorySessionStore::new()),
        hasher.clone(),
        clock.clone(),
    );
    Harness {
        service,
        clock,
        hasher,
    }
}

// ---------- 用例 ----------

async fn registered(harness: &Harness) -> (String, String) {
    let grant = harness
        .service
        .register("alice", "password123", "Alice")
        .await
        .expect("register");
    (grant.user.id.into_uuid().to_string(), grant.token)
}

#[tokio::test]
async fn register_issues_session_and_me_roundtrips() {
    let harness = harness();
    let (user_id, token) = registered(&harness).await;

    let authenticated = harness
        .service
        .authenticate(&token)
        .await
        .expect("authenticate");
    assert_eq!(authenticated.user.id.into_uuid().to_string(), user_id);
    assert_eq!(authenticated.user.display_name, "Alice");
    assert_eq!(authenticated.user.username, "alice");
}

#[tokio::test]
async fn register_rejects_invalid_username_password_or_display_name() {
    let harness = harness();
    assert!(matches!(
        harness.service.register("ab", "password123", "A").await,
        Err(AccountError::InvalidUsername { .. })
    ));
    assert!(matches!(
        harness.service.register("alice", "short", "A").await,
        Err(AccountError::InvalidPassword { .. })
    ));
    assert!(matches!(
        harness.service.register("alice", "password123", "  ").await,
        Err(AccountError::InvalidProfile { .. })
    ));
}

#[tokio::test]
async fn register_conflicts_case_insensitively() {
    let harness = harness();
    harness
        .service
        .register("Alice", "password123", "Alice")
        .await
        .expect("register Alice");
    assert!(matches!(
        harness
            .service
            .register("ALICE", "password123", "Again")
            .await,
        Err(AccountError::UsernameTaken)
    ));
}

#[tokio::test]
async fn login_succeeds_and_keeps_original_session_valid() {
    let harness = harness();
    let (_, token) = registered(&harness).await;

    let grant = harness
        .service
        .login("alice", "password123")
        .await
        .expect("login");
    assert_ne!(grant.token, token);
    // 两个会话都有效。
    harness
        .service
        .authenticate(&token)
        .await
        .expect("session 1");
    harness
        .service
        .authenticate(&grant.token)
        .await
        .expect("session 2");
}

#[tokio::test]
async fn login_unifies_error_and_runs_dummy_verify_for_unknown_user() {
    let harness = harness();
    registered(&harness).await;
    let before = *harness.hasher.dummy_calls.lock().unwrap();

    let unknown = harness.service.login("nobody", "password123").await;
    let wrong_password = harness
        .service
        .login("alice", "wrong-password")
        .await
        .expect_err("wrong password rejected");

    assert!(matches!(
        unknown.expect_err("unknown user rejected"),
        AccountError::InvalidCredentials
    ));
    assert!(matches!(wrong_password, AccountError::InvalidCredentials));
    // 未知用户路径执行了假验证，避免响应时间暴露用户名是否存在。
    assert_eq!(*harness.hasher.dummy_calls.lock().unwrap(), before + 1);
}

#[tokio::test]
async fn login_treats_malformed_input_as_invalid_credentials() {
    let harness = harness();
    let result = harness.service.login("UPPER!", "password123").await;
    assert!(matches!(
        result.expect_err("malformed input rejected"),
        AccountError::InvalidCredentials
    ));
}

#[tokio::test]
async fn authenticate_expires_after_idle_window() {
    let harness = harness();
    let (_, token) = registered(&harness).await;
    harness.service.authenticate(&token).await.expect("active");

    harness
        .clock
        .advance(Duration::seconds(7 * 24 * 3600) + Duration::seconds(1));
    assert!(matches!(
        harness.service.authenticate(&token).await,
        Err(AccountError::Unauthenticated)
    ));
}

#[tokio::test]
async fn touch_extends_idle_but_absolute_window_still_wins() {
    let harness = harness();
    let (_, token) = registered(&harness).await;
    harness.service.authenticate(&token).await.expect("active");

    // 每次推进 5 天：空闲窗口被 touch 顺延，绝对窗口到期后仍失效。
    for _ in 0..5 {
        harness.clock.advance(Duration::days(5));
        harness
            .service
            .authenticate(&token)
            .await
            .expect("still active");
    }
    harness.clock.advance(Duration::days(5));
    assert!(matches!(
        harness.service.authenticate(&token).await,
        Err(AccountError::Unauthenticated)
    ));
}

#[tokio::test]
async fn logout_revokes_session() {
    let harness = harness();
    let (_, token) = registered(&harness).await;
    let session_id = harness
        .service
        .authenticate(&token)
        .await
        .expect("active")
        .session_id;

    harness.service.logout(session_id).await.expect("logout");
    harness
        .service
        .logout(session_id)
        .await
        .expect("logout idempotent");
    assert!(matches!(
        harness.service.authenticate(&token).await,
        Err(AccountError::Unauthenticated)
    ));
}

#[tokio::test]
async fn change_password_rotates_token_and_revokes_other_sessions() {
    let harness = harness();
    let (_, token_a) = registered(&harness).await;
    let grant_b = harness
        .service
        .login("alice", "password123")
        .await
        .expect("second login");

    let authenticated = harness
        .service
        .authenticate(&token_a)
        .await
        .expect("session a");
    let new_token = harness
        .service
        .change_password(
            authenticated.user.id,
            authenticated.session_id,
            "password123",
            "newpassword1",
        )
        .await
        .expect("change password");

    // 旧会话 B 被撤销；A 轮换为新令牌。
    assert!(matches!(
        harness.service.authenticate(&grant_b.token).await,
        Err(AccountError::Unauthenticated)
    ));
    assert!(matches!(
        harness.service.authenticate(&token_a).await,
        Err(AccountError::Unauthenticated)
    ));
    harness
        .service
        .authenticate(&new_token)
        .await
        .expect("new token");

    // 旧密码失效、新密码可登录。
    assert!(matches!(
        harness.service.login("alice", "password123").await,
        Err(AccountError::InvalidCredentials)
    ));
    harness
        .service
        .login("alice", "newpassword1")
        .await
        .expect("login new");
}

#[tokio::test]
async fn change_password_rejects_wrong_current_password() {
    let harness = harness();
    let (_, token) = registered(&harness).await;
    let authenticated = harness.service.authenticate(&token).await.expect("auth");

    let result = harness
        .service
        .change_password(
            authenticated.user.id,
            authenticated.session_id,
            "wrong-password",
            "newpassword1",
        )
        .await;
    assert!(matches!(
        result.expect_err("rejected"),
        AccountError::CurrentPasswordMismatch
    ));
}

#[tokio::test]
async fn update_profile_applies_patch_semantics() {
    let harness = harness();
    let (_, token) = registered(&harness).await;
    let authenticated = harness.service.authenticate(&token).await.expect("auth");
    let user_id = authenticated.user.id;

    // 修改与清空。
    let user = harness
        .service
        .update_profile(
            user_id,
            ProfilePatch {
                school: Some(Some("示例大学".to_owned())),
                target_roles: Some(vec!["后端".to_owned()]),
                ..ProfilePatch::default()
            },
        )
        .await
        .expect("update profile");
    assert_eq!(user.school.as_deref(), Some("示例大学"));
    assert_eq!(user.target_roles, vec!["后端".to_owned()]);
    assert_eq!(user.display_name, "Alice");

    // 清空 school；target_roles 保持不变。
    let user = harness
        .service
        .update_profile(
            user_id,
            ProfilePatch {
                school: Some(None),
                ..ProfilePatch::default()
            },
        )
        .await
        .expect("clear school");
    assert_eq!(user.school, None);
    assert_eq!(user.target_roles, vec!["后端".to_owned()]);
}

#[tokio::test]
async fn update_profile_rejects_out_of_bounds_values() {
    let harness = harness();
    let (_, token) = registered(&harness).await;
    let authenticated = harness.service.authenticate(&token).await.expect("auth");

    let result = harness
        .service
        .update_profile(
            authenticated.user.id,
            ProfilePatch {
                display_name: Some("x".repeat(41)),
                ..ProfilePatch::default()
            },
        )
        .await;
    assert!(matches!(
        result.expect_err("rejected"),
        AccountError::InvalidProfile {
            field: "displayName",
            ..
        }
    ));
}

#[tokio::test]
async fn update_profile_for_missing_user_is_unauthenticated() {
    let harness = harness();
    let result = harness
        .service
        .update_profile(UserId::generate(), ProfilePatch::default())
        .await;
    assert!(matches!(
        result.expect_err("rejected"),
        AccountError::Unauthenticated
    ));
}

#[tokio::test]
async fn authenticate_rejects_malformed_token() {
    let harness = harness();
    assert!(matches!(
        harness.service.authenticate("not-a-token").await,
        Err(AccountError::Unauthenticated)
    ));
}

/// `Username` 归一化覆盖大小写与首尾空白。
#[test]
fn username_parse_normalizes_input() {
    assert_eq!(Username::parse("  Alice ").unwrap().as_str(), "alice");
    assert!(Username::parse("Bad Char").is_err());
}
