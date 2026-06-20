# AutoCode Skills Roadmap

## Purpose
This document instructs the AutoCode agent to create a comprehensive library of general-purpose
agentic coding skills. These skills cover the tasks a coding agent most commonly encounters
across any software project in any language.

Create each skill as a single flat file at `skills/<skill_name>.md`. No subfolders.

---

## Skill File Format (REQUIRED — every skill must follow this exactly)

```markdown
---
name: skill-name
description: <2–4 sentences. This is what get_skills scans to decide whether to load
             this file. State clearly WHEN to trigger (keywords, task types, contexts)
             AND what the skill provides. Be specific. Slightly pushy — list the exact
             situations that should cause this skill to be loaded.>
---

# Skill Title

## Overview
<1–2 paragraphs: what this skill covers and the core principles to follow.>

## [Sections with concrete guidance, decision trees, examples, anti-patterns, checklists]
```

**Rules:**
- YAML frontmatter is mandatory. `name` and `description` are required.
- The first 2KB (frontmatter + overview) must orient the agent and confirm relevance.
- Body content must be concrete and actionable. No generic filler.
- Cross-reference other skills rather than duplicating content.
- Target length: 150–300 lines. Focused, not exhaustive.

---

## Skills To Create

Work through this list in order. Write each file completely, confirm it was written, then move on.

---

### 1. `skills/codebase_orientation.md`
**Description:** Use at the start of any task in an unfamiliar or partially-known codebase,
or when asked to find where something is implemented, understand a module's structure, or
trace a data flow. Load before exploring any project you haven't fully read yet.

**Cover:**
- Orientation sequence: entry point → dependency manifest → directory structure → key modules
- Reading manifests: package.json, Cargo.toml, pyproject.toml, go.mod — what to extract
- Finding entry points: main files, index files, app bootstrapping
- Tracing a data flow: type/struct definition → constructors → callers → output
- Efficient reading: scan public interfaces before reading implementations
- When to stop exploring and start implementing

---

### 2. `skills/task_decomposition.md`
**Description:** Use at the start of any non-trivial task before writing a single line of
code. Covers breaking a feature request or bug fix into ordered, verifiable steps that each
leave the codebase in a working state. Load this whenever a task has more than one moving
part or will touch more than two files.

**Cover:**
- Decomposition approach: desired end state → what has to change → in what order
- Bottom-up ordering: data/types first, then logic, then UI/API surface
- Identifying blockers and dependencies between steps
- Writing each step as a verifiable outcome, not a vague action
- Spike vs. implementation: when to prototype before committing
- Definition of done: what does "this task is complete" mean concretely

---

### 3. `skills/file_editing_strategy.md`
**Description:** Use whenever editing an existing file — deciding between surgical patching,
line-range replacement, or full rewrite. Covers how to write reliable old_text for find-replace
operations, when each editing strategy is appropriate, and how to verify edits landed correctly.
Load before any file modification.

**Cover:**
- Decision tree: patch (surgical) vs. line-range replace vs. full overwrite
- Writing reliable old_text: sufficient context, avoid trailing whitespace traps, CRLF awareness
- Full rewrite criteria: file under ~150 lines OR more than 60% changing
- Always verify: read the file after editing, check for double-application
- Handling edit failures: fallback strategies, reading fresh before retrying
- Keeping the codebase in a buildable state after each edit

---

### 4. `skills/shell_usage.md`
**Description:** Use before running any shell command — for builds, installs, file operations,
git, process management, or system inspection. Covers reliable command patterns for both
Windows (cmd/PowerShell) and Unix (bash/sh), output parsing, exit code handling, and safe
command construction. Load this for any non-trivial shell invocation.

**Cover:**
- Detecting the OS and shell before branching command logic
- Build commands across ecosystems: npm/yarn, pip/uv, cargo, go, make
- Git operations: safe read-only vs. mutating, status/diff/log/add/commit/push
- Parsing output: trim, split lines, handle empty or error output
- Exit code checking: non-zero meaning, how to distinguish error types
- Long-running commands: timeouts, background execution, streaming output
- Safe destructive operations: confirm before delete/overwrite

---

### 5. `skills/dependency_management.md`
**Description:** Use when adding, removing, or upgrading dependencies in any project — npm,
pip, cargo, go modules, or others. Covers how to find the right package, check compatibility,
install correctly, and handle lock files. Load when any task involves a library that isn't
already in the project.

**Cover:**
- Finding packages: official registries, evaluating quality (downloads, maintenance, license)
- Installing correctly: dev vs. prod dependencies, version pinning strategies
- Lock files: when to commit them, when not to, how to regenerate
- Compatibility checks: language/runtime version constraints, peer dependencies
- Auditing for security issues: `npm audit`, `cargo audit`, `pip-audit`
- Removing unused dependencies cleanly
- Vendoring vs. registry dependencies

---

### 6. `skills/debugging_workflow.md`
**Description:** Use when a build fails, a test fails, a command returns an unexpected error,
or behavior doesn't match expectations. Covers systematic debugging: reading error messages,
forming hypotheses, isolating the problem, and verifying the fix. Load whenever something
isn't working and the cause isn't immediately obvious.

**Cover:**
- Reading error messages: structure of compiler errors, runtime errors, stack traces
- Hypothesis-driven debugging: observe → hypothesize → test → conclude
- Isolation: minimal reproduction, binary search through changes, comment-out technique
- Adding instrumentation: print/log statements, debug builds, verbose flags
- Common failure categories: type errors, missing dependencies, env issues, logic bugs
- Verifying the fix: confirm the original failure is gone and nothing new broke
- When to stop and ask rather than keep guessing

---

### 7. `skills/writing_tests.md`
**Description:** Use when writing unit tests, integration tests, or end-to-end tests in any
language. Covers test structure, naming, setup/teardown, assertion patterns, mocking, and
what makes a test actually useful. Load when asked to add tests, improve coverage, or debug
a failing test.

**Cover:**
- Test naming: `test_<function>_<scenario>_<expected>` or equivalent convention
- Arrange-Act-Assert structure
- Unit vs. integration vs. e2e: what each tests, where each lives
- Setup and teardown: test fixtures, temp directories, database seeding
- Mocking and stubbing: when to mock, when mocks hide bugs
- Assertions: specific over generic, include helpful failure messages
- Testing for errors and edge cases, not just the happy path
- Running a single test vs. the full suite

---

### 8. `skills/code_refactoring.md`
**Description:** Use when asked to refactor, restructure, rename, deduplicate, or clean up
existing code without changing behavior. Covers safe refactoring sequences that keep code
compiling and passing tests at every step. Load before any refactoring task touching more
than one location.

**Cover:**
- Refactoring sequence: understand → verify tests exist → smallest change → verify → repeat
- Rename workflow: find all usages first, update definition then callers
- Extract function/method: identify the seam, handle return values and parameters
- Split file/module: move types, update imports, fix references
- Interface changes: add new → migrate callers → remove old (never swap in one step)
- Keeping it compiling at each step: check before moving on
- When NOT to refactor: mid-feature, no tests, unclear requirements

---

### 9. `skills/api_integration.md`
**Description:** Use when integrating with any external REST, GraphQL, or WebSocket API —
reading documentation, constructing requests, handling auth, parsing responses, and managing
errors. Load when a task involves calling an external service or implementing a client for one.

**Cover:**
- Reading API docs: endpoints, auth methods, request/response schemas, rate limits
- Auth patterns: API keys, Bearer tokens, OAuth2, cookie-based
- Request construction: headers, query params, request body, content-type
- Response handling: status codes, parsing JSON/XML, extracting fields
- Error handling: 4xx vs. 5xx, retry logic, exponential backoff
- Rate limiting: detecting 429, respecting Retry-After, request queuing
- Testing API integrations: record/replay, mock servers, real sandbox environments

---

### 10. `skills/database_patterns.md`
**Description:** Use when working with any database — SQL or NoSQL — including schema design,
writing queries, migrations, indexing, and ORM usage. Load when a task involves reading from
or writing to a database, designing a schema, or debugging a slow or incorrect query.

