//! catalog PostgreSQL 集成测试；未设置 `DATABASE_URL` 时跳过。
//!
//! 运行前需可用的 PostgreSQL（compose.yaml）；迁移由测试自行执行。
//! 覆盖：种子只读、可见性（他人公开可读不可写、他人私有不可见）、
//! 跨用户写拒绝、命名冲突、软删后重建、流水线跨项目引用。

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use careersim::account::UserId;
use careersim::account::password::Argon2idHasher;
use careersim::account::postgres_account_service;
use careersim::catalog::model::{CatalogError, Company, CompanySource};
use careersim::catalog::service::{
    CompanyInput, CompanyPatchInput, JobInput, JobPatchInput, PipelineInput, ProjectInput,
    ProjectPatchInput,
};
use careersim::catalog::store::CatalogScope;
use careersim::catalog::{
    CatalogService, CompanyId, JobStatus, ProjectId, ProjectStatus, postgres_catalog_service,
};
use careersim::pipeline::StageDefinition;
use serde_json::{Value, json};
use sqlx::postgres::PgPoolOptions;

use careersim::account::AccountService;

async fn setup() -> Option<(AccountService, CatalogService)> {
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
    Some((
        postgres_account_service(pool.clone(), hasher),
        postgres_catalog_service(pool),
    ))
}

fn unique_suffix() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_millis()
}

async fn new_user(service: &AccountService, tag: &str) -> UserId {
    service
        .register(
            &format!("b1_{}_{}", tag, unique_suffix()),
            "password123",
            "B1 测试",
        )
        .await
        .expect("register")
        .user
        .id
}

fn stage(key: &str, kind: &str, title: &str) -> StageDefinition {
    serde_json::from_value(json!({
        "key": key,
        "kind": kind,
        "title": title,
        "minRounds": 1,
        "maxRounds": 3,
        "rubric": {
            "passScore": 60,
            "dimensions": [
                {"key": "communication", "label": "沟通表达", "weight": 100, "criteria": "表达清晰"}
            ]
        }
    }))
    .expect("valid stage definition")
}

fn two_stage_input() -> Vec<StageDefinition> {
    vec![
        stage("hr_screen", "hr_screen", "HR 初筛"),
        stage("offer", "offer", "Offer 沟通"),
    ]
}

async fn create_company(
    catalog: &CatalogService,
    owner: UserId,
    name: &str,
    is_private: bool,
) -> Company {
    catalog
        .create_company(
            owner,
            CompanyInput {
                name: name.to_owned(),
                english_name: String::new(),
                industry: "测试行业".to_owned(),
                city: String::new(),
                size: String::new(),
                tagline: String::new(),
                description: String::new(),
                highlights: vec![],
                logo_text: String::new(),
                accent: String::new(),
                is_private,
            },
            chrono::Utc::now(),
        )
        .await
        .expect("create company")
}

/// 建一家带项目、流水线与 open/draft 岗位各一的公司，返回其 ID。
async fn create_full_company(
    catalog: &CatalogService,
    owner: UserId,
    name: &str,
    is_private: bool,
) -> CompanyId {
    let now = chrono::Utc::now();
    let company = create_company(catalog, owner, name, is_private).await;
    let project = catalog
        .create_project(
            owner,
            company.id,
            ProjectInput {
                name: "2027 校招".to_owned(),
                season: "2027".to_owned(),
                status: None,
                deadline: None,
                description: String::new(),
            },
            now,
        )
        .await
        .expect("create project");
    let pipeline = catalog
        .create_pipeline(
            owner,
            project.id,
            PipelineInput {
                name: "校招主流程".to_owned(),
                stages: Some(two_stage_input()),
            },
            now,
        )
        .await
        .expect("create pipeline");
    for (title, status) in [
        ("后端工程师", JobStatus::Open),
        ("前端工程师", JobStatus::Draft),
    ] {
        catalog
            .create_job(
                owner,
                project.id,
                JobInput {
                    pipeline_id: Some(pipeline.id),
                    title: title.to_owned(),
                    city: "上海".to_owned(),
                    job_type: None,
                    salary_min: Some(15000),
                    salary_max: Some(25000),
                    salary_months: Some(14),
                    tags: vec![],
                    description: String::new(),
                    status: Some(status),
                },
                now,
            )
            .await
            .expect("create job");
    }
    company.id
}

