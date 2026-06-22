```markdown
---
name: rust_guru
description: Use when writing or reviewing Rust code for best practices, performance tips, and modern standard library techniques (Rust 1.96+). Load when asked about Rust programming, optimization, or idioms. Aims for std-only, no external crates, stack ownership, and no_std compatibility.
---

# Rust Guru

## 🚀 Overview

Rust 1.96.0 (May 2026) brings significant ergonomic improvements, particularly in range types and assertion macros 【turn0search0】【turn0search4】. This guide covers modern Rust techniques using **only the standard library** (`std`/`core`/`alloc`) to ensure fast, reliable, and stack-owned code. Key principles: **ownership**, **zero-cost abstractions**, **unsafe code best practices**, and **no_std** compatibility for embedded systems.

## 🆕 Rust 1.96+ Key Features

### 1. New `core::range` Types (Copy Ranges)

Rust 1.96 stabilizes new range types that implement `IntoIterator` instead of `Iterator`, enabling them to be `Copy` 【turn0search0】【turn0search4】. This resolves a long-standing ergonomic issue.

```rust
use core::range::Range;

// Previously impossible: Range wasn't Copy!
#[derive(Clone, Copy)]
pub struct Span(Range<usize>);

impl Span {
    pub fn of(self, s: &str) -> &str {
        &s[self.0] // Direct indexing with Copy range
    }
    
    pub fn new(start: usize, end: usize) -> Self {
        Span(Range { start, end })
    }
}

// Usage
fn main() {
    let span = Span::new(0, 5);
    let text = "Hello, world!";
    println!("{}", span.of(text)); // "Hello"
    
    // Copy semantics work as expected
    let span2 = span;
    println!("{}", span2.of(text)); // Still "Hello"
}
```

**Migration Guidance**: Public APIs should use `impl RangeBounds` to accept both legacy and new range types 【turn0search4】. Full migration to `core::range` types is expected in the **Rust 2027 Edition**.

### 2. Assert Matching Macros

New `assert_matches!` and `debug_assert_matches!` macros with improved error reporting 【turn0search0】【turn0search4】.

```rust
use core::assert_matches;

// Basic pattern matching
fn check_dice_roll(roll: u32) {
    assert_matches!(roll, 1..=6, "Dice roll must be between 1 and 6, got {}", roll);
}

// Complex patterns
enum Command {
    Quit,
    Write(String),
    Execute { cmd: String, args: Vec<String> },
}

fn process_command(cmd: Command) {
    assert_matches!(
        cmd,
        Command::Quit | Command::Write(_) | Command::Execute { .. },
        "Unknown command variant"
    );
}

// Debug-only assertions (zero cost in release builds)
fn debug_check_state(state: &State) {
    debug_assert_matches!(state.mode, Mode::Running, "System should be running in debug mode");
}
```

**Note**: These macros are **not** in the standard prelude to avoid collisions with popular third-party crates 【turn0search0】【turn0search3】. You must explicitly import them.

### 3. WebAssembly Linker Changes

WebAssembly targets no longer pass `--allow-undefined` to the linker by default, making undefined symbols a hard error instead of silently becoming Wasm imports 【turn0search0】【turn0search4】. This change helps catch bugs earlier.

### 4. Const Generics Progress

The const generics system is being overhauled. The `min_generic_const_args` prototype aims to address limitations where generic parameters couldn't be used in const generic arguments 【turn0search4】. This enables patterns like:

```rust
// Future Rust (2027 Edition)
struct Matrix<T, const ROWS: usize, const COLS: usize> {
    data: [[T; COLS]; ROWS],
}

impl<T, const ROWS: usize, const COLS: usize> Matrix<T, ROWS, COLS> {
    fn new() -> Self where T: Default {
        Self {
            data: core::array::from_fn(|_| core::array::from_fn(|_| T::default()))
        }
    }
}
```

## 🏗️ Ownership & Borrowing: Advanced Patterns

### 1. Lifetime Annotations in Complex Structures

```rust
// Tree structure with parent references
struct Node<'a> {
    data: i32,
    parent: Option<&'a Node<'a>>, // Parent must outlive child
    children: Vec<&'a Node<'a>>,  // Children borrow parent
}

