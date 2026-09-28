//! `ResultRepository` behavior: `save_analyzed`, querying (filters, search escaping,
//! sorting, pagination) and the seen/unseen workflow.

#[path = "support/mod.rs"]
mod support;

use academic_core::domain::{Category, DateKind, Priority, ResultDate, ResultQuery, ResultSort};
use academic_core::ports::{
    CatalogRepository, ProcessedMessageRepository, RepoError, ResultRepository,
};
use academic_infra::db::Database;
use chrono::{DateTime, Duration, NaiveDate, NaiveTime, TimeZone, Utc};
use support::{analyzed_message, insert_account, insert_run, new_result};

fn dt(y: i32, m: u32, d: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(y, m, d, 9, 0, 0).unwrap()
}

fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 9, 28).unwrap()
}

#[allow(clippy::too_many_arguments)]
fn seed(
    db: &Database,
    account_id: i64,
    run_id: i64,
    uid: &str,
    subject: &str,
    received_at: DateTime<Utc>,
    category: Category,
    priority: Priority,
    dates: Vec<ResultDate>,
) -> i64 {
    let message = analyzed_message(account_id, run_id, uid, received_at);
    let result = new_result(
        account_id,
        run_id,
        subject,
        received_at,
        category,
        priority,
        dates,
    );
    db.save_analyzed(&message, &result).unwrap()
}

fn base_query() -> ResultQuery {
    ResultQuery {
        search: None,
        seen: None,
        category: None,
        account_id: None,
        course_id: None,
        sort: ResultSort::Received,
        limit: 100,
        offset: 0,
    }
}

#[test]
fn save_analyzed_persists_processed_message_result_and_dates() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let received_at = dt(2026, 9, 20);
    let dates = vec![ResultDate {
        date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
        time: Some(NaiveTime::from_hms_opt(10, 0, 0).unwrap()),
        kind: DateKind::Exam,
    }];

    let message = analyzed_message(account.id, run_id, "uid-1", received_at);
    let result = new_result(
        account.id,
        run_id,
        "Exam date",
        received_at,
        Category::Exam,
        Priority::High,
        dates.clone(),
    );
    let result_id = db.save_analyzed(&message, &result).unwrap();

    assert!(db.exists(account.id, "INBOX", "uid-1").unwrap());
    let stored = db.get(result_id).unwrap().unwrap();
    assert_eq!(stored.subject, "Exam date");
    assert_eq!(stored.category, Category::Exam);
    assert_eq!(stored.priority, Priority::High);
    assert_eq!(stored.dates, dates);
    assert_eq!(stored.account_name, account.name);
    assert!(!stored.seen);
    assert_eq!(stored.seen_at, None);
}

#[test]
fn save_analyzed_conflicts_on_duplicate_provider_message_id() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let received_at = dt(2026, 9, 20);
    seed(
        &db,
        account.id,
        run_id,
        "uid-1",
        "First",
        received_at,
        Category::Notice,
        Priority::Low,
        vec![],
    );

    let message = analyzed_message(account.id, run_id, "uid-1", received_at);
    let result = new_result(
        account.id,
        run_id,
        "Second",
        received_at,
        Category::Notice,
        Priority::Low,
        vec![],
    );
    let err = db.save_analyzed(&message, &result).unwrap_err();
    assert!(matches!(err, RepoError::Conflict(_)));
}

#[test]
fn get_missing_result_returns_none() {
    let db = Database::open_in_memory().unwrap();
    assert_eq!(db.get(999).unwrap(), None);
}

#[test]
fn query_filters_by_search_case_insensitive_substring() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    seed(
        &db,
        account.id,
        run_id,
        "1",
        "Exam schedule change",
        dt(2026, 9, 20),
        Category::Change,
        Priority::Medium,
        vec![],
    );
    seed(
        &db,
        account.id,
        run_id,
        "2",
        "Welcome newsletter",
        dt(2026, 9, 21),
        Category::Notice,
        Priority::Low,
        vec![],
    );

    // "exam schedule" rather than just "exam": the fixture's default sender email
    // (sender@example.test) itself contains "exam" (from "example"), so a bare
    // "exam" search would also match the second row through the sender_email clause.
    let query = ResultQuery {
        search: Some("exam schedule".to_string()),
        ..base_query()
    };
    let page = db.query(&query, today()).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].subject, "Exam schedule change");
}

