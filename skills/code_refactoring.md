---
name: code-refactoring
description: Use when asked to refactor, restructure, rename, deduplicate, or clean up existing code without changing behavior. Covers safe refactoring sequences that keep code compiling and passing tests at every step. Load before any refactoring task touching more than one location.
---

# Code Refactoring

## Overview

Refactoring is changing code's structure without changing its behavior. The core principle: **every refactoring step must leave the codebase in a working, testable state.** If you try to do too much at once, you'll end up with a broken build and no idea which change caused it. Small, verified steps are the only safe way to refactor.

Refactoring is not rewriting. If you're changing behavior, that's a feature change — do it separately from refactoring. Never refactor and add features in the same commit.

## Refactoring Sequence

Follow this order, always:

1. **Understand the current code.** Read it thoroughly. Know what it does and why.
2. **Verify tests exist.** If there are no tests, write them before refactoring. Tests are your safety net.
3. **Make the smallest possible change.** One rename, one extraction, one move at a time.
4. **Verify.** Build and run tests after every change.
5. **Repeat.** Small steps compound into large improvements.

### The Refactoring Loop
```
while (code_needs_improvement):
    make_one_small_change()
    run_tests()
    if tests_pass:
        commit("refactor: <what changed>")
    else:
        revert()
        try_a_different_approach()
```

## Rename Workflow

Renaming is the most common refactoring and the most error-prone when done manually:

1. **Find all usages first.** Use grep/search to find every reference — definition, imports, callers, comments, tests.
2. **Update the definition.** Change the name where it's defined.
3. **Update all callers.** Change every usage to the new name.
4. **Build and test.** The compiler/linter will catch any you missed.
5. **Update comments and docs.** If the name appears in comments, update those too.

**Tip:** Use your IDE's rename refactoring when available — it's faster and more reliable than manual find-replace.

## Extract Function/Method

When a function is too long or does more than one thing:

1. **Identify the seam.** Find the block of code that should be its own function.
2. **Identify inputs.** What variables from the outer scope does the block read? These become parameters.
3. **Identify outputs.** What does the block produce that the outer function needs? This becomes the return value.
4. **Create the new function.** Write it with the identified parameters and return type.
5. **Replace the block with a call.** Call the new function, passing the identified inputs.
6. **Build and test.** Behavior must be identical.

**Example:**
```python
# Before
def process_order(order):
    # ... 20 lines of validation ...
    # ... 10 lines of price calculation ...
    # ... 15 lines of persistence ...

# After
def process_order(order):
    validate_order(order)          # extracted
    total = calculate_total(order) # extracted
    save_order(order, total)       # extracted
```

## Split File/Module

When a file has grown too large or mixes concerns:

1. **Move types first.** Move type/struct/interface definitions to the new file.
2. **Update imports.** Add the import in the old file, remove from where they were.
3. **Move functions.** Move functions that belong to the new module.
4. **Fix references.** Update all callers to import from the new location.
5. **Build and test after each move.** Don't move everything at once.

## Interface Changes

The most dangerous refactoring — changing a public interface:

1. **Add the new interface.** Don't remove the old one yet.
2. **Migrate callers.** Update callers one at a time to use the new interface.
3. **Deprecate the old interface.** Mark it as deprecated with a clear message.
4. **Remove the old interface.** Only after all callers are migrated and the deprecation has been in place for a reasonable time.

**Never swap an interface in one step.** Adding and removing in the same commit leaves no migration path.

## Keeping It Compiling at Each Step

The golden rule of refactoring: **the code must compile and pass tests after every single change.**

Techniques for staying compilable:
- **Add before removing.** Add the new function, migrate callers, then remove the old one.
- **Use deprecation.** Mark old code as deprecated rather than deleting it immediately.
- **Feature flags.** For large refactors, use a flag to switch between old and new code paths.
- **Backward-compatible changes.** Add optional parameters instead of changing required ones.

## When NOT to Refactor

- **Mid-feature.** If you're adding a feature and notice code that could be cleaner, note it but don't refactor now. Finish the feature first.
- **No tests.** Refactoring without tests is rewriting without a safety net. Write tests first.
- **Unclear requirements.** If you don't know what the code should do, you can't know if your refactoring preserved behavior.
- **Near a deadline.** Refactoring introduces risk. Don't do it when you can't afford a regression.
- **The code works and isn't changing.** Don't refactor code that's stable, tested, and not being modified. Leave it alone.

## Anti-Patterns

- **Big bang refactoring.** Rewriting a whole module at once. It never works on the first try.
- **Refactoring without tests.** You have no way to verify behavior is preserved.
- **Refactoring and feature work in the same commit.** If something breaks, you can't tell which change caused it.
- **Changing behavior during refactoring.** "While I'm here, I'll also fix this bug..." — no. Separate commit.
- **Not building between steps.** If you make 5 changes and then build, you have 5 places to look for the error.
- **Renaming without finding all usages.** The compiler will catch it, but why not be thorough?

See also: `task_decomposition` for planning multi-step refactors, `file_editing_strategy` for safe edit techniques.
