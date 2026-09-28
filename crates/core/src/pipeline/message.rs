//! Per-message decision: normalize, dedup, filter, analyze, persist (spec §5.1).

use crate::analyzer::{self, AnalysisInput};
use crate::dates;
use crate::domain::{
    Account, AnalysisOutcome, Email, FilterReason, MessageOutcome as ProcessedOutcome, NewResult,
    ProcessedMessage,
};
use crate::filters::{self, FilterConfig, FilterDecision};
use crate::normalize;
use crate::ports::{ModelError, RepoError};

use super::RunContext;

/// Maximum length (in `char`s) kept from a sanitized error message before it is
/// stored as a `processed_messages.reason`, so an unexpectedly verbose provider
/// error can never balloon that column.
const MAX_REASON_CHARS: usize = 500;

/// What happened to one email, for the caller's counters (spec §5.1: `found`
/// excludes [`MessageOutcome::Skipped`]; `analyzed` counts everything except
/// [`MessageOutcome::Skipped`] and [`MessageOutcome::Filtered`]).
pub(super) enum MessageOutcome {
    /// Already in `processed_messages` for this `(account, folder,
    /// provider_message_id)`; not counted as found.
    Skipped,
    /// Discarded by dedup (`FilterReason::Duplicate`) or by `filters::apply`. The
    /// reason itself is only needed for the `processed_messages` row (already
    /// written by the time this is returned); callers only need the outcome kind
    /// to update their counters.
    Filtered,
    /// Analyzed and judged not relevant to the student.
    Irrelevant,
    /// Analyzed, relevant, and persisted as a new result.
    Relevant,
    /// The model's response failed validation; recorded as a failed message and
    /// otherwise skipped, without stopping the account.
    InvalidResponse,
}

/// A failure that must stop the current folder (and the whole account): either a
/// repository error, or a transient [`ModelError`] (`Auth`, `RateLimited`,
/// `Unavailable` — never `InvalidResponse`, which is handled as
/// [`MessageOutcome::InvalidResponse`] instead).
pub(super) enum MessageError {
    Repo(RepoError),
    Model(ModelError),
}

impl From<RepoError> for MessageError {
    fn from(error: RepoError) -> Self {
        MessageError::Repo(error)
    }
}

/// Processes one fetched `email` through the full per-message pipeline (spec
/// §5.1): dedup against `processed_messages`, deterministic filters, date
/// candidate extraction, AI analysis, and persistence of the outcome.
pub(super) async fn process_message(
    ctx: &RunContext<'_>,
    account: &Account,
    email: &Email,
) -> Result<MessageOutcome, MessageError> {
    let already_seen =
        ctx.repos
            .processed
            .exists(account.id, &email.folder, &email.provider_message_id)?;
    if already_seen {
        return Ok(MessageOutcome::Skipped);
    }

    let normalized_body = normalize::normalize_body(&email.body);
    let content_hash =
        normalize::content_hash(&email.sender.email, &email.subject, &normalized_body);

    let is_duplicate = ctx.repos.processed.is_duplicate(
        account.id,
        email.message_id_header.as_deref(),
        &content_hash,
    )?;
    if is_duplicate {
        record_filtered(ctx, account, email, &content_hash, FilterReason::Duplicate)?;
        return Ok(MessageOutcome::Filtered);
    }

    let filter_config = FilterConfig {
        blocked: ctx.settings.blocked_senders.clone(),
        own_address: account.email_address.clone(),
    };
    if let FilterDecision::Discard(reason) = filters::apply(email, &normalized_body, &filter_config)
    {
        record_filtered(ctx, account, email, &content_hash, reason)?;
        return Ok(MessageOutcome::Filtered);
    }

    let local_received_at = email.received_at.with_timezone(&ctx.clock.local_offset());
    let date_candidates = dates::extract(&email.subject, &normalized_body, local_received_at);

    let input = AnalysisInput {
        email,
        normalized_body: &normalized_body,
        date_candidates: &date_candidates,
        account,
        catalog: ctx.catalog,
    };

    match analyzer::analyze(&input, ctx.model).await {
        Ok(AnalysisOutcome::Irrelevant { .. }) => {
            record_processed(
                ctx,
                account,
                email,
                &content_hash,
                ProcessedOutcome::Irrelevant,
                None,
            )?;
            Ok(MessageOutcome::Irrelevant)
        }
        Ok(AnalysisOutcome::Relevant(analysis)) => {
            save_relevant(ctx, account, email, &content_hash, analysis)?;
            Ok(MessageOutcome::Relevant)
        }
        Err(ModelError::InvalidResponse(message)) => {
            let reason = sanitize(&message);
            record_processed(
                ctx,
                account,
                email,
                &content_hash,
                ProcessedOutcome::Failed,
                Some(reason),
            )?;
            Ok(MessageOutcome::InvalidResponse)
        }
        Err(transient) => Err(MessageError::Model(transient)),
    }
}