#[test]
fn query_search_escapes_percent_and_underscore_wildcards() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    seed(
        &db,
        account.id,
        run_id,
        "1",
        "100% discount",
        dt(2026, 9, 20),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    seed(
        &db,
        account.id,
        run_id,
        "2",
        "100X discount",
        dt(2026, 9, 21),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    seed(
        &db,
        account.id,
        run_id,
        "3",
        "price_tag update",
        dt(2026, 9, 22),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    seed(
        &db,
        account.id,
        run_id,
        "4",
        "priceXtag update",
        dt(2026, 9, 23),
        Category::Notice,
        Priority::Low,
        vec![],
    );

    let percent_query = ResultQuery {
        search: Some("100%".to_string()),
        ..base_query()
    };
    let percent_page = db.query(&percent_query, today()).unwrap();
    assert_eq!(percent_page.items.len(), 1);
    assert_eq!(percent_page.items[0].subject, "100% discount");

    let underscore_query = ResultQuery {
        search: Some("price_tag".to_string()),
        ..base_query()
    };
    let underscore_page = db.query(&underscore_query, today()).unwrap();
    assert_eq!(underscore_page.items.len(), 1);
    assert_eq!(underscore_page.items[0].subject, "price_tag update");
}

#[test]
fn query_filters_by_seen() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let unseen_id = seed(
        &db,
        account.id,
        run_id,
        "1",
        "Unseen",
        dt(2026, 9, 20),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    let seen_id = seed(
        &db,
        account.id,
        run_id,
        "2",
        "Seen",
        dt(2026, 9, 21),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    db.set_seen(seen_id, true, Utc::now()).unwrap();

    let unseen_query = ResultQuery {
        seen: Some(false),
        ..base_query()
    };
    let unseen_page = db.query(&unseen_query, today()).unwrap();
    assert_eq!(
        unseen_page.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![unseen_id]
    );

    let seen_query = ResultQuery {
        seen: Some(true),
        ..base_query()
    };
    let seen_page = db.query(&seen_query, today()).unwrap();
    assert_eq!(
        seen_page.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![seen_id]
    );
}

#[test]
fn query_filters_by_category() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let exam_id = seed(
        &db,
        account.id,
        run_id,
        "1",
        "Exam",
        dt(2026, 9, 20),
        Category::Exam,
        Priority::Medium,
        vec![],
    );
    seed(
        &db,
        account.id,
        run_id,
        "2",
        "Task",
        dt(2026, 9, 21),
        Category::Task,
        Priority::Medium,
        vec![],
    );

    let query = ResultQuery {
        category: Some(Category::Exam),
        ..base_query()
    };
    let page = db.query(&query, today()).unwrap();
    assert_eq!(
        page.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![exam_id]
    );
}

#[test]
fn query_filters_by_account_id() {
    let db = Database::open_in_memory().unwrap();
    let account_a = insert_account(&db, "a");
    let account_b = insert_account(&db, "b");
    let run_id = insert_run(&db);
    let id_a = seed(
        &db,
        account_a.id,
        run_id,
        "1",
        "A result",
        dt(2026, 9, 20),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    seed(
        &db,
        account_b.id,
        run_id,
        "2",
        "B result",
        dt(2026, 9, 21),
        Category::Notice,
        Priority::Low,
        vec![],
    );

    let query = ResultQuery {
        account_id: Some(account_a.id),
        ..base_query()
    };
    let page = db.query(&query, today()).unwrap();
    assert_eq!(
        page.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![id_a]
    );
}

#[test]
fn query_filters_by_course_id() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let uni = db.add_university("UV", Utc::now()).unwrap();
    let course = db
        .add_course(uni.id, "Programacion", &[], Utc::now())
        .unwrap();

    let received_at = dt(2026, 9, 20);
    let message = analyzed_message(account.id, run_id, "with-course", received_at);
    let mut result = new_result(
        account.id,
        run_id,
        "With course",
        received_at,
        Category::Notice,
        Priority::Low,
        vec![],
    );
    result.analysis.course_id = Some(course.id);
    let with_course_id = db.save_analyzed(&message, &result).unwrap();

    seed(
        &db,
        account.id,
        run_id,
        "without-course",
        "Without course",
        dt(2026, 9, 21),
        Category::Notice,
        Priority::Low,
        vec![],
    );

    let query = ResultQuery {
        course_id: Some(course.id),
        ..base_query()
    };
    let page = db.query(&query, today()).unwrap();
    assert_eq!(
        page.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![with_course_id]
    );
    assert_eq!(page.items[0].course_name.as_deref(), Some("Programacion"));
}

#[test]
fn upcoming_sort_orders_by_nearest_future_date_then_recent_received() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);

    let r_a = seed(
        &db,
        account.id,
        run_id,
        "a",
        "A",
        dt(2026, 9, 20),
        Category::Exam,
        Priority::Medium,
        vec![ResultDate {
            date: NaiveDate::from_ymd_opt(2026, 10, 5).unwrap(),
            time: None,
            kind: DateKind::Exam,
        }],
    );
    let r_b = seed(
        &db,
        account.id,
        run_id,
        "b",
        "B",
        dt(2026, 9, 21),
        Category::Exam,
        Priority::Medium,
        vec![ResultDate {
            date: NaiveDate::from_ymd_opt(2026, 9, 29).unwrap(),
            time: None,
            kind: DateKind::Exam,
        }],
    );
    let r_c = seed(
        &db,
        account.id,
        run_id,
        "c",
        "C",
        dt(2026, 9, 23),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    let r_d = seed(
        &db,
        account.id,
        run_id,
        "d",
        "D",
        dt(2026, 9, 22),
        Category::Exam,
        Priority::Medium,
        vec![ResultDate {
            date: NaiveDate::from_ymd_opt(2026, 9, 1).unwrap(),
            time: None,
            kind: DateKind::Exam,
        }],
    );

    let query = ResultQuery {
        sort: ResultSort::Upcoming,
        ..base_query()
    };
    let page = db.query(&query, today()).unwrap();

    assert_eq!(
        page.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![r_b, r_a, r_c, r_d]
    );
}

#[test]
fn priority_sort_orders_urgent_to_low_then_recent_received() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);

    let low = seed(
        &db,
        account.id,
        run_id,
        "low",
        "Low",
        dt(2026, 9, 20),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    let urgent = seed(
        &db,
        account.id,
        run_id,
        "urgent",
        "Urgent",
        dt(2026, 9, 21),
        Category::Task,
        Priority::Urgent,
        vec![],
    );
    let high_older = seed(
        &db,
        account.id,
        run_id,
        "high1",
        "High1",
        dt(2026, 9, 22),
        Category::Task,
        Priority::High,
        vec![],
    );
    let high_newer = seed(
        &db,
        account.id,
        run_id,
        "high2",
        "High2",
        dt(2026, 9, 23),
        Category::Task,
        Priority::High,
        vec![],
    );

    let query = ResultQuery {
        sort: ResultSort::Priority,
        ..base_query()
    };
    let page = db.query(&query, today()).unwrap();

    assert_eq!(
        page.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![urgent, high_newer, high_older, low]
    );
}