impl<'a> Node<'a> {
    fn new(data: i32) -> Self {
        Node {
            data,
            parent: None,
            children: Vec::new(),
        }
    }
    
    fn add_child(&'a self, child: &'a mut Node<'a>) {
        child.parent = Some(self);
        self.children.push(child);
    }
}

// Arena allocation pattern (no external crates)
struct Arena<T> {
    data: Vec<T>,
}

impl<T> Arena<T> {
    fn new() -> Self {
        Arena { data: Vec::new() }
    }
    
    fn alloc(&mut self, value: T) -> &mut T {
        self.data.push(value);
        self.data.last_mut().unwrap()
    }
}

// Usage: Arena keeps everything alive
let mut arena = Arena::new();
let root = arena.alloc(Node::new(42));
let child = arena.alloc(Node::new(10));
root.add_child(child);
```

### 2. PhantomData for Lifetime Management

```rust
use std::marker::PhantomData;

// Iterator that yields references but doesn't own data
struct RefIterator<'a, T> {
    data: *const T, // Raw pointer for flexibility
    _marker: PhantomData<&'a T>, // Lifetime tracking
}

impl<'a, T> Iterator for RefIterator<'a, T> {
    type Item = &'a T;
    
    fn next(&mut self) -> Option<Self::Item> {
        // Implementation
        unsafe { Some(&*self.data) }
    }
}

// Self-referential struct (careful!)
struct SelfReferential {
    data: String,
    reference: *const String,
}

impl SelfReferential {
    fn new(data: String) -> Self {
        let mut s = SelfReferential {
            data,
            reference: core::ptr::null(),
        };
        s.reference = &s.data as *const String;
        s
    }
    
    fn get_ref(&self) -> &String {
        unsafe { &*self.reference }
    }
}
```

### 3. Smart Pointer Patterns Without Crates

```rust
use std::rc::Rc;
use std::cell::RefCell;
use std::sync::{Arc, Mutex};

// Reference-counted mutable data (single-threaded)
let shared_data = Rc::new(RefCell::new(Vec::new()));

// Thread-safe reference-counted mutable data
let shared_arc = Arc::new(Mutex::new(HashMap::new()));

// Custom smart pointer with Drop
struct Box<T> {
    ptr: *mut T,
}

impl<T> Drop for Box<T> {
    fn drop(&mut self) {
        unsafe {
            std::ptr::drop_in_place(self.ptr);
            std::alloc::dealloc(self.ptr as *mut u8, std::alloc::Layout::new::<T>());
        }
    }
}

impl<T> Box<T> {
    fn new(value: T) -> Self {
        let layout = std::alloc::Layout::new::<T>();
        let ptr = unsafe { std::alloc::alloc(layout) as *mut T };
        unsafe { ptr::write(ptr, value) };
        Box { ptr }
    }
    
    fn get(&self) -> &T {
        unsafe { &*self.ptr }
    }
    
    fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.ptr }
    }
}
```

## ⚡ Async/Await: Zero-Cost Concurrency

Rust's async/await is zero-cost and doesn't require external crates for basic functionality.

### Basic Async Patterns

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

// Manual Future implementation
struct TimerFuture {
    shared_state: Arc<Mutex<SharedState>>,
}

struct SharedState {
    completed: bool,
    waker: Option<Waker>,
}

impl Future for TimerFuture {
    type Output = ();
    
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut shared_state = self.shared_state.lock().unwrap();
        
        if shared_state.completed {
            Poll::Ready(())
        } else {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

// Async function (compiler generates state machine)
async fn async_operation() -> u32 {
    // Async body
    42
}

// Async block
let future = async {
    let result1 = async_operation().await;
    let result2 = another_async().await;
    result1 + result2
};
```

### Minimal Async Runtime (No External Crates)

<details>
<summary>📖 Building a Minimal Async Runtime</summary>