#[tokio::test]
async fn seed_templates_are_read_only() {
    let Some((account, catalog)) = setup().await else {
        return;
    };
    let viewer = new_user(&account, "seed_ro").await;

    // 种子模板对任意登录用户可见。
    let (items, _) = catalog
        .list_companies(viewer, CatalogScope::Public, 50, None)
        .await
        .expect("list public");
    let seed = items
        .iter()
        .find(|item| item.company.source == CompanySource::Seed)
        .expect("seed template must exist");
    assert!(seed.company.owner.is_none());
    let seed_id = seed.company.id;

    // 任何写操作都被拒绝（种子 owner 为 NULL，统一按不存在处理）。
    assert!(matches!(
        catalog
            .update_company(
                viewer,
                seed_id,
                CompanyPatchInput {
                    name: Some("篡改".to_owned()),
                    ..CompanyPatchInput::default()
                },
                chrono::Utc::now(),
            )
            .await,
        Err(CatalogError::NotFound)
    ));
    assert!(matches!(
        catalog
            .delete_company(viewer, seed_id, chrono::Utc::now())
            .await,
        Err(CatalogError::NotFound)
    ));
    assert!(matches!(
        catalog
            .create_project(
                viewer,
                seed_id,
                ProjectInput {
                    name: "植入项目".to_owned(),
                    season: String::new(),
                    status: None,
                    deadline: None,
                    description: String::new(),
                },
                chrono::Utc::now(),
            )
            .await,
        Err(CatalogError::NotFound)
    ));

    // 公开列表里种子模板仍然原样可见。
    let detail = catalog
        .get_company(viewer, seed_id)
        .await
        .expect("get seed")
        .expect("seed visible");
    assert!(!detail.company.is_owned_by(viewer));
}

#[tokio::test]
async fn others_public_templates_readable_not_writable() {
    let Some((account, catalog)) = setup().await else {
        return;
    };
    let alice = new_user(&account, "pub_a").await;
    let bob = new_user(&account, "pub_b").await;

    let company_id = create_full_company(
        &catalog,
        alice,
        &format!("公开公司{}", unique_suffix()),
        false,
    )
    .await;

    // Bob 可以读取公开模板与其 open 岗位（draft 岗位不可见）。
    let detail = catalog
        .get_company(bob, company_id)
        .await
        .expect("get")
        .expect("public visible");
    assert_eq!(detail.projects.len(), 1);
    assert_eq!(detail.pipelines.len(), 1);
    assert_eq!(detail.jobs.len(), 1);
    assert_eq!(detail.jobs[0].status, JobStatus::Open);

    // Bob 的公开列表包含它；写操作一律拒绝。
    let (items, _) = catalog
        .list_companies(bob, CatalogScope::Public, 50, None)
        .await
        .expect("list");
    assert!(items.iter().any(|item| item.company.id == company_id));

    assert!(matches!(
        catalog
            .update_company(
                bob,
                company_id,
                CompanyPatchInput {
                    name: Some("抢改".to_owned()),
                    ..CompanyPatchInput::default()
                },
                chrono::Utc::now(),
            )
            .await,
        Err(CatalogError::NotFound)
    ));
    assert!(matches!(
        catalog
            .delete_company(bob, company_id, chrono::Utc::now())
            .await,
        Err(CatalogError::NotFound)
    ));
    assert!(matches!(
        catalog
            .create_project(
                bob,
                company_id,
                ProjectInput {
                    name: "越权项目".to_owned(),
                    season: String::new(),
                    status: None,
                    deadline: None,
                    description: String::new(),
                },
                chrono::Utc::now(),
            )
            .await,
        Err(CatalogError::NotFound)
    ));

    // Alice 自己能看到 draft 岗位。
    let own = catalog
        .get_company(alice, company_id)
        .await
        .expect("get")
        .expect("owner visible");
    assert_eq!(own.jobs.len(), 2);
}

