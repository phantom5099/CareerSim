//! 流水线校验单测：覆盖 plan/01 §3.2 的校验规则与值对象边界。

use chrono::Utc;

use super::PipelineError;
use super::model::OfferError;
use super::model::OfferPolicy;
use super::model::OfferTermsInput;
use super::model::PipelineDefinition;
use super::model::StageDefinition;
use super::model::StageKey;
use super::model::StageKind;
use super::model::StageOutcome;
use super::model::TransitionError;
use super::model::build_offer_terms;
use super::model::plan_transition;
use super::rubric::DimensionScore;
use super::rubric::EvaluationError;
use super::rubric::EvaluationRubric;
use super::rubric::RubricDimension;
use super::rubric::Score;
use super::rubric::evaluate;

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

fn submitted(scores: &[(&str, u8)]) -> Vec<DimensionScore> {
    scores
        .iter()
        .map(|(key, score)| DimensionScore {
            key: (*key).to_owned(),
            score: Score::new(*score).expect("valid in test"),
            note: "评分依据".to_owned(),
        })
        .collect()
}

#[test]
fn evaluate_computes_weighted_total_and_orders_dimensions() {
    let rule = rubric(&[60, 40]);
    let evaluation = evaluate(
        &rule,
        submitted(&[("d1", 90), ("d0", 70)]),
        "整体不错".to_owned(),
    )
    .expect("valid submission");

    // 0.6 * 70 + 0.4 * 90 = 78。
    assert_eq!(evaluation.total.value(), 78);
    assert!(evaluation.passed);
    // 维度按 rubric 顺序输出，与提交顺序无关。
    assert_eq!(
        evaluation
            .dimensions
            .iter()
            .map(|dimension| dimension.key.as_str())
            .collect::<Vec<_>>(),
        vec!["d0", "d1"]
    );
}

#[test]
fn evaluate_equal_weights_round_half_up() {
    let rule = rubric(&[0, 0]);
    let evaluation = evaluate(
        &rule,
        submitted(&[("d0", 50), ("d1", 61)]),
        "总评".to_owned(),
    )
    .expect("valid submission");

    // (50 + 61) / 2 = 55.5 → 56（四舍五入）。
    assert_eq!(evaluation.total.value(), 56);
    assert!(!evaluation.passed);
}

#[test]
fn evaluate_passes_at_pass_score_boundary() {
    let rule = rubric(&[100]);
    let evaluation =
        evaluate(&rule, submitted(&[("d0", 60)]), "总评".to_owned()).expect("valid submission");
    assert_eq!(evaluation.total.value(), 60);
    assert!(evaluation.passed);
}

#[test]
fn evaluate_rejects_incomplete_or_unknown_dimensions() {
    let rule = rubric(&[50, 50]);

    assert!(matches!(
        evaluate(&rule, submitted(&[("d0", 70)]), "总评".to_owned()),
        Err(EvaluationError::MissingDimension(key)) if key == "d1"
    ));
    assert!(matches!(
        evaluate(
            &rule,
            submitted(&[("d0", 70), ("d1", 70), ("extra", 70)]),
            "总评".to_owned(),
        ),
        Err(EvaluationError::UnknownDimension(key)) if key == "extra"
    ));
    assert!(matches!(
        evaluate(
            &rule,
            submitted(&[("d0", 70), ("d0", 70)]),
            "总评".to_owned(),
        ),
        Err(EvaluationError::DuplicateDimension(key)) if key == "d0"
    ));
}

#[test]
fn evaluate_rejects_overlong_text() {
    let rule = rubric(&[100]);

    assert!(matches!(
        evaluate(&rule, submitted(&[("d0", 70)]), String::new()),
        Err(EvaluationError::InvalidSummary)
    ));
    assert!(matches!(
        evaluate(&rule, submitted(&[("d0", 70)]), "评".repeat(2001)),
        Err(EvaluationError::InvalidSummary)
    ));

    let long_note = DimensionScore {
        key: "d0".to_owned(),
        score: Score::new(70).expect("valid in test"),
        note: "据".repeat(501),
    };
    assert!(matches!(
        evaluate(&rule, vec![long_note], "总评".to_owned()),
        Err(EvaluationError::InvalidNote(key)) if key == "d0"
    ));
}

#[test]
fn plan_transition_advances_when_passed() {
    let pipeline = PipelineDefinition::parse(valid_stages()).expect("valid in test");
    let evaluation = evaluate(&rubric(&[0]), submitted(&[("d0", 80)]), "总评".to_owned())
        .expect("valid submission");

    let transition = plan_transition(
        &pipeline,
        &StageKey::parse("hr").expect("valid in test"),
        evaluation,
        None,
    )
    .expect("advance is decidable");

    assert_eq!(transition.from.as_str(), "hr");
    assert!(transition.evaluation.passed);
    assert_eq!(
        transition.outcome,
        StageOutcome::Advance {
            next_stage_key: StageKey::parse("tech_1").expect("valid in test"),
            next_stage_kind: StageKind::TechInterview,
        }
    );
}

