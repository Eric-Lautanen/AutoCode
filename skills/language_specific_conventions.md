---
name: language-specific-conventions
description: Use when starting work in a specific language to recall its conventions, project structure norms, formatting standards, and ecosystem defaults. Covers the most common languages: Python, JavaScript/TypeScript, Rust, Go, Java/Kotlin, and Ruby. Load when beginning work in a language you haven't touched yet in the current task.
---

# Language-Specific Conventions

## Overview

Every language has idioms, conventions, and ecosystem norms. Writing Python like Java or Go like Rust produces code that's technically correct but culturally wrong — hard for other developers to read and maintain. The core principle: **write code in the style of the language and its community.** When in Rome, write Roman code.

## Cross-Language Rule

**Always check for an existing linter/formatter config before writing code.** If the project has `.eslintrc`, `pyproject.toml` with ruff, `rustfmt.toml`, or `.rubocop.yml`, follow those settings. Project conventions override general language conventions.

---

## Python

### Conventions
- **Style**: PEP 8 — 4 spaces, 79 char line limit (88 with Black), no semicolons
- **Naming**: `snake_case` for functions/variables, `PascalCase` for classes, `UPPER_SNAKE` for constants
- **Type hints**: Use for all function signatures (Python 3.9+); `list[X]` not `List[X]`
- **Formatter**: Black (opinionated, zero-config) or Ruff (faster, replaces flake8+isort+Black)
- **Linter**: Ruff (modern) or flake8 (legacy)

### Project Layout
```
my-project/
├── pyproject.toml       # Project metadata, dependencies, tool config
├── src/
│   └── my_package/      # Source code (src layout preferred)
│       ├── __init__.py
│       ├── models.py
│       └── services.py
├── tests/
│   ├── test_models.py
│   └── test_services.py
└── .python-version      # Pin Python version
```

### Key Idioms
- Use `pathlib.Path` over `os.path`
- Use `dataclasses` or `pydantic` for structured data
- Use `with` statements for resource management
- Use list/dict comprehensions over map/filter
- Use `f-strings` over `.format()` or `%`

---

## JavaScript / TypeScript

### Conventions
- **Style**: 2 spaces (Prettier default), semicolons optional (pick one, be consistent)
- **Naming**: `camelCase` for functions/variables, `PascalCase` for classes/components, `UPPER_SNAKE` for constants
- **Modules**: ESM (`import/export`) preferred over CJS (`require/module.exports`) for new projects
- **Formatter**: Prettier (zero-config, standard)
- **Linter**: ESLint with TypeScript plugin

### Project Layout
```
my-project/
├── package.json
├── tsconfig.json        # TypeScript config
├── src/
│   ├── index.ts         # Entry point
│   ├── types.ts         # Shared types
│   └── modules/
├── tests/
│   └── index.test.ts
└── .eslintrc.cjs
```

### Key Idioms
- Use `const` by default, `let` only when reassignment is needed, never `var`
- Use async/await over `.then()` chains
- Use optional chaining (`obj?.prop`) and nullish coalescing (`val ?? default`)
- Prefer `interface` for object shapes, `type` for unions and complex types
- Use `unknown` over `any` for values of unknown type

---

## Rust

### Conventions
- **Style**: 4 spaces, `rustfmt` is canonical (run `cargo fmt`)
- **Naming**: `snake_case` for functions/variables/modules, `PascalCase` for types/traits/enums
- **Linter**: Clippy (`cargo clippy`) — treat clippy warnings as errors
- **Edition**: Use the latest stable edition in `Cargo.toml`

### Project Layout
```
my-project/
├── Cargo.toml           # Package manifest
├── src/
│   ├── main.rs          # Binary entry point
│   ├── lib.rs           # Library root
│   └── module.rs         # or module/mod.rs
├── tests/               # Integration tests
│   └── integration_test.rs
└── benches/             # Benchmarks
```

### Key Idioms
- Use `Result<T, E>` for recoverable errors, `panic!` for bugs
- Use `Option<T>` instead of null, `unwrap()` only in tests or provably-safe cases
- Prefer borrowing (`&T`) over owning (`T`) when you don't need ownership
- Use `match` for exhaustive handling, `if let` for single-variant extraction
- Use the `?` operator for error propagation
- Module system: `mod.rs` or `module.rs` style — pick one per project

---

## Go

### Conventions
- **Style**: Tabs for indentation, `gofmt` is canonical (no config, no debate)
- **Naming**: `camelCase` for unexported, `PascalCase` for exported, `UPPER_SNAKE` for constants
- **Linter**: `golangci-lint` (aggregates multiple linters)
- **Error handling**: Always check errors, never ignore `_ = mightFail()`

