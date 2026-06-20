---
name: writing-tests
description: Use when writing unit tests, integration tests, or end-to-end tests in any language. Covers test structure, naming, setup/teardown, assertion patterns, mocking, and what makes a test actually useful. Load when asked to add tests, improve coverage, or debug a failing test.
---

# Writing Tests

## Overview

Tests are executable specifications of expected behavior. A good test tells you what the code should do, catches regressions when it stops doing that, and is maintainable enough that developers actually run it. The core principle: **test behavior, not implementation.** Tests that are coupled to how code works (rather than what it does) break when you refactor and provide no real safety.

## Test Naming

Use a consistent naming convention that makes failures immediately understandable:

```
test_<function>_<scenario>_<expected>

# Examples:
test_parse_url_valid_input_returns_url_object
test_divide_by_zero_raises_error
test_user_create_duplicate_email_returns_409
```

**Rules:**
- The name should read as a sentence: "test that parse_url with valid input returns a URL object"
- Include the expected outcome — `test_login` tells you nothing when it fails
- Don't use vague names like `test_edge_case` or `test_stuff`

## Arrange-Act-Assert Structure

Every test should have three clear sections:

```python
def test_withdraw_sufficient_balance_reduces_balance():
    # Arrange — set up the test data and preconditions
    account = Account(balance=100)
    
    # Act — perform the operation being tested
    account.withdraw(30)
    
    # Assert — verify the expected outcome
    assert account.balance == 70
```

**Separate these sections with blank lines.** When a test fails, you should be able to see at a glance which section contains the problem.

## Unit vs. Integration vs. E2E

| Type | What it tests | Speed | When to write |
|------|---------------|-------|---------------|
| Unit | Single function/class in isolation | <1ms | For all business logic |
| Integration | Multiple components together (DB, API) | 10ms-1s | For boundaries (DB queries, API calls) |
| E2E | Full user flow through the system | 1s-30s | For critical user paths only |

**Where each lives:**
- Unit: `tests/` or `*_test.go` or `#[cfg(test)]` inline
- Integration: `tests/integration/` or `tests/` with a naming convention
- E2E: `tests/e2e/` or `cypress/` or `playwright/`

## Setup and Teardown

### Test Fixtures
Use fixtures for data that's reused across tests:

```python
@pytest.fixture
def user():
    return User(name="Alice", email="alice@example.com")
```

### Temporary Resources
- **Temp directories**: Use the language's temp dir utility, not hardcoded paths
- **Test databases**: Create a fresh DB per test or per test suite, never share with production
- **Mock servers**: Start/stop in setup/teardown, use a random port

### Cleanup Rules
- Every resource created in a test must be cleaned up, even if the test fails
- Use `try/finally`, `defer`, or framework teardown hooks
- Never leave test data in a shared database

## Mocking and Stubbing

### When to Mock
- External services (APIs, email, payment processors) — you don't control them
- Time-dependent behavior — inject a clock, don't call `now()` directly
- File system / network — too slow and unreliable for unit tests

### When NOT to Mock
- The system under test — if you're mocking the thing you're testing, you're testing the mock
- Business logic you own — test it for real
- Database queries in integration tests — use a real test database

### Mocking Pitfalls
- **Over-mocking**: If every dependency is a mock, your test proves nothing about real behavior
- **Mocking what you don't own**: If the external API changes, your mock won't tell you
- **Verifying implementation details**: `verify(mock, times(1)).method()` couples the test to the call count, not the result

**Rule of thumb:** Mock at the boundary (external services, I/O). Test the internals for real.

## Assertions

### Specific Over Generic
```python
# BAD — generic, unhelpful failure message
assert result is not None

# GOOD — specific, tells you exactly what's wrong
assert result.status == "active"
assert result.id == expected_id
```

### Include Helpful Failure Messages
```python
# Most frameworks let you add a message:
assert len(items) == 3, f"Expected 3 items, got {len(items)}: {items}"
```

### Test for Errors and Edge Cases
Don't just test the happy path:

```python
def test_divide_by_zero_raises_error():
    with pytest.raises(ZeroDivisionError):
        divide(10, 0)

def test_parse_empty_string_returns_none():
    result = parse("")
    assert result is None

def test_create_user_with_long_name_truncates():
    user = create_user(name="A" * 300)
    assert len(user.name) <= 255
```

**Edge cases to always consider:**
- Empty input (empty string, empty list, null)
- Boundary values (0, max int, min int)
- Invalid input (wrong type, malformed data)
- Concurrent access (if applicable)

## Running a Single Test vs. the Full Suite

| Framework | Run single test | Run all |
|-----------|----------------|---------|
| pytest | `pytest test_file.py::test_name` | `pytest` |
| Jest | `jest -t "test name"` | `jest` |
| cargo test | `cargo test test_name` | `cargo test` |
| go test | `go test -run TestName` | `go test ./...` |

**During development:** Run the single test you're working on. It's faster and the feedback loop is tighter.

**Before committing:** Run the full suite. Your change may have broken something you didn't expect.

## Anti-Patterns

- **Testing implementation details.** If renaming a private function breaks a test, the test is wrong.
- **Giant test functions.** If a test is 50+ lines, split it. Each test should test one thing.
- **Interdependent tests.** Test B should not depend on Test A running first. Tests must be independently runnable.
- **Flaky tests.** If a test passes sometimes and fails sometimes, it's testing something non-deterministic. Fix it or remove it — flaky tests erode trust in the entire suite.
- **Not testing error paths.** If you only test the happy path, you'll ship bugs that error handling should have caught.
- **Asserting on mock call counts.** This couples tests to implementation. Assert on outcomes, not on how many times something was called.

See also: `testing_strategies` for test suite design, `debugging_workflow` for debugging failing tests.