#[tokio::test]
async fn others_private_templates_are_invisible() {
    let Some((account, catalog)) = setup().await else {
        return;
    };
    let alice = new_user(&account, "priv_a").await;
    let bob = new_user(&account, "priv_b").await;

    let company_id = create_full_company(
        &catalog,
        alice,
        &format!("私有公司{}", unique_suffix()),
        true,
    )
    .await;

    // Bob 不可见（详情 None、列表不出现）。
    assert!(matches!(
        catalog.get_company(bob, company_id).await,
        Ok(None)
    ));
    let (public_items, _) = catalog
        .list_companies(bob, CatalogScope::Public, 50, None)
        .await
        .expect("list public");
    assert!(
        !public_items
            .iter()
            .any(|item| item.company.id == company_id)
    );
    let (all_items, _) = catalog
        .list_companies(bob, CatalogScope::All, 50, None)
        .await
        .expect("list all");
    assert!(!all_items.iter().any(|item| item.company.id == company_id));

    // Alice 自己在 Mine 与 All 中可见，且能看到 draft 岗位。
    let (mine, _) = catalog
        .list_companies(alice, CatalogScope::Mine, 50, None)
        .await
        .expect("list mine");
    assert!(mine.iter().any(|item| item.company.id == company_id));
    let detail = catalog
        .get_company(alice, company_id)
        .await
        .expect("get")
        .expect("owner visible");
    assert!(detail.company.is_private);
    assert_eq!(detail.jobs.len(), 2);
}

#[tokio::test]
async fn company_name_conflicts_within_owner() {
    let Some((account, catalog)) = setup().await else {
        return;
    };
    let alice = new_user(&account, "conf_a").await;
    let bob = new_user(&account, "conf_b").await;

    let name = format!("冲突公司{}", unique_suffix());
    create_company(&catalog, alice, &name, false).await;

    // 同名（含首尾空白变体，归一化后同键）在同一 owner 内冲突。
    let err = catalog
        .create_company(
            alice,
            CompanyInput {
                name: format!("{name} "),
                industry: "测试行业".to_owned(),
                ..CompanyInput::default()
            },
            chrono::Utc::now(),
        )
        .await
        .expect_err("trimmed duplicate must conflict");
    assert!(matches!(err, CatalogError::Conflict("name")), "got {err:?}");

    // 不同 owner 之间允许同名。
    let _bob_company = create_company(&catalog, bob, &name, false).await;
}

#[tokio::test]
async fn soft_delete_then_recreate() {
    let Some((account, catalog)) = setup().await else {
        return;
    };
    let alice = new_user(&account, "del_a").await;

    let name = format!("重建公司{}", unique_suffix());
    let company_id = create_full_company(&catalog, alice, &name, false).await;

    catalog
        .delete_company(alice, company_id, chrono::Utc::now())
        .await
        .expect("delete");

    // 已删除后不可见（含 owner 本人）。
    assert!(matches!(
        catalog.get_company(alice, company_id).await,
        Ok(None)
    ));
    let (mine, _) = catalog
        .list_companies(alice, CatalogScope::Mine, 50, None)
        .await
        .expect("list mine");
    assert!(!mine.iter().any(|item| item.company.id == company_id));

    // 同名可重建（部分唯一索引只约束未删除行）。
    let recreated = create_company(&catalog, alice, &name, false).await;
    assert_ne!(recreated.id, company_id);
    assert!(
        catalog
            .get_company(alice, recreated.id)
            .await
            .expect("get")
            .is_some()
    );
}

