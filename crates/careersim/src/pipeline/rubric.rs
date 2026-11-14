//! 评分规则、分值类型与服务端评估。

use std::collections::HashMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use super::PipelineError;

/// 通用分值，0..=100；维度分与总分共用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score(u8);

impl Score {
    pub const MAX: u8 = 100;

    /// 构造入口：只有 0..=100 能构造出该类型。
    pub fn new(value: u8) -> Result<Self, PipelineError> {
        if value > Self::MAX {
            return Err(PipelineError::ScoreOutOfRange(value));
        }
        Ok(Self(value))
    }

    pub const fn value(self) -> u8 {
        self.0
    }
}

impl Serialize for Score {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u8(self.0)
    }
}

impl<'de> Deserialize<'de> for Score {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = u8::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// 评分维度；`key` 在流水线内唯一，评估提交必须与之完全匹配。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RubricDimension {
    pub key: String,
    pub label: String,
    /// 可选权重；若使用（非全 0），总和必须 100，全 0 表示等权。
    pub weight: u16,
    pub criteria: String,
}

/// 阶段评估规则；维度 1..=10。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationRubric {
    /// 通过阈值：总分 >= 该值即通过。
    pub pass_score: Score,
    pub dimensions: Vec<RubricDimension>,
}

/// 维度评分上限（评估依据长度，plan/02 §4.3）。
pub(crate) const NOTE_MAX_CHARS: usize = 500;
/// 阶段总评长度上限。
pub(crate) const SUMMARY_MAX_CHARS: usize = 2000;

/// 模型提交的单个维度评分与依据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DimensionScore {
    /// 评估维度键；必须与 rubric 维度键完全一致。
    pub key: String,
    /// 该维度得分，0..=100。
    pub score: Score,
    /// 评分依据，1..=500 字符。
    pub note: String,
}

/// 阶段评估结论；总分与通过判定由服务端计算，不信任模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StageEvaluation {
    /// 各维度分数与依据；集合与 rubric 完全一致。
    pub dimensions: Vec<DimensionScore>,
    /// 服务端按权重计算的总分。
    pub total: Score,
    /// total >= rubric.pass_score；服务端判定。
    pub passed: bool,
    /// 阶段总评，1..=2000 字符；注入后续阶段上下文。
    pub summary: String,
}

/// 评估提交校验失败的原因。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvaluationError {
    #[error("missing dimension `{0}`")]
    MissingDimension(String),
    #[error("unknown dimension `{0}`")]
    UnknownDimension(String),
    #[error("duplicate dimension `{0}`")]
    DuplicateDimension(String),
    #[error("dimension `{0}` note must be 1..={NOTE_MAX_CHARS} characters")]
    InvalidNote(String),
    #[error("summary must be 1..={SUMMARY_MAX_CHARS} characters")]
    InvalidSummary,
    #[error("computed total {0} is out of range")]
    TotalOutOfRange(u64),
}

/// 服务端评估：校验维度集合与文本边界，按权重计算总分并判定是否通过。
///
/// 权重全 0 表示等权；加权平均使用整数运算并四舍五入到整数。
/// 输出的维度按 rubric 顺序排列，保证结论可审计、可复现。
pub fn evaluate(
    rubric: &EvaluationRubric,
    submitted: Vec<DimensionScore>,
    summary: String,
) -> Result<StageEvaluation, EvaluationError> {
    let summary_chars = summary.chars().count();
    if !(1..=SUMMARY_MAX_CHARS).contains(&summary_chars) {
        return Err(EvaluationError::InvalidSummary);
    }

    let mut by_key = HashMap::with_capacity(submitted.len());
    for dimension in submitted {
        if !rubric
            .dimensions
            .iter()
            .any(|expected| expected.key == dimension.key)
        {
            return Err(EvaluationError::UnknownDimension(dimension.key));
        }
        if by_key.contains_key(&dimension.key) {
            return Err(EvaluationError::DuplicateDimension(dimension.key));
        }
        let note_chars = dimension.note.chars().count();
        if !(1..=NOTE_MAX_CHARS).contains(&note_chars) {
            return Err(EvaluationError::InvalidNote(dimension.key));
        }
        by_key.insert(dimension.key.clone(), dimension);
    }

    let mut dimensions = Vec::with_capacity(rubric.dimensions.len());
    let mut weighted_sum: u64 = 0;
    let mut total_weight: u64 = 0;
    for expected in &rubric.dimensions {
        let Some(dimension) = by_key.remove(&expected.key) else {
            return Err(EvaluationError::MissingDimension(expected.key.clone()));
        };
        // 全 0 权重表示等权：每个维度按权重 1 参与计算。
        let weight = u64::from(expected.weight).max(1);
        weighted_sum += weight * u64::from(dimension.score.value());
        total_weight += weight;
        dimensions.push(dimension);
    }

    let rounded = (weighted_sum + total_weight / 2) / total_weight;
    let value = u8::try_from(rounded).map_err(|_| EvaluationError::TotalOutOfRange(rounded))?;
    let total = Score::new(value).map_err(|_| EvaluationError::TotalOutOfRange(rounded))?;

    Ok(StageEvaluation {
        dimensions,
        total,
        passed: total.value() >= rubric.pass_score.value(),
        summary,
    })
}

impl std::fmt::Display for Score {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
