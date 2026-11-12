//! 账户服务单元测试：内存端口替身 + FakeClock/FakeHasher（替身见 `doubles.rs`）。

use std::sync::Arc;

use chrono::Duration;

use super::doubles::{FakeClock, FakeHasher, InMemorySessionStore, InMemoryUserStore};
use crate::account::model::{AccountError, ProfilePatch, UserId, Username};
use crate::account::service::AccountService;

// ---------- 装配 ----------

struct Harness {
    service: AccountService,
    clock: Arc<FakeClock>,
    hasher: Arc<FakeHasher>,
}

fn harness() -> Harness {
    let clock = Arc::new(FakeClock::new());
    let hasher = Arc::new(FakeHasher::new());
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
