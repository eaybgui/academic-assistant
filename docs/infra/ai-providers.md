# AI provider adapters (`academic-infra::jev`, `academic-infra::ollama`)

Both types implement `academic_core::ports::StructuredModel`:

```rust
fn descriptor(&self) -> ModelDescriptor;
async fn ask(&self, state: &serde_json::Value, questions: &QuestionSet) -> Result<AnswerSet, ModelError>;
```

Neither adapter validates that every asked question was answered, that a `Choice`
answer names a real option, etc. — beyond the checks described below, they return
whatever the provider sent. A question id absent from the reply is simply absent from
the returned `AnswerSet`; a fully-populated `AnswerSet` is not guaranteed. Full answer
validation against the `QuestionSet` (missing answers, wrong option, wrong level) is
`core::analyzer`'s job (Task T5), not the adapter's.

Neither adapter ever includes the API key, the `state` sent, or any answer content in an
error message or a log line — see spec §12. Error messages are short, generic
descriptions of what went wrong (a status code, "request timed out", "malformed
response body", the id of a question whose answer didn't match its schema), never a
dump of the request or response body.

## `JevModel` (`crates/infra/src/jev.rs`)

`JevModel::new(http: reqwest::Client, base_url: String, api_key: String, model: String) -> Self`.
`descriptor()` returns `{ provider: "jev", model: <configured model> }`.

### Request

`POST {base_url}/v1/systemone` (constant `JEV_BASE_URL = "https://api.typesafe.ai"`),
header `Authorization: Bearer <api_key>`, JSON body:

```json
{
  "model": "<configured model, e.g. jev-latest>",
  "state": <the `state` argument, verbatim>,
  "questions": {
    "<id>": { "type": "noul", "instructions": "..." },
    "<id>": { "type": "choice", "instructions": "...", "criteria": { "<option>": "<description>", ... } },
    "<id>": { "type": "score", "instructions": "...", "criteria": ["<level 0>", "<level 1>", ...] }
  }
}
```

`Question::Noul` has no `criteria` key at all (not `null`, absent); `Question::Choice`'s
`criteria` is the options map as-is (object); `Question::Score`'s `criteria` is the
`levels` list as-is (array, index = level).

### Response mapping

```json
{
  "model": "jev-1.13.0",
  "answers": {
    "<id>": { "type": "noul", "noul": 0.87, "confidence": 0.9 },
    "<id>": { "type": "choice", "choice": "<option>", "probabilities": {...}, "confidence": 0.6 },
    "<id>": { "type": "score", "score": 1.43, "confidence": 0.35, "legend": {...}, "probabilities": { "0": 0.0, "1": 0.57, "2": 0.43 } }
  },
  "usage": { "input_tokens": 10, "output_tokens": 5 }
}
```

- `noul` → `Answer::Noul { p_yes: noul, confidence }`.
- `choice` → `Answer::Choice { choice, confidence }` (the `probabilities` object, if
  present, is not used — jev already resolved the argmax server-side).
- `score` → `Answer::Score { level, confidence }`, where `level` is the argmax of
  `probabilities` (its keys are string indices, e.g. `"0"`, `"1"`, ...) — **not**
  `round(score)`. Ties are broken by the lowest level, independent of map iteration
  order. If `probabilities` has no parseable index (malformed/empty), that one answer is
  dropped from the result (same as a missing key) rather than failing the whole call.
- `confidence` is `Option<f32>` on every variant; absent in the JSON → `None`.
- Any top-level fields not listed above (`model`, `usage`, `legend`, `score` on a score
  answer) are read from the response but not surfaced in the `AnswerSet`.

### Errors and retries

| Condition | Result |
|---|---|
| HTTP 401 | `ModelError::Auth`, no retry |
| HTTP 422 | `ModelError::InvalidResponse`, no retry (request rejected as invalid) |
| HTTP 429 | retried; `ModelError::RateLimited` if still failing after retries |
| HTTP 529, any other 5xx, or a transport error (timeout, connection refused, ...) | retried; `ModelError::Unavailable` if still failing after retries |
| 2xx with a body that isn't valid JSON matching the expected shape | `ModelError::InvalidResponse`, no retry |

Retries: up to 3 retries after the initial request (4 requests total), with exponential
backoff between attempts — 1 s, 2 s, 4 s by default. The backoff base is configurable via
the doc-hidden `JevModel::with_retry_base(Duration)` builder (not part of the stable
public contract), which tests use to run in milliseconds instead of seconds; production
callers use the default. Each individual HTTP request has a 60 s timeout.

## `OllamaModel` (`crates/infra/src/ollama.rs`)

`OllamaModel::new(http: reqwest::Client, base_url: String, model: String) -> Self`.
`descriptor()` returns `{ provider: "ollama", model: <configured model> }`.

### Request

`POST {base_url}/api/chat` (typically `http://localhost:11434`), JSON body:

```json
{
  "model": "<configured model>",
  "messages": [
    { "role": "system", "content": "You answer typed questions about the provided state. Reply only with JSON matching the schema." },
    { "role": "user", "content": "<state and question instructions, see below>" }
  ],
  "stream": false,
  "think": false,
  "format": { "type": "object", "properties": { "<id>": <schema> }, "required": ["<id>", ...] },
  "options": { "temperature": 0 }
}
```

`format.properties.<id>` per question type:
- `Question::Noul` → `{ "type": "boolean" }`
- `Question::Choice` → `{ "type": "string", "enum": [<option keys, in order>] }`
- `Question::Score` → `{ "type": "integer", "enum": [0, 1, ..., levels.len() - 1] }`

The user message lists the JSON `state`, then each question as `"<id>": <instructions>`
followed by `- <key>: <description>` lines for a choice's options or `- <index>: <level>`
lines for a score's levels.

Single request, no retries (unlike jev). Timeout: 120 s.

### Response mapping

The response is `{ "message": { "role": "assistant", "content": "<JSON string>" }, ... }`.
`message.content` is parsed as JSON and must be an object; for each asked question id
present in it:
- `Question::Noul` → the value must be a JSON boolean; `true` → `Answer::Noul { p_yes: 1.0,
  confidence: None }`, `false` → `p_yes: 0.0`.
- `Question::Choice` → the value must be a JSON string that is one of the question's
  option keys → `Answer::Choice { choice, confidence: None }`.
- `Question::Score` → the value must be a JSON non-negative integer `< levels.len()` →
  `Answer::Score { level, confidence: None }`.

Ollama never reports a confidence, so every mapped answer has `confidence: None`. A
question id absent from `content` is simply absent from the result. A present value that
doesn't match its question's type/options/range makes the whole call fail with
`ModelError::InvalidResponse` (naming the offending question id, never its value).

### Errors

| Condition | Result |
|---|---|
| Transport error (connection refused, timeout, ...) | `ModelError::Unavailable` |
| Non-2xx HTTP status (including 5xx) | `ModelError::Unavailable` |
| `message.content` isn't valid JSON, isn't an object, or a value doesn't match its question's schema | `ModelError::InvalidResponse` |

## Testing

Both adapters are tested against `wiremock` in `crates/infra/tests/jev.rs` and
`crates/infra/tests/ollama.rs`: request shape (body, headers, schema), response mapping,
and every error path above, including jev's retry-then-succeed and retry-then-fail
cases (with a near-zero retry base so the suite stays fast).

Each file also has one `#[ignore]`d test against the real provider:
- `jev.rs::live_jev_answers_a_simple_noul_question` — reads `JEV_API_KEY`; a no-op
  (returns immediately) if the variable isn't set. Run with
  `JEV_API_KEY=<key> cargo test -p academic-infra --test jev -- --ignored`.
- `ollama.rs::live_ollama_answers_a_simple_noul_question` — reads `OLLAMA_MODEL`
  (required) and `OLLAMA_BASE_URL` (optional, defaults to `http://localhost:11434`); a
  no-op if `OLLAMA_MODEL` isn't set. Run with
  `OLLAMA_MODEL=<model> cargo test -p academic-infra --test ollama -- --ignored`
  (add `OLLAMA_BASE_URL=<url>` if the server isn't on localhost:11434).

## Known limitations

- jev's retry loop is a fixed count (3) with a fixed backoff shape (base, 2×base,
  4×base); it does not honor a `Retry-After` header if the API ever sends one.
- The Ollama adapter trusts the server to mostly honor the `format` JSON schema it was
  given; it still validates the reply itself (see above) since local models are not
  guaranteed to be fully schema-compliant.
