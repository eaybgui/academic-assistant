//! Builds the state and [`QuestionSet`] for one [`AnalysisInput`] (spec §6.1, §6.2).

use indexmap::IndexMap;

use crate::domain::Course;
use crate::ports::{Question, QuestionSet};

use super::{AnalysisInput, MAX_COURSE_OPTIONS};

/// Cap on the number of `date_i` questions asked, independent of `core::dates`' own
/// `MAX_CANDIDATES` (also 20): the analyzer does not call into `core::dates` (see the
/// module doc on `analyzer::mod`), so it does not depend on that module's completion.
const MAX_DATE_QUESTIONS: usize = 20;

const RELEVANT_INSTRUCTIONS: &str = "This email is an academic communication relevant to a university student: courses, classes, exams, assignments, grades, schedules, tutoring or academic administrative procedures.";
const ACTION_REQUIRED_INSTRUCTIONS: &str = "The email asks the student to do something: submit, register, reply, attend, prepare, pay or bring something.";
const POSSIBLE_CHANGE_INSTRUCTIONS: &str = "The email changes information communicated before: a new date or time, a new room, a cancellation or new instructions.";
const CATEGORY_INSTRUCTIONS: &str =
    "Classify this email into exactly one of the following categories.";
const PRIORITY_INSTRUCTIONS: &str = "Estimate how urgently the student needs to act on this email: low (informational; no action or dates are far away), medium (action or relevant date within the next weeks), high (action, exam or deadline within the next few days) or urgent (immediate action needed or something happens within 48 hours).";
const UNIVERSITY_INSTRUCTIONS: &str = "Which university is this email about?";
const COURSE_INSTRUCTIONS: &str = "Which of the following courses is this email about, if any?";

const PRIORITY_LEVELS: [&str; 4] = ["low", "medium", "high", "urgent"];

/// Builds the state passed to [`crate::ports::StructuredModel::ask`] (spec §6.1).
pub(super) fn build_state(input: &AnalysisInput) -> serde_json::Value {
    let email = serde_json::json!({
        "subject": input.email.subject,
        "from": {
            "name": input.email.sender.name,
            "email": input.email.sender.email,
        },
        "received_at": input.email.received_at.to_rfc3339(),
        "body": input.normalized_body,
    });

    let mut context = serde_json::Map::new();
    if let Some(name) = default_university_name(input) {
        context.insert(
            "account_university".to_string(),
            serde_json::Value::String(name.to_string()),
        );
    }

    serde_json::json!({ "email": email, "context": context })
}

/// Builds the question set for `input` (spec §6.2). Called both by `build_request`
/// (before asking the model) and by `assemble` (to validate/replay the same set of
/// questions the model was asked).
pub(super) fn build_questions(input: &AnalysisInput) -> QuestionSet {
    let mut questions = QuestionSet::new();

    questions.insert(
        "relevant".to_string(),
        Question::Noul {
            instructions: RELEVANT_INSTRUCTIONS.to_string(),
        },
    );
    questions.insert(
        "category".to_string(),
        Question::Choice {
            instructions: CATEGORY_INSTRUCTIONS.to_string(),
            options: category_options(),
        },
    );
    questions.insert(
        "action_required".to_string(),
        Question::Noul {
            instructions: ACTION_REQUIRED_INSTRUCTIONS.to_string(),
        },
    );
    questions.insert(
        "possible_change".to_string(),
        Question::Noul {
            instructions: POSSIBLE_CHANGE_INSTRUCTIONS.to_string(),
        },
    );
    questions.insert(
        "priority".to_string(),
        Question::Score {
            instructions: PRIORITY_INSTRUCTIONS.to_string(),
            levels: PRIORITY_LEVELS.iter().map(|s| s.to_string()).collect(),
        },
    );

    if !has_default_university(input) && !input.catalog.universities.is_empty() {
        questions.insert(
            "university".to_string(),
            Question::Choice {
                instructions: UNIVERSITY_INSTRUCTIONS.to_string(),
                options: university_options(input),
            },
        );
    }

    let courses = candidate_courses(input);
    if !courses.is_empty() {
        questions.insert(
            "course".to_string(),
            Question::Choice {
                instructions: COURSE_INSTRUCTIONS.to_string(),
                options: course_options(&courses, input),
            },
        );
    }

    for (i, candidate) in input
        .date_candidates
        .iter()
        .take(MAX_DATE_QUESTIONS)
        .enumerate()
    {
        questions.insert(
            format!("date_{i}"),
            Question::Choice {
                instructions: format!(
                    "In this email, what does the date written as «{}» refer to?",
                    candidate.span
                ),
                options: date_options(),
            },
        );
    }

    questions
}

