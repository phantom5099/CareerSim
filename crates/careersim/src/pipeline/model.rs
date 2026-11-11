//! 流水线值类型：阶段键、阶段类型、阶段定义与流水线定义。

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::PipelineError;
use super::rubric::EvaluationRubric;
use super::validate;

/// 阶段稳定标识：`^[a-z][a-z0-9_]{1,31}$`，流水线内唯一。
///
/// 事件、投递、评估均以它引用阶段；改名即破坏既有投递。
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StageKey(String);

impl StageKey {
    /// 归一化入口：只有合法键能构造出该类型。
    pub fn parse(raw: &str) -> Result<Self, PipelineError> {
        let mut chars = raw.chars();
        match chars.next() {
            Some(first) if first.is_ascii_lowercase() => {}
            _ => {
                return Err(PipelineError::InvalidStageKey {
                    key: raw.to_owned(),
                    reason: "must start with a lowercase ascii letter",
                });
            }
        }

        let rest = chars.count();
        if rest < 1 {
            return Err(PipelineError::InvalidStageKey {
                key: raw.to_owned(),
                reason: "must be at least 2 characters",
            });
        }
        if rest > 31 {
            return Err(PipelineError::InvalidStageKey {
                key: raw.to_owned(),
                reason: "must be at most 32 characters",
            });
        }
        let rest_valid = raw
            .chars()
            .skip(1)
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
        if !rest_valid {
            return Err(PipelineError::InvalidStageKey {
                key: raw.to_owned(),
                reason: "must contain only lowercase ascii letters, digits and `_`",
            });
        }

        Ok(Self(raw.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StageKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for StageKey {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for StageKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// 阶段类型；决定角色提示词与执行策略。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageKind {
    /// HR 初筛。
    HrScreen,
    /// 笔试。
    Exam,
    /// 技术面试。
    TechInterview,
    /// 用人经理面试。
    ManagerInterview,
    /// HRBP 面试。
    HrbpInterview,
    /// Offer 谈薪；流水线内唯一且必须最后。
    Offer,
}

impl StageKind {
    /// 与数据库 CHECK 约束一致的单处字符串映射。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HrScreen => "hr_screen",
            Self::Exam => "exam",
            Self::TechInterview => "tech_interview",
            Self::ManagerInterview => "manager_interview",
            Self::HrbpInterview => "hrbp_interview",
            Self::Offer => "offer",
        }
    }
}

/// 单个阶段的定义；字段级与流水线级约束由 `PipelineDefinition::parse` 统一校验。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageDefinition {
    pub key: StageKey,
    pub kind: StageKind,
    /// 展示标题，1..=40 字符。
    pub title: String,
    /// 最少对话轮次；未达到不允许提交阶段结论。
    pub min_rounds: u8,
    /// 最大轮次；达到后强制进入评估轮。
    pub max_rounds: u8,
    pub rubric: EvaluationRubric,
}

/// 校验后的阶段序列；构造入口只有 `parse`，不变量由此证明。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipelineDefinition {
    stages: Vec<StageDefinition>,
}

impl PipelineDefinition {
    /// 校验并构造流水线定义。
    pub fn parse(stages: Vec<StageDefinition>) -> Result<Self, PipelineError> {
        validate::validate_stages(&stages)?;
        Ok(Self { stages })
    }

    pub fn stages(&self) -> &[StageDefinition] {
        &self.stages
    }

    /// 首阶段；`parse` 保证非空。
    pub fn first(&self) -> &StageDefinition {
        &self.stages[0]
    }

    pub fn get(&self, key: &StageKey) -> Option<&StageDefinition> {
        self.stages.iter().find(|stage| &stage.key == key)
    }

    pub fn position(&self, key: &StageKey) -> Option<usize> {
        self.stages.iter().position(|stage| &stage.key == key)
    }
}
