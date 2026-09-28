# `academic-core::analyzer`: email analyzer

Builds the AI question set for one email, and assembles a [`StructuredModel`]'s
answers into an [`AIAnalysis`]/[`AnalysisOutcome`]. Pure: no I/O, no calls into
`core::dates` (date candidates are extracted upstream and handed in).

Module layout (`crates/core/src/analyzer/`):

```
mod.rs          public API: constants, AnalysisInput, AnalysisRequest, build_request, analyze
questions.rs    state + QuestionSet building (spec §6.1, §6.2)
assemble.rs     answer validation + AnalysisOutcome assembly (spec §6.4)
test_support.rs shared test fixtures (#[cfg(test)] only)
```

## Public API

```rust
pub const RELEVANCE_THRESHOLD: f32 = 0.4;
pub const FLAG_THRESHOLD: f32 = 0.5;
pub const MAX_COURSE_OPTIONS: usize = 254;

pub struct AnalysisInput<'a> {
    pub email: &'a Email,
    pub normalized_body: &'a str,
    pub date_candidates: &'a [DateCandidate],
    pub account: &'a Account,
    pub catalog: &'a Catalog,
}

pub struct AnalysisRequest {
    pub state: serde_json::Value,
    pub questions: QuestionSet,
}

pub fn build_request(input: &AnalysisInput) -> AnalysisRequest;
pub fn assemble(input: &AnalysisInput, answers: &AnswerSet, model: ModelDescriptor) -> Result<AnalysisOutcome, ModelError>;
pub async fn analyze(input: &AnalysisInput<'_>, model: &dyn StructuredModel) -> Result<AnalysisOutcome, ModelError>;
```

`analyze` calls `model.ask` exactly once: it builds the request, sends it, and
assembles the returned `AnswerSet` — it does not retry or ask follow-up questions.
`build_request` and `assemble` are exposed separately so a caller can inspect/log the
request before sending it, or assemble an `AnswerSet` obtained through another channel,
as long as it passes the same `AnalysisInput` the questions were built from.

## State (spec §6.1)

```json
{
  "email": {
    "subject": "...",
    "from": { "name": "...", "email": "..." },
    "received_at": "2026-09-28T09:14:00+00:00",
    "body": "<normalized_body>"
  },
  "context": { "account_university": "Universidad de Alcalá" }
}
```

- `email.body` is `AnalysisInput::normalized_body`, not the raw `Email::body`.
- `email.received_at` is `Email::received_at.to_rfc3339()` (RFC 3339; the offset is
  whatever `DateTime<Utc>` produces, i.e. `+00:00`).
- `context.account_university` is present, with the university's name, only when the
  account has a `default_university_id` **and** that id resolves to a university in the
  catalog. Otherwise `context` is present as an empty object.

## Question set (spec §6.2)

Built in this fixed order (a `QuestionSet` is an `IndexMap`, so order is preserved for
prompt building):

| Id | Type | Always asked? |
|---|---|---|
| `relevant` | Noul | yes |
| `category` | Choice (`notice`, `task`, `exam`, `event`, `change`, `general_info`) | yes |
| `action_required` | Noul | yes |
| `possible_change` | Noul | yes |
| `priority` | Score (`low`, `medium`, `high`, `urgent`) | yes |
| `university` | Choice (`u{id}` per catalog university, plus `unknown`) | only when the account has no `default_university_id` **and** the catalog has at least one university |
| `course` | Choice (`c{id}` per candidate course, plus `none`) | only when there is at least one candidate course |
| `date_{i}` | Choice (`exam`, `deadline`, `event`, `class`, `other`, `not_relevant`) | one per date candidate, `i` = index in `date_candidates`, capped at 20 |

Instruction and option texts for `relevant`, `action_required`, `possible_change`,
`category`'s options, `priority`'s levels and `date_{i}`'s options are fixed strings
from the spec/task contract (see `questions.rs`'s constants). `category`, `priority`,
`university` and `course` also carry an `instructions` string (required by
`Question::Choice`/`Question::Score`); the spec does not fix that text verbatim for
these four questions, so `questions.rs` uses a short, direct prompt for each — this is
an implementation detail, not part of the tested contract.

**Candidate courses** (the courses offered by the `course` question, decided *before*
any answer exists): the default university's courses when the account has one, else
every course in the catalog. Capped at `MAX_COURSE_OPTIONS` (254, by catalog order),
leaving room for the fixed `none` option within `Question::Choice`'s 255-option limit.
A course's option description is `"{name} ({university})"`, plus
`" — also known as: {aliases joined with ", "}"` when the course has aliases. If a
course's `university_id` does not resolve to a catalog university (a dangling
reference), the `"({university})"` part is omitted and the description is just the
course name.

