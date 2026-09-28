//! Validates a model's [`AnswerSet`] against the question set built for an
//! [`AnalysisInput`] and assembles the result into an [`AnalysisOutcome`] (spec §6.4).

use crate::domain::{
    AIAnalysis, AnalysisOutcome, Category, DateKind, ModelDescriptor, Priority, ResultDate,
};
use crate::ports::{Answer, AnswerSet, ModelError, Question, QuestionSet};

use super::questions::build_questions;
use super::{AnalysisInput, FLAG_THRESHOLD, RELEVANCE_THRESHOLD};

const PRIORITY_ORDER: [Priority; 4] = [
    Priority::Low,
    Priority::Medium,
    Priority::High,
    Priority::Urgent,
];

/// Rebuilds the question set for `input`, validates `answers` against it and assembles
/// the [`AnalysisOutcome`]. Independent of `analyze`: it can validate/assemble answers
/// obtained through any channel, as long as `input` is the one the questions were built
/// from and `model` identifies the model that produced `answers`.
pub fn assemble(
    input: &AnalysisInput,
    answers: &AnswerSet,
    model: ModelDescriptor,
) -> Result<AnalysisOutcome, ModelError> {
    let questions = build_questions(input);
    validate(&questions, answers)?;

    let p_relevant = answer_p_yes(answers, "relevant");
    if p_relevant < RELEVANCE_THRESHOLD {
        return Ok(AnalysisOutcome::Irrelevant { p_relevant });
    }

    let category: Category = answer_choice(answers, "category")
        .parse()
        .expect("category choice was validated against category_options' keys");
    let priority = PRIORITY_ORDER[answer_level(answers, "priority") as usize];
    let action_required = answer_p_yes(answers, "action_required") >= FLAG_THRESHOLD;
    let possible_change = answer_p_yes(answers, "possible_change") >= FLAG_THRESHOLD;

    let course_id = if questions.contains_key("course") {
        parse_course_choice(answer_choice(answers, "course"))
    } else {
        None
    };
    let university_id = resolve_university_id(input, answers, &questions, course_id);
    let dates = collect_dates(input, answers, &questions);
    let confidence = min_confidence(answers, &questions);

    Ok(AnalysisOutcome::Relevant(AIAnalysis {
        university_id,
        course_id,
        category,
        priority,
        action_required,
        possible_change,
        dates,
        confidence,
        model,
    }))
}

/// Every question in `questions` must have an answer in `answers` of the matching type,
/// with a valid option key (`Choice`) or in-range level (`Score`). Answers in `answers`
/// for ids not in `questions` are ignored.
fn validate(questions: &QuestionSet, answers: &AnswerSet) -> Result<(), ModelError> {
    for (id, question) in questions {
        let answer = answers.get(id).ok_or_else(|| {
            ModelError::InvalidResponse(format!("missing answer for question '{id}'"))
        })?;
        match (question, answer) {
            (Question::Noul { .. }, Answer::Noul { .. }) => {}
            (Question::Choice { options, .. }, Answer::Choice { choice, .. }) => {
                if !options.contains_key(choice) {
                    return Err(ModelError::InvalidResponse(format!(
                        "question '{id}' answered with unknown option '{choice}'"
                    )));
                }
            }
            (Question::Score { levels, .. }, Answer::Score { level, .. }) => {
                if usize::from(*level) >= levels.len() {
                    return Err(ModelError::InvalidResponse(format!(
                        "question '{id}' answered with out-of-range level {level}"
                    )));
                }
            }
            _ => {
                return Err(ModelError::InvalidResponse(format!(
                    "question '{id}' answered with the wrong answer type"
                )));
            }
        }
    }
    Ok(())
}

// The following accessors assume `validate` already confirmed `id`'s answer has the
// matching variant (and, for Choice/Score, a valid option/level); they are only called
// for ids taken from `questions` after a successful `validate`.

fn answer_p_yes(answers: &AnswerSet, id: &str) -> f32 {
    match answers.get(id) {
        Some(Answer::Noul { p_yes, .. }) => *p_yes,
        _ => unreachable!("'{id}' was validated as a Noul answer"),
    }
}

fn answer_choice<'a>(answers: &'a AnswerSet, id: &str) -> &'a str {
    match answers.get(id) {
        Some(Answer::Choice { choice, .. }) => choice,
        _ => unreachable!("'{id}' was validated as a Choice answer"),
    }
}

fn answer_level(answers: &AnswerSet, id: &str) -> u8 {
    match answers.get(id) {
        Some(Answer::Score { level, .. }) => *level,
        _ => unreachable!("'{id}' was validated as a Score answer"),
    }
}

fn answer_confidence(answer: &Answer) -> Option<f32> {
    match answer {
        Answer::Noul { confidence, .. }
        | Answer::Choice { confidence, .. }
        | Answer::Score { confidence, .. } => *confidence,
    }
}

