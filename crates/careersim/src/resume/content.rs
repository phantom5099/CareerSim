//! 简历结构化内容与字段边界（plan/01 §3.4）；与前端 `ResumePayload` 对齐。

use serde::{Deserialize, Serialize};

use super::model::ResumeError;

/// 顶层标量字段上限（字符数）。
pub const MAX_FIELD_CHARS: usize = 120;
/// 自我介绍上限（字符数）。
pub const MAX_SUMMARY_CHARS: usize = 4000;
/// 每类条目数上限。
pub const MAX_ITEMS: usize = 20;
/// 条目内普通字段上限（字符数）。
pub const MAX_ITEM_CHARS: usize = 120;
/// 条目内描述字段上限（字符数）。
pub const MAX_ITEM_DESCRIPTION_CHARS: usize = 2000;
/// 序列化后的整体字节数上限（与迁移 `resumes_content_size` 一致）。
pub const MAX_SERIALIZED_BYTES: usize = 262_144;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeEducation {
    pub school: String,
    pub degree: String,
    pub major: String,
    pub start_date: String,
    pub end_date: String,
    pub ranking: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeExperience {
    pub company: String,
    pub role: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub start_date: String,
    pub end_date: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeProject {
    pub name: String,
    pub role: String,
    pub start_date: String,
    pub end_date: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumePractice {
    pub name: String,
    pub role: String,
    pub start_date: String,
    pub end_date: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeSkill {
    pub name: String,
    pub level: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeCertificate {
    pub name: String,
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResumeHonor {
    pub name: String,
    pub level: String,
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ResumeContent {
    pub name: String,
    pub phone: String,
    pub email: String,
    pub city: String,
    pub target_role: String,
    pub target_city: String,
    pub job_type: String,
    pub available_date: String,
    pub educations: Vec<ResumeEducation>,
    pub experiences: Vec<ResumeExperience>,
    pub projects: Vec<ResumeProject>,
    pub practices: Vec<ResumePractice>,
    pub skills: Vec<ResumeSkill>,
    pub certificates: Vec<ResumeCertificate>,
    pub honors: Vec<ResumeHonor>,
    pub summary: String,
}

impl ResumeContent {
    /// 边界校验（plan/01 §3.4）；数据库 CHECK 兜底整体字节数。
    /// 手机号与邮箱只做长度校验，不做强格式拒绝（解析结果可能不规范）。
    pub fn validate(&self) -> Result<(), ResumeError> {
        for (field, value) in [
            ("name", &self.name),
            ("phone", &self.phone),
            ("email", &self.email),
            ("city", &self.city),
            ("targetRole", &self.target_role),
            ("targetCity", &self.target_city),
            ("jobType", &self.job_type),
            ("availableDate", &self.available_date),
        ] {
            check_field(field, value, MAX_FIELD_CHARS)?;
        }
        check_field("summary", &self.summary, MAX_SUMMARY_CHARS)?;

        check_items("educations", self.educations.len())?;
        for (index, item) in self.educations.iter().enumerate() {
            for (field, value) in [
                ("school", &item.school),
                ("degree", &item.degree),
                ("major", &item.major),
                ("startDate", &item.start_date),
                ("endDate", &item.end_date),
                ("ranking", &item.ranking),
            ] {
                check_item_field("educations", index, field, value, MAX_ITEM_CHARS)?;
            }
        }

        check_items("experiences", self.experiences.len())?;
        for (index, item) in self.experiences.iter().enumerate() {
            for (field, value) in [
                ("company", &item.company),
                ("role", &item.role),
                ("type", &item.kind),
                ("startDate", &item.start_date),
                ("endDate", &item.end_date),
            ] {
                check_item_field("experiences", index, field, value, MAX_ITEM_CHARS)?;
            }
            check_item_field(
                "experiences",
                index,
                "description",
                &item.description,
                MAX_ITEM_DESCRIPTION_CHARS,
            )?;
        }

        check_items("projects", self.projects.len())?;
        for (index, item) in self.projects.iter().enumerate() {
            for (field, value) in [
                ("name", &item.name),
                ("role", &item.role),
                ("startDate", &item.start_date),
                ("endDate", &item.end_date),
            ] {
                check_item_field("projects", index, field, value, MAX_ITEM_CHARS)?;
            }
            check_item_field(
                "projects",
                index,
                "description",
                &item.description,
                MAX_ITEM_DESCRIPTION_CHARS,
            )?;
        }

        check_items("practices", self.practices.len())?;
        for (index, item) in self.practices.iter().enumerate() {
            for (field, value) in [
                ("name", &item.name),
                ("role", &item.role),
                ("startDate", &item.start_date),
                ("endDate", &item.end_date),
            ] {
                check_item_field("practices", index, field, value, MAX_ITEM_CHARS)?;
            }
            check_item_field(
                "practices",
                index,
                "description",
                &item.description,
                MAX_ITEM_DESCRIPTION_CHARS,
            )?;
        }

        check_items("skills", self.skills.len())?;
        for (index, item) in self.skills.iter().enumerate() {
            check_item_field("skills", index, "name", &item.name, MAX_ITEM_CHARS)?;
            check_item_field("skills", index, "level", &item.level, MAX_ITEM_CHARS)?;
        }

        check_items("certificates", self.certificates.len())?;
        for (index, item) in self.certificates.iter().enumerate() {
            check_item_field("certificates", index, "name", &item.name, MAX_ITEM_CHARS)?;
            check_item_field("certificates", index, "date", &item.date, MAX_ITEM_CHARS)?;
        }

        check_items("honors", self.honors.len())?;
        for (index, item) in self.honors.iter().enumerate() {
            check_item_field("honors", index, "name", &item.name, MAX_ITEM_CHARS)?;
            check_item_field("honors", index, "level", &item.level, MAX_ITEM_CHARS)?;
            check_item_field("honors", index, "date", &item.date, MAX_ITEM_CHARS)?;
        }
        Ok(())
    }

    /// 序列化为 JSON；整体字节数超限返回 `ContentTooLarge`。
    pub fn to_json(&self) -> Result<serde_json::Value, ResumeError> {
        let value = serde_json::to_value(self).map_err(|err| ResumeError::Backend {
            message: "serialize resume content".to_owned(),
            source: Some(Box::new(err)),
        })?;
        let bytes = serde_json::to_string(&value)
            .map_err(|err| ResumeError::Backend {
                message: "serialize resume content".to_owned(),
                source: Some(Box::new(err)),
            })?
            .into_bytes()
            .len();
        if bytes > MAX_SERIALIZED_BYTES {
            return Err(ResumeError::ContentTooLarge);
        }
        Ok(value)
    }
}

fn check_field(field: &'static str, value: &str, max: usize) -> Result<(), ResumeError> {
    if value.chars().count() > max {
        return Err(ResumeError::InvalidContent {
            field: field.to_owned(),
            reason: "exceeds the character limit",
        });
    }
    Ok(())
}

fn check_items(field: &'static str, len: usize) -> Result<(), ResumeError> {
    if len > MAX_ITEMS {
        return Err(ResumeError::InvalidContent {
            field: field.to_owned(),
            reason: "exceeds the item limit",
        });
    }
    Ok(())
}

fn check_item_field(
    section: &'static str,
    index: usize,
    field: &'static str,
    value: &str,
    max: usize,
) -> Result<(), ResumeError> {
    if value.chars().count() > max {
        return Err(ResumeError::InvalidContent {
            field: format!("{section}[{index}].{field}"),
            reason: "exceeds the character limit",
        });
    }
    Ok(())
}