#[test]
fn plan_transition_rejects_when_failed() {
    let pipeline = PipelineDefinition::parse(valid_stages()).expect("valid in test");
    let evaluation = evaluate(&rubric(&[0]), submitted(&[("d0", 30)]), "未达标".to_owned())
        .expect("valid submission");

    let transition = plan_transition(
        &pipeline,
        &StageKey::parse("hr").expect("valid in test"),
        evaluation,
        None,
    )
    .expect("rejection is decidable");

    assert!(matches!(transition.outcome, StageOutcome::Rejected));
}

#[test]
fn plan_transition_requires_offer_terms_at_last_stage() {
    let pipeline = PipelineDefinition::parse(valid_stages()).expect("valid in test");
    let evaluation = evaluate(
        &rubric(&[100]),
        submitted(&[("d0", 90)]),
        "谈薪完成".to_owned(),
    )
    .expect("valid submission");
    let offer_stage = StageKey::parse("offer").expect("valid in test");

    assert!(matches!(
        plan_transition(&pipeline, &offer_stage, evaluation.clone(), None),
        Err(TransitionError::OfferTermsRequired)
    ));

    let offer = build_offer_terms(valid_offer_input(), &offer_policy(), Utc::now())
        .expect("valid offer input");
    let transition = plan_transition(&pipeline, &offer_stage, evaluation, Some(offer))
        .expect("offer pending is decidable");
    assert!(matches!(
        transition.outcome,
        StageOutcome::OfferPending { .. }
    ));
}

#[test]
fn plan_transition_reports_unknown_stage() {
    let pipeline = PipelineDefinition::parse(valid_stages()).expect("valid in test");
    let evaluation = evaluate(&rubric(&[0]), submitted(&[("d0", 80)]), "总评".to_owned())
        .expect("valid submission");

    assert!(matches!(
        plan_transition(
            &pipeline,
            &StageKey::parse("ghost").expect("valid in test"),
            evaluation,
            None,
        ),
        Err(TransitionError::UnknownStage(stage)) if stage == "ghost"
    ));
}

fn offer_policy() -> OfferPolicy {
    OfferPolicy {
        city: "杭州".to_owned(),
        salary_min: Some(10_000),
        salary_max: Some(30_000),
        months: Some(12),
        deadline_days: 7,
    }
}

fn valid_offer_input() -> OfferTermsInput {
    OfferTermsInput {
        base_salary: 20_000,
        months: 12,
        bonus: "年终两个月".to_owned(),
    }
}

#[test]
fn build_offer_terms_applies_policy_fields() {
    let now = Utc::now();
    let terms =
        build_offer_terms(valid_offer_input(), &offer_policy(), now).expect("valid offer input");

    assert_eq!(terms.base_salary, 20_000);
    assert_eq!(terms.months, 12);
    assert_eq!(terms.city, "杭州");
    assert_eq!(terms.issued_at, now);
    assert_eq!((terms.deadline - now.date_naive()).num_days(), 7);
}

#[test]
fn build_offer_terms_enforces_salary_and_months() {
    let policy = offer_policy();
    let now = Utc::now();

    assert!(matches!(
        build_offer_terms(
            OfferTermsInput {
                base_salary: 9_999,
                ..valid_offer_input()
            },
            &policy,
            now,
        ),
        Err(OfferError::SalaryBelowMinimum(9_999, 10_000))
    ));
    assert!(matches!(
        build_offer_terms(
            OfferTermsInput {
                base_salary: 30_001,
                ..valid_offer_input()
            },
            &policy,
            now,
        ),
        Err(OfferError::SalaryAboveMaximum(30_001, 30_000))
    ));
    assert!(matches!(
        build_offer_terms(
            OfferTermsInput {
                months: 11,
                ..valid_offer_input()
            },
            &policy,
            now,
        ),
        Err(OfferError::MonthsOutOfRange(11))
    ));
    assert!(matches!(
        build_offer_terms(
            OfferTermsInput {
                months: 24,
                bonus: String::new(),
                ..valid_offer_input()
            },
            &policy,
            now,
        ),
        Err(OfferError::MonthsMismatch(24, 12))
    ));

    // 薪资上下限为闭区间。
    assert!(
        build_offer_terms(
            OfferTermsInput {
                base_salary: 10_000,
                ..valid_offer_input()
            },
            &policy,
            now,
        )
        .is_ok()
    );
    assert!(
        build_offer_terms(
            OfferTermsInput {
                base_salary: 30_000,
                ..valid_offer_input()
            },
            &policy,
            now,
        )
        .is_ok()
    );
}

#[test]
fn build_offer_terms_limits_bonus_length() {
    let policy = OfferPolicy {
        city: "杭州".to_owned(),
        salary_min: None,
        salary_max: None,
        months: None,
        deadline_days: 7,
    };

    assert!(
        build_offer_terms(
            OfferTermsInput {
                base_salary: 20_000,
                months: 12,
                bonus: "奖".repeat(120),
            },
            &policy,
            Utc::now(),
        )
        .is_ok()
    );
    assert!(matches!(
        build_offer_terms(
            OfferTermsInput {
                base_salary: 20_000,
                months: 12,
                bonus: "奖".repeat(121),
            },
            &policy,
            Utc::now(),
        ),
        Err(OfferError::BonusTooLong)
    ));
}
