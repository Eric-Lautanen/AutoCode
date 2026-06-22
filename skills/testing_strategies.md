---
name: testing-strategies
description: Use when deciding what kind of tests to write, how much coverage is enough, how to structure a test suite, or how to make tests that are actually maintainable. Load when planning test coverage for a new feature or when a test suite is slow, brittle, or hard to maintain. Covers the test pyramid, test doubles, brittle test avoidance, and coverage philosophy.
---

# Testing Strategies

## Overview

A good test suite is fast, reliable, and tells you what broke and why. A bad test suite is slow, flaky, and fails for reasons unrelated to your change. The difference isn't how many tests you have — it's what kinds of tests you write, how you structure them, and what you choose not to test. This skill covers the strategic decisions: what to test at each level, how to keep tests maintainable, and when you've tested enough.

For the mechanics of writing individual tests, see `writing_tests.md`.

## The Test Pyramid

```
        /  E2E  \          Few — slow, brittle, expensive
       / Integration \     Some — moderate speed, real dependencies
      /   Unit Tests  \    Many — fast, deterministic, isolated
```

- **Unit tests** (many): Test a single function or module in isolation. Pure logic, edge cases, error paths. Target: <1ms each, thousands of them.
- **Integration tests** (some): Test modules working together — database queries, API calls, file I/O. Use real dependencies where feasible. Target: <100ms each, hundreds of them.
- **E2E tests** (few): Test critical user paths through the full system. Slow and brittle by nature. Target: seconds each, tens of them.

The pyramid is a guideline, not a rule. Some systems (CRUD APIs) may have more integration tests. Some (libraries) may be almost all unit tests. Never invert the pyramid — more E2E than unit is a maintenance nightmare.

## What to Test at Each Level

### Unit Test
- Pure functions and business logic
- Edge cases: empty input, null, zero, max values, off-by-one
- Error paths: invalid input, missing data, permission denied
- State transitions and decision logic
- Data transformations and calculations

### Integration Test
- Database read/write with real schema
- API endpoints with real routing but mocked external services
- File I/O with temp directories
- Message queue publish/consume with real queue
- ORM queries returning correct results

### E2E Test
- Critical user journeys: signup, checkout, data export
- Cross-service workflows
- Authentication and authorization flows end-to-end
- Nothing that can be verified with a unit or integration test

## Test Doubles: When to Use What

| Double | What it does | When to use |
|--------|-------------|-------------|
| **Stub** | Returns canned data | When you need a dependency to return specific values |
| **Mock** | Verifies interactions | When the *call* to the dependency is the behavior under test |
| **Fake** | Working in-memory implementation | When you need realistic behavior without real infrastructure (in-memory DB, fake filesystem) |
| **Spy** | Records calls, also delegates | When you need to verify a call happened but also want real behavior |