### Project Layout
```
my-project/
├── go.mod               # Module definition
├── cmd/
│   └── myapp/
│       └── main.go      # Entry point
├── internal/            # Private application code
│   ├── handler/
│   └── service/
├── pkg/                 # Public library code (optional)
└── go.sum               # Dependency checksums
```

### Key Idioms
- `if err != nil { return err }` — the Go way, don't fight it
- Wrap errors with context: `fmt.Errorf("processing order %d: %w", id, err)`
- Accept interfaces, return structs
- Use `defer` for cleanup (close files, unlock mutexes)
- Don't use `panic` for error handling — use `error` return values
- Range loop variable capture: use local copy `v := v` in Go < 1.22

---

## Java / Kotlin

### Conventions
- **Style**: 4 spaces, no tabs; Google Java Format or Kotlin coding conventions
- **Naming**: `camelCase` for methods/variables, `PascalCase` for classes, `UPPER_SNAKE` for constants
- **Build**: Maven (convention-based) or Gradle (flexible, Kotlin DSL preferred)
- **Linter**: Checkstyle (Java), ktlint + detekt (Kotlin)

### Project Layout (Maven/Gradle)
```
my-project/
├── pom.xml / build.gradle.kts
├── src/
│   ├── main/
│   │   ├── java/com/example/
│   │   └── resources/
│   └── test/
│       ├── java/com/example/
│       └── resources/
```

### Key Idioms
- **Java**: Use Optional instead of null returns, prefer immutable collections, use records for data carriers
- **Kotlin**: Use data classes, null safety (`?` and `!!`), extension functions, sealed classes for hierarchies
- **Kotlin**: Prefer `val` over `var`, use scope functions (`let`, `apply`, `run`) judiciously
- Both: Use dependency injection (Spring, Dagger, Koin), not manual wiring

---

## Ruby

### Conventions
- **Style**: 2 spaces, no semicolons, RuboCop is the standard linter
- **Naming**: `snake_case` for methods/variables, `PascalCase` for classes/modules
- **Gems**: Bundler for dependency management, Gemfile for declaration
- **Framework**: Rails is convention-over-configuration — follow its defaults

### Key Idioms
- Use blocks and iterators over for-loops
- Use `attr_reader`/`attr_accessor` for accessors
- Use symbols for hash keys (`{ name: "Alice" }` not `{ "name" => "Alice" }`)
- Use `freeze` for immutable string constants
- Prefer `Struct` for simple data objects
- In Rails: follow the "fat models, skinny controllers" convention

---

## Windows-Specific Language Notes

### C# / .NET
C# is the primary language for Windows development:

**Conventions:**
- **Style**: 4 spaces, PascalCase for methods/classes, camelCase for variables
- **Naming**: `PascalCase` for everything public, `_camelCase` for private fields
- **Framework**: .NET 6+ for new projects, .NET Framework 4.8 for legacy
- **Build**: MSBuild, `dotnet build`, `dotnet test`

**Project Layout:**
```
my-project/
├── MyProject.sln          # Solution file
├── src/
│   └── MyProject/
│       ├── MyProject.csproj
│       └── Program.cs
└── tests/
    └── MyProject.Tests/
        └── UnitTest1.cs
```

**Key Idioms:**
- Use `async`/`await` for I/O operations
- Use `var` when the type is obvious
- Prefer `IEnumerable<T>` over `List<T>` for return types
- Use `using` statements for IDisposable resources
- Handle Windows-specific APIs with P/Invoke or CsWin32

### PowerShell
PowerShell is essential for Windows automation:

**Conventions:**
- **Style**: 4 spaces, PascalCase for functions, Verb-Noun naming
- **Naming**: `Get-Process`, `Set-Location`, `Invoke-RestMethod`
- **Execution policy**: Scripts may be blocked by default

**Example:**
```powershell
# Good PowerShell
function Get-ServiceStatus {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory=$true)]
        [string]$ServiceName
    )
    
    Get-Service -Name $ServiceName | Select-Object Name, Status
}
```

### Batch / CMD
Legacy but still used on Windows:

**Conventions:**
- Use `.cmd` extension (not `.bat` for new scripts)
- Use `setlocal enabledelayedexpansion` for variables in loops
- Quote paths: `"%VAR%"` not `%VAR%`

## Anti-Patterns

- **Writing language X in language Y's style.** Don't write Python with Java patterns, or Go with Rust patterns.
- **Ignoring the project's existing config.** If the project uses tabs and you add spaces, you create inconsistency.
- **Not running the formatter.** `cargo fmt`, `npx prettier --write`, `ruff format` — just run it.
- **Debating style.** Use the community's formatter and move on. Style debates are a productivity sink.
- **Not considering C# for Windows projects.** C# is the native Windows language.