```rust
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll, Waker};
use std::sync::{Arc, Mutex};
use std::collections::VecDeque;

// Simple task queue
struct TaskQueue {
    queue: Mutex<VecDeque<Pin<Box<dyn Future<Output = ()> + Send>>>>,
    wakers: Mutex<Vec<Waker>>,
}

impl TaskQueue {
    fn new() -> Self {
        TaskQueue {
            queue: Mutex::new(VecDeque::new()),
            wakers: Mutex::new(Vec::new()),
        }
    }
    
    fn spawn<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let future = Box::pin(future);
        self.queue.lock().unwrap().push_back(future);
        
        // Wake up executors
        for waker in self.wakers.lock().unwrap().iter() {
            waker.wake_by_ref();
        }
    }
    
    fn run_one(&self) -> bool {
        let future = self.queue.lock().unwrap().pop_front();
        
        if let Some(mut future) = future {
            // Create a waker for this task
            let waker = noop_waker();
            let mut cx = Context::from_waker(&waker);
            
            // Poll the future
            if future.as_mut().poll(&mut cx).is_pending() {
                // Re-queue if not ready
                self.queue.lock().unwrap().push_back(future);
            }
            true
        } else {
            false
        }
    }
}

// No-op waker for single-threaded execution
fn noop_waker() -> Waker {
    use std::task::{RawWaker, RawWakerVTable};
    
    static VTABLE: RawWakerVTable = RawWakerVTable::new(
        |_| RawWaker::new(std::ptr::null(), &VTABLE),
        |_| {},
        |_| {},
        |_| {},
    );
    
    unsafe { Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

// Usage
fn main() {
    let queue = TaskQueue::new();
    
    queue.spawn(async {
        println!("Task 1 started");
        // Simulate async work
        std::future::ready(()).await;
        println!("Task 1 completed");
    });
    
    queue.spawn(async {
        println!("Task 2 started");
        // Simulate async work
        std::future::ready(()).await;
        println!("Task 2 completed");
    });
    
    // Run tasks until completion
    while queue.run_one() {}
}
```

</details>

### Async Embedded Patterns

For embedded systems, consider interrupt-driven concurrency or RTIC (Real-Time Interrupt-driven Concurrency) patterns 【turn0search9】:

```rust
// RTIC-style task scheduling (simplified)
#[no_std]
#[no_main]

struct Shared {
    counter: u32,
}

struct Local {
    led: Led,
}

#[rtic::app(device = pac, dispatchers = [USART1, USART2])]
mod app {
    use super::*;
    
    #[shared]
    struct Shared {
        counter: u32,
    }
    
    #[local]
    struct Local {
        led: Led,
    }
    
    #[init]
    fn init(_: init::Context) -> (Shared, Local) {
        // Initialization
        (Shared { counter: 0 }, Local { led: Led::new() })
    }
    
    #[task]
    fn toggle_led(_: toggle_led::Context) {
        // Toggle LED
    }
    
    #[task(binds = USART1)]
    fn usart1_interrupt(_: usart1_interrupt::Context) {
        // Handle USART interrupt
    }
}
```

## 🛡️ Unsafe Code Guidelines

### When to Use Unsafe

1. **FFI (Foreign Function Interface)**: Calling C functions
2. **Performance**: Implementing data structures with raw pointers
3. **Hardware Access**: Embedded systems programming
4. **Implementing Abstractions**: Building safe interfaces on top of unsafe primitives

### Unsafe Best Practices

```rust
// 1. Minimize unsafe scope
unsafe fn dangerous_operation() -> i32 {
    // Small, well-documented unsafe block
    let ptr = 0x12345678 as *const i32;
    unsafe { *ptr }
}

// 2. Wrap unsafe in safe abstractions
pub struct SafeWrapper {
    inner: *mut Something,
}

impl SafeWrapper {
    pub fn new() -> Self {
        let ptr = unsafe { allocate_something() };
        SafeWrapper { inner: ptr }
    }
    
    pub fn use_it(&self) -> i32 {
        // Safe interface to unsafe implementation
        unsafe { use_something(self.inner) }
    }
}

impl Drop for SafeWrapper {
    fn drop(&mut self) {
        unsafe { deallocate_something(self.inner) }
    }
}

// 3. Document safety invariants
/// Creates a string from raw parts.
/// 
/// # Safety
/// 
/// The pointer must be valid for reads of `len` bytes.
/// The memory must not be modified during the lifetime of this string.
pub unsafe fn from_raw_parts(ptr: *const u8, len: usize) -> String {
    // Implementation
}
```

