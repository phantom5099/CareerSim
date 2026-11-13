//! 账户领域模型与字段边界。

use std::fmt;

use chrono::{DateTime, Utc};
use uuid::Uuid;

/// 用户唯一标识（uuid v4）；一切归属判断的根。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserId(Uuid);

impl UserId {
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

/// 归一化登录名键：trim + 小写后 3..=32 字节，ASCII `[a-z0-9_]`。
///
/// 展示原文由 `User::username` 保留，唯一性由该归一化键保证。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Username(String);

impl Username {
    pub fn parse(raw: &str) -> Result<Self, AccountError> {
        let normalized = raw.trim().to_ascii_lowercase();
        let len = normalized.len();
        if !(3..=32).contains(&len) {
            return Err(AccountError::InvalidUsername {
                reason: "must be 3..=32 characters",
            });
        }
        if !normalized
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
            return Err(AccountError::InvalidUsername {
                reason: "must contain only lowercase letters, digits and `_`",
            });
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 内存中的明文密码，仅用于哈希与验证；不落库、不进日志。
#[derive(Clone, PartialEq, Eq)]
pub struct Password(String);

impl Password {
    /// 字节边界 8..=128；不 trim，不强制字符类别。
    pub fn parse(raw: &str) -> Result<Self, AccountError> {
        let len = raw.len();
        if !(8..=128).contains(&len) {
            return Err(AccountError::InvalidPassword {
                reason: "must be 8..=128 bytes",
            });
        }
        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Password {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// Argon2id 哈希结果（PHC 字符串），落库为 `users.password_hash`。
#[derive(Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

impl PasswordHash {
    pub(crate) fn new(phc: String) -> Self {
        Self(phc)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PasswordHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// 注册用的用户行输入；时间戳由服务层的 Clock 提供。
pub struct NewUser {
    pub id: UserId,
    /// 登录名展示原文（trim 后，保留大小写）。
    pub username: String,
    /// 归一化登录名键；数据库唯一约束对象。
    pub username_key: Username,
    pub password_hash: PasswordHash,
    pub display_name: String,
    pub registered_at: DateTime<Utc>,
}

/// 登录凭据查询结果；只包含验证所需字段。
pub struct CredentialUser {
    pub id: UserId,
    pub password_hash: PasswordHash,
}

/// 账户用户。
#[derive(Clone, Debug)]
pub struct User {
    pub id: UserId,
    /// 登录名展示原文。
    pub username: String,
    /// 展示昵称，1..=40 字符。
    pub display_name: String,
    /// 学校，可空，≤80 字符。
    pub school: Option<String>,
    /// 专业，可空，≤80 字符。
    pub major: Option<String>,
    /// 毕业年份，1980..=2100，可空。
    pub graduation_year: Option<u16>,
    /// 目标岗位，≤10 项、单项 ≤40 字符。
    pub target_roles: Vec<String>,
    /// 目标城市，≤10 项、单项 ≤40 字符。
    pub target_cities: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 资料修改输入；PATCH 语义：外层 None = 不修改，内层 None = 清空。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfilePatch {
    /// Some = 整体替换（1..=40 字符）。
    pub display_name: Option<String>,
    pub school: Option<Option<String>>,
    pub major: Option<Option<String>>,
    pub graduation_year: Option<Option<u16>>,
    pub target_roles: Option<Vec<String>>,
    pub target_cities: Option<Vec<String>>,
}

impl ProfilePatch {
    /// 字段边界校验（plan/01 §3.1）；数据库 CHECK 兜底。
    pub fn validate(&self) -> Result<(), AccountError> {
        if let Some(name) = &self.display_name {
            let chars = name.chars().count();
            if !(1..=40).contains(&chars) {
                return Err(AccountError::InvalidProfile {
                    field: "displayName",
                    reason: "must be 1..=40 characters",
                });
            }
        }
        for (field, value) in [("school", &self.school), ("major", &self.major)] {
            if let Some(Some(value)) = value {
                let chars = value.chars().count();
                if chars > 80 {
                    return Err(AccountError::InvalidProfile {
                        field,
                        reason: "must be at most 80 characters",
                    });
                }
            }
        }
        if let Some(Some(year)) = self.graduation_year
            && !(1980..=2100).contains(&year)
        {
            return Err(AccountError::InvalidProfile {
                field: "graduationYear",
                reason: "must be between 1980 and 2100",
            });
        }
        for (field, value) in [
            ("targetRoles", &self.target_roles),
            ("targetCities", &self.target_cities),
        ] {
            if let Some(value) = value {
                if value.len() > 10 {
                    return Err(AccountError::InvalidProfile {
                        field,
                        reason: "must contain at most 10 items",
                    });
                }
                if value.iter().any(|item| item.chars().count() > 40) {
                    return Err(AccountError::InvalidProfile {
                        field,
                        reason: "each item must be at most 40 characters",
                    });
                }
            }
        }
        Ok(())
    }
}

/// account 模块错误；HTTP 层映射为统一错误体。
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("invalid username: {reason}")]
    InvalidUsername { reason: &'static str },
    #[error("invalid password: {reason}")]
    InvalidPassword { reason: &'static str },
    #[error("invalid profile field `{field}`: {reason}")]
    InvalidProfile {
        field: &'static str,
        reason: &'static str,
    },
    /// 登录失败统一文案，不区分用户不存在与密码错误。
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("current password does not match")]
    CurrentPasswordMismatch,
    #[error("username already taken")]
    UsernameTaken,
    #[error("session is missing, expired or revoked")]
    Unauthenticated,
    #[error("account backend error: {message}")]
    Backend {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}
