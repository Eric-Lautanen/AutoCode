---
name: typescript-patterns
description: Use when writing TypeScript - typing functions, generics, utility types, narrowing, and structuring types across a codebase. Load when a task involves TypeScript type errors, designing types for a new feature, or improving type safety in existing code.
---

# TypeScript Patterns

## Overview

TypeScript's type system catches bugs at compile time that JavaScript only catches at runtime. The core principle: **let the compiler do the work — if your types are precise, the compiler catches most bugs automatically.** Loose types (`any`, broad unions) defeat the purpose; precise types make refactoring safe and autocomplete useful.

## Type vs. Interface

| Feature | `type` | `interface` |
|---------|--------|-------------|
| Object shapes | ✅ | ✅ |
| Union types | ✅ `type X = A \| B` | ❌ |
| Intersection | ✅ `type X = A & B` | `extends` |
| Declaration merging | ❌ | ✅ (useful for library augmenting) |
| Computed properties | ✅ `{ [K in T]: V }` | ❌ |

**Rule:** Use `interface` for object shapes that might be extended or merged. Use `type` for unions, intersections, and computed types. Be consistent within a project.

## Generics

### When They're Needed
```typescript
// Without generics — loses type information
function firstItem(arr: any[]): any { return arr[0]; }
const name = firstItem(["Alice"]); // type: any — lost!

// With generics — preserves type information
function firstItem<T>(arr: T[]): T { return arr[0]; }
const name = firstItem(["Alice"]); // type: string — preserved!
```

### Constraints
```typescript
// Constrain T to have an 'id' property
function getById<T extends { id: string }>(items: T[], id: string): T | undefined {
    return items.find(item => item.id === id);
}
```

### Defaults
```typescript
// Default type parameter
interface PaginatedResponse<T, Meta = { total: number }> {
    data: T[];
    meta: Meta;
}
```

### Avoid Over-Generalizing
```typescript
// BAD — too generic, loses all type safety
function process<T>(value: T): T { return value; }

// GOOD — generic only where needed
function parseJSON<T>(text: string): T { return JSON.parse(text); }
```

## Utility Types

| Type | What it does | Example |
|------|-------------|---------|
| `Partial<T>` | All properties optional | `Partial<User>` for update payloads |
| `Required<T>` | All properties required | `Required<Config>` after validation |
| `Pick<T, K>` | Subset of properties | `Pick<User, "id" \| "name">` |
| `Omit<T, K>` | Exclude properties | `Omit<User, "password">` |
| `Record<K, V>` | Object with key/value types | `Record<string, number>` |
| `ReturnType<F>` | Return type of a function | `ReturnType<typeof fetchUser>` |
| `Parameters<F>` | Parameter types of a function | `Parameters<typeof handler>` |
| `Readonly<T>` | All properties readonly | `Readonly<Config>` |
| `Exclude<A, B>` | Members of A not in B | `Exclude<"a" \| "b", "a">` = `"b"` |
| `Extract<A, B>` | Members of A in B | `Extract<"a" \| "b", "a">` = `"a"` |

## Narrowing

### typeof
```typescript
function process(value: string | number) {
    if (typeof value === "string") {
        return value.toUpperCase(); // TypeScript knows: string
    }
    return value.toFixed(2); // TypeScript knows: number
}
```

### instanceof
```typescript
if (error instanceof ValidationError) {
    console.log(error.fields); // TypeScript knows: ValidationError
}
```

### Discriminated Unions
```typescript
type Result = 
    | { status: "success"; data: User }
    | { status: "error"; message: string };

function handle(result: Result) {
    if (result.status === "success") {
        console.log(result.data); // TypeScript knows: success branch
    } else {
        console.log(result.message); // TypeScript knows: error branch
    }
}
```

### Type Predicates
```typescript
function isUser(value: unknown): value is User {
    return typeof value === "object" && value !== null && "id" in value;
}

if (isUser(data)) {
    console.log(data.name); // TypeScript knows: User
}
```

