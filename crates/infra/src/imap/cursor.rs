//! Pure cursor planning for [`super::ImapEmailProvider`]: which UIDs to fetch next,
//! independent of any network I/O so it can be unit tested without a server.

/// Picks which of the `found` UIDs to fetch in this batch.
///
/// Filters out any UID at or below `cursor_last_uid` (this also absorbs the IMAP `*`
/// quirk: a `UID SEARCH {last_uid+1}:*` on a mailbox with no new mail can return the
/// highest existing UID even though it is `<= last_uid`), sorts the remainder
/// ascending, and takes at most `limit` of them.
pub fn select_uids(found: &[u32], cursor_last_uid: Option<u32>, limit: usize) -> Vec<u32> {
    let mut uids: Vec<u32> = found
        .iter()
        .copied()
        .filter(|&uid| cursor_last_uid.is_none_or(|last| uid > last))
        .collect();
    uids.sort_unstable();
    uids.truncate(limit);
    uids
}
