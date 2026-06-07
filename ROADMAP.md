# AutoCode Roadmap

## Current Status

Phase 5 of the planned improvements is in progress. The project has been
substantially cleaned up and stabilized.

## Completed Tasks

### Phase 1 — Token Counting (complete)
- Pre-flight context check before API request
- Token counting accumulation and full-request estimation
- API-based token counting
- Offline tiktoken tokenizer fallback
- RAM/Disk display mismatch fix

### Phase 2 — Stability & Race Conditions (complete)
- TOCTOU, PID deadlock, temp-file collision, on_exit yield fixes
- Orphaned tool_calls messages prevention
- Persist orphan cleanup to disk to prevent infinite error loop

### Phase 3 — Redundancies & Best Practices (complete)
- Removed redundant code across all crates
- Fixed all clippy warnings
- Rust 1.95 edition 2024 conformance

### Phase 4 — Dead Code & Cleanup (complete)
- Removed dead code: `session_token_usage` in `crates/core/src/helpers.rs`
- Removed dead code: `prepare_request_messages` in `crates/ai/src/session.rs`
- Removed dead code: `update` in `crates/ai/src/chat.rs`
- Fixed `collapsible_if` clippy warning in `crates/core/src/helpers.rs`
- Fixed `redundant_closure` clippy warning in `crates/core/src/state.rs`

### Phase 5 — Upcoming / In Progress

The following areas are identified for future work:

1. **Session Management Improvements**
   - Consider deduplicating `parse_todo_from_tool_args` (defined in both
     `ai/src/helpers.rs` and used inline in `chat.rs`)
   - Review `ToolChoice::None` and `ToolChoice::Required` variants for
     potential use in tool control

2. **Testing Coverage**
   - Unit tests exist for regex engine and token estimation (35 tests)
   - Need integration tests for tool execution and provider interaction
   - UI testing is currently absent

3. **Security Hardening**
   - API key heap-zeroing is in place
   - Path traversal detection is implemented
   - Consider adding rate limiting configuration UI

4. **Provider Support**
   - OpenRouter, NVIDIA NIM, OpenAI-compatible endpoints supported
   - Per-model manifests for context windows, output limits, reasoning support
   - Consider adding additional providers (Anthropic direct, Google AI)

5. **Documentation**
   - README and Structure.md are current
   - Consider adding ARCHITECTURE.md for deeper design decisions
   - Add crate-level doc comments

## Guiding Principles

- Minimize RAM usage and binary size
- Keep codebase clean, organized, maintainable
- Essential features only, no bloat
- Prefer `std`; minimize deps
