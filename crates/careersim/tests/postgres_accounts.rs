//! 账户 PostgreSQL 集成测试；未设置 `DATABASE_URL` 时跳过。
//!
//! 运行前需可用的 PostgreSQL（compose.yaml）并执行全部迁移。

use careersim::account::model::{AccountError, ProfilePatch};
use careersim::account::password::Argon2idHasher;
use careersim::account::postgres_account_service;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;

async fn service() -> Option<careersim::account::AccountService> {
    let Ok(url) = std::env::var("DATABASE_URL") else {
        return None;
    };
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("connect to PostgreSQL");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("run migrations");
    let hasher = Arc::new(Argon2idHasher::new().expect("argon2 hasher"));
    Some(postgres_account_service(pool, hasher))
}

fn unique_username() -> String {
    // 每次运行使用不同用户名，便于重复执行。
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_millis();
    format!("a0_test_{suffix}")
}

#[tokio::test]
async fn account_roundtrip_against_postgres() {
    let Some(service) = service().await else {
        return;
    };
    let username = unique_username();

    // 注册即登录。
    let grant = service
        .register(&username, "password123", "A0 测试")
        .await
        .expect("register");
    assert_eq!(grant.user.username, username);

    // 认证当前会话。
    let authenticated = service
        .authenticate(&grant.token)
        .await
        .expect("authenticate");
    let user_id = authenticated.user.id;

    // 登录。
    let login_grant = service
        .login(&username, "password123")
        .await
        .expect("login");

    // 资料修改（PATCH 语义）。
    let updated = service
        .update_profile(
            user_id,
            ProfilePatch {
                school: Some(Some("示例大学".to_owned())),
                graduation_year: Some(Some(2027)),
                ..ProfilePatch::default()
            },
        )
        .await
        .expect("update profile");
    assert_eq!(updated.school.as_deref(), Some("示例大学"));
    assert_eq!(updated.graduation_year, Some(2027));

    // 改密：轮换令牌并撤销其他会话。
    let new_token = service
        .change_password(
            user_id,
            authenticated.session_id,
            "password123",
            "newpassword1",
        )
        .await
        .expect("change password");
    assert!(matches!(
        service.authenticate(&login_grant.token).await,
        Err(AccountError::Unauthenticated)
    ));
    service.authenticate(&new_token).await.expect("new token");

    // 注销。
    let session = service.authenticate(&new_token).await.expect("auth");
    service.logout(session.session_id).await.expect("logout");
    assert!(matches!(
        service.authenticate(&new_token).await,
        Err(AccountError::Unauthenticated)
    ));
}

#[tokio::test]
async fn duplicate_username_conflicts() {
    let Some(service) = service().await else {
        return;
    };
    let username = unique_username();
    service
        .register(&username, "password123", "第一次")
        .await
        .expect("first register");
    assert!(matches!(
        service.register(&username, "password123", "第二次").await,
        Err(AccountError::UsernameTaken)
    ));
}

#[tokio::test]
async fn wrong_password_rejected() {
    let Some(service) = service().await else {
        return;
    };
    let username = unique_username();
    service
        .register(&username, "password123", "A0 测试")
        .await
        .expect("register");
    assert!(matches!(
        service.login(&username, "wrong-pass-1").await,
        Err(AccountError::InvalidCredentials)
    ));
}
