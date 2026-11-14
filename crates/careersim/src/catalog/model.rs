//! catalog 领域模型：公司模板、招聘项目、流水线与岗位。
//!
//! 校验后的类型是唯一合法构造入口；ID 与时间戳由服务层分配。

use std::fmt;

use chrono::{DateTime, NaiveDate, Utc};
use uuid::Uuid;

use crate::account::UserId;
use crate::pipeline::PipelineDefinition;

/// 公司模板 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompanyId(Uuid);

impl CompanyId {
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

/// 招聘项目 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProjectId(Uuid);

impl ProjectId {
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

/// 流水线 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PipelineId(Uuid);

impl PipelineId {
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

/// 岗位 ID。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct JobId(Uuid);

impl JobId {
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

/// 公司来源：种子模板只读，用户模板默认公开。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanySource {
    Seed,
    User,
}

impl CompanySource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Seed => "seed",
            Self::User => "user",
        }
    }
}

/// 项目状态；与数据库 CHECK 约束一致。`open`/`closed` 只表示拥有者是否
/// 手动关闭；对外读取时以 [`ProjectStatus::effective`] 的派生结果为准。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectStatus {
    Open,
    Closed,
}

impl ProjectStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }

    /// 读取时的有效状态：存储为 `closed`，或 `deadline` 已过（按当天日期
    /// 比较，截止日当天仍算开放），都对外呈现为 `closed`。
    pub fn effective(self, deadline: Option<NaiveDate>, today: NaiveDate) -> Self {
        match self {
            Self::Closed => Self::Closed,
            Self::Open if deadline.is_some_and(|date| date < today) => Self::Closed,
            Self::Open => Self::Open,
        }
    }
}

/// 岗位类型；与数据库 CHECK 约束一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobType {
    Campus,
    Internship,
    Experienced,
}

impl JobType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Campus => "campus",
            Self::Internship => "internship",
            Self::Experienced => "experienced",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "campus" => Some(Self::Campus),
            "internship" => Some(Self::Internship),
            "experienced" => Some(Self::Experienced),
            _ => None,
        }
    }
}

/// 岗位状态；非 owner 仅见 open。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobStatus {
    Draft,
    Open,
    Closed,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Open => "open",
            Self::Closed => "closed",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "draft" => Some(Self::Draft),
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

/// 公司模板聚合。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Company {
    pub id: CompanyId,
    pub source: CompanySource,
    /// 模板归属者；种子模板为 `None`。
    pub owner: Option<UserId>,
    pub is_private: bool,
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
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Company {
    /// 当前查看者是否为归属者；种子模板没有归属者。
    pub fn is_owned_by(&self, viewer: UserId) -> bool {
        self.owner == Some(viewer)
    }
}

/// 招聘项目。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecruitmentProject {
    pub id: ProjectId,
    pub company_id: CompanyId,
    pub name: String,
    pub season: String,
    pub status: ProjectStatus,
    pub deadline: Option<NaiveDate>,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 流水线；阶段序列已由 [`PipelineDefinition`] 证明不变量。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pipeline {
    pub id: PipelineId,
    pub project_id: ProjectId,
    pub name: String,
    pub definition: PipelineDefinition,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 岗位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Job {
    pub id: JobId,
    pub project_id: ProjectId,
    pub pipeline_id: PipelineId,
    pub title: String,
    pub city: String,
    pub job_type: JobType,
    pub salary_min: Option<u32>,
    pub salary_max: Option<u32>,
    pub salary_months: Option<u8>,
    pub tags: Vec<String>,
    pub description: String,
    pub status: JobStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 公司列表项：公司字段加上可见项目与岗位的计数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanySummary {
    pub company: Company,
    pub project_count: u32,
    pub visible_job_count: u32,
}

/// 公司详情：公司字段加上项目、流水线与（可见的）岗位。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompanyDetail {
    pub company: Company,
    pub projects: Vec<RecruitmentProject>,
    pub pipelines: Vec<Pipeline>,
    pub jobs: Vec<Job>,
}

/// 列表游标：`(updated_at, id)` 的不透明编码。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListCursor {
    pub updated_at: DateTime<Utc>,
    pub id: Uuid,
}

