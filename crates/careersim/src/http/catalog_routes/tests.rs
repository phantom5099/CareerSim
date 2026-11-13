//! catalog 路由测试：状态码、错误映射与可见性语义（plan/01 §6.2）。
//!
//! 通过完整路由 + oneshot 发请求；存储用 [`InMemoryCatalogStore`]
//! （镜像 PG 的可见性、归属链与唯一约束语义）。

use axum::body::Body;
use axum::http::{Request, StatusCode};
use serde_json::{Value, json};

use crate::testing::{ALLOWED_ORIGIN, body_json, json_request, registered_cookie, send};

// ---------- 请求辅助 ----------

/// 携带 Cookie 与 Origin 的 JSON 请求。
fn authed_json(method: &str, uri: &str, cookie: &str, body: Value) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .header(axum::http::header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

/// 携带 Cookie 与 Origin 的无体请求（GET 不校验 Origin，带上无害）。
fn authed_empty(method: &str, uri: &str, cookie: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(uri)
        .header(axum::http::header::ORIGIN, ALLOWED_ORIGIN)
        .header(axum::http::header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap()
}

fn company_payload(name: &str) -> Value {
    json!({
        "name": name,
        "industry": "人工智能",
        "city": "上海",
        "highlights": ["弹性工作"],
    })
}

/// 两阶段流水线：hr_screen + offer。
fn pipeline_payload(name: &str) -> Value {
    let rubric = json!({
        "passScore": 60,
        "dimensions": [
            {"key": "communication", "label": "沟通表达", "weight": 100, "criteria": "表达清晰"},
        ],
    });
    json!({
        "name": name,
        "stages": [
            {"key": "hr_screen", "kind": "hr_screen", "title": "HR 初筛",
             "minRounds": 1, "maxRounds": 3, "rubric": rubric},
            {"key": "offer", "kind": "offer", "title": "Offer 沟通",
             "minRounds": 1, "maxRounds": 3, "rubric": rubric},
        ],
    })
}

fn job_payload(pipeline_id: &str, title: &str) -> Value {
    json!({
        "pipelineId": pipeline_id,
        "title": title,
        "city": "上海",
        "jobType": "campus",
        "salaryMin": 20000,
        "salaryMax": 30000,
        "salaryMonths": 4,
        "tags": ["rust"],
    })
}

/// 创建公司并返回 (状态码, 响应体)。
async fn create_company(app: &axum::Router, cookie: &str, name: &str) -> (StatusCode, Value) {
    let response = send(
        app.clone(),
        authed_json("POST", "/api/v1/companies", cookie, company_payload(name)),
    )
    .await;
    let status = response.status();
    (status, body_json(response).await)
}

// ---------- 认证前提 ----------

#[tokio::test]
async fn create_company_without_cookie_is_unauthenticated() {
    let response = send(
        crate::testing::app(),
        json_request("POST", "/api/v1/companies", company_payload("星桥")),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "unauthenticated");
}

// ---------- 公司创建与校验 ----------

#[tokio::test]
async fn create_company_returns_created_with_owner_dto() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let (status, body) = create_company(&app, &cookie, "星桥科技").await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(body["name"], "星桥科技");
    assert_eq!(body["source"], "user");
    assert_eq!(body["isOwner"], true);
    assert!(body["id"].is_string());
}

#[tokio::test]
async fn create_company_with_blank_name_maps_to_validation_error() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let response = send(
        app,
        authed_json(
            "POST",
            "/api/v1/companies",
            &cookie,
            json!({ "name": "  ", "industry": "人工智能" }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "validation_failed");
    assert!(
        body["error"]["fields"]
            .as_array()
            .expect("fields list")
            .iter()
            .any(|field| field == "name")
    );
}

#[tokio::test]
async fn create_company_with_duplicate_name_maps_to_conflict() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let (first, _) = create_company(&app, &cookie, "星桥科技").await;
    assert_eq!(first, StatusCode::CREATED);
    // 大小写不敏感的唯一键：重名不论大小写都冲突。
    let (second, body) = create_company(&app, &cookie, "星桥科技").await;
    assert_eq!(second, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "conflict");
}

// ---------- 详情、更新与删除 ----------

#[tokio::test]
async fn company_crud_roundtrip_preserves_unpatched_fields() {
    let app = crate::testing::app();
    let cookie = registered_cookie(&app, "alice").await;
    let (_, created) = create_company(&app, &cookie, "星桥科技").await;
    let company_id = created["id"].as_str().unwrap().to_owned();

    // 局部更新：只改 city，其余字段保持原值。
    let response = send(
        app.clone(),
        authed_json(
            "PATCH",
            &format!("/api/v1/companies/{company_id}"),
            &cookie,
            json!({ "city": "北京" }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["city"], "北京");
    assert_eq!(body["industry"], "人工智能");
    assert_eq!(body["highlights"], json!(["弹性工作"]));

    // 删除后读取 404。
    let response = send(
        app.clone(),
        authed_empty(
            "DELETE",
            &format!("/api/v1/companies/{company_id}"),
            &cookie,
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = send(
        app,
        authed_empty("GET", &format!("/api/v1/companies/{company_id}"), &cookie),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "not_found");
}

#[tokio::test]
async fn update_company_by_non_owner_maps_to_not_found() {
    let app = crate::testing::app();
    let alice = registered_cookie(&app, "alice").await;
    let bob = registered_cookie(&app, "bob").await;
    let (_, created) = create_company(&app, &alice, "星桥科技").await;
    let company_id = created["id"].as_str().unwrap();

    let response = send(
        app,
        authed_json(
            "PATCH",
            &format!("/api/v1/companies/{company_id}"),
            &bob,
            json!({ "city": "劫持" }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------- 可见性 ----------

#[tokio::test]
async fn private_company_hidden_from_others_but_visible_to_owner() {
    let app = crate::testing::app();
    let alice = registered_cookie(&app, "alice").await;
    let bob = registered_cookie(&app, "bob").await;
    let (_, created) = create_company(&app, &alice, "私密公司").await;
    let company_id = created["id"].as_str().unwrap().to_owned();
    // 创建时标记为私有。
    let response = send(
        app.clone(),
        authed_json(
            "PATCH",
            &format!("/api/v1/companies/{company_id}"),
            &alice,
            json!({ "isPrivate": true }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);

    // bob：详情 404、公开列表不含。
    let response = send(
        app.clone(),
        authed_empty("GET", &format!("/api/v1/companies/{company_id}"), &bob),
    )
    .await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = send(
        app.clone(),
        authed_empty("GET", "/api/v1/companies?scope=public", &bob),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["items"].as_array().unwrap().len(), 0);

    // alice：详情可见、mine 列表含且计数正确。
    let response = send(
        app.clone(),
        authed_empty("GET", &format!("/api/v1/companies/{company_id}"), &alice),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let response = send(
        app,
        authed_empty("GET", "/api/v1/companies?scope=mine", &alice),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    let items = body["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["name"], "私密公司");
    // 列表计数沿用 PG 实现的 snake_case 列名（真实 wire 契约）。
    assert_eq!(items[0]["project_count"], 0);
    assert_eq!(items[0]["visible_job_count"], 0);
}

#[tokio::test]
async fn draft_jobs_hidden_from_public_detail_but_visible_to_owner() {
    let app = crate::testing::app();
    let alice = registered_cookie(&app, "alice").await;
    let bob = registered_cookie(&app, "bob").await;
    let (_, company) = create_company(&app, &alice, "星桥科技").await;
    let company_id = company["id"].as_str().unwrap().to_owned();

    let response = send(
        app.clone(),
        authed_json(
            "POST",
            &format!("/api/v1/companies/{company_id}/projects"),
            &alice,
            json!({ "name": "2027 校招" }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let project_id = body_json(response).await["id"].as_str().unwrap().to_owned();

    let response = send(
        app.clone(),
        authed_json(
            "POST",
            &format!("/api/v1/projects/{project_id}/pipelines"),
            &alice,
            pipeline_payload("校招主流程"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let pipeline_id = body_json(response).await["id"].as_str().unwrap().to_owned();

    // 岗位默认 draft。
    let response = send(
        app.clone(),
        authed_json(
            "POST",
            &format!("/api/v1/projects/{project_id}/jobs"),
            &alice,
            job_payload(&pipeline_id, "后端工程师"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let body = body_json(response).await;
    assert_eq!(body["status"], "draft");

    // bob 的详情看不到 draft 岗位；alice 看得到。
    let response = send(
        app.clone(),
        authed_empty("GET", &format!("/api/v1/companies/{company_id}"), &bob),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        body_json(response).await["jobs"].as_array().unwrap().len(),
        0
    );
    let response = send(
        app,
        authed_empty("GET", &format!("/api/v1/companies/{company_id}"), &alice),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        body_json(response).await["jobs"].as_array().unwrap().len(),
        1
    );
}

// ---------- 流水线与岗位约束 ----------

#[tokio::test]
async fn update_pipeline_rejects_stage_modification() {
    let app = crate::testing::app();
    let alice = registered_cookie(&app, "alice").await;
    let (_, company) = create_company(&app, &alice, "星桥科技").await;
    let company_id = company["id"].as_str().unwrap().to_owned();
    let response = send(
        app.clone(),
        authed_json(
            "POST",
            &format!("/api/v1/companies/{company_id}/projects"),
            &alice,
            json!({ "name": "2027 校招" }),
        ),
    )
    .await;
    let project_id = body_json(response).await["id"].as_str().unwrap().to_owned();
    let response = send(
        app.clone(),
        authed_json(
            "POST",
            &format!("/api/v1/projects/{project_id}/pipelines"),
            &alice,
            pipeline_payload("校招主流程"),
        ),
    )
    .await;
    let pipeline_id = body_json(response).await["id"].as_str().unwrap().to_owned();

    let response = send(
        app,
        authed_json(
            "PATCH",
            &format!("/api/v1/pipelines/{pipeline_id}"),
            &alice,
            json!({ "name": "新名字", "stages": [] }),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "validation_failed");
}

#[tokio::test]
async fn create_job_with_pipeline_from_other_project_is_rejected() {
    let app = crate::testing::app();
    let alice = registered_cookie(&app, "alice").await;
    let (_, company) = create_company(&app, &alice, "星桥科技").await;
    let company_id = company["id"].as_str().unwrap().to_owned();
    for name in ["项目A", "项目B"] {
        let response = send(
            app.clone(),
            authed_json(
                "POST",
                &format!("/api/v1/companies/{company_id}/projects"),
                &alice,
                json!({ "name": name }),
            ),
        )
        .await;
        assert_eq!(response.status(), StatusCode::CREATED);
    }
    let response = send(
        app.clone(),
        authed_empty("GET", &format!("/api/v1/companies/{company_id}"), &alice),
    )
    .await;
    let detail = body_json(response).await;
    let projects = detail["projects"].as_array().unwrap();
    let (project_a, project_b) = (
        projects[0]["id"].as_str().unwrap().to_owned(),
        projects[1]["id"].as_str().unwrap().to_owned(),
    );

    let response = send(
        app.clone(),
        authed_json(
            "POST",
            &format!("/api/v1/projects/{project_a}/pipelines"),
            &alice,
            pipeline_payload("A 的流程"),
        ),
    )
    .await;
    let pipeline_a = body_json(response).await["id"].as_str().unwrap().to_owned();

    // 把 A 项目的流水线挂到 B 项目的岗位上 → 400 引用错误。
    let response = send(
        app,
        authed_json(
            "POST",
            &format!("/api/v1/projects/{project_b}/jobs"),
            &alice,
            job_payload(&pipeline_a, "跨项目岗位"),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = body_json(response).await;
    assert_eq!(body["error"]["code"], "validation_failed");
}

// ---------- 模板导入 ----------

#[tokio::test]
async fn import_validate_reports_ok_with_preview() {
    let app = crate::testing::app();
    let alice = registered_cookie(&app, "alice").await;
    let response = send(
        app,
        authed_json(
            "POST",
            "/api/v1/companies/import/validate",
            &alice,
            valid_template(),
        ),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body = body_json(response).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["preview"]["projectCount"], 1);
    assert_eq!(body["preview"]["pipelineCount"], 1);
    assert_eq!(body["preview"]["jobCount"], 1);
}

#[tokio::test]
async fn import_template_creates_browsable_company() {
    let app = crate::testing::app();
    let alice = registered_cookie(&app, "alice").await;
    let response = send(
        app.clone(),
        authed_json("POST", "/api/v1/companies/import", &alice, valid_template()),
    )
    .await;
    assert_eq!(response.status(), StatusCode::CREATED);
    let company_id = body_json(response).await["companyId"]
        .as_str()
        .unwrap()
        .to_owned();

    let response = send(
        app,
        authed_empty("GET", &format!("/api/v1/companies/{company_id}"), &alice),
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let detail = body_json(response).await;
    // CompanyDetailDto 扁平化公司字段，顶层即 company 的全部键。
    assert_eq!(detail["name"], "虚构星桥科技");
    assert_eq!(detail["projects"].as_array().unwrap().len(), 1);
    assert_eq!(detail["pipelines"].as_array().unwrap().len(), 1);
    assert_eq!(detail["jobs"].as_array().unwrap().len(), 1);
}

/// 与 `catalog/tests.rs` 相同结构的最小合法模板。
fn valid_template() -> Value {
    let rubric = json!({
        "passScore": 60,
        "dimensions": [
            {"key": "communication", "label": "沟通表达", "weight": 100, "criteria": "表达清晰"},
        ],
    });
    let stage = |key: &str, kind: &str, title: &str| {
        json!({
            "key": key, "kind": kind, "title": title,
            "minRounds": 1, "maxRounds": 3, "rubric": rubric,
        })
    };
    json!({
        "schemaVersion": 1,
        "company": { "name": "虚构星桥科技", "industry": "人工智能" },
        "projects": [ { "key": "p1", "name": "2027 校招" } ],
        "pipelines": [ {
            "key": "pipe1", "projectKey": "p1", "name": "校招主流程",
            "stages": [stage("hr_screen", "hr_screen", "HR 初筛"), stage("offer", "offer", "Offer 沟通")],
        } ],
        "jobs": [ {
            "key": "j1", "projectKey": "p1", "pipelineKey": "pipe1",
            "title": "后端工程师", "jobType": "campus", "city": "上海",
            "description": "负责核心服务", "status": "open",
        } ],
    })
}
