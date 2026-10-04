//! Tests for the pure UID-selection logic used to plan each IMAP fetch batch.

use academic_infra::imap::select_uids;

#[test]
fn no_cursor_keeps_everything_sorted_up_to_limit() {
    let found = [42, 3, 17, 8];
    assert_eq!(select_uids(&found, None, 200), vec![3, 8, 17, 42]);
}

#[test]
fn no_cursor_respects_the_limit() {
    let found = [5, 4, 3, 2, 1];
    assert_eq!(select_uids(&found, None, 3), vec![1, 2, 3]);
}

#[test]
fn cursor_filters_out_uids_at_or_below_last_uid() {
    let found = [10, 11, 12];
    assert_eq!(select_uids(&found, Some(11), 200), vec![12]);
}

#[test]
fn cursor_filters_the_star_quirk_returning_only_the_old_uid() {
    // `UID SEARCH 101:*` on a mailbox whose highest UID is still 100 (no new mail)
    // can come back with just UID 100 due to the trailing `*`; it must not be
    // re-fetched since it is not greater than the cursor's last_uid.
    let found = [100];
    assert_eq!(select_uids(&found, Some(100), 200), Vec::<u32>::new());
}

#[test]
fn empty_found_yields_an_empty_selection() {
    assert_eq!(select_uids(&[], Some(5), 200), Vec::<u32>::new());
}

#[test]
fn selection_never_exceeds_max_batch() {
    let found: Vec<u32> = (1..=500).collect();
    assert_eq!(select_uids(&found, None, 200).len(), 200);
}
