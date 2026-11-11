//! 简历 PostgreSQL 集成测试；未设置 `DATABASE_URL` 时跳过。
//!
//! 覆盖：份数上限（行锁路径）、默认简历唯一与删除后提升、附件上传解析失败
//! 保留附件、下载流一致性、超限/空文件/格式不符拒绝。

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use careersim::account::model::UserId;
use careersim::account::password::Argon2idHasher;
use careersim::account::postgres_account_service;
use careersim::resume::model::MAX_ATTACHMENT_BYTES;
use careersim::resume::storage::{LocalFsAttachmentStore, read_attachment_bytes};
use careersim::resume::{ParseStatus, ResumeContent, ResumeError, postgres_resume_service};
use sqlx::postgres::PgPoolOptions;

async fn setup(tag: &str) -> Option<(UserId, careersim::resume::ResumeService, PathBuf)> {
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

    // 每个用例注册独立用户，避免测试间干扰。
    let hasher = Arc::new(Argon2idHasher::new().expect("argon2 hasher"));
    let account = postgres_account_service(pool.clone(), hasher);
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let grant = account
        .register(
            &format!("b2{tag}{}", suffix % 1_000_000_000),
            "password123",
            "B2 集成测试",
        )
        .await
        .expect("register user");
    let owner = grant.user.id;

    let root = std::env::temp_dir().join(format!(
        "careersim-b2-resume-it-{tag}-{}",
        uuid::Uuid::new_v4()
    ));
    let service = postgres_resume_service(pool, Arc::new(LocalFsAttachmentStore::new(&root)));
    Some((owner, service, root))
}

fn fake_pdf_bytes() -> Vec<u8> {
    let mut bytes = b"%PDF-1.4\n".to_vec();
    bytes.extend(std::iter::repeat_n(b'x', 1024));
    bytes
}

fn count_files(root: &Path) -> usize {
    std::fs::read_dir(root)
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

#[tokio::test]
async fn resume_limit_and_default_promotion() {
    let Some((owner, service, root)) = setup("limit").await else {
        return;
    };

    let mut created = Vec::new();
    for index in 1..=3 {
        let resume = service
            .create_online(owner, &format!("简历{index}"), ResumeContent::default())
            .await
            .expect("create online resume");
        created.push(resume.id);
        // 拉开 updated_at，保证“提升最新一份”的判定确定。
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    // 首份自动成为默认；列表默认在前、新→旧排序。
    let list = service.list(owner).await.expect("list");
    assert_eq!(list.len(), 3);
    assert!(list[0].is_default);
    assert_eq!(list[0].id, created[0]);

    // 第 4 份触发上限。
    assert!(matches!(
        service
            .create_online(owner, "第四份", ResumeContent::default())
            .await,
        Err(ResumeError::LimitReached)
    ));

    // 上限已满时上传附件：DB 写入失败 → 已落盘附件被清理。
    assert!(matches!(
        service
            .upload_attachment(owner, "第五份", "resume.pdf", &fake_pdf_bytes())
            .await,
        Err(ResumeError::LimitReached)
    ));
    assert_eq!(count_files(&root), 0, "孤儿附件文件应被清理");

    // 删除默认简历 → 事务内提升最新一份。
    service
        .delete(owner, created[0])
        .await
        .expect("delete default");
    let list = service.list(owner).await.expect("list after delete");
    assert_eq!(list.len(), 2);
    assert!(list[0].is_default);
    assert_eq!(list[0].id, created[2], "最新的简历应被提升为默认");

    // 删除非默认简历不影响默认。
    service
        .delete(owner, created[1])
        .await
        .expect("delete non-default");
    let list = service.list(owner).await.expect("list after second delete");
    assert_eq!(list.len(), 1);
    assert!(list[0].is_default && list[0].id == created[2]);

    std::fs::remove_dir_all(&root).ok();
}

#[tokio::test]
async fn attachment_upload_parse_and_download() {
    let Some((owner, service, root)) = setup("attachment").await else {
        return;
    };

    // 伪造 PDF：双重格式校验通过，但解析失败（corrupt）→ 附件保留、状态 failed。
    let fake_pdf = fake_pdf_bytes();
    let resume = service
        .upload_attachment(owner, "附件简历", "我的简历.pdf", &fake_pdf)
        .await
        .expect("upload attachment");
    assert!(resume.attachment.is_some());
    assert!(matches!(resume.parse_status, ParseStatus::Failed { .. }));
    let attachment = resume.attachment.as_ref().expect("attachment");
    assert_eq!(attachment.file_name, "我的简历.pdf");
    assert_eq!(attachment.mime, "application/pdf");
    assert_eq!(attachment.size, fake_pdf.len() as u64);

    // 下载流内容一致，且不暴露存储键。
    let opened = service
        .open_attachment(owner, resume.id)
        .await
        .expect("open attachment");
    assert_eq!(opened.file_name, "我的简历.pdf");
    let bytes = read_attachment_bytes(opened.stream)
        .await
        .expect("read attachment");
    assert_eq!(bytes, fake_pdf);

    // 在线简历没有附件可下载。
    let online = service
        .create_online(owner, "在线", ResumeContent::default())
        .await
        .expect("create online");
    assert!(matches!(
        service.open_attachment(owner, online.id).await,
        Err(ResumeError::NotFound)
    ));

    // 超限、空文件、扩展名与内容不符一律拒绝。
    let oversized = vec![0u8; MAX_ATTACHMENT_BYTES + 1];
    assert!(matches!(
        service
            .upload_attachment(owner, "超限", "big.pdf", &oversized)
            .await,
        Err(ResumeError::AttachmentTooLarge)
    ));
    assert!(matches!(
        service
            .upload_attachment(owner, "空文件", "empty.pdf", b"")
            .await,
        Err(ResumeError::UnsupportedAttachment)
    ));
    assert!(matches!(
        service
            .upload_attachment(owner, "伪装", "fake.docx", &fake_pdf)
            .await,
        Err(ResumeError::UnsupportedAttachment)
    ));

    // 清理：软删除后列表为空，附件文件被移除。
    service
        .delete(owner, resume.id)
        .await
        .expect("delete attachment");
    service
        .delete(owner, online.id)
        .await
        .expect("delete online");
    assert_eq!(service.list(owner).await.expect("list").len(), 0);
    assert_eq!(count_files(&root), 0);

    std::fs::remove_dir_all(&root).ok();
}
