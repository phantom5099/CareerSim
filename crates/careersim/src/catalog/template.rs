//! `CompanyTemplateV1` 导入模板：解析、校验与规范化。
//!
//! 校验收集全部错误并带 JSON 路径（plan/01 §6.2）；dry-run 与创建共用
//! [`validate_template`]。跨阶段规则复用 [`PipelineDefinition::parse`]。

use chrono::NaiveDate;
use serde_json::{Map, Value, json};

use super::model::{
    ACCENT_MAX_CHARS, CITY_MAX_CHARS, COMPANY_ACCENTS, COMPANY_DESCRIPTION_MAX_CHARS,
    COMPANY_NAME_MAX_CHARS, COMPANY_SIZES, ENGLISH_NAME_MAX_CHARS, HIGHLIGHT_ITEM_MAX_CHARS,
    HIGHLIGHT_MAX_ITEMS, INDUSTRY_MAX_CHARS, JOB_DESCRIPTION_MAX_CHARS, JOB_TAGS_MAX_ITEMS,
    JOB_TITLE_MAX_CHARS, JobStatus, JobType, LOGO_TEXT_MAX_CHARS, PROJECT_DESCRIPTION_MAX_CHARS,
    PROJECT_NAME_MAX_CHARS, ProjectStatus, SALARY_MONTHS_MAX, SEASON_MAX_CHARS, TAGLINE_MAX_CHARS,
    is_hex_color, normalize_key,
};
use crate::pipeline::{
    PipelineError,
    model::{PipelineDefinition, StageDefinition},
};

/// 当前模板格式版本；未知版本直接拒绝，不做静默降级。
pub const SCHEMA_VERSION: u32 = 1;

/// 单条校验错误；`path` 为模板 JSON 内的路径。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateError {
    pub path: String,
    pub message: String,
}