**Cover:**
- Schema design principles: normalization, naming conventions, nullable vs. required
- Query patterns: SELECT with joins, filtering, ordering, pagination (LIMIT/OFFSET vs. cursor)
- Mutations: INSERT, UPDATE, DELETE with safe parameterization (never string interpolation)
- Migrations: forward-only, idempotent, tested before applying to production
- Indexing: when to add an index, composite indexes, avoiding over-indexing
- ORM vs. raw SQL: when each is appropriate
- Transactions: when to use, isolation levels, avoiding deadlocks
- Debugging slow queries: EXPLAIN/EXPLAIN ANALYZE, missing indexes

---

### 11. `skills/web_research.md`
**Description:** Use when a task requires looking up documentation, finding a library,
checking an API spec, or researching a solution before implementing it. Covers effective
search query construction, evaluating sources, fetching and extracting relevant content,
and synthesizing findings into a decision or implementation plan. Load before any web_search
or fetch_url call on a research task.

**Cover:**
- Query construction: specific over generic, include language/framework/version
- Source quality hierarchy: official docs > source code > reputable tutorials > forums
- Fetching docs pages: how to navigate to the right section, ignore nav chrome
- Extracting signal: focus on code examples and parameter descriptions
- Evaluating multiple sources when they conflict
- Synthesizing: write a summary of findings before writing code
- Knowing when you have enough information to proceed

---

### 12. `skills/git_workflows.md`
**Description:** Use for any git operation — committing, branching, merging, rebasing,
resolving conflicts, reading history, or understanding what changed. Load when a task
involves version control operations or when you need to understand the state of the repo
before making changes.

**Cover:**
- Reading state: `git status`, `git diff`, `git log`, `git blame`
- Committing: staging selectively, writing useful commit messages (what + why)
- Branching: creating, switching, tracking remote branches
- Merging vs. rebasing: when to use each, how to do it safely
- Conflict resolution: reading conflict markers, choosing or combining both sides
- Undoing things: `git restore`, `git reset`, `git revert` — which is safe when
- Stashing: save/restore work in progress
- Reading history to understand why code is the way it is

---

### 13. `skills/environment_and_config.md`
**Description:** Use when dealing with environment variables, configuration files, secrets,
.env files, or differences between dev/staging/production environments. Load when a task
involves setting up an environment, configuring a service, or debugging a config-related
failure.

**Cover:**
- Environment variable patterns: reading, defaulting, required vs. optional
- .env files: format, loading libraries per language, never committing secrets
- Config file formats: JSON, YAML, TOML, INI — when to use each
- Environment parity: keeping dev/staging/prod as similar as possible
- Secrets management: env vars vs. secret managers, rotation
- Debugging config issues: print effective config at startup, validate on load
- Feature flags via config

---

### 14. `skills/performance_profiling.md`
**Description:** Use when a task involves making something faster, diagnosing high CPU or
memory usage, reducing latency, or understanding why something is slow. Covers profiling
methodology, common bottlenecks across languages, and how to measure before and after a
change. Load before making any optimization without first measuring.

**Cover:**
- Measure first: never optimize without a baseline
- Profiling tools by language: perf/flamegraph, py-spy, node --inspect, cargo flamegraph
- Reading profiles: hot paths, cumulative vs. self time, call stacks
- Common bottlenecks: N+1 queries, unnecessary allocations, synchronous I/O, serialization
- Big-O awareness: when algorithmic complexity is the issue
- Memory profiling: heap usage, leak detection, allocation hotspots
- Benchmarking: isolated micro-benchmarks vs. end-to-end, statistical significance
- Verifying improvement: measure after, confirm the bottleneck moved

---

### 15. `skills/error_handling_design.md`
**Description:** Use when designing or implementing error handling in any language — deciding
what errors to define, how to propagate them, what to surface to users, and how to log them.
Load when implementing a new module, designing an API boundary, or cleaning up inconsistent
error handling in existing code.

**Cover:**
- Error categories: expected (recoverable) vs. unexpected (bugs), transient vs. permanent
- Error types: when to define custom types vs. use strings vs. generic error wrappers
- Propagation: bubbling up vs. handling at the site vs. converting at boundaries
- User-facing errors: what to say, what not to expose (stack traces, internal paths)
- Logging: what to log, at what level, include enough context to diagnose
- Retry logic: which errors are retriable, backoff strategy
- Failing fast vs. degraded operation: how to decide

---

### 16. `skills/code_review_checklist.md`
**Description:** Use when reviewing code — your own before committing or someone else's in
a PR. Covers what to look for: correctness, security, performance, readability, test coverage,
and edge cases. Load when asked to review code, audit a file for issues, or do a final check
before marking a task complete.

**Cover:**
- Correctness: does it do what it's supposed to, are edge cases handled
- Security: input validation, injection risks, credential handling, path traversal
- Error handling: are all failure paths handled, are errors surfaced correctly
- Performance: obvious inefficiencies, N+1, unnecessary work in hot paths
- Readability: naming, function length, single responsibility, comments where needed
- Test coverage: are the important paths tested, are tests meaningful
- API/interface design: is the public surface minimal and clear
- Things to always flag: hardcoded secrets, `.unwrap()`/unchecked nulls, TODO left in

---

### 17. `skills/documentation_writing.md`
**Description:** Use when writing or updating documentation — README files, inline code
comments, API docs, changelogs, or architecture decision records. Load when asked to document
something, add comments to code, or write a README for a project.

**Cover:**
- README structure: what it is, how to install, how to run, how to contribute
- Inline comments: comment the why, not the what; comment non-obvious decisions
- Docstrings/doc comments: format per language (JSDoc, rustdoc, docstring), what to include
- API documentation: parameters, return values, errors, examples
- Changelogs: what goes in, how to structure entries (Keep a Changelog format)
- Architecture decision records: when to write one, what to include
- Keeping docs in sync with code: what to update when changing behavior

---

### 18. `skills/security_basics.md`
**Description:** Use when implementing any feature that handles user input, authentication,
file paths, credentials, external data, or network requests. Covers the most common security
mistakes in application code and how to avoid them. Load before implementing auth, file
handling, user input processing, or any external-facing interface.

**Cover:**
- Input validation: validate at the boundary, whitelist over blacklist
- Injection: SQL injection, shell injection, path traversal — parameterize everything
- Authentication: never roll your own crypto, use established libraries
- Secrets: never hardcode, never log, never put in URLs or error messages
- File handling: validate paths, restrict to expected directories, check file types
- Dependencies: known-vulnerable packages, supply chain basics
- Least privilege: request only the permissions actually needed
- HTTPS: always for external communication, certificate validation

---

### 19. `skills/long_running_task_management.md`
**Description:** Use when a task is large enough to span multiple sessions, risk hitting
context limits, or require tracking progress across many steps. Covers how to structure
long work, checkpoint progress, write handoff notes, and resume without losing state.
Load at the start of any task estimated to require significant back-and-forth or many file changes.

**Cover:**
- Estimating task size before starting: file count, step count, unknowns
- Checkpointing: each step should leave the codebase in a working/buildable state
- Progress tracking: maintain a running task list, mark steps complete as you go
- Handoff notes: what to write so a fresh session can resume without re-reading everything
- Resuming: read handoff notes, verify last checkpoint, confirm next step before acting
- Avoiding wasted work: commit or save state before risky operations
- Communicating blockers: when to stop and ask vs. make a reasonable decision and proceed

---

### 20. `skills/language_specific_conventions.md`
**Description:** Use when starting work in a specific language to recall its conventions,
project structure norms, formatting standards, and ecosystem defaults. Covers the most
common languages: Python, JavaScript/TypeScript, Rust, Go, Java/Kotlin, and Ruby.
Load when beginning work in a language you haven't touched yet in the current task.