#[test]
fn received_sort_orders_by_received_at_descending() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);

    let older = seed(
        &db,
        account.id,
        run_id,
        "older",
        "Older",
        dt(2026, 9, 20),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    let newer = seed(
        &db,
        account.id,
        run_id,
        "newer",
        "Newer",
        dt(2026, 9, 25),
        Category::Notice,
        Priority::Low,
        vec![],
    );

    let page = db.query(&base_query(), today()).unwrap();

    assert_eq!(
        page.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![newer, older]
    );
}

#[test]
fn query_paginates_and_reports_total_ignoring_limit() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let ids: Vec<i64> = (0..5)
        .map(|i| {
            seed(
                &db,
                account.id,
                run_id,
                &format!("uid-{i}"),
                &format!("Subject {i}"),
                dt(2026, 9, 20) + Duration::days(i),
                Category::Notice,
                Priority::Low,
                vec![],
            )
        })
        .collect();

    let query = ResultQuery {
        limit: 2,
        offset: 0,
        ..base_query()
    };
    let page1 = db.query(&query, today()).unwrap();
    assert_eq!(page1.total, 5);
    assert_eq!(
        page1.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![ids[4], ids[3]]
    );

    let query_page2 = ResultQuery { offset: 2, ..query };
    let page2 = db.query(&query_page2, today()).unwrap();
    assert_eq!(page2.total, 5);
    assert_eq!(
        page2.items.iter().map(|r| r.id).collect::<Vec<_>>(),
        vec![ids[2], ids[1]]
    );
}

