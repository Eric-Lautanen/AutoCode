---
name: go-patterns
description: Use when writing Go code - idiomatic patterns, error handling, interfaces, goroutines, channels, and project layout. Load when any task involves writing, reviewing, or debugging Go code.
---

# Go Patterns

## Overview

Go's design philosophy is simplicity and explicitness. The core principle: **write Go the way Go was designed to be written — no magic, no hidden control flow, no clever abstractions.** If you're writing Go that looks like Java or Rust, you're fighting the language. Embrace the verbosity — it's intentional.

## Error Handling

### Always Check Errors
```go
// NEVER ignore errors
result, _ := DoSomething()  // BAD

// ALWAYS check
result, err := DoSomething()
if err != nil {
    return fmt.Errorf("doing something: %w", err)
}
```

### Wrapping Errors
```go
import "errors"

// Use %w to wrap — allows errors.Is() and errors.As()
if err != nil {
    return fmt.Errorf("processing order %d: %w", orderID, err)
}

// Check for specific errors
if errors.Is(err, sql.ErrNoRows) {
    return ErrNotFound
}

// Check for error type
var appErr *AppError
if errors.As(err, &appErr) {
    return appErr.StatusCode
}
```

### Sentinel Errors
```go
var (
    ErrNotFound    = errors.New("not found")
    ErrUnauthorized = errors.New("unauthorized")
    ErrConflict    = errors.New("conflict")
)
```

## Interfaces

### Small Interfaces
```go
// GOOD — one method, easy to implement
type Reader interface {
    Read(p []byte) (n int, err error)
}

type Writer interface {
    Write(p []byte) (n int, err error)
}

// BAD — large interface, hard to implement
type UserRepository interface {
    Get(id int) (*User, error)
    List(filter Filter) ([]User, error)
    Create(user *User) error
    Update(user *User) error
    Delete(id int) error
    GetByEmail(email string) (*User, error)
    // ... 10 more methods
}
```

**Rule:** "The bigger the interface, the weaker the abstraction." — Rob Pike

### Implicit Implementation
```go
// Any type with a Read method satisfies Reader — no "implements" keyword
type MyReader struct{}

func (r MyReader) Read(p []byte) (int, error) {
    // ...
}
// MyReader now implements Reader automatically
```

### Accept Interfaces, Return Structs
```go
// Function accepts interface for flexibility
func ProcessData(r io.Reader) error {
    // Works with any Reader: files, network, bytes, etc.
}

// Function returns concrete type for clarity
func NewUser(name string) *User {
    return &User{Name: name}
}
```

## Goroutines and Channels

### Spawning Safely
```go
// Always handle goroutine lifecycle
func process(ctx context.Context, items []Item) error {
    errCh := make(chan error, 1)
    go func() {
        defer close(errCh)
        for _, item := range items {
            select {
            case <-ctx.Done():
                errCh <- ctx.Err()
                return
            default:
                if err := processItem(item); err != nil {
                    errCh <- err
                    return
                }
            }
        }
        errCh <- nil
    }()
    return <-errCh
}
```

### Channel Directions
```go
func producer(out chan<- int) {  // Send-only channel
    out <- 42
    close(out)
}

func consumer(in <-chan int) {  // Receive-only channel
    val := <-in
}
```

### Select Statement
```go
select {
case result := <-ch1:
    handle(result)
case err := <-errCh:
    log.Error(err)
case <-time.After(5 * time.Second):
    return ErrTimeout
case <-ctx.Done():
    return ctx.Err()
}
```

## Defer

### Cleanup Pattern
```go
func ReadFile(path string) (string, error) {
    f, err := os.Open(path)
    if err != nil {
        return "", err
    }
    defer f.Close()  // Always runs, even on error
    
    data, err := io.ReadAll(f)
    return string(data), err
}
```

### Deferred in Loops — Don't
```go
// BAD — files don't close until the function returns, not the loop iteration
for _, path := range paths {
    f, err := os.Open(path)
    defer f.Close()  // Accumulates open files!
    process(f)
}

// GOOD — use a helper function
for _, path := range paths {
    if err := processFile(path); err != nil {
        return err
    }
}

func processFile(path string) error {
    f, err := os.Open(path)
    if err != nil {
        return err
    }
    defer f.Close()  // Closes after each call
    return process(f)
}
```

### Order of Execution
Defers run in LIFO order (last deferred, first executed):
```go
defer fmt.Println("first")   // Runs last
defer fmt.Println("second")  // Runs first
```

## Structs and Methods

### Value vs. Pointer Receivers
```go
// Value receiver — doesn't modify the struct, safe for small types
func (u User) Name() string {
    return u.name
}

// Pointer receiver — can modify, avoids copying large structs
func (u *User) SetName(name string) {
    u.name = name
}
```

**Rule:** Use pointer receivers when you need to modify the receiver, or when the struct is large. Use value receivers for small, immutable types.

### Embedding Over Inheritance
```go
// Go has no inheritance — use composition with embedding
type BaseService struct {
    logger *slog.Logger
    db     *sql.DB
}

func (s *BaseService) Log(msg string) {
    s.logger.Info(msg)
}

type UserService struct {
    BaseService  // Embedded — inherits Log() and access to logger/db
}

func (s *UserService) GetUser(id int) (*User, error) {
    s.Log("getting user", "id", id)
    // ...
}
```

## Project Layout

```
my-app/
├── cmd/
│   └── myapp/
│       └── main.go        # Entry point
├── internal/              # Private application code
│   ├── handler/           # HTTP handlers
│   ├── service/           # Business logic
│   └── repository/       # Data access
├── pkg/                   # Public library code (optional)
├── go.mod                 # Module definition
└── go.sum                 # Dependency checksums
```

**`internal/`** is enforced by the Go compiler — code outside the module cannot import it.

## Common Mistakes

- **Goroutine leaks**: Goroutines that never return (blocked on channel, infinite loop). Always use `context.Context` for cancellation.
- **Nil map writes**: `var m map[string]int; m["key"] = 1` panics. Initialize: `m = make(map[string]int)`
- **Range loop variable capture**: Pre-Go 1.22, `for _, v := range items` captures the same variable. Use `v := v` inside the loop body.
- **Not closing channels**: Producers should close channels, not consumers. Closed channels return zero values forever.
- **Ignoring context cancellation**: Long-running operations should check `ctx.Done()`.

## Anti-Patterns

- **Large interfaces.** Keep interfaces small (1-3 methods).
- **Returning interfaces from constructors.** Return concrete types; let callers decide what interface they need.
- **Panic for error handling.** Use `error` returns. Panic only for truly unrecoverable programmer errors.
- **Not using context.Context.** Every function that does I/O should accept a context.
- **Goroutines without lifecycle management.** Every goroutine should have a clear exit condition.