**Cover:**
- Python: PEP8, type hints, virtual environments, project layout (src layout vs flat)
- JavaScript/TypeScript: ESLint, Prettier, module systems (ESM vs CJS), tsconfig basics
- Rust: rustfmt, clippy, edition conventions, module system (mod declarations)
- Go: gofmt, package naming, error handling idiom, project layout
- Java/Kotlin: Maven/Gradle structure, naming conventions, null safety (Kotlin)
- Ruby: RuboCop, Bundler, convention-over-configuration (Rails) vs plain Ruby
- Cross-language: always check for an existing linter/formatter config before writing code

---

---

### 21. `skills/rest_api_design.md`
**Description:** Use when designing or implementing a REST API — defining routes, request/
response shapes, status codes, versioning, authentication, and pagination. Load when asked
to build an API, add endpoints, or review an existing API design for correctness.

**Cover:**
- Resource naming: nouns not verbs, plural collections, nested vs. flat routes
- HTTP method semantics: GET/POST/PUT/PATCH/DELETE — what each means and when to use it
- Status codes: 200/201/204/400/401/403/404/409/422/429/500 — the ones that matter and why
- Request/response shape: consistent envelope vs. bare resource, error response structure
- Versioning strategies: URL prefix vs. header, when to version
- Pagination: cursor-based vs. offset, response envelope fields
- Auth: where credentials go (Authorization header), token validation flow
- Idempotency: which methods must be idempotent and how to implement it

---

### 22. `skills/data_modeling.md`
**Description:** Use when designing data structures, types, schemas, or domain models for
any application — deciding how to represent entities, relationships, and state in code or
a database. Load when starting a new feature that introduces new data, or when refactoring
messy or unclear data structures.

**Cover:**
- Identifying entities, value objects, and aggregates
- Choosing representation: struct/class vs. map/dict vs. database row vs. JSON blob
- Relationships: one-to-one, one-to-many, many-to-many — and when to denormalize
- Nullability: what null means, when to use Option/Maybe vs. sentinel values vs. required
- Immutability: when to make data immutable, benefits for concurrency and correctness
- Validation at the boundary: parse-don't-validate pattern
- Versioning data models: how to evolve them without breaking existing data
- Naming: clear, domain-accurate names over technical abbreviations

---

### 23. `skills/logging_and_observability.md`
**Description:** Use when adding logging, metrics, tracing, or any observability to an
application. Load when asked to add logs, debug a production issue with insufficient
visibility, instrument a service, or set up structured logging.

**Cover:**
- Log levels: DEBUG/INFO/WARN/ERROR — what belongs at each level
- Structured logging: key-value pairs over string interpolation, JSON output
- What to log: request/response boundaries, errors with context, slow operations
- What NOT to log: passwords, tokens, PII, full request bodies by default
- Correlation IDs: threading a request ID through the call chain
- Metrics: counters, gauges, histograms — what each is for
- Distributed tracing basics: spans, trace IDs, parent-child relationships
- Avoiding log spam: rate limiting noisy logs, sampling in high-throughput paths

---

### 24. `skills/docker_and_containers.md`
**Description:** Use when writing Dockerfiles, docker-compose files, or working with
containerized applications — building images, managing services, debugging container
issues, or optimizing image size. Load when any task involves Docker, containers,
or containerized deployment.

**Cover:**
- Dockerfile best practices: layer ordering, cache efficiency, multi-stage builds
- Base image selection: official images, slim vs. full, pinning versions
- .dockerignore: what to exclude, why it matters for build context size
- docker-compose: service definitions, depends_on, volumes, networking, env vars
- Common commands: build, run, exec, logs, ps, inspect — when to use each
- Debugging containers: exec into running container, read logs, inspect mounts
- Image size reduction: multi-stage builds, removing build deps, using alpine
- Environment parity: making containers behave consistently across dev/CI/prod

---

### 25. `skills/ci_cd_pipelines.md`
**Description:** Use when writing or editing CI/CD pipeline configuration — GitHub Actions,
GitLab CI, CircleCI, or similar. Covers job structure, caching, secrets, test/build/deploy
stages, and common failure patterns. Load when asked to set up, fix, or improve a pipeline.

**Cover:**
- Pipeline structure: triggers, jobs, steps, artifacts, dependencies between jobs
- GitHub Actions specifics: workflow syntax, actions marketplace, reusable workflows
- Caching: dependency caches (node_modules, .cargo, pip), cache key strategies
- Secrets: how to inject them, never echo them, least-privilege tokens
- Test stages: run tests in CI the same way they run locally
- Build and artifact stages: what to produce, where to store it
- Deploy stages: environment promotion, manual approval gates, rollback triggers
- Common failures: flaky tests, cache invalidation, env differences, permission errors

---

### 26. `skills/regex_patterns.md`
**Description:** Use when writing, reading, or debugging regular expressions for any purpose —
input validation, parsing, search/replace, log analysis, or code search. Load when a task
involves regex construction, explaining what a regex does, or fixing a broken pattern.

**Cover:**
- Core syntax: character classes, quantifiers, anchors, groups, alternation
- Greedy vs. lazy quantifiers: `*` vs. `*?` and when it matters
- Capture groups vs. non-capturing groups: `()` vs. `(?:)`
- Lookahead and lookbehind: `(?=...)`, `(?!...)`, `(?<=...)`, `(?<!...)`
- Common patterns: email, URL, IP address, version numbers, file paths, dates
- Flags: case-insensitive, multiline, dotall — what each does
- Testing regexes: tools (regex101), unit testing patterns
- Performance pitfalls: catastrophic backtracking, when to use a parser instead

---

### 27. `skills/data_migration.md`
**Description:** Use when migrating data between schemas, formats, systems, or storage
backends — including database migrations, ETL scripts, file format conversions, and
API-to-API data moves. Load when a task involves transforming or moving existing data.

**Cover:**
- Migration principles: idempotent, reversible where possible, testable
- Schema migrations: forward-only vs. up/down, zero-downtime strategies
- ETL pattern: extract cleanly → transform with validation → load atomically
- Handling bad data: reject vs. coerce vs. skip — log all three
- Batching: never migrate all at once, use chunks, track progress
- Dry run first: verify row counts and spot-check output before committing
- Rollback plan: what happens if something goes wrong mid-migration
- Post-migration verification: row counts, sample checks, application smoke test

---

### 28. `skills/cli_tool_design.md`
**Description:** Use when building a command-line tool or script — argument parsing,
help text, exit codes, output formatting, stdin/stdout/stderr usage, and making the
tool composable with other tools. Load when asked to build a CLI, add subcommands,
or improve an existing CLI's usability.

**Cover:**
- Argument design: positional vs. flags, short vs. long flags, required vs. optional
- Help text: every flag documented, examples in --help output
- Exit codes: 0 for success, non-zero for failure, consistent meanings
- Output: stdout for data, stderr for logs/errors, machine-readable option (--json)
- Stdin: reading piped input, detecting TTY vs. pipe
- Subcommand pattern: when to use it, how to structure help for subcommands
- Composability: play well with pipes, grep, xargs, shell scripting
- Configuration: flag > env var > config file precedence order

---

### 29. `skills/websocket_and_realtime.md`
**Description:** Use when implementing WebSocket connections, server-sent events, long
polling, or any real-time communication between client and server. Load when a task
involves live updates, push notifications, chat, streaming data, or any persistent
connection between client and server.

**Cover:**
- WebSocket vs. SSE vs. long polling: when to use each
- Connection lifecycle: open, message, error, close — handle all four
- Reconnection: exponential backoff, jitter, max attempts
- Message framing: JSON envelopes, message types, sequence numbers
- Authentication: how to auth a WebSocket (initial HTTP handshake)
- Heartbeat/ping-pong: detecting dead connections
- Scaling: sticky sessions, pub/sub backends (Redis), horizontal scaling constraints
- Server-sent events: simpler than WebSocket, one-way, HTTP/2 friendly

---

### 30. `skills/file_format_handling.md`
**Description:** Use when reading, writing, parsing, or generating structured file formats —
JSON, CSV, YAML, TOML, XML, Markdown, or binary formats. Load when a task involves
processing files in a specific format, converting between formats, or handling malformed input.

