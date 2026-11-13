//! 会话令牌、会话记录与过期规则（S-A02、S-A04）。

use std::fmt;

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Duration, Utc};
use rand::{Rng as _, rng};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::model::UserId;

/// 空闲超时：7 天。
pub const IDLE_TIMEOUT: Duration = Duration::seconds(7 * 24 * 3600);
/// 绝对超时：30 天。
pub const ABSOLUTE_TIMEOUT: Duration = Duration::seconds(30 * 24 * 3600);
/// `last_seen_at` 节流窗口：60 秒，避免每次请求都写库。
pub const TOUCH_THROTTLE: Duration = Duration::seconds(60);
/// 会话 Cookie 名；与方案 01 §6.1 一致。
pub const SESSION_COOKIE: &str = "careersim_session";
/// Cookie Max-Age（秒），与绝对超时一致。
pub const COOKIE_MAX_AGE_SECS: u64 = 30 * 24 * 3600;

/// 会话标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(Uuid);

impl SessionId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// 32 字节 CSPRNG 会话令牌；只经 Cookie 传输，数据库只存其 SHA-256。
pub struct SessionToken([u8; 32]);

impl SessionToken {
    pub fn generate() -> Self {
        // ThreadRng 由 OS 熵播种（ChaCha12），满足会话令牌的 CSPRNG 要求。
        let mut bytes = [0_u8; 32];
        rng().fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// 从 Cookie 值还原令牌；只接受 32 字节的 Base64URL 无填充编码。
    pub fn decode(raw: &str) -> Option<Self> {
        let bytes = URL_SAFE_NO_PAD.decode(raw).ok()?;
        bytes.try_into().ok().map(Self)
    }

    /// Cookie 值：Base64URL 无填充。
    pub fn encode(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    pub fn hash(&self) -> SessionTokenHash {
        let digest: [u8; 32] = Sha256::digest(self.0).into();
        SessionTokenHash(digest)
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// 令牌的 SHA-256 摘要；数据库 `login_sessions.token_hash` 只存它。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionTokenHash([u8; 32]);

impl SessionTokenHash {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    /// 定长数组视图；测试替身与适配器用它做键。
    pub fn as_array(&self) -> &[u8; 32] {
        &self.0
    }
}

/// 会话行投影；过期与撤销判断集中在 [`LoginSession::is_active_at`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginSession {
    pub id: SessionId,
    pub user_id: UserId,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub idle_expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

impl LoginSession {
    /// 会话在 `now` 是否可用：未撤销且未超空闲/绝对超时。
    pub fn is_active_at(&self, now: DateTime<Utc>) -> bool {
        self.revoked_at.is_none() && now < self.idle_expires_at && now < self.absolute_expires_at
    }

    /// 是否超过节流窗口，需要顺延活跃时间。
    pub fn needs_touch(&self, now: DateTime<Utc>) -> bool {
        now - self.last_seen_at > TOUCH_THROTTLE
    }

    /// 顺延后的新空闲过期时间；不超过绝对过期（数据库 CHECK 约束）。
    pub fn touched_idle_expires_at(&self, now: DateTime<Utc>) -> DateTime<Utc> {
        let extended = now + IDLE_TIMEOUT;
        if extended > self.absolute_expires_at {
            self.absolute_expires_at
        } else {
            extended
        }
    }
}

/// 新会话行输入；`last_seen_at` 落库等于 `created_at`。
pub struct NewLoginSession {
    pub id: SessionId,
    pub user_id: UserId,
    pub token_hash: SessionTokenHash,
    pub created_at: DateTime<Utc>,
    pub idle_expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
}

/// 按登录时间构造新会话输入；过期时间按 S-A04 推导。
pub fn new_login_session(
    user_id: UserId,
    token: &SessionToken,
    now: DateTime<Utc>,
) -> (SessionId, NewLoginSession) {
    let id = SessionId::generate();
    let session = NewLoginSession {
        id,
        user_id,
        token_hash: token.hash(),
        created_at: now,
        idle_expires_at: now + IDLE_TIMEOUT,
        absolute_expires_at: now + ABSOLUTE_TIMEOUT,
    };
    (id, session)
}
