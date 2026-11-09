//! 求职流程阶段。当前阶段唯一决定当前角色。

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    HrScreen,
    Exam,
    TechInterview,
    ManagerInterview,
    HrbpInterview,
    Offer,
    Review,
}

impl Stage {
    pub const ALL: [Stage; 7] = [
        Stage::HrScreen,
        Stage::Exam,
        Stage::TechInterview,
        Stage::ManagerInterview,
        Stage::HrbpInterview,
        Stage::Offer,
        Stage::Review,
    ];

    pub fn role(self) -> Role {
        match self {
            Stage::HrScreen => Role::HrScreener,
            Stage::Exam => Role::ExamAuthor,
            Stage::TechInterview => Role::TechInterviewer,
            Stage::ManagerInterview => Role::HiringManager,
            Stage::HrbpInterview => Role::Hrbp,
            Stage::Offer => Role::OfferNegotiator,
            Stage::Review => Role::Coach,
        }
    }

    pub fn next(self) -> Option<Stage> {
        match self {
            Stage::HrScreen => Some(Stage::Exam),
            Stage::Exam => Some(Stage::TechInterview),
            Stage::TechInterview => Some(Stage::ManagerInterview),
            Stage::ManagerInterview => Some(Stage::HrbpInterview),
            Stage::HrbpInterview => Some(Stage::Offer),
            Stage::Offer => Some(Stage::Review),
            Stage::Review => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Stage::HrScreen => "hr_screen",
            Stage::Exam => "exam",
            Stage::TechInterview => "tech_interview",
            Stage::ManagerInterview => "manager_interview",
            Stage::HrbpInterview => "hrbp_interview",
            Stage::Offer => "offer",
            Stage::Review => "review",
        }
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown stage: {0}")]
pub struct ParseStageError(pub String);

impl FromStr for Stage {
    type Err = ParseStageError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "hr_screen" => Ok(Stage::HrScreen),
            "exam" => Ok(Stage::Exam),
            "tech_interview" => Ok(Stage::TechInterview),
            "manager_interview" => Ok(Stage::ManagerInterview),
            "hrbp_interview" => Ok(Stage::HrbpInterview),
            "offer" => Ok(Stage::Offer),
            "review" => Ok(Stage::Review),
            other => Err(ParseStageError(other.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests;
