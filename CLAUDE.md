# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Command Executor is a Rust library providing a multiple producer/multiple consumer (MPMC) thread pool implementation with bounded blocking queues. The library is designed for concurrent command execution with backpressure control, primarily targeting use cases where memory consumption must be bounded (e.g., processing very large datasets that cannot fit into memory).

## Build and Test Commands

```bash
# Build the library
cargo build

# Build with release optimizations
cargo build --release

# Run all tests
cargo test

# Run a specific test
cargo test test_name

# Run tests with output
cargo test -- --nocapture

# Run benchmarks
cargo bench

# Run a specific example
cargo run --example thread_pool
cargo run --example pipeline
cargo run --example thread_local

# Check code without building
cargo check

# Run clippy linter
cargo clippy

# Format code
cargo fmt
```

## Core Architecture

### Command Pattern Implementation

The library uses the Command Pattern where tasks implement the `Command` trait:

- **`Command` trait** (`src/command.rs`): Single method `execute(&self) -> Result<(), anyhow::Error>` that gets executed in worker threads
- Commands are submitted as `Box<dyn Command + Send + Sync>` for thread-safe dynamic dispatch

### Thread Pool Components

1. **`ThreadPool`** (`src/thread_pool.rs`): Main orchestrator managing worker threads and task distribution
   - Creates and manages worker threads using `Builder::new().spawn()`
   - Uses `Arc<AtomicBool>` for shutdown coordination
   - Implements barrier-based synchronization for `in_all_threads()` operations
   - Supports two lifecycle modes via `ShutdownMode`

2. **`ThreadPoolBuilder`** (`src/thread_pool_builder.rs`): Builder pattern for configuration
   - Chain methods like `with_name()`, `with_tasks()`, `with_queue_size()`
   - Default values: 1 thread, queue size 16, immediate shutdown mode

3. **Queue Abstraction**:
   - **`BlockingQueueAdapter`** (`src/blocking_queue_adapter.rs`): Enum wrapper supporting multiple queue implementations
   - **`QueueType`** (`src/queue_type.rs`): `BlockingQueue` or `CrossbeamBlockingQueue`
   - Default is `CrossbeamBlockingQueue` (from crossbeam crate)
   - Custom blocking queue implementation in `src/blocking_queue.rs`

4. **`ShutdownMode`** (`src/shutdown_mode.rs`):
   - `Immediate`: Stop after current tasks
   - `CompletePending`: Wait for all queued tasks to complete

### Key Design Patterns

**Backpressure via Bounded Queue**: The bounded queue blocks producers when full, preventing unbounded memory growth. This is the library's primary value proposition.

**Thread-Local Storage**: Use `set_thread_local()` to initialize thread-local values across all worker threads. The library submits barrier-synchronized commands to ensure all threads execute the initialization before proceeding.

**Multi-Stage Pipelines**: Chain multiple `ThreadPool` instances by storing the next stage's pool in thread-local storage, allowing commands to submit work to downstream stages (see `examples/pipeline.rs`).

**In-All-Threads Execution**: The `in_all_threads()` and `in_all_threads_mut()` methods use barriers to execute a function across all threads synchronously. Useful for flushing thread-local state or gathering results.

## Testing Practices

- Unit tests are in the same files as implementation (e.g., `src/thread_pool.rs` has `mod tests`)
- Integration tests in `tests/` directory
- Use `Arc<AtomicUsize>` for verifying execution counts in tests
- Test shutdown modes explicitly (see `test_shutdown_complete_pending`)
- Test error handling via custom join error handlers

## Important Implementation Notes

- Worker threads run until `stopped` flag is set via `AtomicBool`
- After shutdown is called, attempting to submit work will panic (assertion at `src/thread_pool.rs:299`)
- The join operation collects errors from all threads and calls the configured error handler
- Thread naming follows pattern: `{name}-{index}` where index is 0-based
- `EmptyCommand` is submitted internally to wake threads during shutdown

## Common Pitfalls

1. **Use After Shutdown**: Calling `submit()` after `shutdown()` will panic. Test `test_use_after_join` specifically checks this.

2. **Barrier Deadlocks**: `in_all_threads()` will block indefinitely if any worker thread is stuck. This is documented as a caveat in the function comments.

3. **Error Handling**: Errors in `Command::execute()` don't stop the thread but are collected and reported during `join()`. Last error per thread is preserved.

4. **Thread-Local Initialization**: Must happen before submitting regular work if commands depend on thread-local state being present.
