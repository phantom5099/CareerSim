//! 流水线校验规则（plan/01 §3.2 的 7 条）；纯函数，无 IO。

use std::collections::HashSet;

use super::PipelineError;
use super::model::{StageDefinition, StageKind};

/// 阶段标题长度上限（plan/01 §3.2）。
const TITLE_MAX_CHARS: usize = 40;
/// 阶段最大轮次上限。
const MAX_ROUNDS_LIMIT: u8 = 20;
/// 维度数量上限。
const DIMENSION_LIMIT: usize = 10;

/// 按规则逐条校验；规则 7（同类阶段可重复、顺序即执行顺序）无需检查。
pub(super) fn validate_stages(stages: &[StageDefinition]) -> Result<(), PipelineError> {
    // 规则 1：至少 1 个阶段。
    if stages.is_empty() {
        return Err(PipelineError::Empty);
    }

    // 规则 2：stage_key 流水线内唯一（格式由 StageKey::parse 保证）。
    let mut seen = HashSet::with_capacity(stages.len());
    for stage in stages {
        if !seen.insert(stage.key.as_str()) {
            return Err(PipelineError::DuplicateStageKey(
                stage.key.as_str().to_owned(),
            ));
        }
    }

    // 规则 3：Offer 恰好 1 个且必须在最后。
    let offer_count = stages
        .iter()
        .filter(|stage| stage.kind == StageKind::Offer)
        .count();
    if offer_count != 1 {
        return Err(PipelineError::OfferCount(offer_count));
    }
    if stages.last().map(|stage| stage.kind) != Some(StageKind::Offer) {
        return Err(PipelineError::OfferNotLast);
    }

    for stage in stages {
        validate_stage_fields(stage)?;
        validate_rubric(stage)?;
    }

    Ok(())
}

/// 规则 4：轮次边界；附带标题长度约束（plan/01 §3.2 StageDefinition）。
fn validate_stage_fields(stage: &StageDefinition) -> Result<(), PipelineError> {
    let title_chars = stage.title.chars().count();
    if !(1..=TITLE_MAX_CHARS).contains(&title_chars) {
        return Err(PipelineError::InvalidStage {
            key: stage.key.as_str().to_owned(),
            reason: format!("title must be 1..={TITLE_MAX_CHARS} characters"),
        });
    }
    if stage.min_rounds < 1
        || stage.max_rounds < stage.min_rounds
        || stage.max_rounds > MAX_ROUNDS_LIMIT
    {
        return Err(PipelineError::InvalidStage {
            key: stage.key.as_str().to_owned(),
            reason: format!(
                "rounds must satisfy 1 <= min_rounds <= max_rounds <= {MAX_ROUNDS_LIMIT}"
            ),
        });
    }
    Ok(())
}

/// 规则 5 与规则 6：维度数量、维度键唯一、权重总和。
fn validate_rubric(stage: &StageDefinition) -> Result<(), PipelineError> {
    let rubric = &stage.rubric;
    if rubric.dimensions.is_empty() || rubric.dimensions.len() > DIMENSION_LIMIT {
        return Err(PipelineError::InvalidRubric {
            key: stage.key.as_str().to_owned(),
            reason: format!("dimensions must contain 1..={DIMENSION_LIMIT} items"),
        });
    }

    let mut keys = HashSet::with_capacity(rubric.dimensions.len());
    for dimension in &rubric.dimensions {
        if dimension.key.is_empty() {
            return Err(PipelineError::InvalidRubric {
                key: stage.key.as_str().to_owned(),
                reason: "dimension key must not be empty".to_owned(),
            });
        }
        if !keys.insert(dimension.key.as_str()) {
            return Err(PipelineError::InvalidRubric {
                key: stage.key.as_str().to_owned(),
                reason: format!("duplicate dimension key `{}`", dimension.key),
            });
        }
    }

    let total_weight: u32 = rubric
        .dimensions
        .iter()
        .map(|dimension| u32::from(dimension.weight))
        .sum();
    let has_zero = rubric
        .dimensions
        .iter()
        .any(|dimension| dimension.weight == 0);
    // 规则 6：全 0 表示等权；否则不允许出现单个 0 权重（评估器把 0 权重当作 1
    // 参与计算，部分零权重会使总分失真并翻转判定）且总和必须 100。
    if total_weight == 0 {
        // 全 0：等权，合法；空维度列表由前置校验拒绝。
    } else if has_zero {
        return Err(PipelineError::InvalidRubric {
            key: stage.key.as_str().to_owned(),
            reason: "individual dimension weights must not be 0 unless all weights are 0 (equal weighting)".to_owned(),
        });
    } else if total_weight != 100 {
        return Err(PipelineError::InvalidRubric {
            key: stage.key.as_str().to_owned(),
            reason: format!("weights must sum to 100 when set, got {total_weight}"),
        });
    }

    Ok(())
}
