# Contributing to agentrs

Thanks for considering a contribution. This project is young, so the bar is
mostly: does it work, is it tested, does it stay in scope.

## Setup

```bash
git clone https://github.com/mahmudsudo/agentrs.git
cd agentrs
cargo build
cargo test
cargo run --example quickstart
cargo run --example full_pipeline
```

## Before opening a PR

- `cargo test` passes.
- `cargo clippy --all-targets -- -D warnings` is clean.
- `cargo fmt` has been run.
- New behavior has a test in `tests/`, following the style in
  `tests/resilience_tests.rs` (a fake `TestError` enum with `Transient`/
  `Permanent` variants, driven through `AgentGuard` directly).
- Public API additions have doc comments (`///`) — this crate targets docs.rs.

## Scope for v0.1.x

The intent is to keep the core small: retries, circuit breaking, output
validation, rate-limit backoff, and tracing for a single wrapped async call.
Please open an issue before submitting a PR for:

- A hosted dashboard or any networked telemetry backend.
- A second-language port (e.g. TypeScript).
- Provider-specific SDK wrappers (OpenAI/Anthropic clients, etc.) — the goal
  is to stay provider-agnostic via generics/closures.

Bug fixes, doc improvements, and additional error-classification helpers are
always welcome without prior discussion.

## Reporting issues

Please include:
- The `agentrs` version and Rust version (`rustc --version`).
- A minimal reproduction if possible (a failing `#[tokio::test]` is ideal).
- What you expected vs. what happened.