impl TemplateError {
    fn new(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedCompany {
    pub name: String,
    pub english_name: String,
    pub is_private: bool,
    pub industry: String,
    pub city: String,
    pub size: String,
    pub tagline: String,
    pub description: String,
    pub highlights: Vec<String>,
    pub logo_text: String,
    pub accent: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedProject {
    /// 模板内引用键；不落库为标识。
    pub key: String,
    pub name: String,
    pub season: String,
    pub status: ProjectStatus,
    pub deadline: Option<NaiveDate>,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ValidatedPipeline {
    pub key: String,
    pub project_key: String,
    pub name: String,
    pub definition: PipelineDefinition,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedJob {
    pub key: String,
    pub project_key: String,
    pub pipeline_key: String,
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

/// 校验通过的模板；`*.key` 只用于集合内相互引用。
#[derive(Debug, Clone)]
pub struct ValidatedTemplate {
    pub company: ValidatedCompany,
    pub projects: Vec<ValidatedProject>,
    pub pipelines: Vec<ValidatedPipeline>,
    pub jobs: Vec<ValidatedJob>,
}

/// 校验模板 JSON；返回全部错误而不是首个错误。
pub fn validate_template(raw: &Value) -> Result<ValidatedTemplate, Vec<TemplateError>> {
    let mut errors = Vec::new();
    let Some(root) = raw.as_object() else {
        return Err(vec![TemplateError::new(
            "",
            "template must be a JSON object",
        )]);
    };

    validate_schema_version(root, &mut errors);
    let company = validate_company(root.get("company"), &mut errors);
    let projects = validate_projects(root.get("projects"), &mut errors);
    let pipelines = validate_pipelines(root.get("pipelines"), &projects, &mut errors);
    let jobs = validate_jobs(root.get("jobs"), &projects, &pipelines, &mut errors);

    if errors.is_empty() {
        Ok(ValidatedTemplate {
            company: company.unwrap_or_else(placeholder_company),
            projects,
            pipelines: pipelines.into_iter().map(|(_, p)| p).collect(),
            jobs,
        })
    } else {
        Err(errors)
    }
}

/// 占位公司：仅当 `company` 校验失败时被构造，随后整体丢弃。
fn placeholder_company() -> ValidatedCompany {
    ValidatedCompany {
        name: String::new(),
        english_name: String::new(),
        is_private: false,
        industry: String::new(),
        city: String::new(),
        size: String::new(),
        tagline: String::new(),
        description: String::new(),
        highlights: Vec::new(),
        logo_text: String::new(),
        accent: String::new(),
    }
}

fn validate_schema_version(root: &Map<String, Value>, errors: &mut Vec<TemplateError>) {
    let ok = matches!(root.get("schemaVersion"), Some(Value::Number(n)) if n.as_u64() == Some(u64::from(SCHEMA_VERSION)));
    if !ok {
        errors.push(TemplateError::new(
            "schemaVersion",
            format!("must be {SCHEMA_VERSION}"),
        ));
    }
}

fn object_of<'a>(
    value: Option<&'a Value>,
    path: &str,
    errors: &mut Vec<TemplateError>,
) -> Option<&'a Map<String, Value>> {
    match value {
        Some(Value::Object(map)) => Some(map),
        Some(_) => {
            errors.push(TemplateError::new(path, "must be a JSON object"));
            None
        }
        None => {
            errors.push(TemplateError::new(path, "is required"));
            None
        }
    }
}

fn array_of<'a>(
    value: Option<&'a Value>,
    path: &str,
    errors: &mut Vec<TemplateError>,
) -> Option<&'a Vec<Value>> {
    match value {
        Some(Value::Array(items)) => Some(items),
        Some(Value::Null) | None => None,
        Some(_) => {
            errors.push(TemplateError::new(path, "must be a JSON array"));
            None
        }
    }
}

/// 读取必填字符串；trim 后非空。
fn required_text(
    obj: &Map<String, Value>,
    key: &str,
    path: &str,
    errors: &mut Vec<TemplateError>,
) -> Option<String> {
    match obj.get(key) {
        Some(Value::String(text)) if !text.trim().is_empty() => Some(text.trim().to_owned()),
        Some(_) => {
            errors.push(TemplateError::new(path, "must be a non-empty string"));
            None
        }
        None => {
            errors.push(TemplateError::new(path, "is required"));
            None
        }
    }
}

/// 读取有长度上限的文本字段；`required` 时缺失或空白即报错，
/// 否则空值放行为空串。超长一律报错。
fn bounded_text(
    obj: &Map<String, Value>,
    key: &str,
    path: &str,
    required: bool,
    max_chars: usize,
    errors: &mut Vec<TemplateError>,
) -> Option<String> {
    let text = match obj.get(key) {
        Some(Value::String(text)) => Some(text.trim().to_owned()),
        Some(Value::Null) | None => {
            if required {
                errors.push(TemplateError::new(path, "is required"));
                return None;
            }
            return Some(String::new());
        }
        Some(_) => {
            errors.push(TemplateError::new(path, "must be a string"));
            return None;
        }
    };
    let text = text.expect("string branch returns Some");
    if text.is_empty() {
        if required {
            errors.push(TemplateError::new(path, "must be a non-empty string"));
            return None;
        }
        return Some(text);
    }
    if text.chars().count() > max_chars {
        errors.push(TemplateError::new(
            path,
            format!("must be 1..={max_chars} characters"),
        ));
        return None;
    }
    Some(text)
}

fn validate_company(
    value: Option<&Value>,
    errors: &mut Vec<TemplateError>,
) -> Option<ValidatedCompany> {
    let obj = object_of(value, "company", errors)?;
    let name = bounded_text(
        obj,
        "name",
        "company.name",
        true,
        COMPANY_NAME_MAX_CHARS,
        errors,
    );
    let industry = bounded_text(
        obj,
        "industry",
        "company.industry",
        true,
        INDUSTRY_MAX_CHARS,
        errors,
    );

    let is_private = match obj.get("isPrivate") {
        Some(Value::Bool(flag)) => *flag,
        Some(Value::Null) | None => false,
        Some(_) => {
            errors.push(TemplateError::new("company.isPrivate", "must be a boolean"));
            false
        }
    };

    let highlights = match array_of(obj.get("highlights"), "company.highlights", errors) {
        Some(items) => {
            let mut highlights = Vec::new();
            for (index, item) in items.iter().enumerate() {
                match item.as_str() {
                    Some(text) if !text.trim().is_empty() => {
                        highlights.push(text.trim().to_owned())
                    }
                    _ => errors.push(TemplateError::new(
                        format!("company.highlights[{index}]"),
                        "must be a non-empty string",
                    )),
                }
            }
            if highlights.len() > HIGHLIGHT_MAX_ITEMS {
                errors.push(TemplateError::new(
                    "company.highlights",
                    format!("must contain at most {HIGHLIGHT_MAX_ITEMS} items"),
                ));
            }
            if highlights
                .iter()
                .any(|item| item.chars().count() > HIGHLIGHT_ITEM_MAX_CHARS)
            {
                errors.push(TemplateError::new(
                    "company.highlights",
                    format!("each item must be at most {HIGHLIGHT_ITEM_MAX_CHARS} characters"),
                ));
            }
            highlights
        }
        None => Vec::new(),
    };

    let logo_text = bounded_text(
        obj,
        "logoText",
        "company.logoText",
        true,
        LOGO_TEXT_MAX_CHARS,
        errors,
    );
    let accent = bounded_text(
        obj,
        "accent",
        "company.accent",
        false,
        ACCENT_MAX_CHARS,
        errors,
    );
    if let Some(color) = &accent
        && !color.is_empty()
        && (!is_hex_color(color) || !COMPANY_ACCENTS.contains(&color.as_str()))
    {
        errors.push(TemplateError::new(
            "company.accent",
            format!("must be one of {}", COMPANY_ACCENTS.join(" / ")),
        ));
    }

    let english_name = bounded_text(
        obj,
        "englishName",
        "company.englishName",
        true,
        ENGLISH_NAME_MAX_CHARS,
        errors,
    );
    let city = bounded_text(obj, "city", "company.city", true, CITY_MAX_CHARS, errors);
    // size 的真实约束是枚举归属；长度上限只作兜底，取值由下方 membership 校验。
    let size = bounded_text(obj, "size", "company.size", true, CITY_MAX_CHARS, errors);
    if let Some(size_text) = &size
        && !COMPANY_SIZES.contains(&size_text.as_str())
    {
        errors.push(TemplateError::new(
            "company.size",
            format!("must be one of {}", COMPANY_SIZES.join(" / ")),
        ));
    }
    let tagline = bounded_text(
        obj,
        "tagline",
        "company.tagline",
        true,
        TAGLINE_MAX_CHARS,
        errors,
    );
    let description = bounded_text(
        obj,
        "description",
        "company.description",
        true,
        COMPANY_DESCRIPTION_MAX_CHARS,
        errors,
    );

    Some(ValidatedCompany {
        name: name?,
        english_name: english_name?,
        is_private,
        industry: industry?,
        city: city?,
        size: size?,
        tagline: tagline?,
        description: description?,
        highlights,
        logo_text: logo_text?,
        accent: accent.unwrap_or_default(),
    })
}

fn parse_date(value: &Value, path: &str, errors: &mut Vec<TemplateError>) -> Option<NaiveDate> {
    let text = value.as_str()?;
    match NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d") {
        Ok(date) => Some(date),
        Err(_) => {
            errors.push(TemplateError::new(path, "must be a YYYY-MM-DD date"));
            None
        }
    }
}

fn validate_projects(
    value: Option<&Value>,
    errors: &mut Vec<TemplateError>,
) -> Vec<ValidatedProject> {
    let Some(items) = array_of(value, "projects", errors) else {
        return Vec::new();
    };
    let mut projects = Vec::new();
    let mut seen_keys = Vec::new();
    let mut seen_names = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let path = format!("projects[{index}]");
        let Some(obj) = object_of(Some(item), &path, errors) else {
            continue;
        };
        let key = required_text(obj, "key", &format!("{path}.key"), errors);
        let name = required_text(obj, "name", &format!("{path}.name"), errors);
        if let Some(name) = &name
            && name.chars().count() > PROJECT_NAME_MAX_CHARS
        {
            errors.push(TemplateError::new(
                format!("{path}.name"),
                format!("must be 1..={PROJECT_NAME_MAX_CHARS} characters"),
            ));
        }

        let status = match obj.get("status") {
            Some(Value::String(raw)) => match ProjectStatus::parse(raw.trim()) {
                Some(status) => Some(status),
                None => {
                    errors.push(TemplateError::new(
                        format!("{path}.status"),
                        "must be one of open / closed",
                    ));
                    None
                }
            },
            Some(Value::Null) | None => Some(ProjectStatus::Open),
            Some(_) => {
                errors.push(TemplateError::new(
                    format!("{path}.status"),
                    "must be a string",
                ));
                None
            }
        };

        let deadline = obj
            .get("deadline")
            .filter(|value| !value.is_null())
            .and_then(|value| parse_date(value, &format!("{path}.deadline"), errors));

        if let Some(key) = &key {
            if seen_keys.contains(key) {
                errors.push(TemplateError::new(
                    format!("{path}.key"),
                    format!("duplicate key `{key}`"),
                ));
            } else {
                seen_keys.push(key.clone());
            }
        }
        if let Some(name) = &name {
            let name_key = normalize_key(name);
            if seen_names.contains(&name_key) {
                errors.push(TemplateError::new(
                    format!("{path}.name"),
                    format!("duplicate project name `{name}`"),
                ));
            } else {
                seen_names.push(name_key);
            }
        }

        if let (Some(key), Some(name), Some(status)) = (key, name, status) {
            let season = bounded_text(
                obj,
                "season",
                &format!("{path}.season"),
                false,
                SEASON_MAX_CHARS,
                errors,
            )
            .unwrap_or_default();
            let description = bounded_text(
                obj,
                "description",
                &format!("{path}.description"),
                false,
                PROJECT_DESCRIPTION_MAX_CHARS,
                errors,
            )
            .unwrap_or_default();
            projects.push(ValidatedProject {
                key,
                name,
                season,
                status,
                deadline,
                description,
            });
        }
    }
    projects
}

