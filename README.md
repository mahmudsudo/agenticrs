# agenticrs

[![CI](https://github.com/mahmudsudo/agenticrs/actions/workflows/ci.yml/badge.svg)](https://github.com/mahmudsudo/agenticrs/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A reliability and observability layer purpose-built for LLM and agentic workloads.

Most agent frameworks retry blindly, swallow malformed model output, and give
you no visibility into why a tool-call failed. `agenticrs` wraps any async
tool-call or LLM call with retries, a circuit breaker, output validation with
a correction loop, proactive rate-limit backoff, and OTel-compatible tracing
— without tying you to a specific provider's SDK.

## Installation

```bash
cargo add agenticrs
```

Or add it directly to `Cargo.toml`:

```toml
[dependencies]
agenticrs = "0.1"
```

## Features

- **Universal Wrapper:** Simple async wrapper that guards any LLM API or tool call.
- **Resilient Retries:** Exponential backoff with full jitter, configurable by transient vs. permanent error types.
- **Circuit Breaker:** Ported thread-safe three-state (Closed/Open/Half-Open) circuit breaker logic.
- **Structured Output Validation:** Optional JSON schema validator hook with correction loop support.
- **Proactive Rate-Limit Backoff:** Parses OpenAI/Anthropic headers and delays requests before hitting rate limit thresholds.
- **Telemetry:** Instruments OTel-compliant spans recording latency, retry counts, errors, and token usage.

## Quickstart

```rust
use agenticrs::{guard, Error};

#[derive(Debug)]
struct CallError(String);

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for CallError {}

#[tokio::main]
async fn main() -> Result<(), Error<CallError>> {
    // `guard` retries transient failures, records an OTel-compatible
    // tracing span (latency, retry count, failure reason), and returns
    // the first successful result.
    let response = guard(|_correction| async {
        call_model("Explain circuit breakers in one sentence").await
    })
    .await?;

    println!("Model said: {response}");
    Ok(())
}

async fn call_model(prompt: &str) -> Result<String, CallError> {
    // Swap this for your real LLM or tool call.
    Ok(format!("(stub) response to: {prompt}"))
}
```

For circuit breakers, JSON-schema validation with correction loops, and
proactive rate-limit backoff, see `AgentGuard` in `examples/full_pipeline.rs`
and run it with:

```bash
cargo run --example full_pipeline
```

## License

MIT