fn has_default_university(input: &AnalysisInput) -> bool {
    input.account.default_university_id.is_some()
}

fn default_university_name<'a>(input: &'a AnalysisInput) -> Option<&'a str> {
    let id = input.account.default_university_id?;
    input
        .catalog
        .universities
        .iter()
        .find(|u| u.id == id)
        .map(|u| u.name.as_str())
}

/// The default university's courses, or every catalog course when there is none
/// (spec §6.2: "the default university's courses, or all courses when there is none").
fn candidate_courses<'a>(input: &'a AnalysisInput) -> Vec<&'a Course> {
    match input.account.default_university_id {
        Some(university_id) => input
            .catalog
            .courses
            .iter()
            .filter(|c| c.university_id == university_id)
            .collect(),
        None => input.catalog.courses.iter().collect(),
    }
}

fn university_options(input: &AnalysisInput) -> IndexMap<String, String> {
    let mut options = IndexMap::new();
    for university in &input.catalog.universities {
        options.insert(format!("u{}", university.id), university.name.clone());
    }
    options.insert(
        "unknown".to_string(),
        "None of these universities".to_string(),
    );
    options
}

fn course_options(courses: &[&Course], input: &AnalysisInput) -> IndexMap<String, String> {
    let mut options = IndexMap::new();
    for course in courses.iter().take(MAX_COURSE_OPTIONS) {
        let university_name = input
            .catalog
            .universities
            .iter()
            .find(|u| u.id == course.university_id)
            .map(|u| u.name.as_str());
        let mut description = match university_name {
            Some(name) => format!("{} ({})", course.name, name),
            None => course.name.clone(),
        };
        if !course.aliases.is_empty() {
            description.push_str(" — also known as: ");
            description.push_str(&course.aliases.join(", "));
        }
        options.insert(format!("c{}", course.id), description);
    }
    options.insert(
        "none".to_string(),
        "The email is not about any of these courses".to_string(),
    );
    options
}

fn category_options() -> IndexMap<String, String> {
    IndexMap::from([
        (
            "notice".to_string(),
            "general announcement or reminder for students".to_string(),
        ),
        (
            "task".to_string(),
            "assignment, submission, exercise or practical work to complete".to_string(),
        ),
        (
            "exam".to_string(),
            "exam, test or assessment information".to_string(),
        ),
        (
            "event".to_string(),
            "talk, seminar, session or activity at a specific time".to_string(),
        ),
        (
            "change".to_string(),
            "modifies or cancels previously communicated information".to_string(),
        ),
        (
            "general_info".to_string(),
            "administrative or informational content with no specific action".to_string(),
        ),
    ])
}

