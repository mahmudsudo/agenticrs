//! Minimal example: wrap a single async call with default retry behavior
//! and an OTel-compatible tracing span.
//!
//! Run with: cargo run --example quickstart

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
    tracing_subscriber::fmt::init();

    let response =
        guard(|_correction| async { call_model("Explain circuit breakers in one sentence").await })
            .await?;

    println!("Model said: {response}");
    Ok(())
}

async fn call_model(prompt: &str) -> Result<String, CallError> {
    // Swap this for your real LLM or tool call.
    Ok(format!("(stub) response to: {prompt}"))
}