/// 把流水线校验错误映射回阶段数组路径。
fn push_pipeline_error(
    pipeline_index: usize,
    stages: &[Value],
    error: PipelineError,
    errors: &mut Vec<TemplateError>,
) {
    let base = format!("pipelines[{pipeline_index}].stages");
    let stage_path = |position: u16, suffix: &str| {
        let index = stages.iter().position(|stage| {
            stage.get("position").and_then(Value::as_u64) == Some(u64::from(position))
        });
        match index {
            Some(index) => format!("{base}[{index}]{suffix}"),
            None => format!("{base}[*]{suffix}"),
        }
    };
    match error {
        PipelineError::Empty => {
            errors.push(TemplateError::new(base, "must contain at least one stage"))
        }
        PipelineError::OfferCount(count) => errors.push(TemplateError::new(
            base,
            format!("pipeline must contain exactly one offer stage, found {count}"),
        )),
        PipelineError::OfferNotLast => errors.push(TemplateError::new(
            format!("{base}[*].kind"),
            "the offer stage must be the last stage",
        )),
        PipelineError::InvalidStage { position, reason } => errors.push(TemplateError::new(
            stage_path(position, ""),
            format!("invalid stage at position {position}: {reason}"),
        )),
        PipelineError::InvalidRubric { position, reason } => errors.push(TemplateError::new(
            stage_path(position, ".rubric"),
            format!("invalid rubric for stage at position {position}: {reason}"),
        )),
        PipelineError::ScoreOutOfRange(value) => errors.push(TemplateError::new(
            base,
            format!("score must be within 0..=100, got {value}"),
        )),
    }
}