**Rules of thumb:**
- Prefer fakes over mocks — fakes behave like the real thing, mocks only do what you program.
- Mock external services (you don't control them). Fake or use real internal dependencies.
- If your mock setup is longer than the test, you're mocking at the wrong level.
- Never mock the system under test — you'd be testing your mock, not your code.

## Avoiding Brittle Tests

Brittle tests fail when implementation changes even though behavior is correct. Signs and fixes:

- **Testing implementation details**: Don't assert which private methods were called. Assert outputs and side effects.
- **Snapshot tests gone wrong**: Snapshots are fine for truly stable output (serialized formats). They're terrible for UI — any CSS change breaks them. Use targeted assertions instead.
- **Over-mocking**: If changing an internal detail breaks 20 mock-based tests, you have too many mocks. Use fakes or real dependencies.
- **Order-dependent tests**: Tests that only pass when run in a specific order. Always isolate: setup before each test, teardown after.
- **Time-dependent tests**: Hard-coded timestamps or `Date.now()`. Inject a clock. See `date_and_time_handling.md`.
- **Flaky tests**: Tests that pass sometimes and fail sometimes. Fix immediately or quarantine — flaky tests erode trust in the whole suite.

## Test Speed Matters

Slow tests don't get run. If your unit test suite takes more than 10 seconds, developers will skip it. Guidelines:

- Unit tests: <1ms each, full suite <10 seconds
- Integration tests: <100ms each, full suite <2 minutes
- E2E tests: <10s each, full suite <15 minutes
- Total feedback loop for a single change: under 5 minutes

**Speed tips:**
- Use in-memory databases (SQLite :memory:, H2) for integration tests
- Parallelize test execution
- Avoid network calls in unit tests — mock or fake them
- Don't create heavy fixtures when a simple one suffices

## Coverage Philosophy

- **100% line coverage is not the goal.** 100% coverage means every line ran, not that every edge case was tested.
- **Cover the important paths**: business logic, error handling, security boundaries, money calculations.
- **Don't cover trivial code**: getters/setters, one-line delegations, framework boilerplate.
- **Branch coverage > line coverage**: A line can execute without testing both branches of its `if`.
- **Coverage as a signal, not a target**: A drop in coverage is a red flag. 100% with poor assertions is meaningless.

## Property-Based Testing

When the space of inputs is large and you can't enumerate all cases, property-based testing finds bugs you'd miss:

- **When to use**: parsers, encoders/decoders, data transformations, mathematical functions, sorting algorithms
- **What it finds**: edge cases you didn't think of — empty strings, unicode, very large numbers, negative zero
- **How it works**: the framework generates random inputs and checks that properties hold (e.g., "decode(encode(x)) == x")
- **Tools**: Hypothesis (Python), QuickCheck (Haskell/many ports), fast-check (JS/TS), proptest (Rust)
- **Cost**: slower to write, slower to run, but finds bugs unit tests miss

**Pattern**: Write property tests for invariants, unit tests for specific cases. Use both.

## Structuring a Test Suite

```
tests/
  unit/           # Fast, isolated, no I/O
  integration/    # Real dependencies, test DBs
  e2e/            # Full system, real browser or HTTP client
  fixtures/       # Shared test data
  helpers/        # Test utilities, factory functions
```

- Each test file mirrors one source file: `user_service.ts` → `user_service.test.ts`
- Shared fixtures in a dedicated directory, not copy-pasted across tests
- Factory functions for creating test data — never hand-construct complex objects in each test

## Windows-Specific Notes

### Windows Test Environment
- **Line endings**: Tests that compare file content may fail due to CRLF vs LF. Normalize or use `.gitattributes`.
- **Path separators**: Use `pathlib` or `os.path.join` in tests, never hardcode `/` or `\`.
- **File locking**: Windows locks files during tests. Ensure proper cleanup in teardown.

```python
import tempfile
import os

def test_file_processing():
    # Use tempfile for cross-platform temp files
    with tempfile.NamedTemporaryFile(mode='w', delete=False) as f:
        f.write("test data")
        temp_path = f.name
    
    try:
        result = process_file(temp_path)
        assert result == "expected"
    finally:
        # Windows: file may be locked, retry deletion
        for _ in range(5):
            try:
                os.unlink(temp_path)
                break
            except PermissionError:
                time.sleep(0.1)
```

### Windows CI/CD Testing
GitHub Actions Windows runners have specific considerations:
```yaml
# .github/workflows/test.yml
jobs:
  test-windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: '20'
      - run: npm ci
      - run: npm test
```

### PowerShell Testing
```powershell
# Pester is the standard testing framework for PowerShell
Describe "MyFunction" {
    It "Returns expected output" {
        $result = MyFunction
        $result | Should -Be "expected"
    }
}
```

## Checklist

- [ ] Test pyramid shape: many unit, some integration, few E2E
- [ ] Unit tests are fast (<1ms each) and deterministic
- [ ] Integration tests use real dependencies where feasible
- [ ] E2E tests cover only critical user paths
- [ ] No test depends on another test's execution
- [ ] Mocks are for external services; fakes or real deps for internals
- [ ] Coverage focuses on important paths, not line count
- [ ] Flaky tests are fixed or quarantined, not ignored
