//! Demonstrates the full AgentGuard pipeline: retries, a shared circuit
//! breaker, JSON-schema validation with a correction loop, and proactive
//! rate-limit backoff, all wired to a single wrapped call.
//!
//! Run with: cargo run --example full_pipeline

use agenticrs::{AgentGuard, CircuitBreaker, Error, RateLimitInfo, RateLimitState, SchemaValidator};
use serde_json::json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug)]
struct ModelError(String);

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for ModelError {}

#[tokio::main]
async fn main() -> Result<(), Error<ModelError>> {
    tracing_subscriber::fmt::init();

    // Shared across every call you want to fail together, e.g. all calls
    // to the same provider/endpoint.
    let breaker = Arc::new(CircuitBreaker::new(3, Duration::from_secs(30)));

    // Shared so rate-limit headers observed on one call inform the delay
    // applied before the next one.
    let rate_limits = RateLimitState::new();

    // Require the model to return `{ "name": string, "age": integer >= 18 }`.
    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer", "minimum": 18 }
        },
        "required": ["name", "age"]
    });
    let validator = SchemaValidator::new(schema).expect("valid schema");

    let attempt = AtomicU32::new(0);

    let result: Result<String, Error<ModelError>> = AgentGuard::new()
        .with_circuit_breaker(breaker.clone())
        .with_rate_limits(rate_limits.clone())
        .with_rate_limit_extractor(|_result: &Result<String, ModelError>| {
            // In a real integration, parse this from response headers, e.g.:
            // agenticrs::parse_rate_limits(response.headers())
            Some(RateLimitInfo {
                requests_remaining: Some(42),
                requests_reset: Some(Duration::from_secs(1)),
                tokens_remaining: None,
                tokens_reset: None,
            })
        })
        .with_validator(move |raw: &String| validator.validate(raw))
        .with_correction(2)
        .run(|correction| {
            let n = attempt.fetch_add(1, Ordering::SeqCst) + 1;
            async move {
                match correction {
                    // First call: pretend the model returned an under-age value.
                    None => Ok(r#"{"name": "Ada", "age": 15}"#.to_string()),
                    // After a correction prompt, pretend the model fixes it.
                    Some(_) => {
                        println!("retrying with correction, attempt {n}");
                        Ok(r#"{"name": "Ada", "age": 30}"#.to_string())
                    }
                }
            }
        })
        .await;

    match result {
        Ok(json) => println!("Validated response: {json}"),
        Err(e) => println!("Failed after retries/correction: {e}"),
    }

    Ok(())
}