fn validate_pipelines(
    value: Option<&Value>,
    projects: &[ValidatedProject],
    errors: &mut Vec<TemplateError>,
) -> Vec<(String, ValidatedPipeline)> {
    let Some(items) = array_of(value, "pipelines", errors) else {
        return Vec::new();
    };
    let mut pipelines = Vec::new();
    let mut seen_keys = Vec::new();
    let mut seen_names = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let path = format!("pipelines[{index}]");
        let Some(obj) = object_of(Some(item), &path, errors) else {
            continue;
        };
        let key = required_text(obj, "key", &format!("{path}.key"), errors);
        let project_key = required_text(obj, "projectKey", &format!("{path}.projectKey"), errors);
        let name = required_text(obj, "name", &format!("{path}.name"), errors);
        if let Some(name) = &name
            && name.chars().count() > PROJECT_NAME_MAX_CHARS
        {
            errors.push(TemplateError::new(
                format!("{path}.name"),
                format!("must be 1..={PROJECT_NAME_MAX_CHARS} characters"),
            ));
        }

        if let Some(project_key) = &project_key
            && !projects.iter().any(|project| &project.key == project_key)
        {
            errors.push(TemplateError::new(
                format!("{path}.projectKey"),
                format!("unknown project key `{project_key}`"),
            ));
        }

        if let (Some(project_key), Some(name)) = (&project_key, &name) {
            let name_key = (project_key.clone(), normalize_key(name));
            if seen_names.contains(&name_key) {
                errors.push(TemplateError::new(
                    format!("{path}.name"),
                    format!("duplicate pipeline name `{name}` in project `{project_key}`"),
                ));
            } else {
                seen_names.push(name_key);
            }
        }
        if let Some(key) = &key {
            if seen_keys.contains(key) {
                errors.push(TemplateError::new(
                    format!("{path}.key"),
                    format!("duplicate key `{key}`"),
                ));
            } else {
                seen_keys.push(key.clone());
            }
        }

        let definition = array_of(obj.get("stages"), &format!("{path}.stages"), errors)
            .and_then(|stages| validate_stages(index, stages, errors));

        if let (Some(key), Some(project_key), Some(name), Some(definition)) =
            (key, project_key, name, definition)
        {
            pipelines.push((
                key.clone(),
                ValidatedPipeline {
                    key,
                    project_key,
                    name,
                    definition,
                },
            ));
        }
    }
    pipelines
}

