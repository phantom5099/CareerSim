//! 评分规则与分值类型。

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