impl fmt::Display for ListCursor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.updated_at.timestamp_millis(), self.id)
    }
}

/// catalog 错误；`Backend` 只在存储边界出现。
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("invalid {field}: {reason}")]
    InvalidInput { field: &'static str, reason: String },
    #[error("template validation failed")]
    InvalidTemplate(Vec<super::template::TemplateError>),
    #[error("referenced {field} does not exist")]
    InvalidReference { field: &'static str },
    #[error("not found")]
    NotFound,
    #[error("`{0}` already exists")]
    Conflict(&'static str),
    #[error("catalog store backend error: {message}")]
    Backend {
        message: String,
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },
}

/// 公司名归一化键：trim + 小写；唯一性按它判断。
pub fn normalize_key(raw: &str) -> String {
    raw.trim().to_lowercase()
}

pub(crate) const COMPANY_NAME_MAX_CHARS: usize = 80;
pub(crate) const HIGHLIGHT_MAX_ITEMS: usize = 12;
pub(crate) const HIGHLIGHT_ITEM_MAX_CHARS: usize = 60;
pub(crate) const LOGO_TEXT_MAX_CHARS: usize = 8;
pub(crate) const ACCENT_MAX_CHARS: usize = 7;
pub(crate) const PROJECT_NAME_MAX_CHARS: usize = 60;
/// 薪资月数上限；表单与模板导入共用。
pub(crate) const SALARY_MONTHS_MAX: u8 = 24;
/// 岗位标签数量上限；表单与模板导入共用。
pub(crate) const JOB_TAGS_MAX_ITEMS: usize = 12;

/// 公司模板各文本字段的长度上限；前端表单 maxlength 与此严格一致。
pub(crate) const ENGLISH_NAME_MAX_CHARS: usize = 40;
pub(crate) const INDUSTRY_MAX_CHARS: usize = 24;
pub(crate) const CITY_MAX_CHARS: usize = 24;
pub(crate) const TAGLINE_MAX_CHARS: usize = 60;
pub(crate) const COMPANY_DESCRIPTION_MAX_CHARS: usize = 600;
pub(crate) const SEASON_MAX_CHARS: usize = 24;
pub(crate) const PROJECT_DESCRIPTION_MAX_CHARS: usize = 300;
pub(crate) const JOB_TITLE_MAX_CHARS: usize = 60;
pub(crate) const JOB_DESCRIPTION_MAX_CHARS: usize = 600;

/// 团队规模枚举：模板导入与前端表单下拉共用这套取值。
pub(crate) const COMPANY_SIZES: [&str; 5] = [
    "50 人以下",
    "50–200 人",
    "100–499 人",
    "500–2000 人",
    "2000 人以上",
];

/// 品牌色枚举：表单色板、内置公司模板与导入校验共用这套取值；允许留空表示用默认色。
pub(crate) const COMPANY_ACCENTS: [&str; 8] = [
    "#3D5A45", "#2F5D50", "#4F6042", "#A4482F", "#8A5A2B", "#5B4A7A", "#29405E", "#2C4A7C",
];

/// 品牌色必须是 `#RRGGBB` 六位十六进制；模板导入允许留空。
pub(crate) fn is_hex_color(text: &str) -> bool {
    text.len() == 7
        && text.starts_with('#')
        && text[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(crate) fn validate_company_fields(
    name: &str,
    highlights: &[String],
    logo_text: &str,
    accent: &str,
) -> Result<(), CatalogError> {
    let name_chars = name.trim().chars().count();
    if !(1..=COMPANY_NAME_MAX_CHARS).contains(&name_chars) {
        return Err(CatalogError::InvalidInput {
            field: "name",
            reason: format!("must be 1..={COMPANY_NAME_MAX_CHARS} characters"),
        });
    }
    if highlights.len() > HIGHLIGHT_MAX_ITEMS {
        return Err(CatalogError::InvalidInput {
            field: "highlights",
            reason: format!("must contain at most {HIGHLIGHT_MAX_ITEMS} items"),
        });
    }
    for highlight in highlights {
        if highlight.trim().is_empty() || highlight.chars().count() > HIGHLIGHT_ITEM_MAX_CHARS {
            return Err(CatalogError::InvalidInput {
                field: "highlights",
                reason: format!("each item must be 1..={HIGHLIGHT_ITEM_MAX_CHARS} characters"),
            });
        }
    }
    if logo_text.chars().count() > LOGO_TEXT_MAX_CHARS {
        return Err(CatalogError::InvalidInput {
            field: "logoText",
            reason: format!("must be at most {LOGO_TEXT_MAX_CHARS} characters"),
        });
    }
    if !accent.is_empty() && !is_hex_color(accent) {
        return Err(CatalogError::InvalidInput {
            field: "accent",
            reason: "must be a #RRGGBB hex color".to_owned(),
        });
    }
    Ok(())
}

pub(crate) fn validate_project_name(name: &str) -> Result<(), CatalogError> {
    let chars = name.trim().chars().count();
    if !(1..=PROJECT_NAME_MAX_CHARS).contains(&chars) {
        return Err(CatalogError::InvalidInput {
            field: "name",
            reason: format!("must be 1..={PROJECT_NAME_MAX_CHARS} characters"),
        });
    }
    Ok(())
}

pub(crate) fn validate_salary(
    salary_min: Option<u32>,
    salary_max: Option<u32>,
    salary_months: Option<u8>,
) -> Result<(), CatalogError> {
    let limit = i32::MAX as u32;
    if salary_min.is_some_and(|v| v > limit) || salary_max.is_some_and(|v| v > limit) {
        return Err(CatalogError::InvalidInput {
            field: "salary",
            reason: "salary values must fit in a signed 32-bit integer".to_owned(),
        });
    }
    if let (Some(min), Some(max)) = (salary_min, salary_max)
        && max < min
    {
        return Err(CatalogError::InvalidInput {
            field: "salaryMax",
            reason: "must be greater than or equal to salaryMin".to_owned(),
        });
    }
    if salary_months.is_some_and(|v| !(1..=SALARY_MONTHS_MAX).contains(&v)) {
        return Err(CatalogError::InvalidInput {
            field: "salaryMonths",
            reason: format!("must be within 1..={SALARY_MONTHS_MAX}"),
        });
    }
    Ok(())
}

/// 手工创建或修改公司时的字段集合；`is_private` 只影响可见性，不影响归属。
#[derive(Debug, Clone, Default)]
pub struct NewCompany {
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

/// 公司部分更新；`None` 表示不修改。
#[derive(Debug, Clone, Default)]
pub struct CompanyPatch {
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

#[derive(Debug, Clone)]
pub struct NewProject {
    pub name: String,
    pub season: String,
    pub status: ProjectStatus,
    pub deadline: Option<NaiveDate>,
    pub description: String,
}

#[derive(Debug, Clone, Default)]
pub struct ProjectPatch {
    pub name: Option<String>,
    pub season: Option<String>,
    pub status: Option<ProjectStatus>,
    /// 外层 `None` 不修改，内层 `None` 清空截止日期。
    pub deadline: Option<Option<NaiveDate>>,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewPipeline {
    pub name: String,
    pub definition: PipelineDefinition,
}

#[derive(Debug, Clone)]
pub struct NewJob {
    pub pipeline_id: PipelineId,
    pub title: String,
    pub city: String,
    pub job_type: JobType,
    pub salary_min: Option<u32>,
    pub salary_max: Option<u32>,
    pub salary_months: Option<u8>,
    pub tags: Vec<String>,
    pub description: String,
    pub status: JobStatus,
}

#[derive(Debug, Clone, Default)]
pub struct JobPatch {
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