**Cover:**
- JSON: parsing, serialization, schema validation, handling nulls and missing keys
- CSV: delimiter detection, quoting rules, header rows, encoding issues
- YAML: anchors and aliases, multiline strings, type coercion gotchas
- TOML: when to prefer it over YAML, key types, array of tables
- XML: element vs. attribute, namespaces, XPath basics, prefer libraries over manual parsing
- Binary formats: endianness, fixed vs. variable length fields, magic bytes
- Handling malformed input: validate before processing, clear error messages with position
- Large files: stream parsing over loading into memory, chunk processing

---

### 31. `skills/authentication_and_authorization.md`
**Description:** Use when implementing login, session management, token handling, role-based
access, or any system that controls who can do what. Load when a task involves auth flows,
JWT tokens, sessions, permissions, or access control logic.

**Cover:**
- AuthN vs. AuthZ: authentication (who are you) vs. authorization (what can you do)
- Password handling: bcrypt/argon2, never store plain or MD5/SHA1, salting
- Session-based auth: session tokens, cookie flags (HttpOnly, Secure, SameSite)
- JWT: structure, signing vs. encryption, validation, expiry, refresh tokens
- OAuth2 / OIDC: flows (authorization code, client credentials), when to use a library
- RBAC basics: roles, permissions, checking at the right layer
- Common mistakes: authorization checked only in UI, tokens without expiry, broad scopes
- Logout: invalidating sessions/tokens, clearing cookies, revoking refresh tokens

---

### 32. `skills/caching_strategies.md`
**Description:** Use when adding caching to improve performance, reduce external calls, or
handle rate limits — in-memory, distributed (Redis/Memcached), HTTP caching, or database
query caching. Load when a task involves caching data, setting TTLs, or debugging stale data.

**Cover:**
- Cache placement: client, CDN, API gateway, application, database query cache
- Cache-aside vs. read-through vs. write-through vs. write-behind
- TTL strategy: how long is too long, when to use no expiry, sliding vs. fixed TTL
- Cache keys: what makes a good key, namespacing, versioning keys on schema change
- Invalidation: the hard problem — event-driven vs. TTL-only vs. cache bust on write
- Cache stampede: thundering herd problem and mitigations (locking, probabilistic early expiry)
- What not to cache: user-specific sensitive data, rapidly changing data, large blobs
- Redis patterns: strings, hashes, sorted sets, pub/sub — when each fits

---

### 33. `skills/concurrency_patterns.md`
**Description:** Use when implementing concurrent or parallel code in any language —
threads, async/await, worker pools, queues, or any shared state across execution contexts.
Load when a task involves background work, parallel processing, race conditions, or
synchronization primitives.

**Cover:**
- Concurrency vs. parallelism: I/O-bound (concurrency wins) vs. CPU-bound (parallelism wins)
- Thread safety: shared mutable state, locks, atomics, immutability as the safest option
- Async/await: the event loop model, what blocks vs. what yields, avoid blocking in async
- Worker pool pattern: bounded concurrency, queue depth, backpressure
- Producer/consumer: channel-based decoupling, buffer sizing
- Race conditions: how they happen, how to detect them, how to prevent them
- Deadlocks: lock ordering, avoiding nested locks, lock timeouts
- Language specifics: Python GIL implications, JS single-thread model, Go goroutines

---

### 34. `skills/memory_management.md`
**Description:** Use when diagnosing memory leaks, high memory usage, or implementing
code in languages with manual or complex memory management. Load when a task involves
memory optimization, fixing leaks, understanding ownership, or working in C/C++/Rust.

**Cover:**
- Stack vs. heap: what lives where, allocation cost differences
- Memory leaks: common causes (event listeners, circular references, forgotten caches)
- Garbage collected languages: how GC works, generational collection, GC pressure
- Reference counting: cycles, weak references, when refcount doesn't free memory
- Ownership and borrowing (Rust-style): move semantics, borrow rules, lifetimes conceptually
- Buffer management: fixed vs. dynamic buffers, pre-allocation, pooling
- Detecting leaks: heap profilers, growth over time, resident set size monitoring
- Large data: streaming over loading, generators/iterators, chunked processing

---

### 35. `skills/search_and_filtering.md`
**Description:** Use when implementing search, filtering, sorting, or querying across
collections of data — in-memory, database, or full-text search engines. Load when asked
to add search functionality, implement filters, or optimize a slow query/search.

**Cover:**
- In-memory filtering: predicate functions, early termination, indexed lookups
- Database filtering: WHERE clauses, index usage, avoiding full table scans
- Full-text search: LIKE vs. full-text indexes vs. dedicated engines (Elasticsearch, Typesense)
- Fuzzy search: edit distance, trigrams, phonetic matching — when to use each
- Sorting: stable sort, multi-field sort, case-insensitive string sort
- Pagination with filtering: cursor-based pagination with filter/sort stability
- Autocomplete: prefix indexes, debouncing on the client side
- Relevance ranking: boolean vs. scored results, boosting fields

---

### 36. `skills/background_jobs_and_queues.md`
**Description:** Use when implementing background job processing, task queues, scheduled
jobs, or async work that runs outside the request/response cycle. Load when a task involves
offloading work to a queue, scheduling recurring jobs, or debugging stuck or failed jobs.

**Cover:**
- When to use a queue: slow operations, retryable work, decoupling producers from consumers
- Job queue options: Redis-backed (BullMQ, Sidekiq, RQ), database-backed, cloud (SQS)
- Job design: idempotent jobs, serializable payloads, no closures/live objects
- Retry strategy: max attempts, exponential backoff, dead letter queues
- Scheduled jobs (cron): cron syntax, at-least-once vs. exactly-once semantics
- Concurrency: worker count, job locking to prevent duplicate processing
- Monitoring: queue depth, job failure rate, stuck job detection
- Graceful shutdown: drain in-progress jobs before stopping workers

---

### 37. `skills/frontend_basics.md`
**Description:** Use when working on frontend code — HTML structure, CSS layout, JavaScript
DOM manipulation, event handling, form validation, or browser APIs. Load when a task involves
building or editing UI in a browser context, whether vanilla JS or a framework.

**Cover:**
- HTML semantics: use the right element, accessibility implications of div-soup
- CSS layout: flexbox vs. grid — when each is the right tool
- CSS specificity: how it works, why `!important` is a smell, BEM naming
- JS DOM: querySelector, event listeners, event delegation, avoiding memory leaks
- Forms: input types, validation (HTML5 built-in vs. JS), prevent default
- Async in the browser: fetch API, promise chains, async/await, error handling
- Browser storage: localStorage vs. sessionStorage vs. cookies vs. IndexedDB
- Common pitfalls: layout thrashing, synchronous XHR, blocking the main thread

---

### 38. `skills/react_patterns.md`
**Description:** Use when writing or debugging React code — components, hooks, state
management, effects, context, and performance optimization. Load when any task involves
React components, JSX, or React-specific patterns like useEffect or custom hooks.

**Cover:**
- Component design: single responsibility, props vs. state, controlled vs. uncontrolled
- Hooks: useState, useEffect, useRef, useCallback, useMemo — when each is appropriate
- useEffect pitfalls: missing dependencies, infinite loops, cleanup functions
- Custom hooks: extracting reusable stateful logic, naming conventions
- Context: when to use it, when it's overkill vs. when prop drilling is worse
- Performance: React.memo, useCallback, useMemo — measure before applying
- State management: local state vs. context vs. external store (Zustand, Redux)
- Common mistakes: state mutation, stale closures, key prop misuse

---

### 39. `skills/typescript_patterns.md`
**Description:** Use when writing TypeScript — typing functions, generics, utility types,
narrowing, and structuring types across a codebase. Load when a task involves TypeScript
type errors, designing types for a new feature, or improving type safety in existing code.

**Cover:**
- Type vs. interface: when to use each, extending vs. intersecting
- Generics: when they're needed, constraints, defaults, avoid over-generalizing
- Utility types: Partial, Required, Pick, Omit, Record, ReturnType, Parameters
- Narrowing: typeof, instanceof, discriminated unions, type predicates
- Unknown vs. any: use unknown for external data, never use any except as last resort
- Readonly and const assertions: immutability in the type system
- Module augmentation and declaration merging: when and why
- tsconfig settings that matter: strict, noUncheckedIndexedAccess, exactOptionalPropertyTypes

