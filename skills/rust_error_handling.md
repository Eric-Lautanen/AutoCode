# Rust Error Handling Patterns

## General principles

- Use `anyhow::Result` for application-level error handling
- Use `thiserror` for library crate error enums
- Prefer `map_err` over unwrap/expect in production code
- Use `eyre` for richer error context in CLI tools

## Pattern: Custom error type

```rust
use std::fmt;
use std::io;

#[derive(Debug)]
pub enum AppError {
    Io(io::Error),
    Parse(String),
    NotFound { path: String },
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "I/O error: {}", e),
            AppError::Parse(msg) => write!(f, "Parse error: {}", msg),
            AppError::NotFound { path } => write!(f, "Not found: {}", path),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for AppError {
    fn from(e: io::Error) -> Self { AppError::Io(e) }
}
```

## Pattern: anyhow with context

```rust
use anyhow::{Context, Result};

fn read_config(path: &str) -> Result<String> {
    std::fs::read_to_string(path)
        .with_context(|| format!("failed to read config at {}", path))
}
```

## Avoid

- `unwrap()` in library code
- `expect()` with vague messages like "unreachable"
- Naked `String` error types in public APIs