### Common Unsafe Patterns

<details>
<summary>🔧 Advanced Unsafe Techniques</summary>

```rust
// 1. Union types for type punning
#[repr(C)]
union MyUnion {
    f: f32,
    u: u32,
}

impl MyUnion {
    fn new(f: f32) -> Self {
        MyUnion { f }
    }
    
    fn as_u32(&self) -> u32 {
        // Reading from a union field is unsafe
        unsafe { self.u }
    }
}

// 2. Variadic functions (FFI)
extern "C" {
    fn printf(format: *const i8, ...) -> i32;
}

fn call_printf() {
    let format = b"%d %s\n\0";
    unsafe {
        printf(format.as_ptr() as *const i8, 42, b"hello\0".as_ptr() as *const i8);
    }
}

// 3. Inline assembly (nightly only)
#[cfg(feature = "nightly")]
feature asm {
    use std::arch::asm;
    
    fn get_time() -> u64 {
        let mut low: u32;
        let mut high: u32;
        
        unsafe {
            asm!(
                "rdtsc",
                out("eax") low,
                out("edx") high,
            );
        }
        
        ((high as u64) << 32) | (low as u64)
    }
}

// 4. Memory pool with zero-sized references (embedded pattern)
struct Pool<T> {
    // Implementation details...
}

impl<T> Pool<T> {
    fn alloc(&self) -> Option<Box<T>> {
        // Allocate from pool
        unimplemented!()
    }
    
    fn dealloc(&self, block: Box<T>) {
        // Return to pool
        unimplemented!()
    }
}

// Zero-sized reference pattern (advanced)
struct ZSR<T> {
    _marker: PhantomData<T>,
}

impl<T> ZSR<T> {
    fn new() -> Self {
        ZSR { _marker: PhantomData }
    }
    
    // Simulate reference behavior without storage overhead
}
```

</details>

## 📊 Performance Optimization Techniques

### 1. Stack Allocation Patterns

```rust
// Array-based stack allocation
fn process_data() {
    // Stack-allocated array (no heap allocation)
    let mut buffer = [0u8; 1024];
    
    // Use slice for operations
    let slice: &mut [u8] = &mut buffer;
    process_slice(slice);
}

// SmallVec pattern (without crate)
enum SmallVec<T> {
    Inline(T), // Single element on stack
    Heap(Vec<T>), // Multiple elements on heap
}

impl<T> SmallVec<T> {
    fn new(value: T) -> Self {
        SmallVec::Inline(value)
    }
    
    fn push(&mut self, value: T) {
        // Promotion logic from stack to heap
        match self {
            SmallVec::Inline(_) => {
                // Move to heap
                let mut v = Vec::new();
                v.push(value);
                *self = SmallVec::Heap(v);
            }
            SmallVec::Heap(v) => v.push(value),
        }
    }
}

// Const generics for fixed-size arrays
fn process_array<T, const N: usize>(arr: &[T; N]) {
    // Compile-time known size
    for i in 0..N {
        // Process each element
    }
}
```

### 2. Zero-Cost Abstractions

```rust
// Iterator adapters are zero-cost
fn sum_of_squares(iter: impl Iterator<Item = i32>) -> i32 {
    iter.map(|x| x * x).sum()
}

// Custom iterator with zero-cost overhead
struct Counter {
    current: usize,
    max: usize,
}

impl Iterator for Counter {
    type Item = usize;
    
    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.max {
            self.current += 1;
            Some(self.current - 1)
        } else {
            None
        }
    }
}

// Usage - compiles to efficient machine code
let counter = Counter { current: 0, max: 5 };
let sum: usize = counter.sum(); // No heap allocation, no virtual calls
```