fn validate_stages(
    pipeline_index: usize,
    stages: &[Value],
    errors: &mut Vec<TemplateError>,
) -> Option<PipelineDefinition> {
    let mut definitions = Vec::with_capacity(stages.len());
    for (stage_index, stage) in stages.iter().enumerate() {
        let mut stage = stage.clone();
        if let Some(object) = stage.as_object_mut() {
            object
                .entry("position")
                .or_insert_with(|| json!(stage_index));
            object.remove("key");
        }
        match serde_json::from_value::<StageDefinition>(stage) {
            Ok(definition) => definitions.push(definition),
            Err(error) => errors.push(TemplateError::new(
                format!("pipelines[{pipeline_index}].stages[{stage_index}]"),
                error.to_string(),
            )),
        }
    }
    if definitions.len() != stages.len() {
        return None;
    }
    match PipelineDefinition::parse(definitions) {
        Ok(definition) => Some(definition),
        Err(error) => {
            push_pipeline_error(pipeline_index, stages, error, errors);
            None
        }
    }
}

fn read_salary(
    obj: &Map<String, Value>,
    key: &str,
    path: &str,
    errors: &mut Vec<TemplateError>,
) -> Option<Option<u32>> {
    match obj.get(key) {
        Some(Value::Null) | None => Some(None),
        Some(value) => match value.as_u64().and_then(|raw| u32::try_from(raw).ok()) {
            Some(raw) => Some(Some(raw)),
            _ => {
                errors.push(TemplateError::new(path, "must be a non-negative integer"));
                None
            }
        },
    }
}

