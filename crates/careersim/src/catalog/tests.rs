//! 模板校验单元测试：错误收集、JSON 路径与边界规则。
//!
//! dry-run 与创建共用 [`validate_template`]，这里直接覆盖该函数。

use serde_json::{Value, json};

use super::service::CatalogService;
use super::template::{self, TemplateError};

fn rubric(pass_score: u8) -> Value {
    json!({
        "passScore": pass_score,
        "dimensions": [
            {"key": "communication", "label": "沟通表达", "weight": 100, "criteria": "表达清晰、逻辑连贯"}
        ]
    })
}

fn stage(key: &str, kind: &str, title: &str) -> Value {
    json!({
        "key": key,
        "kind": kind,
        "title": title,
        "minRounds": 1,
        "maxRounds": 3,
        "rubric": rubric(60),
    })
}

fn two_stage_pipeline(key: &str, project_key: &str, name: &str) -> Value {
    json!({
        "key": key,
        "projectKey": project_key,
        "name": name,
        "stages": [
            stage("hr_screen", "hr_screen", "HR 初筛"),
            stage("offer", "offer", "Offer 沟通"),
        ],
    })
}

/// 一份能通过校验的最小模板；各测试在其上做变异。
fn valid_template() -> Value {
    json!({
        "schemaVersion": 1,
        "company": {
            "name": "虚构星桥科技",
            "industry": "人工智能",
        },
        "projects": [
            {"key": "p1", "name": "2027 校招"},
        ],
        "pipelines": [two_stage_pipeline("pipe1", "p1", "校招主流程")],
        "jobs": [
            {
                "key": "j1",
                "projectKey": "p1",
                "pipelineKey": "pipe1",
                "title": "后端工程师",
                "jobType": "campus",
                "salaryMin": 15000,
                "salaryMax": 25000,
                "salaryMonths": 14,
            },
        ],
    })
}

fn errors_of(raw: &Value) -> Vec<TemplateError> {
    template::validate_template(raw).expect_err("template should fail validation")
}

fn find<'a>(errors: &'a [TemplateError], path: &str) -> Option<&'a TemplateError> {
    errors.iter().find(|error| error.path == path)
}

#[test]
fn valid_template_passes() {
    let validated = template::validate_template(&valid_template()).expect("should validate");
    assert_eq!(validated.company.name, "虚构星桥科技");
    assert!(!validated.company.is_private);
    assert_eq!(validated.projects.len(), 1);
    assert_eq!(
        validated.projects[0].status,
        super::model::ProjectStatus::Open
    );
    assert_eq!(validated.pipelines.len(), 1);
    assert_eq!(validated.pipelines[0].key, "pipe1");
    assert_eq!(validated.pipelines[0].definition.stages().len(), 2);
    assert_eq!(validated.jobs.len(), 1);
    assert_eq!(validated.jobs[0].job_type, super::model::JobType::Campus);
    // 未提供 status 的岗位缺省为草稿。
    assert_eq!(validated.jobs[0].status, super::model::JobStatus::Draft);
}

#[test]
fn unknown_schema_version_rejected() {
    let mut raw = valid_template();
    raw["schemaVersion"] = json!(2);
    let errors = errors_of(&raw);
    assert!(
        find(&errors, "schemaVersion").is_some(),
        "errors: {errors:?}"
    );
}

#[test]
fn duplicate_keys_rejected() {
    let mut raw = valid_template();
    raw["projects"] = json!([
        {"key": "p1", "name": "2027 校招"},
        {"key": "p1", "name": "另一个项目"},
    ]);
    raw["pipelines"] = json!([
        two_stage_pipeline("pipe1", "p1", "校招主流程"),
        two_stage_pipeline("pipe1", "p1", "另一条流水线"),
    ]);
    raw["jobs"] = json!([
        {
            "key": "j1",
            "projectKey": "p1",
            "pipelineKey": "pipe1",
            "title": "后端工程师",
            "jobType": "campus",
        },
        {
            "key": "j1",
            "projectKey": "p1",
            "pipelineKey": "pipe1",
            "title": "前端工程师",
            "jobType": "campus",
        },
    ]);
    let errors = errors_of(&raw);
    assert!(
        find(&errors, "projects[1].key").is_some(),
        "errors: {errors:?}"
    );
    assert!(
        find(&errors, "pipelines[1].key").is_some(),
        "errors: {errors:?}"
    );
    assert!(find(&errors, "jobs[1].key").is_some(), "errors: {errors:?}");

    // 流水线内重复阶段键。
    let mut raw = valid_template();
    raw["pipelines"][0]["stages"][1] = stage("hr_screen", "hr_screen", "重复键");
    let errors = errors_of(&raw);
    assert!(
        find(&errors, "pipelines[0].stages[1].key").is_some(),
        "errors: {errors:?}"
    );
}

#[test]
fn duplicate_project_name_case_insensitive() {
    let mut raw = valid_template();
    raw["projects"] = json!([
        {"key": "p1", "name": "2027 校招"},
        {"key": "p2", "name": "2027 校招 "},
    ]);
    let errors = errors_of(&raw);
    assert!(
        find(&errors, "projects[1].name").is_some(),
        "errors: {errors:?}"
    );
}

