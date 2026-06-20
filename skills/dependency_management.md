---
name: dependency-management
description: Use when adding, removing, or upgrading dependencies in any project - npm, pip, cargo, go modules, or others. Covers how to find the right package, check compatibility, install correctly, and handle lock files. Load when any task involves a library that isn't already in the project.
---

# Dependency Management

## Overview

Dependencies are how you avoid reinventing the wheel — but every dependency is also a commitment: to its API, its bugs, its security posture, and its maintenance trajectory. The core principle: **add dependencies deliberately, pin them precisely, and audit them regularly.** A wrong dependency costs more than writing the code yourself.

## Finding Packages

### Where to Search
- **npm**: `npm search <term>` or https://npmjs.com
- **PyPI**: `pip search <term>` (disabled) or https://pypi.org
- **crates.io**: `cargo search <term>` or https://crates.io
- **Go**: `pkg.go.dev` or https://pkg.go.dev

### Evaluating Quality

Before adding a dependency, check:

1. **Maintenance**: Last publish date, commit frequency, open issues/PRs ratio
2. **Popularity**: Download count, GitHub stars — more users = more battle-tested
3. **License**: Permissive (MIT, Apache-2.0, BSD) vs. copyleft (GPL) — know your project's requirements
4. **Dependencies of the dependency**: A lightweight package with 50 transitive deps isn't lightweight
5. **Alternatives**: Is there a simpler option? A stdlib equivalent? Can you write it in 20 lines?

**Red flags:**
- No updates in 2+ years with open security issues
- Single maintainer with no bus factor
- Excessive transitive dependencies for simple functionality
- Unclear or no license

## Installing Correctly

### Dev vs. Prod Dependencies

| Ecosystem | Prod | Dev |
|-----------|------|-----|
| npm | `npm install <pkg>` | `npm install -D <pkg>` |
| pip | `pip install <pkg>` | `pip install <pkg>` (separate in requirements-dev.txt) |
| cargo | `[dependencies]` | `[dev-dependencies]` |
| go | `go get <pkg>` | No distinction in go.mod |

**Rule:** If it's only needed for building/testing/linting, it's a dev dependency. If it's needed at runtime, it's a prod dependency. Never put test frameworks in prod dependencies.

### Version Pinning Strategies

- **Exact pin** (`1.2.3`): Maximum reproducibility. Use for apps, not libraries.
- **Caret range** (`^1.2.3`): Allow minor/patch updates. Default in npm and Cargo. Good for most cases.
- **Tilde range** (`~1.2.3`): Allow patch updates only. Use when you need stability but want security fixes.
- **Wildcard** (`*`, `latest`): Never use in production. Acceptable for quick prototypes only.

**For libraries you publish:** Use caret ranges. Consumers should be able to get patches and compatible minor updates.

**For applications:** Pin exact versions in production. Reproducibility matters more than flexibility.

## Lock Files

| Ecosystem | Lock file | Commit it? |
|-----------|-----------|------------|
| npm | `package-lock.json` | Yes, for apps. No for libraries. |
| yarn | `yarn.lock` | Yes, for apps. No for libraries. |
| pip | `requirements.txt` (pinned) | Yes |
| uv | `uv.lock` | Yes |
| cargo | `Cargo.lock` | Yes, for apps/binaries. No for libraries. |
| go | `go.sum` | Yes, always |

**When to regenerate:**
- After changing dependency versions in the manifest
- When builds fail due to a corrupted lock file
- Never delete and regenerate just because — you may pick up breaking changes

**Regeneration commands:**
```bash
npm install              # Regenerates package-lock.json
cargo update             # Updates Cargo.lock within version ranges
go mod tidy              # Cleans up go.sum
pip compile              # Regenerates from pip-tools
```

## Compatibility Checks

Before adding a dependency, verify:

1. **Language/runtime version**: Does the package support your project's minimum version? Check `engines` in package.json, `python_requires` in pyproject.toml, `rust-version` in Cargo.toml.
2. **Peer dependencies**: Some packages require a specific version of a framework (e.g., a React component library requiring React 18). Mismatches cause silent bugs.
3. **Platform support**: Does the package work on your target OS/architecture? Native modules (node-gyp, C extensions) may not.
4. **Ecosystem compatibility**: ESM vs. CJS in Node, Python 3 vs. 2, Rust edition compatibility.

## Auditing for Security Issues

Run these regularly:

```bash
npm audit              # Check for known vulnerabilities
npm audit fix          # Auto-fix where possible
cargo audit            # Check Rust advisory database
pip audit              # Check Python packages (install: pip install pip-audit)
```

**When a vulnerability is found:**
1. Check if the vulnerable code path is actually used (many vulns are in unused features)
2. Update to a patched version if available
3. If no patch exists, evaluate alternatives or implement a workaround
4. Never ignore a high/critical vulnerability in a prod dependency

## Removing Unused Dependencies

1. Find unused deps: `depcheck` (npm), `pip-autoremove` (Python), `cargo-udeps` (Rust)
2. Remove from the manifest file
3. Run the install command to update the lock file
4. Build and test to confirm nothing was actually using it

## Vendoring vs. Registry Dependencies

- **Registry (default)**: Download from npm/PyPI/crates.io. Easier to update, standard workflow.
- **Vendoring**: Copy the source into your project. Use when you need to modify the dependency, when the registry is unavailable, or for maximum reproducibility.

**Vendor when:**
- You need to patch a dependency and can't wait for upstream
- Building in an air-gapped environment
- The dependency is tiny and you want zero external dependencies

**Don't vendor when:**
- You can use the registry version as-is
- You won't maintain the vendored copy (security updates)

## Anti-Patterns

- **Adding a dependency for a one-liner.** If you can write it in 5 lines, do that instead.
- **Not checking transitive dependencies.** A small package that pulls in 200MB of dependencies is not small.
- **Pinning to `latest` or `*`.** Your build will break when the package publishes a breaking change.
- **Ignoring audit warnings.** Known vulnerabilities get exploited.
- **Mixing dev and prod dependencies.** Test frameworks in production increase attack surface and bundle size.
