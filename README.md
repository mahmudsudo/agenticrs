# agentrs

A reliability and observability layer purpose-built for LLM and agentic workloads.

## Features

- **Universal Wrapper:** Simple async wrapper that guards any LLM API or tool call.
- **Resilient Retries:** Exponential backoff with full jitter, configurable by transient vs. permanent error types.
- **Circuit Breaker:** Ported thread-safe three-state (Closed/Open/Half-Open) circuit breaker logic.
- **Structured Output Validation:** Optional JSON schema validator hook with correction loop support.
- **Proactive Rate-Limit Backoff:** Parses OpenAI/Anthropic headers and delays requests before hitting rate limit thresholds.
- **Telemetry:** Instruments OTel-compliant spans recording latency, retry counts, errors, and token usage.

## Quickstart

```rust
use agentrs::{guard, Error};

#[tokio::main]
async fn main() -> Result<(), Error<reqwest::Error>> {
    // 15-line quickstart showing wrap -> retry -> trace working out of the box
    let response = guard(|| async {
        reqwest::get("https://api.openai.com/v1/models").await
    }).await?;
    println!("API Call succeeded: {:?}", response.status());
    Ok(())
}
```

## License

MIT
