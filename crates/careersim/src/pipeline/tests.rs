//! 流水线校验单测：覆盖 plan/01 §3.2 的校验规则与值对象边界。

use super::PipelineError;
use super::model::{PipelineDefinition, StageDefinition, StageKey, StageKind};
use super::rubric::{EvaluationRubric, RubricDimension, Score};

fn dimension(key: &str, weight: u16) -> RubricDimension {
    RubricDimension {
        key: key.to_owned(),
        label: format!("维度 {key}"),
        weight,
        criteria: "评分说明".to_owned(),
    }
}

fn rubric(weights: &[u16]) -> EvaluationRubric {
    EvaluationRubric {
        pass_score: Score::new(60).expect("60 is a valid score"),
        dimensions: weights
            .iter()
            .enumerate()
            .map(|(index, weight)| dimension(&format!("d{index}"), *weight))
            .collect(),
    }
}

fn stage(
    key: &str,
    kind: StageKind,
    min_rounds: u8,
    max_rounds: u8,
    weights: &[u16],
) -> StageDefinition {
    StageDefinition {
        key: StageKey::parse(key).expect("test key is valid"),
        kind,
        title: "阶段标题".to_owned(),
        min_rounds,
        max_rounds,
        rubric: rubric(weights),
    }
}

fn valid_stages() -> Vec<StageDefinition> {
    vec![
        stage("hr", StageKind::HrScreen, 1, 2, &[0]),
        stage("tech_1", StageKind::TechInterview, 2, 8, &[60, 40]),
        stage("offer", StageKind::Offer, 1, 4, &[100]),
    ]
}

#[test]
fn parse_accepts_pipeline_with_repeated_kinds() {
    // 规则 7：同类阶段允许重复，用不同 key 区分；顺序即执行顺序。
    let parsed = PipelineDefinition::parse(vec![
        stage("tech_1", StageKind::TechInterview, 1, 2, &[0]),
        stage("tech_2", StageKind::TechInterview, 1, 2, &[0]),
        stage("offer", StageKind::Offer, 1, 2, &[0]),
    ]);
    assert!(parsed.is_ok());
}

#[test]
fn parse_accepts_single_offer_stage() {
    let parsed = PipelineDefinition::parse(vec![stage("offer", StageKind::Offer, 1, 1, &[0])]);
    let parsed = parsed.expect("single offer is valid");
    assert_eq!(parsed.first().key.as_str(), "offer");
    assert_eq!(
        parsed.position(&StageKey::parse("offer").expect("valid in test")),
        Some(0)
    );
    assert!(
        parsed
            .get(&StageKey::parse("missing").expect("valid in test"))
            .is_none()
    );
}

#[test]
fn parse_rejects_empty_pipeline() {
    assert_eq!(
        PipelineDefinition::parse(Vec::new()),
        Err(PipelineError::Empty)
    );
}

#[test]
fn parse_rejects_duplicate_stage_keys() {
    let mut stages = valid_stages();
    stages.insert(1, stage("hr", StageKind::Exam, 1, 2, &[0]));
    assert_eq!(
        PipelineDefinition::parse(stages),
        Err(PipelineError::DuplicateStageKey("hr".to_owned()))
    );
}

#[test]
fn parse_rejects_missing_or_duplicated_offer() {
    let no_offer = vec![
        stage("hr", StageKind::HrScreen, 1, 2, &[0]),
        stage("tech_1", StageKind::TechInterview, 1, 2, &[0]),
    ];
    assert_eq!(
        PipelineDefinition::parse(no_offer),
        Err(PipelineError::OfferCount(0))
    );

    let two_offers = vec![
        stage("offer_1", StageKind::Offer, 1, 2, &[0]),
        stage("offer_2", StageKind::Offer, 1, 2, &[0]),
    ];
    assert_eq!(
        PipelineDefinition::parse(two_offers),
        Err(PipelineError::OfferCount(2))
    );
}