**Date questions**: capped at `core::dates::MAX_CANDIDATES` (20), one `date_i` question
per date candidate in `AnalysisInput::date_candidates` (itself already capped at that
same constant by `core::dates::extract`, so the `.take(...)` here is a defensive upper
bound rather than a normally-reachable cap).

## Validation and assembly (spec §6.4)

`assemble` rebuilds the same `QuestionSet` `build_request` would have produced for
`input`, then validates `answers` against it:

- Every question in the rebuilt set must have a matching answer in `answers`, of the
  same variant (`Noul`/`Choice`/`Score`), with — for `Choice` — a key present in that
  question's `options`, and — for `Score` — a `level` within `0..levels.len()`.
  Any violation is `ModelError::InvalidResponse("...")` naming the question id and the
  problem (missing, wrong type, unknown option, or out-of-range level).
- Answers in `answers` for ids not present in the rebuilt question set are ignored.

Assembly, once validation passes:

- `relevant.p_yes < RELEVANCE_THRESHOLD` (strictly below 0.4) → `AnalysisOutcome::Irrelevant { p_relevant }`.
  Otherwise → `AnalysisOutcome::Relevant(AIAnalysis { .. })`.
- `category` / `priority`: parsed directly from the answered option/level (the option
  keys are the domain enums' own snake_case strings).
- `action_required` / `possible_change`: `true` iff the corresponding `p_yes >= FLAG_THRESHOLD` (0.5).
- `university_id`: the account's `default_university_id` if set; else, if the
  `university` question was asked, its answer (`unknown` → `None`, `u{id}` → `Some(id)`);
  else (the catalog had no universities, so no `university` question) the chosen
  course's `university_id`, if a course was chosen.
- `course_id`: the `course` answer (`none` → `None`, `c{id}` → `Some(id)`) when the
  `course` question was asked; otherwise `None`.
- `dates`: one `ResultDate` per date candidate whose answer is not `not_relevant`,
  carrying the candidate's `date`/`time` and the answered kind (`exam`, `deadline`,
  `event`, `class`, `other`).
- `confidence`: the minimum `confidence` among every answer to an asked question
  (across all question types); `None` if none of them carry one.
- `model`: passed through as given to `assemble`/`analyze` (the calling model's
  `descriptor()`).

## Errors

Only `ModelError::InvalidResponse` originates in this module (from validation). Other
`ModelError` variants (`Auth`, `RateLimited`, `Unavailable`) only reach callers of
`analyze` when `StructuredModel::ask` itself returns them — `analyze` propagates them
unchanged via `?`.

## Testing

Unit tests live next to the code they cover: `questions.rs` (state shape, which
questions are asked and their options) and `assemble.rs` (thresholds, resolution
order, validation errors, confidence) each have a `#[cfg(test)] mod tests`; `mod.rs`
has a `FakeModel` (`StructuredModel`) test verifying `analyze` calls `ask` exactly
once. Shared fixtures (a base `Email`/`Account`/`Catalog`/`DateCandidate`) live in
`test_support.rs`, gated on `#[cfg(test)]` via its `mod` declaration in `mod.rs`.

Run with `cargo test -p academic-core analyzer::`.

## Known limitations

- No bound on the number of `university` options: unlike `course`, the spec does not
  fix a cap, so a catalog with a very large number of universities would produce a
  correspondingly large `Question::Choice` (still subject to that type's documented
  2..=255-option range, which is not enforced by this module — a catalog with more
  than 254 universities would violate it).
- `context.account_university` is silently omitted (not an error) when
  `default_university_id` is set but does not resolve in the catalog. The same dangling
  reference falls back to the bare course name (no `"(university)"` suffix) in `course`
  option descriptions. Neither case is expected in practice (both ids come from the
  same catalog) and neither is covered by a dedicated test.