#[test]
fn set_seen_true_then_false_toggles_seen_at() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let id = seed(
        &db,
        account.id,
        run_id,
        "1",
        "Subject",
        dt(2026, 9, 20),
        Category::Notice,
        Priority::Low,
        vec![],
    );

    let at = Utc::now();
    let seen = db.set_seen(id, true, at).unwrap();
    assert!(seen.seen);
    assert_eq!(seen.seen_at, Some(at));

    let unseen = db.set_seen(id, false, Utc::now()).unwrap();
    assert!(!unseen.seen);
    assert_eq!(unseen.seen_at, None);
}

#[test]
fn set_seen_missing_result_is_not_found() {
    let db = Database::open_in_memory().unwrap();
    let err = db.set_seen(999, true, Utc::now()).unwrap_err();
    assert!(matches!(err, RepoError::NotFound));
}

#[test]
fn mark_all_seen_marks_every_unseen_result_and_returns_count() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    let a = seed(
        &db,
        account.id,
        run_id,
        "1",
        "A",
        dt(2026, 9, 20),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    let b = seed(
        &db,
        account.id,
        run_id,
        "2",
        "B",
        dt(2026, 9, 21),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    db.set_seen(a, true, Utc::now()).unwrap();

    let changed = db.mark_all_seen(Utc::now()).unwrap();
    assert_eq!(changed, 1);

    let b_result = db.get(b).unwrap().unwrap();
    assert!(b_result.seen);
}

#[test]
fn unseen_counts_groups_by_category_and_ignores_seen_results() {
    let db = Database::open_in_memory().unwrap();
    let account = insert_account(&db, "acc");
    let run_id = insert_run(&db);
    seed(
        &db,
        account.id,
        run_id,
        "task",
        "Task",
        dt(2026, 9, 20),
        Category::Task,
        Priority::Low,
        vec![],
    );
    seed(
        &db,
        account.id,
        run_id,
        "exam",
        "Exam",
        dt(2026, 9, 21),
        Category::Exam,
        Priority::Low,
        vec![],
    );
    seed(
        &db,
        account.id,
        run_id,
        "event",
        "Event",
        dt(2026, 9, 22),
        Category::Event,
        Priority::Low,
        vec![],
    );
    seed(
        &db,
        account.id,
        run_id,
        "change",
        "Change",
        dt(2026, 9, 23),
        Category::Change,
        Priority::Low,
        vec![],
    );
    seed(
        &db,
        account.id,
        run_id,
        "notice",
        "Notice",
        dt(2026, 9, 24),
        Category::Notice,
        Priority::Low,
        vec![],
    );
    let seen_id = seed(
        &db,
        account.id,
        run_id,
        "seen",
        "Seen general info",
        dt(2026, 9, 25),
        Category::GeneralInfo,
        Priority::Low,
        vec![],
    );
    db.set_seen(seen_id, true, Utc::now()).unwrap();

    let counts = db.unseen_counts().unwrap();
    assert_eq!(counts.new_total, 5);
    assert_eq!(counts.tasks, 1);
    assert_eq!(counts.exams, 1);
    assert_eq!(counts.events, 1);
    assert_eq!(counts.changes_and_notices, 2);
}
