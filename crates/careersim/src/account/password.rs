//! 密码哈希端口与 Argon2id 实现（OWASP 推荐参数）。

use argon2::password_hash::{self, PasswordHasher as _, PasswordVerifier as _};
use argon2::{Algorithm, Argon2, Params, Version};

use super::model::{Password, PasswordHash};

/// OWASP 推荐下限：19 MiB 内存。
const MEMORY_KIB_DEFAULT: u32 = 19_456;
/// OWASP 推荐下限：时间成本 2。
const TIME_COST_DEFAULT: u32 = 2;
/// OWASP 推荐下限：并行度 1。
const PARALLELISM_DEFAULT: u32 = 1;

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("argon2 parameters are invalid")]
    Params(#[source] argon2::Error),
    #[error("failed to hash password")]
    Hash(#[source] password_hash::Error),
    #[error("failed to verify password")]
    Verify(#[source] password_hash::Error),
}

/// 同步端口；Argon2 属 CPU 密集任务，由服务层放入 `spawn_blocking` 执行。
pub trait PasswordHasher: Send + Sync {
    fn hash(&self, password: &Password) -> Result<PasswordHash, PasswordError>;
    /// 恒时比较由 Argon2 实现保证。
    fn verify(&self, password: &Password, hash: &PasswordHash) -> Result<bool, PasswordError>;
    /// 拉平未知用户与已知用户的响应时间，防用户名枚举。
    fn dummy_verify(&self, password: &Password);
}

/// Argon2id 哈希器；参数集中在构造期固定。
pub struct Argon2idHasher {
    params: Params,
    dummy_hash: PasswordHash,
}

impl Argon2idHasher {
    /// 使用 OWASP 推荐下限构造；同时预计算一次假哈希供 `dummy_verify` 使用。
    pub fn new() -> Result<Self, PasswordError> {
        Self::with_params(MEMORY_KIB_DEFAULT, TIME_COST_DEFAULT, PARALLELISM_DEFAULT)
    }

    /// 以显式参数构造；参数合法性由 `Params::new` 校验。
    pub fn with_params(
        memory_kib: u32,
        time_cost: u32,
        parallelism: u32,
    ) -> Result<Self, PasswordError> {
        let params =
            Params::new(memory_kib, time_cost, parallelism, None).map_err(PasswordError::Params)?;
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params.clone());
        // 盐由 password-hash 内部用 OS 熵生成（getrandom feature）。
        let dummy_hash = argon
            .hash_password(DUMMY_PASSWORD.as_bytes())
            .map_err(PasswordError::Hash)?
            .to_string();
        Ok(Self {
            params,
            dummy_hash: PasswordHash::new(dummy_hash),
        })
    }
}

/// 假验证使用的固定密码；只用于拉平耗时，不对应任何真实账户。
const DUMMY_PASSWORD: &str = "careersim-dummy-password";

impl PasswordHasher for Argon2idHasher {
    fn hash(&self, password: &Password) -> Result<PasswordHash, PasswordError> {
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, self.params.clone());
        let phc = argon
            .hash_password(password.as_str().as_bytes())
            .map_err(PasswordError::Hash)?
            .to_string();
        Ok(PasswordHash::new(phc))
    }

    fn verify(&self, password: &Password, hash: &PasswordHash) -> Result<bool, PasswordError> {
        let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, self.params.clone());
        match argon.verify_password(password.as_str().as_bytes(), hash.as_str()) {
            Ok(()) => Ok(true),
            // 密码不匹配是正常业务结果；其余错误才是后端故障。
            Err(password_hash::Error::PasswordInvalid) => Ok(false),
            Err(err) => Err(PasswordError::Verify(err)),
        }
    }

    fn dummy_verify(&self, password: &Password) {
        // 与真实验证执行同样的 Argon2 计算；结果必然不匹配，忽略即可。
        let _ = self.verify(password, &self.dummy_hash.clone());
    }
}
