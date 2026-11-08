//! 模拟招聘角色。

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    HrScreener,
    ExamAuthor,
    TechInterviewer,
    HiringManager,
    Hrbp,
    OfferNegotiator,
    Coach,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HrScreener => "hr_screener",
            Self::ExamAuthor => "exam_author",
            Self::TechInterviewer => "tech_interviewer",
            Self::HiringManager => "hiring_manager",
            Self::Hrbp => "hrbp",
            Self::OfferNegotiator => "offer_negotiator",
            Self::Coach => "coach",
        }
    }

    /// 角色系统提示词；提示词与代码分离，存放在 `agent/role/prompts`。
    pub fn system_prompt(self) -> &'static str {
        match self {
            Self::HrScreener => include_str!("role/prompts/hr_screener.md"),
            Self::ExamAuthor => include_str!("role/prompts/exam_author.md"),
            Self::TechInterviewer => include_str!("role/prompts/tech_interviewer.md"),
            Self::HiringManager => include_str!("role/prompts/hiring_manager.md"),
            Self::Hrbp => include_str!("role/prompts/hrbp.md"),
            Self::OfferNegotiator => include_str!("role/prompts/offer_negotiator.md"),
            Self::Coach => include_str!("role/prompts/coach.md"),
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