### 3. Cache-Friendly Data Layout

```rust
// Structure of Arrays (SoA) pattern
struct ParticleSystem {
    positions: Vec<[f32; 3]>,
    velocities: Vec<[f32; 3]>,
    accelerations: Vec<[f32; 3]>,
    masses: Vec<f32>,
}

impl ParticleSystem {
    fn update(&mut self, dt: f32) {
        // Process all positions first (cache-friendly)
        for i in 0..self.positions.len() {
            self.positions[i] = update_position(&self.positions[i], &self.velocities[i], dt);
        }
        
        // Then all velocities
        for i in 0..self.velocities.len() {
            self.velocities[i] = update_velocity(&self.velocities[i], &self.accelerations[i], dt);
        }
    }
}

// Array of Structures (AoS) - less cache-friendly
struct Particle {
    position: [f32; 3],
    velocity: [f32; 3],
    acceleration: [f32; 3],
    mass: f32,
}

impl Particle {
    fn update(&mut self, dt: f32) {
        self.position = update_position(&self.position, &self.velocity, dt);
        self.velocity = update_velocity(&self.velocity, &self.acceleration, dt);
    }
}

// Const generics for fixed-size arrays (better cache locality)
fn process_particles<const N: usize>(particles: &mut [Particle; N]) {
    for p in particles.iter_mut() {
        p.update(0.016); // 60 FPS
    }
}
```

## 🔧 Error Handling Without Crates

### Custom Error Types

```rust
use std::fmt;

// Custom error type without external crates
#[derive(Debug)]
enum AppError {
    Io(std::io::Error),
    Parse(std::num::ParseIntError),
    Custom(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Io(e) => write!(f, "IO error: {}", e),
            AppError::Parse(e) => write!(f, "Parse error: {}", e),
            AppError::Custom(s) => write!(f, "Custom error: {}", s),
        }
    }
}

impl std::error::Error for AppError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AppError::Io(e) => Some(e),
            AppError::Parse(e) => Some(e),
            AppError::Custom(_) => None,
        }
    }
}

// From impls for automatic conversion
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e)
    }
}

impl From<std::num::ParseIntError> for AppError {
    fn from(e: std::num::ParseIntError) -> Self {
        AppError::Parse(e)
    }
}

// Usage
fn read_config() -> Result<i32, AppError> {
    let s = std::fs::read_to_string("config.txt")?; // Io error -> AppError
    let n: i32 = s.parse()?; // Parse error -> AppError
    Ok(n)
}
```

### Result Combinators

```rust
// Custom Result extension methods
trait ResultExt<T, E> {
    fn map_err_to_string(self) -> Result<T, String>;
}

impl<T, E: fmt::Display> ResultExt<T, E> for Result<T, E> {
    fn map_err_to_string(self) -> Result<T, String> {
        self.map_err(|e| e.to_string())
    }
}

// Usage
fn parse_and_process() -> Result<i32, String> {
    "not a number"
        .parse::<i32>()
        .map_err_to_string()
        .and_then(|n| if n > 0 { Ok(n * 2) } else { Err("Must be positive".to_string()) })
}
```

## 🧩 No_std & Core Library Mastery

For embedded systems, WebAssembly, or minimal binaries, `#![no_std]` is essential. The core library provides foundational types without OS dependencies.

### No_std Hierarchy

```mermaid
flowchart LR
    A[#![no_std]] --> B[core crate]
    B --> C[alloc crate<br/>Box, Vec, String]
    C --> D[std crate<br/>OS functionality]
    
    E[Embedded Targets] --> A
    F[WebAssembly] --> A
    G[Minimal Binaries] --> A
```

### Essential No_std Patterns

<details>
<summary>⚙️ Advanced No_std Configuration</summary>