fn date_options() -> IndexMap<String, String> {
    IndexMap::from([
        (
            "exam".to_string(),
            "An exam, test or assessment".to_string(),
        ),
        (
            "deadline".to_string(),
            "A submission or registration deadline".to_string(),
        ),
        (
            "event".to_string(),
            "A talk, seminar, meeting or activity".to_string(),
        ),
        (
            "class".to_string(),
            "A regular class or lab session".to_string(),
        ),
        ("other".to_string(), "Another academic date".to_string()),
        (
            "not_relevant".to_string(),
            "Not relevant for the student (e.g. the date the email was sent)".to_string(),
        ),
    ])
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use crate::analyzer::test_support;

    use super::*;

    #[test]
    fn state_has_empty_context_without_default_university() {
        let email = test_support::email();
        let account = test_support::account(None);
        let catalog = test_support::catalog(vec![], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "Normalized body",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };

        let state = build_state(&input);

        assert_eq!(state["email"]["subject"], "Reminder: exam next week");
        assert_eq!(state["email"]["from"]["name"], "Prof. Ada Lovelace");
        assert_eq!(state["email"]["from"]["email"], "ada@example.edu");
        assert_eq!(state["email"]["body"], "Normalized body");
        assert_eq!(
            state["email"]["received_at"],
            email.received_at.to_rfc3339()
        );
        assert_eq!(state["context"], serde_json::json!({}));
    }

    #[test]
    fn state_includes_account_university_when_account_has_a_default() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(
            vec![test_support::university(1, "Universidad de Alcalá")],
            vec![],
        );
        let input = AnalysisInput {
            email: &email,
            normalized_body: "body",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };

        let state = build_state(&input);

        assert_eq!(
            state["context"]["account_university"],
            "Universidad de Alcalá"
        );
    }

    #[test]
    fn mandatory_questions_are_always_present() {
        let email = test_support::email();
        let account = test_support::account(None);
        let catalog = test_support::catalog(vec![], vec![]);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "body",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };

        let questions = build_questions(&input);

        for id in [
            "relevant",
            "category",
            "action_required",
            "possible_change",
            "priority",
        ] {
            assert!(questions.contains_key(id), "missing question '{id}'");
        }
    }

    #[test]
    fn university_question_present_only_without_default_and_nonempty_catalog() {
        let email = test_support::email();
        let catalog = test_support::catalog(vec![test_support::university(1, "UAH")], vec![]);
        let empty_catalog = test_support::catalog(vec![], vec![]);

        let no_default = test_support::account(None);
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &no_default,
            catalog: &catalog,
        };
        assert!(build_questions(&input).contains_key("university"));

        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &no_default,
            catalog: &empty_catalog,
        };
        assert!(!build_questions(&input).contains_key("university"));

        let with_default = test_support::account(Some(1));
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &with_default,
            catalog: &catalog,
        };
        assert!(!build_questions(&input).contains_key("university"));
    }

    #[test]
    fn course_options_are_scoped_to_the_default_university() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(
            vec![
                test_support::university(1, "UAH"),
                test_support::university(2, "UCM"),
            ],
            vec![
                test_support::course(10, 1, "Algorithms", vec![]),
                test_support::course(20, 2, "Databases", vec![]),
            ],
        );
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };

        let questions = build_questions(&input);
        let Question::Choice { options, .. } = &questions["course"] else {
            panic!("expected a Choice question");
        };

        assert!(options.contains_key("c10"));
        assert!(!options.contains_key("c20"));
        assert!(options.contains_key("none"));
    }

    #[test]
    fn course_description_includes_aliases_when_present() {
        let email = test_support::email();
        let account = test_support::account(None);
        let catalog = test_support::catalog(
            vec![test_support::university(1, "UAH")],
            vec![test_support::course(
                10,
                1,
                "Algorithms",
                vec!["ALG", "Algo I"],
            )],
        );
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };

        let questions = build_questions(&input);
        let Question::Choice { options, .. } = &questions["course"] else {
            panic!("expected a Choice question");
        };

        assert_eq!(
            options["c10"],
            "Algorithms (UAH) — also known as: ALG, Algo I"
        );
    }

    #[test]
    fn no_course_question_when_there_are_no_candidate_courses() {
        let email = test_support::email();
        let account = test_support::account(Some(1));
        let catalog = test_support::catalog(
            vec![
                test_support::university(1, "UAH"),
                test_support::university(2, "UCM"),
            ],
            vec![test_support::course(20, 2, "Databases", vec![])],
        );
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &[],
            account: &account,
            catalog: &catalog,
        };

        assert!(!build_questions(&input).contains_key("course"));
    }

    #[test]
    fn one_date_question_per_candidate() {
        let email = test_support::email();
        let account = test_support::account(None);
        let catalog = test_support::catalog(vec![], vec![]);
        let candidates = [
            test_support::date_candidate(
                "27 de octubre",
                NaiveDate::from_ymd_opt(2026, 10, 27).unwrap(),
            ),
            test_support::date_candidate("3/11", NaiveDate::from_ymd_opt(2026, 11, 3).unwrap()),
        ];
        let input = AnalysisInput {
            email: &email,
            normalized_body: "b",
            date_candidates: &candidates,
            account: &account,
            catalog: &catalog,
        };

        let questions = build_questions(&input);

        let Question::Choice { instructions, .. } = &questions["date_0"] else {
            panic!("expected a Choice question");
        };
        assert_eq!(
            instructions,
            "In this email, what does the date written as «27 de octubre» refer to?"
        );
        assert!(questions.contains_key("date_1"));
        assert!(!questions.contains_key("date_2"));
    }
}