#[test]
fn parse_rejects_offer_before_last() {
    let stages = vec![
        stage("offer", StageKind::Offer, 1, 2, &[0]),
        stage("tech_1", StageKind::TechInterview, 1, 2, &[0]),
    ];
    assert_eq!(
        PipelineDefinition::parse(stages),
        Err(PipelineError::OfferNotLast)
    );
}

#[test]
fn parse_rejects_invalid_round_bounds() {
    for (min_rounds, max_rounds) in [(0, 2), (3, 2), (1, 21)] {
        let stages = vec![stage(
            "offer",
            StageKind::Offer,
            min_rounds,
            max_rounds,
            &[0],
        )];
        assert!(
            matches!(
                PipelineDefinition::parse(stages),
                Err(PipelineError::InvalidStage { .. })
            ),
            "rounds ({min_rounds}, {max_rounds}) must be rejected"
        );
    }
}

#[test]
fn parse_rejects_invalid_dimension_sets() {
    let stages = vec![stage("offer", StageKind::Offer, 1, 2, &[])];

    assert!(matches!(
        PipelineDefinition::parse(stages),
        Err(PipelineError::InvalidRubric { .. })
    ));

    let too_many: Vec<u16> = vec![0; 11];
    let stages = vec![stage("offer", StageKind::Offer, 1, 2, &too_many)];
    assert!(matches!(
        PipelineDefinition::parse(stages),
        Err(PipelineError::InvalidRubric { .. })
    ));
}

#[test]
fn parse_rejects_duplicate_dimension_keys() {
    let mut hr = stage("hr", StageKind::HrScreen, 1, 2, &[50, 50]);
    hr.rubric.dimensions[1].key = hr.rubric.dimensions[0].key.clone();
    let stages = vec![hr, stage("offer", StageKind::Offer, 1, 2, &[0])];
    assert!(matches!(
        PipelineDefinition::parse(stages),
        Err(PipelineError::InvalidRubric { .. })
    ));
}

#[test]
fn parse_rejects_weights_not_summing_to_100() {
    let stages = vec![stage("offer", StageKind::Offer, 1, 2, &[40, 40])];
    assert!(matches!(
        PipelineDefinition::parse(stages),
        Err(PipelineError::InvalidRubric { .. })
    ));

    // 全 0 表示等权，合法。
    let stages = vec![stage("offer", StageKind::Offer, 1, 2, &[0, 0])];
    assert!(PipelineDefinition::parse(stages).is_ok());
}

#[test]
fn parse_rejects_empty_or_overlong_title() {
    let mut stages = stage("offer", StageKind::Offer, 1, 2, &[0]);
    stages.title = String::new();
    assert!(matches!(
        PipelineDefinition::parse(vec![stages]),
        Err(PipelineError::InvalidStage { .. })
    ));

    let mut stages = stage("offer", StageKind::Offer, 1, 2, &[0]);
    stages.title = "长".repeat(41);
    assert!(matches!(
        PipelineDefinition::parse(vec![stages]),
        Err(PipelineError::InvalidStage { .. })
    ));
}

#[test]
fn stage_key_enforces_format() {
    for key in ["", "1abc", "Abc", "abc-", "a", &"a".repeat(33)] {
        assert!(
            matches!(
                StageKey::parse(key),
                Err(PipelineError::InvalidStageKey { .. })
            ),
            "key `{key}` must be rejected"
        );
    }
    assert_eq!(
        StageKey::parse("tech_round_1")
            .expect("valid in test")
            .as_str(),
        "tech_round_1"
    );
}

#[test]
fn score_enforces_range() {
    assert_eq!(Score::new(101), Err(PipelineError::ScoreOutOfRange(101)));
    assert_eq!(Score::new(0).expect("valid in test").value(), 0);
    assert_eq!(Score::new(100).expect("valid in test").value(), 100);
}