```rust
#![no_std] // Disable standard library
#![no_main] // For custom entry points (embedded)

// Panic handler for no_std environments
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    // Custom panic implementation
    // For embedded: often just a infinite loop
    loop {}
}

// Optional: Include alloc for heap allocations
extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

// Global allocator for embedded systems
#[global_allocator]
static ALLOC: WeeAlloc = WeeAlloc::INIT;

// Heap-allocated collections in no_std
fn use_heap() {
    let mut v: Vec<u32> = Vec::new();
    v.push(42);
    
    let s: String = String::from("Hello, no_std!");
}
```

**Important**: `#![no_std]` doesn't prevent using crates that require `std` 【turn0search22】. Dependencies can still pull in `std` through transitive dependencies. Use `#![no_std]` + `extern crate alloc` for embedded targets with heap support.

</details>

### Core Library Highlights

| Module | Key Types | Use Case |
|--------|-----------|----------|
| `core::cell` | `Cell`, `RefCell` | Interior mutability without `std` |
| `core::sync::atomic` | `AtomicBool`, `AtomicUsize` | Lock-free synchronization |
| `core::ops` | `Range`, `Deref`, `Drop` | Operator overloading |
| `core::cmp` | `Ord`, `PartialEq` | Comparison traits |
| `core::fmt` | `Debug`, `Display` | Formatting without `std` |

### Embedded Memory Management

For embedded systems without heap allocation, consider these patterns:

```rust
// Stack-allocated collections (heapless pattern)
struct StackVec<T, const N: usize> {
    data: [MaybeUninit<T>; N],
    len: usize,
}

impl<T, const N: usize> StackVec<T, N> {
    fn new() -> Self {
        Self {
            data: [MaybeUninit::uninit(); N],
            len: 0,
        }
    }
    
    fn push(&mut self, value: T) -> Result<(), T> {
        if self.len < N {
            self.data[self.len].write(value);
            self.len += 1;
            Ok(())
        } else {
            Err(value)
        }
    }
    
    fn pop(&mut self) -> Option<T> {
        if self.len > 0 {
            self.len -= 1;
            Some(unsafe { self.data[self.len].assume_init_read() })
        } else {
            None
        }
    }
}

// Memory pool pattern (zero-sized references)
struct Pool<T, const BLOCK_SIZE: usize> {
    // Implementation details...
}

impl<T, const BLOCK_SIZE: usize> Pool<T, BLOCK_SIZE> {
    fn alloc(&self) -> Option<Box<T>> {
        // Allocate from pool
        unimplemented!()
    }
    
    fn dealloc(&self, block: Box<T>) {
        // Return to pool
        unimplemented!()
    }
}
```

## 📋 Rust Guru Checklist

### Core Rust 1.96+ Features
- [ ] Use new `core::range::Range` types for `Copy` semantics 【turn0search0】【turn0search4】
- [ ] Import `assert_matches!`/`debug_assert_matches!` explicitly 【turn0search0】【turn0search4】
- [ ] Use `impl RangeBounds` in public APIs for compatibility 【turn0search4】
- [ ] Understand `#![no_std]` hierarchy: `core` → `alloc` → `std` 【turn0search19】【turn0search23】
- [ ] Master ownership patterns: lifetimes, `PhantomData`, interior mutability
- [ ] Implement async/await without external crates for single-threaded contexts
- [ ] Follow unsafe code guidelines: minimize scope, document invariants, wrap in safe abstractions
- [ ] Optimize for cache locality: SoA vs AoS patterns
- [ ] Create custom error types without external crates
- [ ] Use zero-cost abstractions: iterators, closures, generics

### Performance Considerations
- [ ] Prefer stack allocation for small, short-lived data
- [ ] Use `#[inline]` for small, hot functions
- [ ] Avoid unnecessary allocations in inner loops
- [ ] Use `&[T]` instead of `&Vec<T>` for function parameters
- [ ] Consider `Cow<'_, T>` for borrowed/owned data
- [ ] Use `array::map` for fixed-size transformations
- [ ] Benchmark with `#[bench]` (nightly) or custom timing