#[tokio::test]
async fn pipeline_must_belong_to_same_project() {
    let Some((account, catalog)) = setup().await else {
        return;
    };
    let alice = new_user(&account, "fk_a").await;
    let now = chrono::Utc::now();

    let company = create_company(
        &catalog,
        alice,
        &format!("外键公司{}", unique_suffix()),
        false,
    )
    .await;
    let project_a = catalog
        .create_project(
            alice,
            company.id,
            ProjectInput {
                name: "项目甲".to_owned(),
                season: String::new(),
                status: None,
                deadline: None,
                description: String::new(),
            },
            now,
        )
        .await
        .expect("project a");
    let project_b = catalog
        .create_project(
            alice,
            company.id,
            ProjectInput {
                name: "项目乙".to_owned(),
                season: String::new(),
                status: None,
                deadline: None,
                description: String::new(),
            },
            now,
        )
        .await
        .expect("project b");
    let pipeline = catalog
        .create_pipeline(
            alice,
            project_a.id,
            PipelineInput {
                name: "甲流程".to_owned(),
                stages: Some(two_stage_input()),
            },
            now,
        )
        .await
        .expect("pipeline");

    // 复合外键：岗位引用其他项目的流水线被拒绝。
    let err = catalog
        .create_job(
            alice,
            project_b.id,
            JobInput {
                pipeline_id: Some(pipeline.id),
                title: "跨项目岗位".to_owned(),
                city: String::new(),
                job_type: None,
                salary_min: None,
                salary_max: None,
                salary_months: None,
                tags: vec![],
                description: String::new(),
                status: None,
            },
            now,
        )
        .await
        .expect_err("cross-project pipeline must be rejected");
    assert!(
        matches!(
            err,
            CatalogError::InvalidReference {
                field: "pipelineId"
            }
        ),
        "got {err:?}"
    );

    // 更新为跨项目流水线同样拒绝；同项目合法。
    let job = catalog
        .create_job(
            alice,
            project_a.id,
            JobInput {
                pipeline_id: Some(pipeline.id),
                title: "合法岗位".to_owned(),
                city: String::new(),
                job_type: None,
                salary_min: None,
                salary_max: None,
                salary_months: None,
                tags: vec![],
                description: String::new(),
                status: Some(JobStatus::Open),
            },
            now,
        )
        .await
        .expect("job");
    let err = catalog
        .update_job(
            alice,
            job.id,
            careersim::catalog::service::JobPatchInput {
                pipeline_id: Some(
                    catalog
                        .create_pipeline(
                            alice,
                            project_b.id,
                            PipelineInput {
                                name: "乙流程".to_owned(),
                                stages: Some(two_stage_input()),
                            },
                            now,
                        )
                        .await
                        .expect("pipeline b")
                        .id,
                ),
                ..JobPatchInput::default()
            },
            now,
        )
        .await
        .expect_err("cross-project update must be rejected");
    assert!(
        matches!(
            err,
            CatalogError::InvalidReference {
                field: "pipelineId"
            }
        ),
        "got {err:?}"
    );
}

