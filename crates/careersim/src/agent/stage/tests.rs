use super::*;

#[test]
fn stages_form_a_fixed_sequence() {
    let mut current = Stage::HrScreen;
    let mut visited = vec![current];
    while let Some(next) = current.next() {
        current = next;
        visited.push(current);
    }

    assert_eq!(visited, Stage::ALL.to_vec());
    assert_eq!(current, Stage::Review);
    assert_eq!(Stage::Review.next(), None);
}

#[test]
fn role_follows_stage() {
    assert_eq!(Stage::HrScreen.role(), Role::HrScreener);
    assert_eq!(Stage::Exam.role(), Role::ExamAuthor);
    assert_eq!(Stage::TechInterview.role(), Role::TechInterviewer);
    assert_eq!(Stage::ManagerInterview.role(), Role::HiringManager);
    assert_eq!(Stage::HrbpInterview.role(), Role::Hrbp);
    assert_eq!(Stage::Offer.role(), Role::OfferNegotiator);
    assert_eq!(Stage::Review.role(), Role::Coach);
    assert!(!Role::Coach.system_prompt().is_empty());
}

#[test]
fn stage_text_roundtrips() {
    for stage in Stage::ALL {
        assert_eq!(Stage::from_str(stage.as_str()), Ok(stage));
    }
    assert!(Stage::from_str("unknown").is_err());
}