fn parse_university_choice(choice: &str) -> Option<i64> {
    if choice == "unknown" {
        None
    } else {
        Some(
            choice
                .strip_prefix('u')
                .and_then(|id| id.parse::<i64>().ok())
                .expect("university choice keys are generated as 'u{id}' or 'unknown'"),
        )
    }
}

fn parse_course_choice(choice: &str) -> Option<i64> {
    if choice == "none" {
        None
    } else {
        Some(
            choice
                .strip_prefix('c')
                .and_then(|id| id.parse::<i64>().ok())
                .expect("course choice keys are generated as 'c{id}' or 'none'"),
        )
    }
}

/// `university_id`: account default -> else the `university` choice (`unknown` -> None)
/// -> else the chosen course's university (spec §6.4).
fn resolve_university_id(
    input: &AnalysisInput,
    answers: &AnswerSet,
    questions: &QuestionSet,
    course_id: Option<i64>,
) -> Option<i64> {
    if let Some(id) = input.account.default_university_id {
        return Some(id);
    }
    if questions.contains_key("university") {
        return parse_university_choice(answer_choice(answers, "university"));
    }
    course_id.and_then(|id| {
        input
            .catalog
            .courses
            .iter()
            .find(|c| c.id == id)
            .map(|c| c.university_id)
    })
}

/// Candidates whose answer is `not_relevant` are dropped; the rest map to a
/// [`ResultDate`] with the candidate's date/time and the answered [`DateKind`].
fn collect_dates(
    input: &AnalysisInput,
    answers: &AnswerSet,
    questions: &QuestionSet,
) -> Vec<ResultDate> {
    input
        .date_candidates
        .iter()
        .enumerate()
        .filter_map(|(i, candidate)| {
            let id = format!("date_{i}");
            if !questions.contains_key(&id) {
                return None;
            }
            let choice = answer_choice(answers, &id);
            if choice == "not_relevant" {
                return None;
            }
            let kind: DateKind = choice
                .parse()
                .expect("date choice was validated against date_options' keys");
            Some(ResultDate {
                date: candidate.date,
                time: candidate.time,
                kind,
            })
        })
        .collect()
}

