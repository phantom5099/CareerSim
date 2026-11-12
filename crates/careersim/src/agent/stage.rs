//! 阶段类型到角色的映射；阶段键与流水线定义在 `pipeline` 域。

use super::Role;
use crate::pipeline::StageKind;

impl StageKind {
    /// 当前阶段唯一决定当前角色；`Coach` 不对应流水线阶段。
    pub fn role(self) -> Role {
        match self {
            Self::HrScreen => Role::HrScreener,
            Self::Exam => Role::ExamAuthor,
            Self::TechInterview => Role::TechInterviewer,
            Self::ManagerInterview => Role::HiringManager,
            Self::HrbpInterview => Role::Hrbp,
            Self::Offer => Role::OfferNegotiator,
        }
    }
}

#[cfg(test)]
mod tests;