### Safety Guidelines
- [ ] Audit all `unsafe` blocks for safety invariants
- [ ] Use `#[deny(unsafe_code)]` where possible
- [ ] Document safety requirements for public APIs
- [ ] Use `MaybeUninit<T>` for uninitialized memory
- [ ] Prefer `pin` API for self-referential structs
- [ ] Validate raw pointers before dereferencing
- [ ] Use `NonZero*` types for non-zero invariants

### Modern Rust Idioms (2026)
- [ ] Use `let-else` for early returns with binding
- [ ] Pattern match on `Option`/`Result` with `?` operator
- [ ] Use `if let` chains for conditional binding
- [ ] Implement `Default` for types with sensible defaults
- [ ] Use `#[must_use]` on important types/functions
- [ ] Leverage `derive` macros for common traits
- [ ] Use `async fn` in traits (stabilized in 2024)
- [ ] Adopt `core::range` types in new code 【turn0search0】【turn0search4】

## 🔮 Looking Ahead: Rust 2027 Edition

The **Rust 2027 Edition** (expected late 2026/early 2027) will bring significant changes 【turn0search4】【turn0search7】:

1. **Full Range Migration**: Range syntax `0..1` will produce `core::range::Range` types instead of `std::ops::Range`
2. **Const Generics Improvements**: Generic parameters allowed in const generic arguments 【turn0search4】
3. **Edition Transition**: Existing code may need updates for the new range types
4. **New Features**: Additional language features currently in development

### Preparation Steps

<details>
<summary>📦 Preparing for Rust 2027</summary>

```rust
// 1. Use impl RangeBounds in public APIs
pub fn process_range<T>(range: impl RangeBounds<usize>) -> &[T] {
    // Works with both old and new range types
}

// 2. Gradually migrate to core::range types
use core::range::{Range, RangeFrom, RangeInclusive};

// 3. Update from std::ops::Range to core::range::Range
// Old:
// let r: std::ops::Range<usize> = 0..10;
// New:
let r: core::range::Range<usize> = Range { start: 0, end: 10 };

// 4. Test with edition 2027
// cargo +nightly build -Z unstable-options --edition 2027
```

**Timeline**:
- **1.96** (May 2026): Library portion stabilized 【turn0search0】【turn0search4】
- **1.97** (July 2026): Beta testing continues 【turn0search7】
- **1.98** (August 2026): Nightly features for 2027 edition 【turn0search7】
- **2027 Edition**: Expected Q4 2026/Q1 2027 【turn0search4】

</details>

## 📚 Additional Resources

### Official Documentation
- [Rust 1.96.0 Release Notes](https://blog.rust-lang.org/releases/latest) 【turn0search0】
- [The Rust Standard Library Documentation](https://doc.rust-lang.org/std) 【turn0search14】
- [Rust Internals Forum](https://internals.rust-lang.org) 【turn0search3】

### Community Resources
- [Rust Project Goals](https://rust-lang.github.io/rust-project-goals) 【turn0search17】
- [Verify Rust Standard Library](https://github.com/model-checking/verify-rust-std) 【turn0search16】
- [Rust Changelogs](https://releases.rs) 【turn0search10】

### Version Information
- **Stable**: 1.96.0 (May 28, 2026) 【turn0search4】【turn0search13】
- **Beta**: 1.97.0 (Expected July 9, 2026) 【turn0search7】
- **Nightly**: 1.98.0 (Expected August 20, 2026) 【turn0search7】
- **Next Edition**: Rust 2027 【turn0search4】

---

> ⚠️ **Safety Note**: Always document safety invariants for `unsafe` code and consider using tools like `miri` to catch undefined behavior during testing.

> 🚀 **Performance Tip**: Profile your code with `cargo flamegraph` or `perf` to identify bottlenecks. Optimize hot paths only after profiling.

This document provides a comprehensive foundation for modern Rust development using only the standard library. By mastering these techniques, you'll write efficient, safe, and maintainable Rust code without external dependencies.
```

See also: `egui_guru` for egui/eframe gui tips tricks and guidance.
