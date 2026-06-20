---
name: environment-and-config
description: Use when dealing with environment variables, configuration files, secrets, .env files, or differences between dev/staging/production environments. Load when a task involves setting up an environment, configuring a service, or debugging a config-related failure.
---

# Environment and Config

## Overview

Configuration is how the same code runs differently in dev, staging, and production. The core principle: **code is the same everywhere; configuration varies per environment.** Secrets are a special category of configuration that must never be committed, logged, or exposed in error messages.

## Environment Variable Patterns

### Reading Environment Variables
```python
# Python — with default
port = int(os.environ.get("PORT", "8000"))

# Python — required, fail fast
database_url = os.environ["DATABASE_URL"]  # KeyError if missing

# Node.js
const port = process.env.PORT || 3000;

# Rust
let port: u16 = env::var("PORT").unwrap_or_else(|_| "8000".parse().unwrap());
```

### Required vs. Optional
- **Required**: The app cannot start without it. Fail fast with a clear error message: `"DATABASE_URL is required. Set it in .env or environment."`
- **Optional with default**: Provide a sensible default that works in development. Document what the default is.

**Rule:** Every environment variable should have a documented default or a clear error message when missing. Silent failures from missing config are the worst kind of bug.

## .env Files

### Format
```bash
# .env — never commit this file
DATABASE_URL=postgres://localhost:5432/myapp_dev
API_KEY=sk_test_abc123
LOG_LEVEL=debug
PORT=3000

# Comments are allowed
# No quotes needed for simple values
# Quotes are needed for values with spaces
DATABASE_NAME="my app db"
```

### Loading Libraries
| Language | Library | Usage |
|----------|---------|-------|
| Python | `python-dotenv` | `load_dotenv()` in entry point |
| Node.js | `dotenv` | `require('dotenv').config()` before other imports |
| Rust | `dotenvy` | `dotenvy::dotenv().ok();` |
| Go | `godotenv` | `godotenv.Load()` |

### Rules for .env Files
- **Never commit `.env`.** Add `.env` to `.gitignore` on day one.
- **Commit `.env.example`.** Include all required variables with placeholder values. This documents what config the app needs.
- **One `.env` per environment.** Dev, test, staging, production each have their own.
- **No production secrets in dev `.env`.** Use test/sandbox credentials for development.

## Config File Formats

| Format | Best for | Pros | Cons |
|--------|----------|------|------|
| JSON | Simple configs, browser | Parsed everywhere | No comments, strict syntax |
| YAML | Docker Compose, CI configs | Comments, readable, anchors | Type coercion gotchas (yes=true, no=false) |
| TOML | Python (pyproject.toml), Rust | Comments, clear types, simple | Less widely supported |
| INI | Legacy apps, simple configs | Familiar, simple | No nested structures, inconsistent parsing |

**When to use each:**
- **JSON**: When the consumer is JavaScript or when you need strict typing
- **YAML**: When humans write it (Docker Compose, CI, Kubernetes)
- **TOML**: When you want a modern alternative to YAML with fewer surprises
- **INI**: Only for legacy compatibility

## Environment Parity

Keep dev, staging, and production as similar as possible:

- **Same database engine.** Don't use SQLite in dev and PostgreSQL in prod.
- **Same dependency versions.** Lock files ensure this.
- **Same configuration structure.** Same env var names, same config keys.
- **Same data shape.** Use production-like data in dev (anonymized).

**Allowed differences:**
- Log levels (debug in dev, warn in prod)
- Cache TTLs (shorter in dev for faster iteration)
- Feature flags (new features off in prod, on in dev)
- Secret values (test API keys vs. production API keys)

## Secrets Management

### Environment Variables vs. Secret Managers

| Approach | Good for | Limitation |
|----------|----------|------------|
| Env vars | Small projects, few secrets | No rotation, visible in process list |
| .env files | Development only | Must never be committed |
| Secret managers (Vault, AWS SM) | Production | Requires infrastructure, but proper rotation and access control |

### Secret Rotation
- Rotate secrets regularly (every 90 days for production)
- Support two active secrets during rotation (old and new both valid)
- Automate rotation where possible
- Never hardcode secrets — even test secrets change

### Debugging Config Issues
1. **Print effective config at startup.** Log the resolved config (with secrets masked) when the app starts.
2. **Validate config on load.** Check types, ranges, and required fields before the app starts serving requests.
3. **Fail fast.** If a required config is missing, crash immediately with a clear message — don't limp along.

```python
# Good: validate at startup
config = load_config()
if not config.database_url:
    raise SystemExit("DATABASE_URL is required. See .env.example")
```

## Feature Flags via Config

Feature flags let you deploy code without activating it:

```bash
# .env
FEATURE_NEW_CHECKOUT=false
FEATURE_DARK_MODE=true
```

**Patterns:**
- **Kill switches**: `FEATURE_X_ENABLED=true` — set to false to disable instantly
- **Gradual rollout**: `FEATURE_X_ROLLOUT_PERCENT=25` — enable for 25% of users
- **A/B testing**: `FEATURE_X_VARIANT=b` — control which variant users see

**Rules:**
- Clean up feature flags after the feature is fully rolled out
- Don't nest feature flags (flag A depends on flag B) — it creates a combinatorial explosion
- Default new flags to `false` in production

## Anti-Patterns

- **Hardcoding config values.** `const dbUrl = "postgres://prod-server/mydb"` — this will end up in git.
- **Committing .env files.** Even "just for reference" — secrets in git history are forever.
- **Logging secrets.** `console.log("Connecting to", dbUrl)` — log files are not secure.
- **Different config structures per environment.** If dev uses JSON and prod uses env vars, you'll have bugs that only appear in production.
- **Silent defaults for required config.** If the database URL is missing, the app should crash, not silently use localhost.
- **Not documenting required config.** New developers should be able to copy `.env.example` and start the app.