/// Minimum confidence across every answer to an asked question; `None` when none of
/// them carry a confidence.
fn min_confidence(answers: &AnswerSet, questions: &QuestionSet) -> Option<f32> {
    questions
        .keys()
        .filter_map(|id| answers.get(id))
        .filter_map(answer_confidence)
        .reduce(f32::min)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use chrono::NaiveDate;

    use crate::analyzer::test_support;

    use super::*;

    fn model_descriptor() -> ModelDescriptor {
        ModelDescriptor {
            provider: "fake".to_string(),
            model: "fake-1".to_string(),
        }
    }

    fn noul(p_yes: f32) -> Answer {
        Answer::Noul {
            p_yes,
            confidence: Some(0.9),
        }
    }

    fn choice(value: &str) -> Answer {
        Answer::Choice {
            choice: value.to_string(),
            confidence: Some(0.9),
        }
    }

    fn score(level: u8) -> Answer {
        Answer::Score {
            level,
            confidence: Some(0.9),
        }
    }

    /// Answers to the five mandatory questions only; valid for an input whose account
    /// has a default university and no date candidates or candidate courses.
    fn base_answers() -> AnswerSet {
        HashMap::from([
            ("relevant".to_string(), noul(0.9)),
            ("category".to_string(), choice("exam")),
            ("action_required".to_string(), noul(0.8)),
            ("possible_change".to_string(), noul(0.1)),
            ("priority".to_string(), score(2)),
        ])
    }

    fn relevant_analysis(outcome: AnalysisOutcome) -> AIAnalysis {
        match outcome {
            AnalysisOutcome::Relevant(analysis) => analysis,
            AnalysisOutcome::Irrelevant { p_relevant } => {
                panic!("expected Relevant, got Irrelevant {{ p_relevant: {p_relevant} }}")
            }
        }
    }

    #[test]
    fn below_relevance_threshold_is_irrelevant() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert("relevant".to_string(), noul(0.39));

        let outcome = assemble(&input, &answers, model_descriptor()).unwrap();

        assert_eq!(outcome, AnalysisOutcome::Irrelevant { p_relevant: 0.39 });
    }

    #[test]
    fn at_relevance_threshold_is_relevant() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert("relevant".to_string(), noul(0.4));

        let outcome = assemble(&input, &answers, model_descriptor()).unwrap();

        assert!(matches!(outcome, AnalysisOutcome::Relevant(_)));
    }

    #[test]
    fn action_required_and_possible_change_flag_at_threshold() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert("action_required".to_string(), noul(0.5));
        answers.insert("possible_change".to_string(), noul(0.5));

        let analysis = relevant_analysis(assemble(&input, &answers, model_descriptor()).unwrap());

        assert!(analysis.action_required);
        assert!(analysis.possible_change);
    }

    #[test]
    fn university_id_uses_account_default_when_set() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };

        let analysis =
            relevant_analysis(assemble(&input, &base_answers(), model_descriptor()).unwrap());

        assert_eq!(analysis.university_id, Some(1));
    }

    #[test]
    fn university_id_uses_choice_when_no_default() {
        let email = test_support::email();
        let account = test_support::account(None);
        let catalog = test_support::catalog(
            vec![
                test_support::university(1, "UAH"),
                test_support::university(2, "UCM"),
            ],
            vec![],
        );
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert("university".to_string(), choice("u2"));

        let analysis = relevant_analysis(assemble(&input, &answers, model_descriptor()).unwrap());

        assert_eq!(analysis.university_id, Some(2));
    }

    #[test]
    fn university_id_falls_back_to_course_university_when_catalog_has_no_universities() {
        let email = test_support::email();
        let account = test_support::account(None);
        let catalog = test_support::catalog(
            vec![],
            vec![test_support::course(10, 5, "Algorithms", vec![])],
        );
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert("course".to_string(), choice("c10"));

        let analysis = relevant_analysis(assemble(&input, &answers, model_descriptor()).unwrap());

        assert_eq!(analysis.university_id, Some(5));
        assert_eq!(analysis.course_id, Some(10));
    }

    #[test]
    fn unknown_university_and_none_course_resolve_to_none() {
        let email = test_support::email();
        let account = test_support::account(None);
        let catalog = test_support::catalog(
            vec![test_support::university(1, "UAH")],
            vec![test_support::course(10, 1, "Algorithms", vec![])],
        );
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert("university".to_string(), choice("unknown"));
        answers.insert("course".to_string(), choice("none"));

        let analysis = relevant_analysis(assemble(&input, &answers, model_descriptor()).unwrap());

        assert_eq!(analysis.university_id, None);
        assert_eq!(analysis.course_id, None);
    }

    #[test]
    fn not_relevant_dates_are_dropped_and_others_map_to_their_kind() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let candidates = [
            test_support::date_candidate("27/10", NaiveDate::from_ymd_opt(2026, 10, 27).unwrap()),
            test_support::date_candidate("hoy", NaiveDate::from_ymd_opt(2026, 9, 28).unwrap()),
        ];
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &candidates,
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert("date_0".to_string(), choice("exam"));
        answers.insert("date_1".to_string(), choice("not_relevant"));

        let analysis = relevant_analysis(assemble(&input, &answers, model_descriptor()).unwrap());

        assert_eq!(analysis.dates.len(), 1);
        assert_eq!(analysis.dates[0].kind, DateKind::Exam);
        assert_eq!(analysis.dates[0].date, candidates[0].date);
    }

    #[test]
    fn confidence_is_the_minimum_of_all_present_answer_confidences() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert(
            "priority".to_string(),
            Answer::Score {
                level: 2,
                confidence: Some(0.3),
            },
        );

        let analysis = relevant_analysis(assemble(&input, &answers, model_descriptor()).unwrap());

        assert_eq!(analysis.confidence, Some(0.3));
    }

    #[test]
    fn confidence_is_none_when_no_answer_carries_one() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let answers: AnswerSet = HashMap::from([
            (
                "relevant".to_string(),
                Answer::Noul {
                    p_yes: 0.9,
                    confidence: None,
                },
            ),
            (
                "category".to_string(),
                Answer::Choice {
                    choice: "exam".to_string(),
                    confidence: None,
                },
            ),
            (
                "action_required".to_string(),
                Answer::Noul {
                    p_yes: 0.1,
                    confidence: None,
                },
            ),
            (
                "possible_change".to_string(),
                Answer::Noul {
                    p_yes: 0.1,
                    confidence: None,
                },
            ),
            (
                "priority".to_string(),
                Answer::Score {
                    level: 0,
                    confidence: None,
                },
            ),
        ]);

        let analysis = relevant_analysis(assemble(&input, &answers, model_descriptor()).unwrap());

        assert_eq!(analysis.confidence, None);
    }

    #[test]
    fn missing_answer_is_invalid_response() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.remove("priority");

        let err = assemble(&input, &answers, model_descriptor()).unwrap_err();

        assert!(matches!(err, ModelError::InvalidResponse(_)));
    }

    #[test]
    fn wrong_answer_type_is_invalid_response() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert("relevant".to_string(), choice("yes"));

        let err = assemble(&input, &answers, model_descriptor()).unwrap_err();

        assert!(matches!(err, ModelError::InvalidResponse(_)));
    }

    #[test]
    fn unknown_choice_option_is_invalid_response() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert("category".to_string(), choice("bogus"));

        let err = assemble(&input, &answers, model_descriptor()).unwrap_err();

        assert!(matches!(err, ModelError::InvalidResponse(_)));
    }

    #[test]
    fn out_of_range_score_level_is_invalid_response() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };
        let mut answers = base_answers();
        answers.insert("priority".to_string(), score(4));

        let err = assemble(&input, &answers, model_descriptor()).unwrap_err();

        assert!(matches!(err, ModelError::InvalidResponse(_)));
    }
}