#[tokio::test]
async fn import_template_roundtrip() {
    let Some((account, catalog)) = setup().await else {
        return;
    };
    let alice = new_user(&account, "imp_a").await;
    let bob = new_user(&account, "imp_b").await;

    let name = format!("导入公司{}", unique_suffix());
    let template: Value = json!({
        "schemaVersion": 1,
        "company": {
            "name": name,
            "industry": "虚构行业",
            "isPrivate": false,
        },
        "projects": [{"key": "p1", "name": "2027 校招", "status": "open"}],
        "pipelines": [{
            "key": "pipe1",
            "projectKey": "p1",
            "name": "校招主流程",
            "stages": [
                {"key": "hr_screen", "kind": "hr_screen", "title": "HR 初筛", "minRounds": 1, "maxRounds": 3,
                 "rubric": {"passScore": 60, "dimensions": [
                     {"key": "communication", "label": "沟通表达", "weight": 100, "criteria": "表达清晰"}]}},
                {"key": "offer", "kind": "offer", "title": "Offer 沟通", "minRounds": 1, "maxRounds": 2,
                 "rubric": {"passScore": 70, "dimensions": [
                     {"key": "motivation", "label": "求职动机", "weight": 100, "criteria": "动机明确"}]}}
            ],
        }],
        "jobs": [{
            "key": "j1",
            "projectKey": "p1",
            "pipelineKey": "pipe1",
            "title": "后端工程师",
            "jobType": "campus",
            "status": "open",
            "salaryMin": 15000,
            "salaryMax": 25000,
            "salaryMonths": 14,
        }],
    });

    // dry-run 与创建共用同一校验函数。
    assert!(CatalogService::validate_template(&template).is_ok());
    let company_id = catalog
        .import_template(alice, &template, chrono::Utc::now())
        .await
        .expect("import");

    // 导入结果完整可读；Bob 仅见 open 岗位。
    let detail = catalog
        .get_company(alice, company_id)
        .await
        .expect("get")
        .expect("imported visible");
    assert_eq!(detail.projects.len(), 1);
    assert_eq!(detail.pipelines.len(), 1);
    assert_eq!(detail.pipelines[0].definition.stages().len(), 2);
    assert_eq!(detail.jobs.len(), 1);
    assert!(detail.company.is_owned_by(alice));

    let bob_detail = catalog
        .get_company(bob, company_id)
        .await
        .expect("get")
        .expect("public visible");
    assert_eq!(bob_detail.jobs.len(), 1);

    // 重复导入同名公司整体回滚并冲突。
    assert!(matches!(
        catalog
            .import_template(alice, &template, chrono::Utc::now())
            .await,
        Err(CatalogError::Conflict("name"))
    ));

    // 非法模板（未知版本）不落库。
    let mut invalid = template.clone();
    invalid["schemaVersion"] = json!(2);
    assert!(matches!(
        catalog
            .import_template(alice, &invalid, chrono::Utc::now())
            .await,
        Err(CatalogError::InvalidTemplate(_))
    ));
}

#[tokio::test]
async fn project_and_pipeline_updates_and_deletes() {
    let Some((account, catalog)) = setup().await else {
        return;
    };
    let alice = new_user(&account, "upd_a").await;
    let bob = new_user(&account, "upd_b").await;
    let now = chrono::Utc::now();

    let company = create_full_company(
        &catalog,
        alice,
        &format!("更新公司{}", unique_suffix()),
        false,
    )
    .await;
    let detail = catalog
        .get_company(alice, company)
        .await
        .expect("get")
        .expect("visible");
    let project_id: ProjectId = detail.projects[0].id;
    let pipeline_id = detail.pipelines[0].id;

    // 项目改名 + 状态 + 清空截止日期。
    let updated_project = catalog
        .update_project(
            alice,
            project_id,
            ProjectPatchInput {
                name: Some("改名项目".to_owned()),
                status: Some(ProjectStatus::Closing),
                deadline: Some(None),
                ..ProjectPatchInput::default()
            },
            now,
        )
        .await
        .expect("update project");
    assert_eq!(updated_project.name, "改名项目");
    assert_eq!(updated_project.status, ProjectStatus::Closing);
    assert!(updated_project.deadline.is_none());

    // 流水线改名；Bob 越权拒绝。
    let renamed = catalog
        .update_pipeline(alice, pipeline_id, "新流程名", now)
        .await
        .expect("rename pipeline");
    assert_eq!(renamed.name, "新流程名");
    assert!(matches!(
        catalog
            .update_pipeline(bob, pipeline_id, "越权名", now)
            .await,
        Err(CatalogError::NotFound)
    ));

    // 删除流水线与项目；删除后项目不可见。
    catalog
        .delete_pipeline(alice, pipeline_id, now)
        .await
        .expect("delete pipeline");
    catalog
        .delete_project(alice, project_id, now)
        .await
        .expect("delete project");
    let after = catalog
        .get_company(alice, company)
        .await
        .expect("get")
        .expect("visible");
    assert!(after.projects.is_empty());
    assert!(after.pipelines.is_empty());
    assert!(after.jobs.is_empty());
}