---

### 40. `skills/python_patterns.md`
**Description:** Use when writing Python code — idiomatic patterns, type annotations,
common stdlib usage, virtual environments, packaging, and Python-specific pitfalls.
Load when any task involves writing or refactoring Python code.

**Cover:**
- Pythonic idioms: list comprehensions, generators, context managers, unpacking
- Type annotations: basic types, Optional, Union, TypedDict, Protocol, generics
- Common stdlib: pathlib over os.path, dataclasses, itertools, functools, contextlib
- Virtual environments: venv vs. virtualenv vs. uv, activating, requirements files
- Error handling: specific exceptions over bare except, exception chaining
- Pitfalls: mutable default arguments, late binding closures, `is` vs. `==`
- Packaging: pyproject.toml, src layout, entry points
- Performance: generators over lists for large data, avoid quadratic string concat

---

### 41. `skills/go_patterns.md`
**Description:** Use when writing Go code — idiomatic patterns, error handling, interfaces,
goroutines, channels, and project layout. Load when any task involves writing, reviewing,
or debugging Go code.

**Cover:**
- Error handling: always check errors, wrapping with `fmt.Errorf("%w", err)`, sentinel errors
- Interfaces: small interfaces, implicit implementation, accept interfaces return structs
- Goroutines and channels: spawning safely, channel directions, select statement
- Defer: cleanup pattern, deferred in loops (don't), order of execution
- Structs and methods: value vs. pointer receivers, embedding over inheritance
- Project layout: cmd/, internal/, pkg/ conventions
- go.mod and go.sum: module management, replace directives
- Common mistakes: goroutine leaks, nil map writes, range loop variable capture

---

### 42. `skills/monorepo_management.md`
**Description:** Use when working in a monorepo — navigating multiple packages, running
scoped commands, managing shared dependencies, understanding build tool configuration
(Turborepo, Nx, Bazel, Cargo workspaces). Load when a task involves a repo with multiple
packages/apps or when changes span more than one package.

**Cover:**
- Monorepo structure: apps/, packages/, libs/ conventions
- Scoped commands: running build/test/lint for only the changed package
- Shared packages: internal libraries, versioning strategies (fixed vs. independent)
- Dependency graphs: understanding which packages depend on which, change impact
- Build tools: Turborepo/Nx task pipelines and caching, Cargo workspace basics
- Cross-package changes: update the library, update consumers, test both
- CI in a monorepo: only build/test what changed, affected package detection
- Common pitfalls: circular dependencies, shared config drift, version skew

---

### 43. `skills/webscraping.md`
**Description:** Use when extracting data from websites — fetching HTML, parsing structure,
handling pagination, dealing with JavaScript-rendered content, and respecting rate limits.
Load when a task involves scraping data from a website or automating web data extraction.

**Cover:**
- Fetch first: try fetch_url / HTTP GET before reaching for a headless browser
- HTML parsing: CSS selectors vs. XPath, finding stable selectors (avoid positional)
- Pagination: next-page links, offset params, infinite scroll detection
- JavaScript-rendered content: when you need a headless browser (Playwright, Puppeteer)
- Rate limiting: delay between requests, respect robots.txt, rotate user agents carefully
- Session and cookies: logging in, maintaining session across requests
- Data extraction: target the data you need, don't parse everything
- Fragility: scrapers break when sites change — build in error detection

---

### 44. `skills/json_and_data_serialization.md`
**Description:** Use when serializing, deserializing, transforming, or validating JSON or
other data interchange formats (MessagePack, Protocol Buffers, Avro). Load when a task
involves parsing API responses, writing serialization code, validating schemas, or
converting between data formats.

**Cover:**
- JSON parsing safely: handle missing keys, null values, wrong types defensively
- Schema validation: JSON Schema basics, validating at the boundary
- Serialization libraries by language: serde (Rust), Jackson (Java), pydantic (Python), Zod (TS)
- Handling large JSON: streaming parsers vs. loading all into memory
- Protocol Buffers: .proto files, generated code, backward compatibility rules
- Date/time serialization: always ISO 8601, always UTC, timezone pitfalls
- Floating point: precision loss, when to use decimal/string instead
- Versioning serialized formats: adding fields (safe), removing (breaking), renaming (breaking)

---

### 45. `skills/testing_strategies.md`
**Description:** Use when deciding what kind of tests to write, how much coverage is
enough, how to structure a test suite, or how to make tests that are actually maintainable.
Load when planning test coverage for a new feature or when a test suite is slow, brittle,
or hard to maintain.

**Cover:**
- Test pyramid: many unit, some integration, few e2e — and why
- What to unit test: pure functions, business logic, edge cases
- What to integration test: database interactions, external service boundaries
- What to e2e test: critical user paths, not implementation details
- Test doubles: when to stub, mock, fake, or use the real thing
- Brittle tests: testing implementation vs. behavior, snapshot tests gone wrong
- Test speed: slow tests don't get run — keep unit tests under 1ms each
- Coverage: 100% is not the goal, covering the important paths is
- Property-based testing: when it's worth it, what kinds of bugs it finds

---

### 46. `skills/infrastructure_as_code.md`
**Description:** Use when writing or modifying infrastructure configuration — Terraform,
Pulumi, CloudFormation, Ansible, or similar. Load when a task involves provisioning
cloud resources, managing infrastructure state, or automating environment setup.

**Cover:**
- IaC principles: declarative over imperative, idempotent, version controlled
- Terraform basics: providers, resources, variables, outputs, state
- State management: remote state (S3 + DynamoDB lock), never edit state manually
- Plan before apply: always review `terraform plan` output before `apply`
- Modules: when to extract a module, input/output variables, reuse
- Secrets in IaC: never hardcode, use secret stores (Vault, SSM Parameter Store)
- Drift detection: what happens when infra is changed outside IaC
- Destroying resources: what `terraform destroy` does, when it's safe

---

### 47. `skills/code_generation.md`
**Description:** Use when writing code that generates other code — templates, scaffolding
scripts, AST manipulation, codegen from schemas (OpenAPI, GraphQL, Protobuf), or macro
systems. Load when a task involves generating boilerplate, building a code generator,
or working with schema-driven code generation.

**Cover:**
- When codegen is the right answer: repetitive boilerplate that must stay in sync with a schema
- Template-based generation: text templates vs. AST-based — tradeoffs
- Schema-driven: OpenAPI → client/server stubs, Protobuf → typed messages, GraphQL → types
- Output hygiene: generated files should be clearly marked, not manually edited
- Determinism: same input must always produce same output (no timestamps, random IDs)
- Running codegen in CI: detect uncommitted generated changes and fail the build
- AST manipulation: when to parse and transform vs. when to use text templates
- Testing generators: test the output, not the generator internals

---

### 48. `skills/plugin_and_extension_systems.md`
**Description:** Use when designing or implementing a plugin system, extension API,
hook system, or any architecture where external code plugs into a core system. Load
when a task involves making a system extensible, writing a plugin for an existing system,
or designing an API for third-party extensions.

**Cover:**
- Plugin patterns: hooks/events, middleware chains, strategy pattern, dynamic loading
- Interface design: stable plugin APIs, versioning, backward compatibility obligations
- Discovery: file-based (scan a directory), registry-based, manifest-based
- Isolation: sandboxing plugins, limiting what they can access
- Lifecycle: load → initialize → run → teardown, error handling at each phase
- Extension points: identifying what to expose vs. what to keep internal
- Testing plugins: test the plugin against the real host, not a mock
- Documentation: plugin authors need examples, not just interface signatures

---

### 49. `skills/state_machine_design.md`
**Description:** Use when modeling something that has distinct states and transitions —
order workflows, connection lifecycles, UI flows, game states, or protocol implementations.
Load when a task involves entities that change state over time, or when if/else chains
for state logic are getting out of hand.

**Cover:**
- When a state machine is the right model: finite states, explicit transitions, guards
- States vs. transitions: define both completely before writing code
- Representing state machines: enum-based, table-driven, object-based (state pattern)
- Guards and actions: conditions on transitions, side effects on entry/exit/transition
- Invalid transitions: explicit rejection vs. silent ignore — always explicit
- Hierarchical state machines: when states contain sub-states
- Testing: cover every transition, every guard condition, every invalid input
- Visualization: draw the state diagram first, code second

---

### 50. `skills/date_and_time_handling.md`
**Description:** Use when working with dates, times, timestamps, timezones, durations,
or scheduling logic. Load when any task involves storing, displaying, calculating, or
comparing dates and times — timezone bugs are among the most common and painful bugs
in production.

**Cover:**
- Always store in UTC, convert to local only for display
- ISO 8601: the correct format for serialization and interchange
- Timezone handling: named zones (America/New_York) over offsets (+05:00), DST implications
- Libraries: don't use built-in date parsing in most languages — use a proper library
- Arithmetic: adding days vs. adding seconds — different things across DST boundaries
- Durations vs. instants: a "1 hour meeting" is a duration, "3pm Friday" is an instant
- Common bugs: assuming UTC when the system clock isn't, off-by-one on date ranges
- Testing: inject a clock interface, never call `now()` directly in business logic

---

### 51. `skills/ui_design_fundamentals.md`
**Description:** Use when building any user interface — web, desktop, or mobile — and
design decisions need to be made about layout, spacing, typography, color, or visual
hierarchy. Load when asked to make something look good, improve a UI, design a component,
or when starting a frontend task from scratch.

**Cover:**
- Visual hierarchy: size, weight, color, and spacing to guide the eye
- Spacing system: use a consistent scale (4px/8px base), never arbitrary values
- Typography: 2 fonts max, clear size scale, line-height and letter-spacing basics
- Color: primary/secondary/neutral/semantic (error/success/warning), contrast ratios for accessibility
- Layout: alignment, grid systems, whitespace as a design tool not an afterthought
- Consistency: reuse the same patterns — same border radius, same shadow, same button style
- Dark mode: design for both from the start, use semantic color tokens not hardcoded values
- Common mistakes: too many colors, inconsistent spacing, low contrast, center-aligning everything

---

### 52. `skills/component_design.md`
**Description:** Use when designing or building reusable UI components — deciding on props,
variants, states, and composition patterns. Load when building a component library, adding
a new component, or when an existing component is getting too complex or hard to reuse.

**Cover:**
- Single responsibility: one component, one job — split when it gets complicated
- Props API design: minimal surface, sensible defaults, avoid boolean prop explosion
- Variants: use a variant prop (`size`, `intent`) over many boolean flags
- States: every component has states — default, hover, focus, disabled, loading, error
- Composition over configuration: slots/children over deeply nested prop trees
- Accessibility: keyboard navigation, ARIA roles, focus management, screen reader text
- Naming: clear, consistent, domain-accurate — Button not Btn, Modal not Popup
- Documentation: show every variant and state with an example, not just the happy path

---

### 53. `skills/ux_principles.md`
**Description:** Use when making decisions about how a feature should behave from the
user's perspective — flows, feedback, error states, loading states, empty states, and
overall usability. Load when designing a new feature flow, reviewing UX, or when a user
complains something is confusing or hard to use.

**Cover:**
- Feedback: every action needs a response — loading, success, error, nothing is invisible
- Error messages: say what went wrong and what the user can do about it, not "Error 500"
- Empty states: don't show a blank screen — explain why it's empty and what to do
- Loading states: skeleton screens over spinners for layout-heavy content
- Progressive disclosure: show what's needed now, reveal complexity on demand
- Affordance: interactive things should look interactive, static things shouldn't
- Forgiveness: let users undo, confirm destructive actions, don't punish mistakes
- Consistency: same action should always look and work the same way across the app

---

### 54. `skills/accessibility.md`
**Description:** Use when building any UI that needs to be usable by people with disabilities
— keyboard navigation, screen readers, color contrast, focus management, and ARIA. Load
when asked to improve accessibility, fix a11y issues, or build any frontend component or page.

**Cover:**
- Semantic HTML: the right element does half the work (button vs div, nav vs div)
- Keyboard navigation: every interactive element reachable and operable by keyboard
- Focus management: visible focus ring, logical tab order, trap focus in modals
- Screen readers: alt text for images, labels for inputs, ARIA only when HTML isn't enough
- Color contrast: 4.5:1 for normal text, 3:1 for large text (WCAG AA minimum)
- Don't rely on color alone: use icons, text, or patterns alongside color to convey meaning
- ARIA: roles, states (`aria-expanded`, `aria-checked`), live regions for dynamic content
- Testing: keyboard-only walkthrough, screen reader spot check (NVDA/VoiceOver), axe DevTools

---

### 55. `skills/design_tokens.md`
**Description:** Use when setting up or working with a design token system — colors,
spacing, typography, shadows, and border radii defined as named variables shared across
code and design. Load when starting a new UI project, building a component library,
or when hardcoded values are creating inconsistency across a codebase.

**Cover:**
- What design tokens are: named constants for visual decisions, not magic numbers in CSS
- Token hierarchy: global tokens (raw values) → semantic tokens (purpose-named) → component tokens
- Naming: by purpose not value — `color.text.error` not `color.red.500`
- Format: CSS custom properties, JS/TS constants, JSON for multi-platform sharing
- Spacing scale: 4px base, powers of 2 or T-shirt sizing (xs/sm/md/lg/xl)
- Color system: primitive palette + semantic layer (background, surface, border, text, brand)
- Typography tokens: font family, size scale, weight, line height, letter spacing
- Keeping tokens in sync: single source of truth, generated from a tool (Figma Tokens, Style Dictionary)

---

### 56. `skills/prompt_engineering.md`
**Description:** Use when writing, improving, or debugging prompts for LLMs — system
prompts, user prompts, few-shot examples, chain-of-thought instructions, output format
constraints, or tool use instructions. Load when a task involves building an LLM-powered
feature, improving AI output quality, or designing a prompt for any model.

**Cover:**
- System vs. user prompt: what belongs where, how models weight each
- Clarity over cleverness: specific instructions outperform elaborate framing
- Few-shot examples: when they help, how many, format consistency matters
- Output format control: asking for JSON, XML, markdown — and validating it
- Chain-of-thought: when to ask the model to reason before answering
- Negative instructions: "do not" is weaker than "only do" — prefer positive framing
- Token efficiency: long prompts degrade performance, cut filler
- Iterating on prompts: change one thing at a time, test with varied inputs
- Prompt injection: how it happens, how to defend against user input in prompts

---

### 57. `skills/networking_fundamentals.md`
**Description:** Use when debugging connection issues, implementing network clients,
understanding latency, working with DNS, TLS, proxies, or any task that requires
understanding how data actually moves between machines. Load when a task involves
network errors, connection timeouts, certificate issues, or building anything that
communicates over a network.

**Cover:**
- TCP: connection lifecycle (SYN/ACK), why connections fail, TIME_WAIT
- DNS: resolution chain, TTL, common record types (A, CNAME, MX, TXT), propagation delays
- HTTP/1.1 vs HTTP/2 vs HTTP/3: key differences, multiplexing, when it matters
- TLS: handshake overview, certificate validation, common errors (expired, wrong host, self-signed)
- Ports and firewalls: well-known ports, how to diagnose blocked connections
- Proxies and load balancers: how they affect headers, IPs, and TLS termination
- Debugging tools: curl, ping, traceroute, netstat, dig, openssl s_client
- Latency vs. throughput: the difference, what affects each, how to measure

---

### 58. `skills/filesystem_operations.md`
**Description:** Use when working with the filesystem beyond simple read/write — file
permissions, symlinks, watching for changes, atomic writes, temp files, path manipulation,
and cross-platform path differences. Load when a task involves file permissions errors,
path handling bugs, file watching, or safe file update patterns.

**Cover:**
- Path manipulation: absolute vs. relative, joining paths safely, normalizing, no string concat
- Permissions: read/write/execute, Unix octal notation, chmod, chown, common permission errors
- Symlinks: hard vs. soft links, following vs. not following, detecting link vs. real file
- Atomic writes: write to temp → fsync → rename, why it matters for crash safety
- Temp files: create in system temp dir, always clean up, use libraries not manual paths
- File watching: inotify/FSEvents/ReadDirectoryChangesW, debouncing rapid events
- Globbing: platform differences, recursive glob, hidden files
- Cross-platform: path separators, case sensitivity (Windows insensitive, macOS sometimes, Linux always)

---

### 59. `skills/library_and_package_design.md`
**Description:** Use when designing or publishing a library meant for other developers to
consume — public API design, semver versioning, documentation, publishing to npm/PyPI/
crates.io, and maintaining backward compatibility. Load when building a reusable library,
adding a public API, or preparing a package for release.

**Cover:**
- Public API surface: expose the minimum needed, everything public is a commitment
- Semver: major (breaking) / minor (additive) / patch (fix) — be strict
- Breaking changes: what counts as breaking (removing, renaming, changing types/behavior)
- Backward compatibility: deprecation cycle before removal, maintain old signatures
- Documentation: every public symbol needs a doc comment with an example
- Publishing: package.json/pyproject.toml/Cargo.toml metadata, README, license
- Changelogs: Keep a Changelog format, tag releases in git
- Testing as a consumer: write tests that use your library the way users would

---

### 60. `skills/encryption_and_hashing.md`
**Description:** Use when implementing encryption, decryption, hashing, signing, or any
cryptographic operation. Load when a task involves storing sensitive data, verifying
integrity, generating tokens, handling certificates, or any situation where "secure"
matters in a cryptographic sense.

**Cover:**
- Hashing vs. encryption: hashing is one-way (integrity/passwords), encryption is two-way (confidentiality)
- Password hashing: bcrypt/argon2/scrypt only — never MD5, SHA1, or raw SHA256
- Symmetric encryption: AES-GCM for most use cases, key management is the hard part
- Asymmetric encryption: RSA/EC for key exchange and signing, not for bulk data
- Signing: HMAC for message authentication, RSA/EC signatures for non-repudiation
- Key management: generation, storage (never hardcode), rotation, derivation (PBKDF2/HKDF)
- Randomness: use cryptographically secure RNG, never Math.random() for secrets
- Common mistakes: ECB mode, rolling your own crypto, reusing nonces, weak key sizes

---

### 61. `skills/sql_advanced.md`
**Description:** Use when writing complex SQL — window functions, CTEs, subqueries,
query optimization, execution plans, and advanced aggregations. Load when a simple
query isn't enough, when a query is slow and needs optimization, or when asked to
implement reporting, ranking, or analytical queries.

**Cover:**
- CTEs: `WITH` clause, readability over subquery nesting, recursive CTEs for trees/graphs
- Window functions: `ROW_NUMBER`, `RANK`, `LAG`/`LEAD`, `SUM OVER`, `PARTITION BY`
- Subqueries: correlated vs. uncorrelated, when to use vs. JOIN vs. CTE
- Aggregations: GROUP BY gotchas, HAVING vs. WHERE, grouping sets
- Query execution plans: EXPLAIN / EXPLAIN ANALYZE output, seq scan vs. index scan
- Index strategies: covering indexes, partial indexes, expression indexes
- Set operations: UNION vs. UNION ALL, INTERSECT, EXCEPT
- Performance patterns: avoid SELECT *, avoid functions on indexed columns in WHERE

---

### 62. `skills/bash_scripting.md`
**Description:** Use when writing shell scripts — bash or sh — with conditionals, loops,
functions, argument parsing, error handling, and string manipulation. Load when a task
involves writing a .sh script, automating a multi-step shell workflow, or debugging
an existing shell script.

**Cover:**
- Shebang and portability: `#!/usr/bin/env bash` vs `#!/bin/sh`, what POSIX sh lacks
- Variables: quoting rules (always quote `"$var"`), local vs. global, arrays
- Conditionals: `[[ ]]` vs `[ ]`, string vs. integer comparison, file tests (`-f`, `-d`, `-z`)
- Loops: for/while/until, iterating files safely, `read` loop over command output
- Functions: declaration, local variables, return values (exit code vs. stdout)
- Error handling: `set -euo pipefail`, trapping ERR and EXIT, cleanup on failure
- Argument parsing: `$1`/`$@`, `getopts` for flags, checking required args
- String manipulation: substring, replace, trim, split — bash parameter expansion

---

### 63. `skills/system_design.md`
**Description:** Use when designing the high-level architecture of a system — how to
split responsibilities, what components are needed, how they communicate, and what
tradeoffs are being made. Load when asked "how should I structure this", when starting
a significant new system, or when an existing system has scaling or maintainability problems.

**Cover:**
- Monolith vs. microservices: start with a monolith, split when you have a reason
- Vertical vs. horizontal scaling: when each applies, what limits each
- Stateless vs. stateful services: why stateless is easier to scale, where state must live
- Synchronous vs. asynchronous communication: REST/RPC vs. queues/events — tradeoffs
- CAP theorem: consistency vs. availability under partition — what your system chooses
- Data partitioning: sharding strategies, hotspots, consistent hashing
- Single points of failure: identify them, decide which to eliminate
- Drawing the design: boxes (services/stores), arrows (sync/async), data flows

---

### 64. `skills/event_driven_architecture.md`
**Description:** Use when designing or implementing systems where components communicate
through events rather than direct calls — pub/sub, event sourcing, CQRS, or domain
events. Load when a task involves decoupling services, implementing an event bus,
designing audit logs, or building systems that react to state changes.

**Cover:**
- Events vs. commands vs. queries: what each is, when to use each
- Pub/sub pattern: publishers don't know consumers, topics/channels, fan-out
- Event sourcing: storing events as the source of truth, replaying to rebuild state
- CQRS: separate read and write models, when the complexity is worth it
- Event schema design: include event type, timestamp, version, aggregate ID
- Ordering and delivery guarantees: at-most-once, at-least-once, exactly-once
- Event versioning: adding fields (safe), removing (breaking), schema registry
- Pitfalls: event storms, distributed transaction problems, debugging async flows

---

### 65. `skills/dependency_injection.md`
**Description:** Use when structuring code to be testable and loosely coupled through
dependency injection — passing dependencies in rather than constructing them internally.
Load when a task involves making code testable, wiring up application components,
using a DI framework (Spring, Angular, InversifyJS, Python injector), or untangling
tightly coupled code.

**Cover:**
- The core idea: don't construct dependencies inside a class, receive them from outside
- Constructor injection vs. property injection vs. method injection — prefer constructor
- Interfaces/protocols: depend on abstractions, not concrete implementations
- Composition root: one place in the app that wires everything together
- DI containers/frameworks: what they automate, when they're overkill (small apps don't need them)
- Testing benefit: inject mocks/fakes instead of real dependencies
- Common mistakes: service locator anti-pattern, injecting the container itself
- Language specifics: Spring (Java), Angular DI, InversifyJS (TS), manual DI in Go/Rust