## Unknown vs. Any

```typescript
// NEVER use any — it disables all type checking
const data: any = JSON.parse(text);
data.nonexistent.method(); // No error at compile time, crashes at runtime

// ALWAYS use unknown — requires narrowing before use
const data: unknown = JSON.parse(text);
data.nonexistent.method(); // Compile error: Object is of type 'unknown'

if (typeof data === "object" && data !== null && "name" in data) {
    console.log((data as { name: string }).name); // Safe after narrowing
}
```

**Rule:** `any` is never acceptable except as a last resort escape hatch with a comment explaining why. `unknown` is the correct type for values you don't know the shape of.

## Readonly and Const Assertions

```typescript
// Const assertion — literal types, readonly
const ROUTES = {
    home: "/",
    users: "/users",
} as const;
// Type: { readonly home: "/"; readonly users: "/users" }

// Readonly for function parameters — prevent mutation
function freeze<T>(obj: T): Readonly<T> { return Object.freeze(obj); }
```

## Module Augmentation and Declaration Merging

```typescript
// Add a property to an existing module's type
declare module "express" {
    interface Request {
        userId?: string;  // Our custom property
    }
}
```

**When to use:** When a library's types don't include a property that the library actually supports (e.g., middleware adds `req.userId`).

**When NOT to use:** To work around type errors that indicate a real problem. Fix the code, not the types.

## tsconfig Settings That Matter

```json
{
    "compilerOptions": {
        "strict": true,                        // Enable all strict checks
        "noUncheckedIndexedAccess": true,       // arr[0] returns T | undefined
        "exactOptionalPropertyTypes": true,     // Optional props can't be explicitly undefined
        "noImplicitReturns": true,              // All code paths must return
        "noFallthroughCasesInSwitch": true      // Switch cases must break/return
    }
}
```

**`noUncheckedIndexedAccess`** is the most impactful: `arr[0]` returns `T | undefined` instead of `T`, preventing the most common runtime error (accessing an array element that doesn't exist).

## Windows-Specific Notes

### Windows Path Types in TypeScript
```typescript
// Use path.join for cross-platform compatibility
import path from 'path';

const filePath = path.join('data', 'users', `${userId}.json`);
// Works on both Windows (backslash) and Unix (forward slash)

// For type-safe path handling
function isWindowsPath(path: string): boolean {
    return /^[A-Za-z]:[/\\\\]/.test(path) || path.startsWith('\\\\');
}
```

### Windows Development Environment
- **Line endings**: Configure VS Code for consistent LF:
  ```json
  {
    "files.eol": "\n",
    "editor.formatOnSave": true
  }
  ```
- **TypeScript compiler**: Use `tsc` with `--pretty` for colored output on Windows terminals
- **npm scripts**: Use `cross-env` for environment variables:
  ```bash
  npm install --save-dev cross-env
  ```
  ```json
  {
    "scripts": {
      "build": "cross-env NODE_ENV=production tsc"
    }
  }
  ```

### Windows File System Types
```typescript
// Handle Windows-specific file system operations
import fs from 'fs';
import path from 'path';

function readConfigWindows(configPath: string): Config {
    // Resolve to absolute path on Windows
    const resolved = path.resolve(configPath);
    
    // Check for long path (over 260 chars)
    if (process.platform === 'win32' && resolved.length > 260) {
        throw new Error(`Path too long: ${resolved}`);
    }
    
    return JSON.parse(fs.readFileSync(resolved, 'utf-8'));
}
```

## Anti-Patterns

- **Using `any`.** It defeats the type system. Use `unknown` instead.
- **Type assertions without checks.** `data as User` without verifying it actually is a User.
- **Over-generalized generics.** If the generic doesn't preserve type information, it's not helping.
- **Not using discriminated unions.** If you're checking `if (x.type === "a")`, use a discriminated union to get narrowing.
- **Ignoring `noUncheckedIndexedAccess`.** Array access without undefined checks is a runtime error waiting to happen.
