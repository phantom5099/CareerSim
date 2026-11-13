//! 流水线纯领域契约：阶段键、阶段类型、流水线定义与评分规则。
//!
//! 本模块无 IO；catalog 与 agent 只消费这里通过 `PipelineDefinition::parse`
//! 证明过不变量的类型。

use thiserror::Error;

pub mod model;
pub mod rubric;
mod validate;

#[cfg(test)]
mod tests;

pub use model::{PipelineDefinition, StageDefinition, StageKey, StageKind};
pub use rubric::{EvaluationRubric, RubricDimension, Score};

/// 流水线构造与校验错误。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PipelineError {
    #[error("pipeline must contain at least one stage")]
    Empty,
    #[error("duplicate stage key `{0}`")]
    DuplicateStageKey(String),
    #[error("pipeline must contain exactly one offer stage, found {0}")]
    OfferCount(usize),
    #[error("the offer stage must be the last stage")]
    OfferNotLast,
    #[error("invalid stage key `{key}`: {reason}")]
    InvalidStageKey { key: String, reason: &'static str },
    #[error("invalid stage `{key}`: {reason}")]
    InvalidStage { key: String, reason: String },
    #[error("invalid rubric for stage `{key}`: {reason}")]
    InvalidRubric { key: String, reason: String },
    #[error("score must be within 0..=100, got {0}")]
    ScoreOutOfRange(u8),
}