fn validate_jobs(
    value: Option<&Value>,
    projects: &[ValidatedProject],
    pipelines: &[(String, ValidatedPipeline)],
    errors: &mut Vec<TemplateError>,
) -> Vec<ValidatedJob> {
    let Some(items) = array_of(value, "jobs", errors) else {
        return Vec::new();
    };
    let mut jobs = Vec::new();
    let mut seen_keys = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let path = format!("jobs[{index}]");
        let Some(obj) = object_of(Some(item), &path, errors) else {
            continue;
        };
        let key = required_text(obj, "key", &format!("{path}.key"), errors);
        let project_key = required_text(obj, "projectKey", &format!("{path}.projectKey"), errors);
        let pipeline_key =
            required_text(obj, "pipelineKey", &format!("{path}.pipelineKey"), errors);
        let title = bounded_text(
            obj,
            "title",
            &format!("{path}.title"),
            true,
            JOB_TITLE_MAX_CHARS,
            errors,
        );

        let job_type = match obj.get("jobType") {
            Some(Value::String(raw)) => match JobType::parse(raw.trim()) {
                Some(job_type) => Some(job_type),
                None => {
                    errors.push(TemplateError::new(
                        format!("{path}.jobType"),
                        "must be one of campus / internship / experienced",
                    ));
                    None
                }
            },
            Some(_) => {
                errors.push(TemplateError::new(
                    format!("{path}.jobType"),
                    "must be a string",
                ));
                None
            }
            None => {
                errors.push(TemplateError::new(format!("{path}.jobType"), "is required"));
                None
            }
        };

        let status = match obj.get("status") {
            Some(Value::String(raw)) => match JobStatus::parse(raw.trim()) {
                Some(status) => Some(status),
                None => {
                    errors.push(TemplateError::new(
                        format!("{path}.status"),
                        "must be one of draft / open / closed",
                    ));
                    None
                }
            },
            Some(Value::Null) | None => Some(JobStatus::Draft),
            Some(_) => {
                errors.push(TemplateError::new(
                    format!("{path}.status"),
                    "must be a string",
                ));
                None
            }
        };

        if let Some(project_key) = &project_key
            && !projects.iter().any(|project| &project.key == project_key)
        {
            errors.push(TemplateError::new(
                format!("{path}.projectKey"),
                format!("unknown project key `{project_key}`"),
            ));
        }
        if let (Some(project_key), Some(pipeline_key)) = (&project_key, &pipeline_key) {
            match pipelines
                .iter()
                .find(|(_, pipeline)| &pipeline.key == pipeline_key)
            {
                Some((_, pipeline)) if &pipeline.project_key == project_key => {}
                Some((_, pipeline)) => errors.push(TemplateError::new(
                    format!("{path}.pipelineKey"),
                    format!(
                        "pipeline `{}` belongs to project `{}`, not `{project_key}`",
                        pipeline.key, pipeline.project_key
                    ),
                )),
                None => errors.push(TemplateError::new(
                    format!("{path}.pipelineKey"),
                    format!("unknown pipeline key `{pipeline_key}`"),
                )),
            }
        }

        if let Some(key) = &key {
            if seen_keys.contains(key) {
                errors.push(TemplateError::new(
                    format!("{path}.key"),
                    format!("duplicate key `{key}`"),
                ));
            } else {
                seen_keys.push(key.clone());
            }
        }
        let salary_min = read_salary(obj, "salaryMin", &format!("{path}.salaryMin"), errors);
        let salary_max = read_salary(obj, "salaryMax", &format!("{path}.salaryMax"), errors);
        if let (Some(Some(min)), Some(Some(max))) = (salary_min, salary_max)
            && max < min
        {
            errors.push(TemplateError::new(
                format!("{path}.salaryMax"),
                "must be greater than or equal to salaryMin",
            ));
        }
        let salary_months = match obj.get("salaryMonths") {
            Some(Value::Null) | None => Some(None),
            Some(value) => match value.as_u64().and_then(|raw| u8::try_from(raw).ok()) {
                Some(raw) if (1..=SALARY_MONTHS_MAX).contains(&raw) => Some(Some(raw)),
                _ => {
                    errors.push(TemplateError::new(
                        format!("{path}.salaryMonths"),
                        format!("must be an integer within 1..={SALARY_MONTHS_MAX}"),
                    ));
                    None
                }
            },
        };

        let tags = match array_of(obj.get("tags"), &format!("{path}.tags"), errors) {
            Some(items) => {
                let mut tags = Vec::new();
                for (tag_index, tag) in items.iter().enumerate() {
                    match tag.as_str() {
                        Some(text) if !text.trim().is_empty() => tags.push(text.trim().to_owned()),
                        _ => errors.push(TemplateError::new(
                            format!("{path}.tags[{tag_index}]"),
                            "must be a non-empty string",
                        )),
                    }
                }
                if tags.len() > JOB_TAGS_MAX_ITEMS {
                    errors.push(TemplateError::new(
                        format!("{path}.tags"),
                        format!("must contain at most {JOB_TAGS_MAX_ITEMS} items"),
                    ));
                }
                tags
            }
            None => Vec::new(),
        };

        if let (
            Some(key),
            Some(project_key),
            Some(pipeline_key),
            Some(title),
            Some(job_type),
            Some(status),
            Some(salary_min),
            Some(salary_max),
            Some(salary_months),
        ) = (
            key,
            project_key,
            pipeline_key,
            title,
            job_type,
            status,
            salary_min,
            salary_max,
            salary_months,
        ) {
            jobs.push(ValidatedJob {
                key,
                project_key,
                pipeline_key,
                title,
                city: bounded_text(
                    obj,
                    "city",
                    &format!("{path}.city"),
                    false,
                    CITY_MAX_CHARS,
                    errors,
                )
                .unwrap_or_default(),
                job_type,
                salary_min,
                salary_max,
                salary_months,
                tags,
                description: bounded_text(
                    obj,
                    "description",
                    &format!("{path}.description"),
                    false,
                    JOB_DESCRIPTION_MAX_CHARS,
                    errors,
                )
                .unwrap_or_default(),
                status,
            });
        }
    }
    jobs
}