---

### 66. `skills/code_migration.md`
**Description:** Use when porting code between languages, migrating between frameworks,
or rewriting a module while keeping behavior identical. Load when asked to convert code
from one language to another, migrate from one framework to a newer one, or rewrite
a component without changing its external behavior.

**Cover:**
- Understand before migrating: map the behavior, edge cases, and tests first
- Test coverage first: if tests don't exist, write them before migrating
- Strangler fig pattern: run old and new in parallel, migrate piece by piece
- Mapping constructs: how idioms in one language translate to another (e.g. Go channels → JS async)
- What doesn't translate: language-specific features with no direct equivalent
- Data migration alongside code: schema changes that accompany a rewrite
- Verification: same inputs must produce same outputs — automated comparison testing
- Cutover: feature flags, parallel running, rollback plan before switching fully

---

### 67. `skills/css_layout.md`
**Description:** Use when implementing any CSS layout — flexbox, grid, positioning,
responsive design, or when something isn't laying out the way it should. Load when
a task involves arranging elements on a page, building a responsive layout, fixing
a layout bug, or implementing a design that requires precise spatial control.

**Cover:**
- Flexbox: main axis vs. cross axis, justify-content, align-items, flex-grow/shrink/basis, wrapping
- Grid: template columns/rows, fr units, grid-template-areas, auto-placement, spanning
- When to use which: flexbox for 1D (rows or columns), grid for 2D (both at once)
- Positioning: static/relative/absolute/fixed/sticky — what each does, stacking context
- Responsive design: mobile-first, breakpoints, fluid vs. fixed widths, clamp()
- Common layout patterns: sidebar + main, card grid, sticky header, centered content, holy grail
- Overflow: visible/hidden/scroll/auto, how overflow creates scroll containers
- Debugging layouts: browser devtools overlay, visualizing box model, finding the culprit element

