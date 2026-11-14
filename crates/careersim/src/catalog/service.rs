//! catalog 用例服务：查询、CRUD 与模板导入。
//!
//! 输入校验在本层完成；写权限与可见性由存储层 SQL 统一保证。

use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};

use super::model::{
    CatalogError, Company, CompanyDetail, CompanyId, CompanyPatch, CompanySummary, Job, JobId,
    JobPatch, JobStatus, JobType, ListCursor, NewCompany, NewJob, NewPipeline, NewProject,
    Pipeline, PipelineId, ProjectId, ProjectPatch, ProjectStatus, RecruitmentProject,
    validate_company_fields, validate_project_name, validate_salary,
};
use super::store::{CatalogScope, CatalogStore};
use super::template::{self, ValidatedTemplate};
use crate::account::UserId;
use crate::pipeline::{PipelineDefinition, StageDefinition};

/// catalog 对外服务；HTTP 层经 [`AppState`](crate::http::router::AppState) 持有。
pub struct CatalogService {
    store: Arc<dyn CatalogStore>,
}

impl CatalogService {
    pub fn new(store: Arc<dyn CatalogStore>) -> Self {
        Self { store }
    }

    pub async fn list_companies(
        &self,
        viewer: UserId,
        scope: CatalogScope,
        limit: u32,
        cursor: Option<ListCursor>,
    ) -> Result<(Vec<CompanySummary>, Option<ListCursor>), CatalogError> {
        self.store
            .list_companies(viewer, scope, limit, cursor)
            .await
    }

    pub async fn get_company(
        &self,
        viewer: UserId,
        id: CompanyId,
    ) -> Result<Option<CompanyDetail>, CatalogError> {
        self.store.get_company(viewer, id).await
    }

    pub async fn create_company(
        &self,
        owner: UserId,
        input: CompanyInput,
        now: DateTime<Utc>,
    ) -> Result<Company, CatalogError> {
        let new = input.validate()?;
        self.store.create_company(owner, new, now).await
    }

    pub async fn update_company(
        &self,
        owner: UserId,
        id: CompanyId,
        input: CompanyPatchInput,
        now: DateTime<Utc>,
    ) -> Result<Company, CatalogError> {
        let patch = input.validate()?;
        self.store.update_company(owner, id, &patch, now).await
    }

