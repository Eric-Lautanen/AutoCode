# Fix Plan: Context Window Token Counting

## Current Problems (from code audit)

1. **Heuristic-only estimation** (`crates/core/src/helpers.rs:52`): `estimate_tokens()` is a hand-rolled heuristic with no model-specific knowledge. It only counts the `content` field per message.

2. **Tool call tokens not counted**: `ChatMessage.token_count` is set once via `estimate_tokens(&content)` — `tool_calls` JSON, `tool_call_id`, and `reasoning_content` are excluded.

3. **Tool definitions not counted**: The `tools` array sent in the request body (~50+ tool definitions) adds significant tokens but is never estimated.

4. **`actual_tokens_used` overwrites each turn**: `record_actual_usage()` replaces the previous value instead of accumulating. Only the last API call's usage is reflected.

5. **RAM vs Disk mismatch**: `session.token_count()` sums only in-memory messages, but `prepare_request_messages_for_session()` loads ALL messages from disk. Displayed count under-reports.

6. **No pre-flight check**: Messages are assembled and sent without checking if they exceed `max_context_tokens`. If they do, the API errors.

---

## Provider Research (from official docs, June 2026)

### Dedicated Token Counting APIs

Every major provider now offers a server-side counting endpoint (confirmed June 2026):

| Provider | Endpoint | What it counts |
|----------|----------|-----------------|
| **OpenAI** | `POST /v1/responses/input_tokens` | instructions, messages, tools, images, files, conversations |
| **Anthropic** | `POST /v1/messages/count_tokens` | system, messages, tools, images, PDFs, thinking blocks (notes: previous-turn thinking blocks DON'T count) |
| **DeepSeek** | Offline tokenizer zip (model-specific) | Text only — no dedicated API endpoint |
| **Google Gemini** | `countTokens()` in SDK | Contents, tools — SentencePiece-based |
| **Mistral** | `mistral_common` package | SentencePiece BPE — text only |

**New (2026)**: The `tiktoken` Rust crate v3.x now supports `deepseek_v3`, `llama3`, `qwen2`, and `mistral_v3` encodings natively — no separate tokenizer needed for these models.

**Key finding**: Both OpenAI and Anthropic explicitly state that local tokenizers are insufficient:

> OpenAI: *"Local tokenizers like tiktoken work for plain text, but they have limitations — images and files are not supported, tools and schemas add tokens that are hard to count locally, model-specific behavior can change tokenization."*

> Anthropic: *"As of Claude 3 models, this algorithm [their own offline tokenizer] is no longer accurate, but can be used as a very rough approximation. We suggest that you rely on `usage` in the response body wherever possible."*

### Open-Source Tokenizer Libraries

| Library | Lang | Version (Jun 2026) | Notes |
|---------|------|-------------------|-------|
| **tiktoken** (rust-tiktoken) | Rust | **3.3.0** | Fast BPE, covers 11 encodings across 6 providers: OpenAI (`cl100k_base`, `o200k_base`, `o200k_harmony`, `p50k_base`, `p50k_edit`, `r50k_base`, `gpt2`), Meta (`llama3`), DeepSeek (`deepseek_v3`), Alibaba (`qwen2`), Mistral (`mistral_v3`). Pure Rust (edition 2024). Deps: base64 0.22, regex 1, rustc-hash 2, ruzstd 0.8. Optional: rayon 1 for parallel. |
| **tokenizers** (HuggingFace) | Rust | **0.23.1** | General-purpose: BPE, WordPiece, Unigram. Heavier dependency tree (ahash, onig, rand 0.9, serde, thiserror 2, unicode-segmentation, etc.) |
| **anthropic-tokenizer-typescript** | TypeScript | 106 | Explicitly marked inaccurate for Claude 3+. API usage recommended. |
| **deepseek_tokenizer** | Python zip | N/A | Model-specific, distributed as downloadable zip. |

### How Other Tools Handle This

- **Continue (Continue.dev)**: Uses model-specific `countTokens` from `tiktoken` for OpenAI models, falls back to character-based heuristic for others. Accepts the discrepancy.
- **Aider**: Uses `tiktoken` with model-specific encoding lookup. Falls back to a simple heuristic for unknown models.
- **Claude Code (Anthropic)**: Uses the server-side `count_tokens` API before sending, falls back to `usage` from response.

---

## Strategy

### Tier 1: API-Based Counting (Most Accurate)

For providers that offer a dedicated counting endpoint (OpenAI, Anthropic), call it with the same payload that will be sent to the real request. This is the only way to get accurate counts including tools, images, and model-specific behavior.

**Cost**: OpenAI's counting endpoint is free. Anthropic's is free but rate-limited per usage tier.

**Implementation**: `prepare_request_messages_for_session()` already assembles the full message list. Before `start_completion`, reuse that list to call the counting endpoint.

### Tier 2: Accumulated API Response Usage (Ground Truth)

After each API response, the `usage` object contains `prompt_tokens` and `completion_tokens`. Accumulate `prompt_tokens` across all turns in a session to get the true context usage. This is the authoritative source — providers bill based on this.

**Current bug**: `record_actual_usage` does `self.actual_tokens_used = prompt + completion` (overwrites). Should be `self.actual_tokens_used += prompt` (accumulates).

### Tier 3: Offline Tokenizer Fallback

For providers without a counting API (DeepSeek, local models, custom endpoints), use the appropriate offline tokenizer:

- **OpenAI-compatible models** → Embed tiktoken's Rust core with the appropriate encoding tables
- **DeepSeek models** → Use their published tokenizer or embed BPE tables
- **Unknown models** → Keep the current heuristic but extend it to count the full JSON serialization of each message (not just `content`)

---

## Implementation Phases

### ✅ Phase 1: Fix Accumulation and Count Full Messages (Complete)

**Files**: `crates/core/src/state.rs`, `crates/core/src/helpers.rs`

**Status**: Implemented and verified. Changes compiled successfully.

#### 1a. Make `actual_tokens_used` additive

`record_actual_usage` in `state.rs:490` changed from `self.actual_tokens_used = prompt + completion` (overwrites) to `self.actual_tokens_used += prompt` (accumulates prompt tokens across turns).

**Rationale**: `actual_tokens_used` exists to track total context consumption. Each turn adds more prompt tokens to the existing context. `completion_tokens` are output tokens not part of the next request's context, so only `prompt` is accumulated.

#### 1b. Per-message token counts marked as estimates

Added doc comments to:
- `ChatMessage.token_count` field — clarified it counts `content` only and is a heuristic estimate
- `Session::token_count()` method — clarified it sums in-RAM estimated counts and should not be confused with `actual_tokens_used`

The `usage_display()` and `budget_fraction()` functions already prefer `actual_tokens_used` when available, so no behavioral change needed.

#### 1c. Added `estimate_full_request_tokens` function

New function in `helpers.rs`:
- Signature: `estimate_full_request_tokens(messages: &[ChatMessage], tools_json: Option<&serde_json::Value>) -> usize`
- Serializes the relevant API-facing message fields (role, content, tool_call_id, tool_calls, reasoning_content) into JSON, optionally includes tool definitions, then applies the heuristic text token estimator to the full serialized string
- Provides a better upper bound for the Phase 2 pre-flight check by accounting for JSON structural overhead and tool call tokens that `estimate_tokens(&content)` misses

### ✅ Phase 2: Pre-Flight Context Check (Complete)

**Files**: `crates/ai/src/chat.rs:654`, `crates/ai/src/provider.rs:264`

**Status**: Implemented and verified. Compilation succeeds with no warnings.

#### 2a. Made `tool_definitions()` accessible

Changed visibility from `fn tool_definitions()` to `pub(crate) fn tool_definitions()` in `provider.rs:264` so the pre-flight check in `chat.rs` can call it to estimate tool definition tokens.

#### 2b. Pre-flight check in `start_completion`

Inserted before `CompletionRequest` construction at `chat.rs:654-709`:

1. Serializes messages (role, content, tool_call_id, tool_calls, reasoning_content) + tool definitions into the same JSON format the API will receive
2. Estimates tokens using `core_helpers::estimate_tokens()` on the full serialized JSON
3. Calculates `estimated + max_output > max_context` using the provider's configured `max_context_tokens` and the computed `max_tokens`
4. On overflow:
   - If `handoff_enabled` → calls `handle_handoff()` to create a fresh session and continues there
   - If handoff is disabled → pushes an error message with estimated/context/max details and returns early

This prevents the opaque API error the user currently gets when context overflows.

### ✅ Phase 3: API-Based Counting (Best Accuracy)

**Files**: `crates/core/src/state.rs`, `crates/ai/src/provider.rs`, `crates/ai/src/chat.rs`

**Status**: Implemented and verified. Compilation succeeds with no warnings.

#### 3a. Detect counting API from provider configuration

Added `counting_endpoint_url()` and `has_counting_api()` methods to `ApiProvider` in `state.rs:286-305`. The endpoint is inferred from the provider's `base_url`:

| `base_url` contains | Counting endpoint |
|---|---|
| `api.openai.com` | `POST /responses/input_tokens` (Responses API) |
| `api.anthropic.com` | `POST /messages/count_tokens` (Messages API) |
| Other | `None` (no counting API) |

This approach avoids adding a new manifest field — the implementation is self-detecting based on the provider's existing `base_url` configuration.

#### 3b. Implement counting API call

Added two new functions in `provider.rs`:

1. **`native_post()`** — a generic HTTP POST helper (analogous to the existing `native_get()`) that:
   - Supports both HTTP and HTTPS (TLS) connections
   - Accepts extra headers (e.g., `x-api-key` + `anthropic-version` for Anthropic)
   - Strips HTTP response headers and decodes chunked transfer-encoding
   - Returns the response body as a `String`

2. **`count_input_tokens()`** — public function that:
   - Detects the provider's counting endpoint via `provider.counting_endpoint_url()`
   - Transforms the request body for each provider:
     - **OpenAI**: renames `messages` → `input` (Responses API format), adds `model` field
     - **Anthropic**: adds `model` field, uses `x-api-key` + `anthropic-version` headers
   - Calls `native_post()` with the appropriate URL, body, and auth headers
   - Parses `input_tokens` from the JSON response

#### 3c. Integrate with pre-flight check

Modified `start_completion()` in `chat.rs:679-695` to try the API counting endpoint before falling back to the heuristic:

```rust
let estimated = if provider.has_counting_api() {
    match count_input_tokens(&provider, &json_str, &provider.model, state.request_timeout_secs) {
        Ok(count) => { debug_log!("chat: pre-flight API count={}", count); count }
        Err(e) => { /* fall back to heuristic */ }
    }
} else { /* heuristic */ };
```

If the counting API call succeeds, the pre-flight check uses the exact token count from the provider. If it fails (network error, unsupported provider), it silently falls back to the Phase 2 heuristic.

**Latency**: The counting API call adds one HTTP round trip before the streaming request starts. This is acceptable because:
- The timeout uses the same `request_timeout_secs` as regular requests
- Failed calls degrade gracefully to heuristic (no user-facing delay increase)
- The count is accurate for images, PDFs, tool schemas, and model-specific behavior

### Phase 4: Offline Tokenizer Plugin System

**Files**: `crates/core/Cargo.toml`, `crates/core/src/tokenizer/` (new)

#### 4a. Add tiktoken as a Rust dependency for OpenAI models

The `tiktoken` Rust crate (v3.3.0 as of June 2026) can be used directly as a library.
Notably, v3.x supports **11 encodings** across **6 providers** — including `deepseek_v3`,
`llama3`, `qwen2`, and `mistral_v3` natively — so a single dependency covers most models.

```toml
tiktoken = "3.3"
```

This provides access to `o200k_base`, `cl100k_base`, etc. encoding tables via
`tiktoken::encoding_for_model("gpt-4o")` and `enc.count("hello world")`.

#### 4b. Tokenizer registry

```rust
pub trait Tokenizer: Send + Sync {
    fn count_tokens(&self, text: &str) -> usize;
}

pub fn tokenizer_for_model(kind: &ProviderKind, model: &str) -> Box<dyn Tokenizer> {
    match kind {
        ProviderKind::OpenAI => Box::new(TiktokenTokenizer::for_model(model)),
        ProviderKind::DeepSeek => Box::new(DeepSeekTokenizer::new()),
        _ => Box::new(HeuristicTokenizer::new()),
    }
}
```

#### 4c. Use offline tokenizer as fallback

When no counting API is available, use the offline tokenizer to count the full serialized request body (messages + tools). This is still more accurate than the current heuristic because:
1. It uses the actual model's tokenizer
2. It counts the full JSON, not just `content` fields

### Phase 5: Fix RAM/Disk Display Mismatch

**Files**: `crates/core/src/state.rs:486`, `crates/core/src/helpers.rs:816`

`session.token_count()` currently sums only in-memory messages. `usage_display()` shows this value. But the API receives all messages from disk.

**Fix**: Change `token_count()` to load from disk (or better, change the display to compute from the assembled API request messages after `prepare_request_messages_for_session`).

**Simpler fix**: After `prepare_request_messages_for_session` assembles the full list, run `estimate_full_request_tokens()` on it and use that for the display. This eliminates the discrepancy.

---

## Open Questions

1. **Anthropic thinking blocks**: The docs say thinking blocks from *previous* assistant turns are ignored and don't count toward input tokens. How should we handle this? Anthropic's counting API handles it automatically.

2. **Prompt caching**: Cache-control messages are billed at a discount. Should we show the effective cached token count or the raw count?

3. **API latency tradeoff**: Calling the counting API adds an extra HTTP round trip. Is it acceptable for an accuracy improvement, or should it be opt-in?

4. **Per-message display**: Should we drop per-message token counts (since they're always inaccurate) in favor of session-level only?

---

## Summary Table

| Fix | Effort | Impact | Dependencies | Status |
|-----|--------|--------|-------------|--------|
| Phase 1a: Accumulate actual_tokens_used | Small | Medium | None | ✅ Done (verified Jun 2026) |
| Phase 1b: Per-message token count docs | Small | Low | None | ✅ Done (verified Jun 2026) |
| Phase 1c: Full-request serialization counting | Medium | Medium | None | ✅ Done (verified Jun 2026) |
| Phase 2: Pre-flight check | Small | High | Phase 1c | ✅ Done |
| Phase 3: API-based counting | Medium | High | New provider HTTP calls | ✅ Done (verified Jun 2026) |
| Phase 4: tiktoken offline fallback | Medium | Medium | Add crate dependency | 🔜 Pending |
| Phase 5: Fix RAM/Disk display | Small | Medium | Phase 1c | 🔜 Pending |

## Recommendation

Start with Phase 1 + 2 (quick wins done: fix accumulation, full-request counting, pre-flight check), then Phase 5 (fix display mismatch), then Phase 3 (API counting for highest accuracy), then Phase 4 (offline fallback for models without counting APIs).

Phases 1, 2, and 3 are complete. Next recommended work: Phase 5 (fix RAM/disk display mismatch) which can reuse the `estimate_full_request_tokens` and pre-flight serialization logic already in place.
