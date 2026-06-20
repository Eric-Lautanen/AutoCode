---
name: ci-cd-pipelines
description: Use when writing or editing CI/CD pipeline configuration - GitHub Actions, GitLab CI, CircleCI, or similar. Covers job structure, caching, secrets, test/build/deploy stages, and common failure patterns. Load when asked to set up, fix, or improve a pipeline.
---

# CI/CD Pipelines

## Overview

CI/CD automates the path from code commit to production deployment. The core principle: **the pipeline should give fast, reliable feedback and never deploy something that hasn't been verified.** A good pipeline catches problems before humans review code; a bad pipeline is slow, flaky, and ignored.

## Pipeline Structure

### Core Stages
Every pipeline should have these stages in order:

1. **Lint/Format** — fastest, catches style issues (< 1 min)
2. **Build** — compile, bundle, verify the project builds (< 5 min)
3. **Test** — unit tests, integration tests (< 10 min)
4. **Deploy** — only after all above pass, and only on the right branch

### Job Dependencies
```
lint ──┐
build ─┤──→ test ──→ deploy
```
- Lint and build can run in parallel (they're independent)
- Tests depend on build (you test the built artifact, not the source)
- Deploy depends on everything passing

## GitHub Actions Specifics

### Workflow Syntax
```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm
      - run: npm ci
      - run: npm run lint

  test:
    needs: lint
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm
      - run: npm ci
      - run: npm test

  deploy:
    needs: test
    if: github.ref == 'refs/heads/main'
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: echo "Deploy step here"
```

### Key Patterns
- **`needs`**: Declares job dependencies (test runs after lint)
- **`if` on deploy**: Only deploy from main branch, not from PRs
- **`cache`**: Use built-in caching for package managers (`cache: npm`, `cache: pip`)
- **Pin action versions**: `actions/checkout@v4` not `@main`

## Caching

### What to Cache
| Ecosystem | Cache path | Cache key |
|-----------|-----------|-----------|
| npm | `~/.npm` or `node_modules` | `hash(package-lock.json)` |
| pip | `~/.cache/pip` | `hash(requirements.txt)` |
| cargo | `~/.cargo/registry`, `target/` | `hash(Cargo.lock)` |
| go | `~/go/pkg/mod`, `~/.cache/go-build` | `hash(go.sum)` |

### Cache Key Strategy
```yaml
- uses: actions/cache@v3
  with:
    path: ~/.npm
    key: npm-${{ hashFiles('package-lock.json') }}
    restore-keys: npm-  # Fall back to any npm cache if exact key misses
```

**Rules:**
- Key on the lock file, not the manifest — lock files change only when deps change
- Use `restore-keys` as a fallback for partial cache hits
- Don't cache `node_modules` for projects with native modules (platform-specific)

## Secrets

### How to Inject Secrets
```yaml
- name: Deploy
  env:
    API_KEY: ${{ secrets.API_KEY }}
  run: deploy.sh
```

### Rules for Secrets
- **Never echo secrets.** Don't `echo $API_KEY` or print env vars in debug mode
- **Use GitHub Secrets.** Store in repository/org settings, reference with `${{ secrets.NAME }}`
- **Least-privilege tokens.** A deploy token should only have deploy permissions, not admin
- **Mask in logs.** GitHub Actions automatically masks `${{ secrets.* }}`, but not env vars printed explicitly
- **Don't put secrets in cache keys or artifact names.** These are visible in the UI

## Test Stages

### Run Tests the Same Way Locally and in CI
```yaml
# BAD — different test command than local
- run: NODE_ENV=ci npm test -- --no-color --force-exit

# GOOD — same command, CI just sets the environment
- run: npm test
  env:
    CI: true  # Most test runners auto-detect CI mode
```

### Test Parallelism
For large test suites, split across multiple workers:
```yaml
- run: npm test -- --shard=1/3  # Worker 1 of 3
- run: npm test -- --shard=2/3
- run: npm test -- --shard=3/3
```

### Handling Flaky Tests
- **Don't retry flaky tests in CI.** Fix them or quarantine them.
- If you must retry, limit to 1 retry and log the retry
- Track flaky tests separately — they erode trust in the pipeline

## Build and Artifact Stages

### What to Produce
- **Compiled binaries** (Rust, Go, Java): The built executable
- **Bundled JS/CSS** (frontend): The `dist/` or `build/` directory
- **Docker images**: Push to a registry
- **Packages**: npm package, Python wheel, crate

### Where to Store Artifacts
```yaml
- uses: actions/upload-artifact@v4
  with:
    name: build-output
    path: dist/
    retention-days: 5  # Don't keep forever
```

## Deploy Stages

### Environment Promotion
```
PR → Staging (auto-deploy on merge to main)
Staging → Production (manual approval or tag-based)
```

### Manual Approval Gates
```yaml
deploy-production:
  needs: test
  environment:
    name: production
    url: https://myapp.com
  runs-on: ubuntu-latest
  steps:
    - run: deploy.sh
```
GitHub requires a manual approval click before jobs with a `production` environment run.

### Rollback Triggers
- Automated: Deploy on health check failure (rollback to previous version)
- Manual: `git revert` + push, or re-deploy the previous artifact
- Always have a rollback plan before deploying

## Common Failures

| Failure | Cause | Fix |
|---------|-------|-----|
| Flaky tests | Non-deterministic test (timing, external service) | Fix the test, mock external deps |
| Cache invalidation | Lock file changed, cache key mismatch | Expected — first run after dep change is slow |
| Environment differences | Works locally, fails in CI | Check Node/Python/Go version, env vars, OS |
| Permission errors | Missing GitHub token scope | Check workflow permissions, use correct token |
| OOM / timeout | Test suite too large for runner | Split tests, increase timeout, use larger runner |
| Stale cache | Cached deps don't match new code | Clear cache, update key strategy |

## Anti-Patterns

- **No caching.** Every CI run installs all deps from scratch. Add caching.
- **Running everything in one job.** Parallelize independent steps for faster feedback.
- **Deploying on every PR.** Deploy only from main/release branches.
- **Not pinning action versions.** `@main` can break your pipeline at any time.
- **Storing secrets in workflow files.** Use GitHub Secrets, not hardcoded values.
- **Ignoring flaky tests.** A flaky pipeline is an ignored pipeline. Fix the flakes.
- **Not testing the pipeline itself.** Test your CI config by intentionally breaking things.