---

### 68. `skills/css_styling.md`
**Description:** Use when styling elements with CSS — colors, typography, spacing, borders,
shadows, transforms, transitions, animations, and pseudo-classes. Load when a task involves
making something look a specific way, implementing a visual design, or fixing visual bugs.

**Cover:**
- Box model: content, padding, border, margin — and box-sizing: border-box
- Typography: font-family stack, font-size/weight/style, line-height, letter-spacing, text-overflow
- Colors: hex/rgb/hsl/oklch, opacity, currentColor, CSS custom properties for theming
- Spacing: margin vs. padding, collapsing margins, gap in flex/grid
- Borders and shadows: border shorthand, border-radius, box-shadow, text-shadow
- Transforms: translate, scale, rotate, skew — 2D and basic 3D
- Transitions: property, duration, easing, delay — smooth state changes
- Animations: @keyframes, animation shorthand, will-change, performance considerations
- Pseudo-classes and pseudo-elements: :hover, :focus, :nth-child, ::before, ::after

---

### 69. `skills/css_architecture.md`
**Description:** Use when organizing CSS in a large project — naming conventions, file
structure, avoiding specificity wars, scoping styles, and deciding between methodologies
like BEM, utility-first (Tailwind), or CSS Modules. Load when starting a new project's
CSS strategy, when stylesheets are getting hard to maintain, or when styles are
unexpectedly overriding each other.