    pub async fn delete_company(
        &self,
        owner: UserId,
        id: CompanyId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError> {
        self.store.soft_delete_company(owner, id, now).await
    }

    pub async fn create_project(
        &self,
        owner: UserId,
        company_id: CompanyId,
        input: ProjectInput,
        now: DateTime<Utc>,
    ) -> Result<RecruitmentProject, CatalogError> {
        let new = input.validate()?;
        self.store.create_project(owner, company_id, new, now).await
    }

    pub async fn update_project(
        &self,
        owner: UserId,
        id: ProjectId,
        input: ProjectPatchInput,
        now: DateTime<Utc>,
    ) -> Result<RecruitmentProject, CatalogError> {
        let patch = input.validate()?;
        self.store.update_project(owner, id, &patch, now).await
    }

    pub async fn delete_project(
        &self,
        owner: UserId,
        id: ProjectId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError> {
        self.store.soft_delete_project(owner, id, now).await
    }

    pub async fn create_pipeline(
        &self,
        owner: UserId,
        project_id: ProjectId,
        input: PipelineInput,
        now: DateTime<Utc>,
    ) -> Result<Pipeline, CatalogError> {
        let new = input.validate()?;
        self.store
            .create_pipeline(owner, project_id, new, now)
            .await
    }

    pub async fn update_pipeline(
        &self,
        owner: UserId,
        id: PipelineId,
        name: &str,
        now: DateTime<Utc>,
    ) -> Result<Pipeline, CatalogError> {
        let new_name = PipelineInput::validate_name(name)?;
        self.store.update_pipeline(owner, id, &new_name, now).await
    }

    pub async fn delete_pipeline(
        &self,
        owner: UserId,
        id: PipelineId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError> {
        self.store.soft_delete_pipeline(owner, id, now).await
    }

    pub async fn create_job(
        &self,
        owner: UserId,
        project_id: ProjectId,
        input: JobInput,
        now: DateTime<Utc>,
    ) -> Result<Job, CatalogError> {
        let new = input.validate()?;
        self.store.create_job(owner, project_id, new, now).await
    }

    pub async fn update_job(
        &self,
        owner: UserId,
        id: JobId,
        input: JobPatchInput,
        now: DateTime<Utc>,
    ) -> Result<Job, CatalogError> {
        let patch = input.validate()?;
        self.store.update_job(owner, id, &patch, now).await
    }

    pub async fn delete_job(
        &self,
        owner: UserId,
        id: JobId,
        now: DateTime<Utc>,
    ) -> Result<(), CatalogError> {
        self.store.soft_delete_job(owner, id, now).await
    }

    /// 导入 dry-run：只校验，不写库；与 [`CatalogService::import_template`]
    /// 共用同一校验函数。
    pub fn validate_template(
        raw: &serde_json::Value,
    ) -> Result<ValidatedTemplate, Vec<template::TemplateError>> {
        template::validate_template(raw)
    }

    pub async fn import_template(
        &self,
        owner: UserId,
        raw: &serde_json::Value,
        now: DateTime<Utc>,
    ) -> Result<CompanyId, CatalogError> {
        let validated = template::validate_template(raw).map_err(CatalogError::InvalidTemplate)?;
        self.store.import_template(owner, &validated, now).await
    }
}

/// 路由层提交的公司字段（未校验）。
#[derive(Debug, Clone, Default)]
pub struct CompanyInput {
    pub name: String,
    pub english_name: String,
    pub industry: String,
    pub city: String,
    pub size: String,
    pub tagline: String,
    pub description: String,
    pub highlights: Vec<String>,
    pub logo_text: String,
    pub accent: String,
    pub is_private: bool,
}

impl CompanyInput {
    fn validate(self) -> Result<NewCompany, CatalogError> {
        validate_company_fields(&self.name, &self.highlights, &self.logo_text, &self.accent)?;
        if self.industry.trim().is_empty() {
            return Err(CatalogError::InvalidInput {
                field: "industry",
                reason: "must be a non-empty string".to_owned(),
            });
        }
        Ok(NewCompany {
            name: self.name.trim().to_owned(),
            english_name: self.english_name.trim().to_owned(),
            industry: self.industry.trim().to_owned(),
            city: self.city.trim().to_owned(),
            size: self.size.trim().to_owned(),
            tagline: self.tagline.trim().to_owned(),
            description: self.description.trim().to_owned(),
            highlights: self
                .highlights
                .into_iter()
                .map(|item| item.trim().to_owned())
                .collect(),
            logo_text: self.logo_text.trim().to_owned(),
            accent: self.accent.trim().to_owned(),
            is_private: self.is_private,
        })
    }
}

/// 公司部分更新（未校验）；外层 `None` 表示不修改。
#[derive(Debug, Clone, Default)]
pub struct CompanyPatchInput {
    pub name: Option<String>,
    pub english_name: Option<String>,
    pub industry: Option<String>,
    pub city: Option<String>,
    pub size: Option<String>,
    pub tagline: Option<String>,
    pub description: Option<String>,
    pub highlights: Option<Vec<String>>,
    pub logo_text: Option<String>,
    pub accent: Option<String>,
    pub is_private: Option<bool>,
}

impl CompanyPatchInput {
    fn validate(self) -> Result<CompanyPatch, CatalogError> {
        if let Some(name) = &self.name {
            validate_company_fields(name, &[], "", "")?;
        }
        if let Some(highlights) = &self.highlights {
            validate_company_fields("占位", highlights, "", "")?;
        }
        if let Some(logo_text) = &self.logo_text {
            validate_company_fields("占位", &[], logo_text, "")?;
        }
        if let Some(accent) = &self.accent {
            validate_company_fields("占位", &[], "", accent)?;
        }
        if let Some(industry) = &self.industry
            && industry.trim().is_empty()
        {
            return Err(CatalogError::InvalidInput {
                field: "industry",
                reason: "must be a non-empty string".to_owned(),
            });
        }
        Ok(CompanyPatch {
            name: self.name.map(|value| value.trim().to_owned()),
            english_name: self.english_name.map(|value| value.trim().to_owned()),
            industry: self.industry.map(|value| value.trim().to_owned()),
            city: self.city.map(|value| value.trim().to_owned()),
            size: self.size.map(|value| value.trim().to_owned()),
            tagline: self.tagline.map(|value| value.trim().to_owned()),
            description: self.description.map(|value| value.trim().to_owned()),
            highlights: self.highlights.map(|items| {
                items
                    .into_iter()
                    .map(|item| item.trim().to_owned())
                    .collect()
            }),
            logo_text: self.logo_text.map(|value| value.trim().to_owned()),
            accent: self.accent.map(|value| value.trim().to_owned()),
            is_private: self.is_private,
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProjectInput {
    pub name: String,
    pub season: String,
    pub status: Option<ProjectStatus>,
    pub deadline: Option<NaiveDate>,
    pub description: String,
}

impl ProjectInput {
    fn validate(self) -> Result<NewProject, CatalogError> {
        validate_project_name(&self.name)?;
        Ok(NewProject {
            name: self.name.trim().to_owned(),
            season: self.season.trim().to_owned(),
            status: self.status.unwrap_or(ProjectStatus::Open),
            deadline: self.deadline,
            description: self.description.trim().to_owned(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProjectPatchInput {
    pub name: Option<String>,
    pub season: Option<String>,
    pub status: Option<ProjectStatus>,
    pub deadline: Option<Option<NaiveDate>>,
    pub description: Option<String>,
}

impl ProjectPatchInput {
    fn validate(self) -> Result<ProjectPatch, CatalogError> {
        if let Some(name) = &self.name {
            validate_project_name(name)?;
        }
        Ok(ProjectPatch {
            name: self.name.map(|value| value.trim().to_owned()),
            season: self.season.map(|value| value.trim().to_owned()),
            status: self.status,
            deadline: self.deadline,
            description: self.description.map(|value| value.trim().to_owned()),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct PipelineInput {
    pub name: String,
    pub stages: Option<Vec<StageDefinition>>,
}

impl PipelineInput {
    fn validate(self) -> Result<NewPipeline, CatalogError> {
        let name = Self::validate_name(&self.name)?;
        let stages = self.stages.unwrap_or_default();
        let definition =
            PipelineDefinition::parse(stages).map_err(|error| CatalogError::InvalidInput {
                field: "stages",
                reason: error.to_string(),
            })?;
        Ok(NewPipeline { name, definition })
    }

    fn validate_name(name: &str) -> Result<String, CatalogError> {
        validate_project_name(name)?;
        Ok(name.trim().to_owned())
    }
}

#[derive(Debug, Clone, Default)]
pub struct JobInput {
    pub pipeline_id: Option<PipelineId>,
    pub title: String,
    pub city: String,
    pub job_type: Option<JobType>,
    pub salary_min: Option<u32>,
    pub salary_max: Option<u32>,
    pub salary_months: Option<u8>,
    pub tags: Vec<String>,
    pub description: String,
    pub status: Option<JobStatus>,
}

impl JobInput {
    fn validate(self) -> Result<NewJob, CatalogError> {
        let pipeline_id = self.pipeline_id.ok_or(CatalogError::InvalidInput {
            field: "pipelineId",
            reason: "is required".to_owned(),
        })?;
        if self.title.trim().is_empty() {
            return Err(CatalogError::InvalidInput {
                field: "title",
                reason: "must be a non-empty string".to_owned(),
            });
        }
        if self.tags.len() > 12 {
            return Err(CatalogError::InvalidInput {
                field: "tags",
                reason: "must contain at most 12 items".to_owned(),
            });
        }
        validate_salary(self.salary_min, self.salary_max, self.salary_months)?;
        Ok(NewJob {
            pipeline_id,
            title: self.title.trim().to_owned(),
            city: self.city.trim().to_owned(),
            job_type: self.job_type.unwrap_or(JobType::Campus),
            salary_min: self.salary_min,
            salary_max: self.salary_max,
            salary_months: self.salary_months,
            tags: self
                .tags
                .into_iter()
                .map(|tag| tag.trim().to_owned())
                .collect(),
            description: self.description.trim().to_owned(),
            status: self.status.unwrap_or(JobStatus::Draft),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct JobPatchInput {
    pub pipeline_id: Option<PipelineId>,
    pub title: Option<String>,
    pub city: Option<String>,
    pub job_type: Option<JobType>,
    pub salary_min: Option<Option<u32>>,
    pub salary_max: Option<Option<u32>>,
    pub salary_months: Option<Option<u8>>,
    pub tags: Option<Vec<String>>,
    pub description: Option<String>,
    pub status: Option<JobStatus>,
}

impl JobPatchInput {
    fn validate(self) -> Result<JobPatch, CatalogError> {
        if let Some(title) = &self.title
            && title.trim().is_empty()
        {
            return Err(CatalogError::InvalidInput {
                field: "title",
                reason: "must be a non-empty string".to_owned(),
            });
        }
        if let Some(tags) = &self.tags
            && tags.len() > 12
        {
            return Err(CatalogError::InvalidInput {
                field: "tags",
                reason: "must contain at most 12 items".to_owned(),
            });
        }
        let (min, max, months) = match (self.salary_min, self.salary_max, self.salary_months) {
            (None, None, None) => (None, None, None),
            (min, max, months) => {
                // 补丁只带一侧时，用当前语义无法跨字段比较；整组校验交给数据库
                // 兜底，这里只做单值范围检查。
                let flat_min = min.unwrap_or(None);
                let flat_max = max.unwrap_or(None);
                let flat_months = months.unwrap_or(None);
                validate_salary(flat_min, flat_max, flat_months)?;
                (min, max, months)
            }
        };
        Ok(JobPatch {
            pipeline_id: self.pipeline_id,
            title: self.title.map(|value| value.trim().to_owned()),
            city: self.city.map(|value| value.trim().to_owned()),
            job_type: self.job_type,
            salary_min: min,
            salary_max: max,
            salary_months: months,
            tags: self
                .tags
                .map(|items| items.into_iter().map(|tag| tag.trim().to_owned()).collect()),
            description: self.description.map(|value| value.trim().to_owned()),
            status: self.status,
        })
    }
}
