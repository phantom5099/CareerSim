//! 账户端口替身：内存存储 + 假时钟/假哈希器，供服务与 HTTP 层测试共用。
//!
//! 仅在 `cfg(test)` 下编译；不实现任何生产语义（如真实哈希耗时）。

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};

use crate::account::model::{
    CredentialUser, NewUser, Password, PasswordHash, ProfilePatch, User, UserId,
};
use crate::account::password::{PasswordError, PasswordHasher};
use crate::account::service::Clock;
use crate::account::session::{LoginSession, NewLoginSession, SessionId, SessionTokenHash};
use crate::account::store::{SessionStore, SessionStoreError, UserStore, UserStoreError};

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