fn record_filtered(
    ctx: &RunContext<'_>,
    account: &Account,
    email: &Email,
    content_hash: &str,
    reason: FilterReason,
) -> Result<(), RepoError> {
    record_processed(
        ctx,
        account,
        email,
        content_hash,
        ProcessedOutcome::Filtered,
        Some(reason.as_str().to_string()),
    )
}

fn record_processed(
    ctx: &RunContext<'_>,
    account: &Account,
    email: &Email,
    content_hash: &str,
    outcome: ProcessedOutcome,
    reason: Option<String>,
) -> Result<(), RepoError> {
    let message = ProcessedMessage {
        account_id: account.id,
        folder: email.folder.clone(),
        provider_message_id: email.provider_message_id.clone(),
        message_id_header: email.message_id_header.clone(),
        content_hash: content_hash.to_string(),
        received_at: email.received_at,
        outcome,
        reason,
        run_id: ctx.run_id,
        processed_at: ctx.clock.now(),
    };
    ctx.repos.processed.insert(&message)?;
    Ok(())
}

/// Persists the processed message (outcome `Analyzed`) and the new result in one
/// transaction, via [`crate::ports::ResultRepository::save_analyzed`].
fn save_relevant(
    ctx: &RunContext<'_>,
    account: &Account,
    email: &Email,
    content_hash: &str,
    analysis: crate::domain::AIAnalysis,
) -> Result<(), RepoError> {
    let now = ctx.clock.now();
    let message = ProcessedMessage {
        account_id: account.id,
        folder: email.folder.clone(),
        provider_message_id: email.provider_message_id.clone(),
        message_id_header: email.message_id_header.clone(),
        content_hash: content_hash.to_string(),
        received_at: email.received_at,
        outcome: ProcessedOutcome::Analyzed,
        reason: None,
        run_id: ctx.run_id,
        processed_at: now,
    };
    let new_result = NewResult {
        account_id: account.id,
        run_id: ctx.run_id,
        detected_at: now,
        received_at: email.received_at,
        subject: email.subject.clone(),
        sender_name: email.sender.name.clone(),
        sender_email: email.sender.email.clone(),
        source_url: email.source_url.clone(),
        analysis,
    };
    ctx.repos.results.save_analyzed(&message, &new_result)?;
    Ok(())
}

/// Collapses a model error message to a single line and truncates it to
/// [`MAX_REASON_CHARS`], so provider output never lands verbatim (and never with
/// embedded control characters) in a stored `processed_messages.reason`.
fn sanitize(message: &str) -> String {
    let collapsed: String = message.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= MAX_REASON_CHARS {
        collapsed
    } else {
        collapsed.chars().take(MAX_REASON_CHARS).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_collapses_whitespace_and_newlines() {
        assert_eq!(
            sanitize("line one\n  line two\ttab"),
            "line one line two tab"
        );
    }

    #[test]
    fn sanitize_truncates_long_messages() {
        let long = "x".repeat(MAX_REASON_CHARS + 100);
        let sanitized = sanitize(&long);
        assert_eq!(sanitized.chars().count(), MAX_REASON_CHARS);
    }
}
