---
name: monorepo-management
description: Use when working in a monorepo - navigating multiple packages, running scoped commands, managing shared dependencies, understanding build tool configuration (Turborepo, Nx, Bazel, Cargo workspaces). Load when a task involves a repo with multiple packages/apps or when changes span more than one package.
---

# Monorepo Management

## Overview

A monorepo puts multiple projects in one repository for shared tooling, atomic commits, and consistent dependencies. The core principle: **monorepos trade repository simplicity for build complexity — use the right tooling to manage that complexity.** Without scoped builds and dependency graphs, a monorepo becomes slower than separate repos.

## Monorepo Structure

### Common Layout
```
my-monorepo/
├── apps/
│   ├── web/          # Frontend application
│   ├── api/          # Backend API
│   └── admin/        # Admin dashboard
├── packages/
│   ├── ui/           # Shared UI components
│   ├── config/       # Shared configuration
│   └── utils/        # Shared utilities
├── package.json      # Root workspace config
└── turbo.json        # Turborepo pipeline config
```

### Workspace Configuration
```json
// Root package.json
{
    "workspaces": ["apps/*", "packages/*"]
}
```

```toml
# Cargo workspace
[workspace]
members = ["crates/*"]
```

## Scoped Commands

### Running Commands for One Package
```bash
# npm workspaces
npm run build --workspace=apps/web

# Turborepo
turbo run build --filter=web

# Nx
nx build web

# Cargo workspace
cargo build -p my-crate
```

**Rule:** Never run `npm run build` at the root of a monorepo without scoping — it builds everything, which is slow and unnecessary for most changes.

## Shared Packages

### Internal Libraries
- **Versioning**: Use a fixed/locked versioning strategy (all packages same version) or independent versioning
- **Publishing**: Publish shared packages to a private registry or use workspace protocol (`workspace:*`)
- **Dependencies**: Reference workspace packages with `workspace:*` in package.json

```json
// apps/web/package.json
{
    "dependencies": {
        "@myorg/ui": "workspace:*",
        "@myorg/utils": "workspace:*"
    }
}
```

## Dependency Graphs

### Understanding Which Packages Depend on Which
```bash
# Turborepo — visualize the dependency graph
turbo run build --dry-run

# Nx — show the dependency graph
nx graph

# Cargo — show dependency tree
cargo tree
```

### Change Impact
Before making a change to a shared package, check who depends on it:
- A change in `packages/utils` affects `apps/web`, `apps/api`, and `apps/admin`
- A change in `apps/web` affects only `apps/web`

**Rule:** Changes to shared packages require testing all consumers. Changes to apps require testing only that app.

## Build Tools

### Turborepo
```json
// turbo.json
{
    "pipeline": {
        "build": {
            "dependsOn": ["^build"],  // Build dependencies first
            "outputs": ["dist/**"]
        },
        "test": {
            "dependsOn": ["build"]
        },
        "lint": {}
    }
}
```

**Key features:**
- **Task pipelines**: Define build order and dependencies
- **Remote caching**: Cache build outputs across machines (CI + local)
- **Filtering**: `--filter=web` runs only what web depends on

### Nx
- More opinionated than Turborepo
- Computation caching (local + remote)
- Affected command: `nx affected --target=build` — builds only changed packages and their dependents
- Code generation: `nx generate @nx/react:component my-component`

### Cargo Workspaces
```toml
[workspace]
members = ["crates/core", "crates/api", "crates/cli"]

[workspace.dependencies]
serde = { version = "1.0", features = ["derive"] }
```

**Shared dependencies** in `[workspace.dependencies]` ensure all crates use the same version.

## Cross-Package Changes

### Updating a Shared Library
1. Make the change in the shared package
2. Update the shared package's tests
3. Build the shared package
4. Build and test all consumers
5. Verify no consumer is broken

### Safe Interface Changes
- Add new exports (non-breaking)
- Deprecate old exports (non-breaking)
- Remove deprecated exports (breaking — coordinate with consumers)

## CI in a Monorepo

### Only Build/Test What Changed
```yaml
# GitHub Actions with Turborepo
- name: Build affected packages
  run: turbo run build --filter=...[HEAD^1]

- name: Test affected packages
  run: turbo run test --filter=...[HEAD^1]
```

**Affected detection:** Only run CI for packages that changed or packages that depend on changed packages. This is the key to fast CI in a monorepo.

### Common Pitfalls
- **Building everything on every PR**: Too slow. Use affected detection.
- **No caching**: Rebuild packages that haven't changed. Use Turborepo/Nx caching.
- **Circular dependencies**: Package A depends on B, B depends on A. Break the cycle.

## Anti-Patterns

- **No scoped builds.** Building everything on every change is too slow.
- **Circular dependencies between packages.** If A depends on B and B depends on A, extract the shared code into C.
- **Shared config drift.** Each package has its own ESLint/Prettier config that diverges. Use a shared config package.
- **Version skew.** Different packages using different versions of the same dependency. Use workspace-level dependency management.
- **Not using caching.** Without caching, monorepo builds are slower than separate repos.
