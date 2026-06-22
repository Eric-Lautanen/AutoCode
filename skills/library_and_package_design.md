---
name: library-and-package-design
description: Use when designing or publishing a library meant for other developers to consume - public API design, semver versioning, documentation, publishing to npm/PyPI/crates.io, and maintaining backward compatibility. Load when building a reusable library, adding a public API, or preparing a package for release.
---

# Library and Package Design

## Overview

Designing a library is fundamentally different from designing an application. In an app, you control all the callers. In a library, you don't — and every public symbol becomes a commitment. Breaking changes break your users' code. Missing documentation means they can't figure out how to use your library. Poor versioning means they can't trust your updates. This skill covers how to design, version, document, and publish libraries that developers actually want to use.

## Public API Surface

### Expose the Minimum

Everything you make public is a commitment. Everything you keep private is free to change.

- **Export only what consumers need**: The main module should expose a small, coherent set of functions/types
- **Hide implementation details**: Internal helpers, intermediate types, and utility functions should not be exported
- **Use language-level visibility**: `pub` vs private (Rust), `export` vs unexported (Go), `__all__` (Python), `export` vs unexported (TS)

### The API Surface Test

If you can't explain your library's public API in 30 seconds, it's too big. A good library has a clear mental model:

- **Lodash**: "Utility functions for arrays, objects, and strings"
- **Express**: "HTTP request handler with middleware chain"
- **Zod**: "Schema definition → type-safe parsing and validation"

If your library's description is "a collection of various utilities for...", it's probably doing too much.

### Stable vs. Unstable API

Mark experimental or unstable APIs explicitly:

```rust
#[unstable(feature = "async_iter", issue = "12345")]
pub fn async_iter(&self) -> AsyncIter { ... }
```

```typescript
/** @experimental - API may change without a major version bump */
export function experimentalFeature(): void;
```

This sets expectations: consumers know they're taking a risk.

## Semver

### The Rules

Given version `MAJOR.MINOR.PATCH`:

- **PATCH** (`1.2.3` → `1.2.4`): Bug fixes, no API changes. Always safe to upgrade.
- **MINOR** (`1.2.3` → `1.3.0`): New features, new public symbols. Backward compatible. Safe to upgrade.
- **MAJOR** (`1.2.3` → `2.0.0`): Breaking changes. Consumers must update their code.

### What Counts as Breaking

| Change | Breaking? |
|--------|-----------|
| Remove a public function/method | **Yes** |
| Rename a public function/method | **Yes** |
| Change a function's parameter types | **Yes** |
| Add a required parameter | **Yes** |
| Add an optional parameter with default | No (minor) |
| Add a new public function | No (minor) |
| Add a new return field/type | **Usually yes** (in statically typed languages) |
| Change internal behavior (same inputs, same outputs) | No (patch) |
| Change error type thrown | **Usually yes** |
| Widen accepted input types | No (minor) |
| Narrow accepted input types | **Yes** |

**When in doubt, bump major.** Your users will thank you.

## Backward Compatibility

### Deprecation Cycle

Never remove a public API in one step. Follow the cycle:

1. **Mark deprecated** (current minor version): Add `@deprecated` annotation, update docs, log a warning
2. **Keep working** (next minor/major versions): The deprecated API still works
3. **Remove** (next major version): Only after at least one major version of deprecation

```typescript
// Version 1.x
/** @deprecated Use `parseConfig` instead. Will be removed in 3.0. */
export function loadConfig(path: string): Config {
  console.warn("loadConfig is deprecated. Use parseConfig instead.");
  return parseConfig(readFileSync(path, "utf-8"));
}

export function parseConfig(content: string): Config {
  // New, better implementation
}
```

### Maintain Old Signatures

When adding a better API, keep the old one working as a thin wrapper:

```python
# Old API (deprecated but still works)
def process(data, format="json"):
    warnings.warn("process() is deprecated, use process_data()", DeprecationWarning)
    return process_data(data, format=format)
```

## Documentation

### Every Public Symbol Needs a Doc Comment

```typescript
/**
 * Parses a configuration string into a typed Config object.
 *
 * @param content - The configuration content as a string (JSON or YAML)
 * @param options - Parsing options (optional)
 * @returns The parsed Config object
 * @throws {ParseError} If the content is invalid
 *
 * @example
 * ```ts
 * const config = parseConfig('{"port": 3000}');
 * console.log(config.port); // 3000
 * ```
 */
export function parseConfig(content: string, options?: ParseOptions): Config {
```

### What to Include

- **One-line summary**: What does this do?
- **Parameters**: Name, type, meaning (not just repeating the name)
- **Return value**: What's returned, what shape it has
- **Errors thrown**: What errors and when
- **Example**: The simplest possible usage that actually works
- **See also**: Related functions or alternatives

### README Structure