#[test]
fn dangling_project_reference_rejected() {
    let mut raw = valid_template();
    raw["pipelines"][0]["projectKey"] = json!("nope");
    raw["jobs"][0]["projectKey"] = json!("nope");
    let errors = errors_of(&raw);
    assert!(
        find(&errors, "pipelines[0].projectKey").is_some(),
        "errors: {errors:?}"
    );
    assert!(
        find(&errors, "jobs[0].projectKey").is_some(),
        "errors: {errors:?}"
    );
}

#[test]
fn dangling_pipeline_reference_rejected() {
    let mut raw = valid_template();
    raw["jobs"][0]["pipelineKey"] = json!("nope");
    let errors = errors_of(&raw);
    assert!(
        find(&errors, "jobs[0].pipelineKey").is_some(),
        "errors: {errors:?}"
    );
}

#[test]
fn cross_project_pipeline_reference_rejected() {
    let mut raw = valid_template();
    raw["projects"] = json!([
        {"key": "p1", "name": "2027 校招"},
        {"key": "p2", "name": "社招通道"},
    ]);
    raw["pipelines"] = json!([
        two_stage_pipeline("pipe1", "p1", "校招主流程"),
        two_stage_pipeline("pipe2", "p2", "社招流程"),
    ]);
    raw["jobs"] = json!([
        {
            "key": "j1",
            "projectKey": "p1",
            "pipelineKey": "pipe1",
            "title": "后端工程师",
            "jobType": "campus",
        },
        {
            "key": "j2",
            "projectKey": "p1",
            "pipelineKey": "pipe2",
            "title": "前端工程师",
            "jobType": "campus",
        },
    ]);
    let errors = errors_of(&raw);
    let error = find(&errors, "jobs[1].pipelineKey").expect("cross-project reference must fail");
    assert!(
        error.message.contains("belongs to project"),
        "errors: {errors:?}"
    );
}

#[test]
fn offer_not_last_rejected() {
    let mut raw = valid_template();
    raw["pipelines"][0]["stages"] = json!([
        stage("offer", "offer", "Offer 沟通"),
        stage("hr_screen", "hr_screen", "HR 初筛"),
    ]);
    let errors = errors_of(&raw);
    assert!(
        find(&errors, "pipelines[0].stages[1].kind").is_some(),
        "errors: {errors:?}"
    );
}

#[test]
fn pass_score_out_of_range_rejected() {
    let mut raw = valid_template();
    raw["pipelines"][0]["stages"][0]["rubric"]["passScore"] = json!(101);
    let errors = errors_of(&raw);
    assert!(
        find(&errors, "pipelines[0].stages[0]").is_some(),
        "errors: {errors:?}"
    );
}

#[test]
fn field_bounds_rejected() {
    let mut raw = valid_template();
    raw["company"]["name"] = json!("名".repeat(81));
    raw["company"]["highlights"] = json!(["亮点"].repeat(13));
    raw["company"]["logoText"] = json!("超长头像文字超限了");
    raw["company"]["accent"] = json!("#0123456789abcdef");
    raw["projects"][0]["name"] = json!("名".repeat(61));
    let errors = errors_of(&raw);
    assert!(
        find(&errors, "company.name").is_some(),
        "errors: {errors:?}"
    );
    assert!(
        find(&errors, "company.highlights").is_some(),
        "errors: {errors:?}"
    );
    assert!(
        find(&errors, "company.logoText").is_some(),
        "errors: {errors:?}"
    );
    assert!(
        find(&errors, "company.accent").is_some(),
        "errors: {errors:?}"
    );
    assert!(
        find(&errors, "projects[0].name").is_some(),
        "errors: {errors:?}"
    );
}

#[test]
fn salary_rules_rejected() {
    let mut raw = valid_template();
    raw["jobs"] = json!([
        {
            "key": "j1",
            "projectKey": "p1",
            "pipelineKey": "pipe1",
            "title": "后端工程师",
            "jobType": "campus",
            "salaryMin": 25000,
            "salaryMax": 15000,
        },
        {
            "key": "j2",
            "projectKey": "p1",
            "pipelineKey": "pipe1",
            "title": "前端工程师",
            "jobType": "internship",
            "salaryMonths": 25,
        },
    ]);
    let errors = errors_of(&raw);
    assert!(
        find(&errors, "jobs[0].salaryMax").is_some(),
        "errors: {errors:?}"
    );
    assert!(
        find(&errors, "jobs[1].salaryMonths").is_some(),
        "errors: {errors:?}"
    );
}

#[test]
fn errors_are_collected_not_first_only() {
    let mut raw = valid_template();
    raw["schemaVersion"] = json!(0);
    raw["company"]["industry"] = json!("");
    raw["jobs"][0]["jobType"] = json!("旋转门");
    let errors = errors_of(&raw);
    assert!(errors.len() >= 3, "errors: {errors:?}");
}

#[test]
fn service_dry_run_shares_the_same_validator() {
    // dry-run（CatalogService::validate_template）与创建共用同一校验函数，
    // 对合法与非法输入必须给出一致结果。
    let valid = valid_template();
    assert!(CatalogService::validate_template(&valid).is_ok());

    let mut invalid = valid;
    invalid["company"]["name"] = json!("");
    let service_errors =
        CatalogService::validate_template(&invalid).expect_err("invalid template must fail");
    let direct_errors = errors_of(&invalid);
    assert_eq!(service_errors, direct_errors);
}