**Cover:**
- Specificity: how it's calculated, why it causes problems, how to keep it flat
- BEM naming: Block__Element--Modifier, when it works, when it's overkill
- Utility-first (Tailwind): the tradeoffs, colocating styles with markup, purging unused
- CSS Modules: scoped class names, composition, works well with component frameworks
- CSS-in-JS: styled-components/emotion tradeoffs, runtime vs. zero-runtime
- File organization: one file per component, global resets, variables/tokens file
- Resets and normalization: what they do, when to use each
- Custom properties (variables): defining, inheriting, overriding in component scope

---

### 70. `skills/javascript_dom.md`
**Description:** Use when manipulating the DOM with vanilla JavaScript — selecting elements,
handling events, modifying content and attributes, managing forms, and working with
browser APIs without a framework. Load when a task involves writing vanilla JS for
a web page, debugging DOM interaction issues, or when a framework isn't in play.

**Cover:**
- Selecting elements: querySelector/querySelectorAll, getElementById, closest, matches
- Reading and writing: textContent vs. innerHTML (XSS risk), getAttribute/setAttribute, classList
- Creating and inserting: createElement, append, prepend, insertBefore, remove, replaceWith
- Event handling: addEventListener, event object, stopPropagation, preventDefault
- Event delegation: attach to parent, filter by target — better than per-element listeners
- Forms: reading values, FormData API, validation, submit event
- Browser APIs: fetch, localStorage, URLSearchParams, IntersectionObserver, ResizeObserver
- Performance: batch DOM reads before writes, avoid layout thrashing, requestAnimationFrame

---

### 71. `skills/html_structure.md`
**Description:** Use when writing HTML — semantic element selection, document structure,
forms, tables, meta tags, and making markup that is accessible, SEO-friendly, and
well-structured. Load when building a page from scratch, reviewing HTML for correctness,
or when accessibility or SEO is a concern.

**Cover:**
- Document structure: DOCTYPE, html/head/body, lang attribute, charset, viewport meta
- Semantic elements: header, nav, main, section, article, aside, footer — use the right one
- Headings: one h1 per page, logical hierarchy, don't skip levels
- Forms: label + input pairing, fieldset/legend for groups, input types, required/disabled
- Tables: thead/tbody/tfoot, th with scope, caption — only for tabular data
- Images: alt text always, width/height to prevent layout shift, lazy loading, srcset
- Links: meaningful link text (not "click here"), target="_blank" + rel="noopener"
- Meta tags: description, og:title/description/image for social sharing, canonical URL

---

### 72. `skills/web_animation.md`
**Description:** Use when implementing animations on the web — CSS transitions, CSS
animations, JavaScript-driven animation, scroll-triggered effects, and performance-safe
motion. Load when a task involves animating UI elements, implementing motion design,
or when animations are janky or causing performance problems.

**Cover:**
- CSS transitions: when to use, which properties are animatable, easing functions
- CSS animations: @keyframes, animation properties, iteration, direction, fill-mode
- JavaScript animation: requestAnimationFrame loop, when JS is necessary over CSS
- Web Animations API: element.animate(), controlling playback, more powerful than CSS alone
- Performant properties: only animate transform and opacity for 60fps, avoid layout-triggering props
- will-change: what it does, use sparingly, remove after animation
- Scroll-triggered animation: IntersectionObserver pattern, avoid scroll event listeners
- Reduced motion: `prefers-reduced-motion` media query — always respect it

---

### 73. `skills/responsive_images_and_media.md`
**Description:** Use when optimizing images, implementing responsive images, handling
video, or managing media assets on the web. Load when a task involves image performance,
different screen densities, art direction for different viewports, or slow page load
caused by unoptimized media.

**Cover:**
- Image formats: JPEG (photos), PNG (transparency), WebP (modern, smaller), AVIF (best compression), SVG (icons/illustrations)
- srcset and sizes: serving the right resolution for the device, how the browser picks
- Art direction: `<picture>` element with `<source media="...">` for different crops
- Lazy loading: `loading="lazy"`, above-the-fold images should be eager
- Aspect ratio: CSS aspect-ratio property, preventing layout shift (CLS)
- Optimization: compress before serving, max dimensions matching display size
- Video: `<video>` element, autoplay rules (muted required), poster image, multiple formats
- SVG: inline vs. img vs. CSS background, optimizing with SVGO, animating SVG

---

### 74. `skills/browser_performance.md`
**Description:** Use when a web page or app feels slow, has poor Core Web Vitals scores,
or needs optimization for load time, rendering performance, or runtime smoothness. Load
when asked to improve page speed, fix jank, reduce bundle size, or optimize any
frontend performance metric.

**Cover:**
- Core Web Vitals: LCP (load), INP (interactivity), CLS (layout stability) — what affects each
- Critical rendering path: HTML parse → CSSOM → render tree → layout → paint
- Render-blocking resources: defer/async for scripts, preload for critical assets
- Bundle size: code splitting, tree shaking, lazy loading routes/components
- Network: HTTP/2 multiplexing, caching headers, CDN, preconnect/prefetch/preload hints
- Runtime performance: avoid layout thrashing, debounce/throttle event handlers, virtual lists for long lists
- Images: largest contentful paint is usually an image — optimize it first
- Measuring: Lighthouse, Chrome DevTools Performance panel, Web Vitals extension

---

## Completion Checklist

After all 74 skills are written, verify:

- [ ] Every file has valid YAML frontmatter with `name` and `description`
- [ ] Every `description` clearly states trigger conditions and what the skill provides
- [ ] Every file is at `skills/<skill_name>.md` — flat, no subfolders
- [ ] Each file has an Overview section within the first 2KB
- [ ] No file duplicates another's core content
- [ ] Each skill is self-contained but cross-references others where relevant

---

*End of roadmap. Begin with skill 1 and proceed in order. Good night.*