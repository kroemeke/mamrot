# Mamrot - Agent Guidelines

This document provides context and instructions for AI agents (and human developers) working on the `mamrot` repository. It outlines the project structure, development workflows, code style, and architectural patterns.

## Project Overview

`mamrot` is a Rust-based TCP/HTTP fuzzing and load-testing tool. It connects to a target server and sends semi-randomized HTTP requests to stress-test or fuzz the target.

### Key Technologies
- **Language**: Rust (2021 Edition)
- **Async Runtime**: `tokio` (for async I/O and TCP connections)
- **CLI Parsing**: `clap` (derive-based argument parsing)
- **Randomization**: `rand` (for fuzzing logic)

## Development Workflow

Agents must adhere to the following workflow commands. Ensure all checks pass before considering a task complete.

### Build & Run
- **Build**: `cargo build`
- **Run (Release)**: `cargo run --release -- --target <HOST> --port <PORT>`
- **Run (Debug)**: `cargo run -- --target 127.0.0.1 --port 8080`

### Testing & Verification
- **Run All Tests**: `cargo test`
- **Run Specific Test**: `cargo test -- <test_name>`
- **Check Compilation**: `cargo check` (faster than build)

### Linting & Formatting
- **Linting**: `cargo clippy -- -D warnings`
    - *Strictly enforce clippy warnings as errors.*
- **Formatting**: `cargo fmt -- --check`
    - *Code must be formatted with `rustfmt`.*
- **Fix Formatting**: `cargo fmt`

## Code Style & Conventions

Adhere strictly to the following conventions to maintain consistency with the existing codebase.

### General Rust Style
- **Formatting**: Use standard `rustfmt` settings. Indentation is 4 spaces.
- **Naming**:
    - Variables/Functions: `snake_case` (e.g., `load_headers`, `int_size_1`)
    - Types/Traits: `PascalCase` (e.g., `Cube`, `Args`)
    - Constants/Statics: `SCREAMING_SNAKE_CASE` (e.g., `MAGIC_NUMBERS`)
- **Imports**: Group imports by crate. `std` first, then external crates, then internal modules.

### Error Handling
- **Propagation**: Prefer `Result<T, E>` and the `?` operator over `unwrap()` or `panic!`.
- **Existing Patterns**: You may notice `expect("poop")` or `expect("Whoopsie")` in existing code.
    - *New Code*: Use descriptive error messages in `expect()` or handle errors gracefully.
    - *Refactoring*: Only change existing "quirky" error messages if explicitly asked to refactor or improve professionalism.
- **Input Validation**: Validate file inputs (headers, wordlists) using `map_while` to avoid infinite loops on read errors.

### Asynchronous Code
- Use `tokio` for all I/O operations (TCP stream reading/writing).
- Avoid blocking calls (like `std::thread::sleep` or `std::fs` operations inside the async loop, though `std::fs` is currently used during initialization in `Cube`).
- Use `tokio::main` for the entry point.

### Variable Naming Conventions
- The project uses some non-standard naming for fuzzing parameters (e.g., `int_size_1`, `string_1`).
- **Convention**: Continue this pattern if extending the `Cube` struct, unless a refactor is requested.
- **Unused Variables**: Prefix unused variables with `_` (e.g., `_i`, `_n`) to silence the compiler, rather than removing them if they are kept for loop structure or future use.

## Architecture & Structure

### Directory Structure
- `src/bin/mamrot.rs`: Entry point for the main binary. Handles CLI argument parsing (`Args` struct), TCP connection loop, and request construction.
- `src/lib.rs`: Library entry point exporting modules.
- `src/rubik/mod.rs`: Contains the `Cube` struct and logic for headers/wordlist loading and random rotation/mutation logic.
- `src/seed.rs`: Handles ring-buffer seed logging (`SeedLog`) and loading seeds for replay (`load_seeds`).
- `Cargo.toml`: Dependency management.

### Key Components
- **`Cube` Struct**: The core state machine for the fuzzer. It holds loaded headers, wordlists, and internal integer/string states that mutate on `rotate()`.
- **`SeedLog`**: Manages the binary ring buffer file (`seeds.bin`) for reproducible fuzzing.
- **Fuzzing Loop**: Located in `bin/mamrot.rs`.
    1.  Constructs a base request.
    2.  Rotates the `Cube` using a seed (either generated or loaded from file).
    3.  Appends random headers based on `Cube` state.
    4.  Connects to the target via TCP.
    5.  Sends the request and awaits a response.

## Agent Protocol

When working in this repository, follow these behavioral guidelines:

1.  **Safety First**:
    -   Never introduce infinite loops (e.g., verify `while` or `loop` conditions).
    -   Always handle I/O errors when reading files or network streams.
    -   Use `map_while` instead of `filter_map` for iterators that produce `Result` types to prevent hanging on repeated errors.

2.  **Incremental Changes**:
    -   Run `cargo check` after small changes to catch errors early.
    -   Run `cargo clippy` before finalizing any code.

3.  **Context Awareness**:
    -   Read `src/rubik/mod.rs` before modifying fuzzing logic.
    -   Read `src/bin/mamrot.rs` before modifying the networking or CLI interface.

4.  **Tool Usage**:
    -   Use `grep` to find usage of structs before changing their definition.
    -   Use `edit` to modify files, respecting indentation.

5.  **Documentation**:
    -   Update `README.md` if CLI arguments change.
    -   Add comments only for complex logic (e.g., why a specific magic number is used).

## Common Tasks Reference

### Adding a new CLI Argument
1.  Modify `struct Args` in `src/bin/mamrot.rs`.
2.  Add the field with `#[arg(...)]` attributes.
3.  Use the new field in the `main` loop.

### Modifying Fuzzing Logic
1.  Edit `src/rubik/mod.rs`.
2.  Update `Cube` struct fields if necessary.
3.  Update `rotate()` method to alter how state changes.
4.  Update `bin/mamrot.rs` loop to utilize the new state if it affects request construction.

### Debugging
- Use `eprintln!` for debug logs to avoid polluting standard output (which might be piped).
- Check `target` directory artifacts if build fails mysteriously.
