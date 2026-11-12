//! 流水线值类型：阶段键、阶段类型、阶段定义、流水线定义、转移决策与 Offer 条款。

use std::fmt;

use chrono::{DateTime, Days, NaiveDate, Utc};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use super::PipelineError;
use super::rubric::EvaluationRubric;
use super::rubric::StageEvaluation;
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

/// 阶段转移决策；由纯函数 `plan_transition` 产生，事件与投影共同消费。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StageOutcome {
    /// 通过并进入下一阶段。
    Advance {
        next_stage_key: StageKey,
        next_stage_kind: StageKind,
    },
    /// Offer 阶段通过且条款校验通过；等待候选人决定。
    OfferPending { offer: OfferTerms },
    /// 未通过；停留在当前阶段。
    Rejected,
}

/// 一次阶段完成的完整转移记录。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTransition {
    /// 转移起点阶段键。
    pub from: StageKey,
    /// 服务端评估结论。
    pub evaluation: StageEvaluation,
    /// 转移决策。
    pub outcome: StageOutcome,
}

/// 阶段转移决策失败。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("unknown stage `{0}`")]
    UnknownStage(String),
    #[error("offer terms are required when the offer stage passes")]
    OfferTermsRequired,
}

/// 纯函数：根据评估结论与流水线定义决定转移结果。
///
/// 规则（plan/02 §4）：
/// - 阶段不在流水线内 → `UnknownStage`；
/// - 未达通过阈值 → `Rejected`，停留在当前阶段；
/// - 通过且有下一阶段 → `Advance`；
/// - 通过且为最后阶段（按不变量必为 Offer 阶段）→ 必须携带已通过
///   `build_offer_terms` 校验的条款，否则 `OfferTermsRequired`。
pub fn plan_transition(
    pipeline: &PipelineDefinition,
    from: &StageKey,
    evaluation: StageEvaluation,
    offer: Option<OfferTerms>,
) -> Result<StageTransition, TransitionError> {
    let Some(stage) = pipeline.get(from) else {
        return Err(TransitionError::UnknownStage(from.to_string()));
    };
    if !evaluation.passed {
        return Ok(StageTransition {
            from: stage.key.clone(),
            evaluation,
            outcome: StageOutcome::Rejected,
        });
    }
    let position = pipeline
        .position(from)
        .ok_or_else(|| TransitionError::UnknownStage(from.to_string()))?;
    match pipeline.stages().get(position + 1) {
        Some(next) => Ok(StageTransition {
            from: stage.key.clone(),
            evaluation,
            outcome: StageOutcome::Advance {
                next_stage_key: next.key.clone(),
                next_stage_kind: next.kind,
            },
        }),
        None => {
            let offer = offer.ok_or(TransitionError::OfferTermsRequired)?;
            Ok(StageTransition {
                from: stage.key.clone(),
                evaluation,
                outcome: StageOutcome::OfferPending { offer },
            })
        }
    }
}

/// Offer 奖金条款长度上限。
pub(crate) const BONUS_MAX_CHARS: usize = 120;

/// 模型提交的原始 Offer 条款；入库前必须通过 `build_offer_terms` 校验。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfferTermsInput {
    /// 月度基本薪资（元/月）。
    pub base_salary: u32,
    /// 合同月数。
    pub months: u8,
    /// 奖金条款描述；可为空串。
    #[serde(default)]
    pub bonus: String,
}

/// 校验后的 Offer 条款；服务端构建，事件中只出现该形态。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfferTerms {
    /// 月度基本薪资（元/月）。
    pub base_salary: u32,
    /// 合同月数。
    pub months: u8,
    /// 奖金条款描述。
    pub bonus: String,
    /// 工作城市；取自策略，不信任模型。
    pub city: String,
    /// 签发时间。
    pub issued_at: DateTime<Utc>,
    /// 接受截止日期。
    pub deadline: NaiveDate,
}

/// Offer 校验策略；由部署配置提供，不进事件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfferPolicy {
    /// 统一工作城市。
    pub city: String,
    /// 月薪下限（含）；`None` 表示不校验。
    pub salary_min: Option<u32>,
    /// 月薪上限（含）；`None` 表示不校验。
    pub salary_max: Option<u32>,
    /// 要求的合同月数；`None` 表示仅受 12..=24 约束。
    pub months: Option<u8>,
    /// 接受截止天数。
    pub deadline_days: u16,
}

/// Offer 条款校验失败。
#[derive(Debug, Error, PartialEq, Eq)]
pub enum OfferError {
    #[error("base salary {0} is below the policy minimum {1}")]
    SalaryBelowMinimum(u32, u32),
    #[error("base salary {0} is above the policy maximum {1}")]
    SalaryAboveMaximum(u32, u32),
    #[error("contract months must be within 12..=24, got {0}")]
    MonthsOutOfRange(u8),
    #[error("contract months {0} do not match the policy requirement {1}")]
    MonthsMismatch(u8, u8),
    #[error("bonus description must be at most {BONUS_MAX_CHARS} characters")]
    BonusTooLong,
    #[error("offer deadline could not be computed from policy")]
    DeadlineInvalid,
}

/// 纯函数：校验模型提交的 Offer 条款并构建最终形态。
///
/// 校验顺序（plan/02 §6.3）：薪资区间 → 月数范围 → 月数策略 → 奖金长度；
/// 截止日期由策略天数推算，溢出即失败。`city` 与 `deadline` 一律来自服务端。
pub fn build_offer_terms(
    input: OfferTermsInput,
    policy: &OfferPolicy,
    now: DateTime<Utc>,
) -> Result<OfferTerms, OfferError> {
    if let Some(min) = policy.salary_min
        && input.base_salary < min
    {
        return Err(OfferError::SalaryBelowMinimum(input.base_salary, min));
    }
    if let Some(max) = policy.salary_max
        && input.base_salary > max
    {
        return Err(OfferError::SalaryAboveMaximum(input.base_salary, max));
    }
    if !(12..=24).contains(&input.months) {
        return Err(OfferError::MonthsOutOfRange(input.months));
    }
    if let Some(months) = policy.months
        && input.months != months
    {
        return Err(OfferError::MonthsMismatch(input.months, months));
    }
    let bonus_chars = input.bonus.chars().count();
    if bonus_chars > BONUS_MAX_CHARS {
        return Err(OfferError::BonusTooLong);
    }
    let deadline = now
        .date_naive()
        .checked_add_days(Days::new(u64::from(policy.deadline_days)))
        .ok_or(OfferError::DeadlineInvalid)?;
    Ok(OfferTerms {
        base_salary: input.base_salary,
        months: input.months,
        bonus: input.bonus,
        city: policy.city.clone(),
        issued_at: now,
        deadline,
    })
}