```markdown
# library-name

One-line description of what it does.

## Installation
npm install library-name  # or pip, cargo add, etc.

## Quick Start
The simplest possible example that does something useful.

## API Reference
Link to auto-generated docs or a summary of the main exports.

## Changelog
Link to CHANGELOG.md.

## License
MIT (or your license).
```

## Publishing

### Package Metadata

Every package manager needs:

| Field | What | Why it matters |
|-------|------|---------------|
| Name | Unique, lowercase, no spaces | How users install it |
| Version | Semver | How users pin it |
| Description | One line | Searchability in registries |
| License | SPDX identifier | Legal clarity |
| Repository | Git URL | Where to report issues |
| Keywords | 5-10 relevant terms | Discoverability |

### Publishing Checklist

- [ ] Version bumped correctly (patch/minor/major)
- [ ] Changelog updated with this version's changes
- [ ] Git tag created (`v1.2.3`)
- [ ] All tests passing
- [ ] No `console.log` / `print` / debug code left in
- [ ] README reflects current API
- [ ] Package builds cleanly (`npm pack`, `python -m build`, `cargo package`)

### Registry-Specific Notes

| Registry | Publish command | Notes |
|----------|----------------|-------|
| npm | `npm publish` | Run `npm pack` first to check contents |
| PyPI | `twine upload dist/*` | Build with `python -m build` first |
| crates.io | `cargo publish` | Dry run with `cargo publish --dry-run` |
| Maven Central | Complex (Sonatype) | Requires GPG signing, staging release |

## Changelogs

Use [Keep a Changelog](https://keepachangelog.com/) format:

```markdown
# Changelog

## [1.3.0] - 2024-01-15
### Added
- `parseConfig` function for parsing config strings
- YAML support in config parsing

### Changed
- `process()` is now deprecated in favor of `parseConfig()`

### Fixed
- Crash when config contains null values (#42)
```

**Rules**:
- Every release has an entry
- Categorize changes: Added, Changed, Deprecated, Removed, Fixed, Security
- Include issue/PR numbers
- Don't dump git commit messages — write human-readable summaries

## Testing as a Consumer

Write tests that use your library the way a consumer would:

```python
# Bad: testing internal functions
def test_internal_parser():
    result = _parse_token_stream(tokens)
    assert result.type == "object"

# Good: testing the public API
def test_parse_config():
    config = parse_config('{"port": 3000, "host": "localhost"}')
    assert config.port == 3000
    assert config.host == "localhost"
```

- Test the main use cases from the README examples
- Test error messages are helpful (consumers see these)
- Test the library works with common bundlers/runtimes, not just your dev setup
- Integration test: create a minimal project that depends on your library, build and run it

## Windows-Specific Library Notes

### NuGet Package Design (C#)
When publishing libraries for Windows developers:

```xml
<!-- MyLibrary.csproj -->
<Project Sdk="Microsoft.NET.Sdk">
  <PropertyGroup>
    <TargetFrameworks>net6.0;net48</TargetFrameworks>
    <PackageId>MyCompany.MyLibrary</PackageId>
    <Version>1.0.0</Version>
    <Authors>Your Name</Authors>
    <Description>Library description</Description>
    <PackageTags>windows;utility</PackageTags>
    <PackageLicenseExpression>MIT</PackageLicenseExpression>
  </PropertyGroup>
</Project>
```

### Windows-Specific APIs in Libraries
Expose Windows-specific functionality cleanly:

```csharp
// Cross-platform interface
public interface IPlatformService {
    void DoSomething();
}

// Windows implementation
public class WindowsPlatformService : IPlatformService {
    public void DoSomething() {
        // Windows-specific implementation
    }
}
```

### PowerShell Module Design
For PowerShell modules distributed via PowerShell Gallery:

```powershell
# MyModule.psd1
@{
    ModuleVersion = '1.0.0'
    GUID = '12345678-1234-1234-1234-123456789012'
    Author = 'Your Name'
    Description = 'Module description'
    PowerShellVersion = '5.1'
    FunctionsToExport = @('Get-MyData', 'Set-MyData')
}
```

### Windows Installer (MSI/MSIX)
For libraries that need Windows installer packaging:

- **MSI**: Traditional installer, requires admin privileges
- **MSIX**: Modern packaging, sandboxed, auto-updating
- **Chocolatey**: Package manager for Windows
- **winget**: Modern Windows package manager

## Checklist

- [ ] Public API is minimal — only what consumers need
- [ ] Unstable APIs marked as experimental
- [ ] Semver followed strictly — breaking changes bump major
- [ ] Deprecated APIs maintained for at least one major version
- [ ] Every public symbol has a doc comment with an example
- [ ] README covers: what, install, quick start, API reference link
- [ ] Changelog maintained in Keep a Changelog format
- [ ] Package metadata complete (name, version, description, license, repo)
- [ ] Tests exercise the public API, not internals
- [ ] Windows: NuGet package metadata complete
- [ ] Windows: PowerShell module manifest complete
- [ ] Windows: Installer packaging considered (MSI/MSIX)
