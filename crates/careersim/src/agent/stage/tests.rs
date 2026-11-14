use crate::agent::Role;
use crate::pipeline::StageKind;

#[test]
fn role_follows_stage_kind() {
    assert_eq!(StageKind::HrScreen.role(), Role::HrScreener);
    assert_eq!(StageKind::Exam.role(), Role::ExamAuthor);
    assert_eq!(StageKind::TechInterview.role(), Role::TechInterviewer);
    assert_eq!(StageKind::ManagerInterview.role(), Role::HiringManager);
    assert_eq!(StageKind::HrbpInterview.role(), Role::Hrbp);
    assert_eq!(StageKind::Offer.role(), Role::OfferNegotiator);
    assert!(!Role::Coach.system_prompt().is_empty());
}
