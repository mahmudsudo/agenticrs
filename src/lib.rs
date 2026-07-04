#![allow(clippy::type_complexity)]
pub mod circuit_breaker;
pub mod rate_limit;
pub mod retry;
pub mod tracing;
pub mod validation;

use std::future::Future;
use std::sync::Arc;
use std::time::Instant;

pub use circuit_breaker::{CircuitBreaker, CircuitState};
pub use rate_limit::{parse_rate_limits, RateLimitInfo, RateLimitState};
pub use retry::{ClassifyError, DefaultClassifier, RetryPolicy};
pub use tracing::{TokenUsage, TokenUsageInfo};
pub use validation::{Correction, SchemaValidator};

/// Crate-level error type representing failures in the guard wrapper.
#[derive(Debug)]
pub enum Error<E> {
    /// The wrapped async call failed with the inner error.
    Inner(E),
    /// The circuit breaker was Open and rejected the request.
    CircuitBreakerOpen,
    /// The output failed response validation.
    ValidationFailed(String),
}

impl<E: std::fmt::Display> std::fmt::Display for Error<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Inner(e) => write!(f, "{}", e),
            Error::CircuitBreakerOpen => write!(f, "Circuit breaker is open"),
            Error::ValidationFailed(err) => write!(f, "Validation failed: {}", err),
        }
    }
}

impl<E: std::error::Error> std::error::Error for Error<E> {}

/// A wrapper builder to add reliability and observability to async LLM/tool calls.
pub struct AgentGuard<T, E> {
    retry_policy: RetryPolicy,
    classifier: Arc<dyn ClassifyError<E>>,
    circuit_breaker: Option<Arc<CircuitBreaker>>,
    #[allow(clippy::type_complexity)]
    validator: Option<Arc<dyn Fn(&T) -> Result<(), String> + Send + Sync>>,
    max_correction_attempts: usize,
    formatter: Arc<dyn Fn(&T) -> String + Send + Sync>,
    token_extractor: Option<Arc<dyn Fn(&T) -> Option<TokenUsageInfo> + Send + Sync>>,
    rate_limit_state: Option<RateLimitState>,
    rate_limit_extractor: Option<Arc<dyn Fn(&Result<T, E>) -> Option<RateLimitInfo> + Send + Sync>>,
}

impl<T, E> Default for AgentGuard<T, E>
where
    T: std::fmt::Debug,
{
    fn default() -> Self {
        Self {
            retry_policy: RetryPolicy::default(),
            classifier: Arc::new(DefaultClassifier),
            circuit_breaker: None,
            validator: None,
            max_correction_attempts: 0,
            formatter: Arc::new(|val| format!("{:?}", val)),
            token_extractor: None,
            rate_limit_state: None,
            rate_limit_extractor: None,
        }
    }
}

impl<T, E> AgentGuard<T, E>
where
    T: std::fmt::Debug,
{
    /// Creates a new `AgentGuard` with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets a custom retry policy.
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Sets an error classifier to distinguish transient vs. permanent errors.
    pub fn with_classifier<C>(mut self, classifier: C) -> Self
    where
        C: ClassifyError<E> + 'static,
    {
        self.classifier = Arc::new(classifier);
        self
    }

    /// Associates a shared `CircuitBreaker`.
    pub fn with_circuit_breaker(mut self, cb: Arc<CircuitBreaker>) -> Self {
        self.circuit_breaker = Some(cb);
        self
    }

    /// Sets a response validation hook.
    pub fn with_validator<V>(mut self, validator: V) -> Self
    where
        V: Fn(&T) -> Result<(), String> + Send + Sync + 'static,
    {
        self.validator = Some(Arc::new(validator));
        self
    }

    /// Configures the maximum attempts to retry with a correction prompt on validation failures.
    pub fn with_correction(mut self, max_attempts: usize) -> Self {
        self.max_correction_attempts = max_attempts;
        self
    }

    /// Sets a custom output formatter for the correction loop (defaults to `{:?}`).
    pub fn with_formatter<F>(mut self, formatter: F) -> Self
    where
        F: Fn(&T) -> String + Send + Sync + 'static,
    {
        self.formatter = Arc::new(formatter);
        self
    }

    /// Sets an extractor to record token usage on spans.
    pub fn with_token_extractor<TE>(mut self, extractor: TE) -> Self
    where
        TE: Fn(&T) -> Option<TokenUsageInfo> + Send + Sync + 'static,
    {
        self.token_extractor = Some(Arc::new(extractor));
        self
    }

    /// Associates a shared `RateLimitState` for proactive backoff.
    pub fn with_rate_limits(mut self, state: RateLimitState) -> Self {
        self.rate_limit_state = Some(state);
        self
    }

    /// Sets a rate-limit info extractor from results/errors.
    pub fn with_rate_limit_extractor<RLE>(mut self, extractor: RLE) -> Self
    where
        RLE: Fn(&Result<T, E>) -> Option<RateLimitInfo> + Send + Sync + 'static,
    {
        self.rate_limit_extractor = Some(Arc::new(extractor));
        self
    }

    /// Executes the async operation wrapped with resilience and observability.
    pub async fn run<F, Fut>(&self, mut f: F) -> Result<T, Error<E>>
    where
        F: FnMut(Option<Correction>) -> Fut,
        Fut: Future<Output = Result<T, E>>,
        E: std::fmt::Display,
    {
        // 1. Proactive Rate Limit Check
        if let Some(rl) = &self.rate_limit_state {
            rl.check_and_delay().await;
        }

        // 2. Circuit Breaker Check
        if let Some(cb) = &self.circuit_breaker {
            if !cb.check_call_allowed() {
                return Err(Error::CircuitBreakerOpen);
            }
        }

        let mut attempts = 0;
        let mut correction_attempts = 0;
        let mut last_correction: Option<Correction> = None;
        let start_time = Instant::now();

        // Create OTel compatible tracing span
        let span = ::tracing::info_span!(
            "agentrs.guard",
            "otel.name" = "agentrs.guard",
            "otel.kind" = "client",
            retry_count = 0,
            status = "success",
            latency_ms = ::tracing::field::Empty,
            failure_reason = ::tracing::field::Empty,
            failure_type = ::tracing::field::Empty,
            prompt_tokens = ::tracing::field::Empty,
            completion_tokens = ::tracing::field::Empty,
            total_tokens = ::tracing::field::Empty,
        );

        let _enter = span.enter();

        loop {
            attempts += 1;

            // Run the async call
            let result = f(last_correction.clone()).await;

            // Update rate limit status if extractor is configured
            if let Some(rl_state) = &self.rate_limit_state {
                if let Some(extractor) = &self.rate_limit_extractor {
                    if let Some(info) = extractor(&result) {
                        rl_state.update(&info);
                    }
                }
            }

            match result {
                Ok(value) => {
                    // Check validation if validator is configured
                    if let Some(validator) = &self.validator {
                        if let Err(err_msg) = validator(&value) {
                            if correction_attempts < self.max_correction_attempts {
                                correction_attempts += 1;
                                let malformed_str = (self.formatter)(&value);
                                last_correction = Some(Correction {
                                    malformed_output: malformed_str,
                                    error_description: err_msg.clone(),
                                });

                                ::tracing::warn!(
                                    attempt = attempts,
                                    correction_attempt = correction_attempts,
                                    error = %err_msg,
                                    "Response validation failed, retrying with correction"
                                );
                                continue;
                            } else {
                                // Validation failed permanently
                                if let Some(cb) = &self.circuit_breaker {
                                    cb.record_failure();
                                }
                                span.record("status", "error");
                                span.record("failure_reason", &err_msg);
                                span.record("failure_type", "ValidationError");
                                span.record("latency_ms", start_time.elapsed().as_millis() as u64);
                                span.record("retry_count", attempts - 1);
                                return Err(Error::ValidationFailed(err_msg));
                            }
                        }
                    }

                    // Success!
                    if let Some(cb) = &self.circuit_breaker {
                        cb.record_success();
                    }

                    // Record tokens if extractor or default Trait is available
                    let mut tokens = None;
                    if let Some(extractor) = &self.token_extractor {
                        tokens = extractor(&value);
                    }
                    // Record tokens on tracing span
                    if let Some(t) = tokens {
                        span.record("prompt_tokens", t.prompt_tokens);
                        span.record("completion_tokens", t.completion_tokens);
                        if let Some(tot) = t.total_tokens {
                            span.record("total_tokens", tot);
                        }
                    }

                    span.record("latency_ms", start_time.elapsed().as_millis() as u64);
                    span.record("retry_count", attempts - 1);
                    return Ok(value);
                }
                Err(err) => {
                    let is_transient = self.classifier.is_transient(&err);
                    let is_last_attempt = attempts >= self.retry_policy.max_attempts;

                    if is_transient && !is_last_attempt {
                        let delay = self.retry_policy.calculate_delay(attempts);
                        ::tracing::warn!(
                            attempt = attempts,
                            error = %err,
                            delay_ms = delay.as_millis(),
                            "Encountered transient error, retrying"
                        );
                        tokio::time::sleep(delay).await;
                        continue;
                    } else {
                        // Permanent failure or max attempts reached
                        if let Some(cb) = &self.circuit_breaker {
                            cb.record_failure();
                        }
                        span.record("status", "error");
                        span.record("failure_reason", err.to_string());
                        let failure_type = if is_transient {
                            "TransientError"
                        } else {
                            "PermanentError"
                        };
                        span.record("failure_type", failure_type);
                        span.record("latency_ms", start_time.elapsed().as_millis() as u64);
                        span.record("retry_count", attempts - 1);
                        return Err(Error::Inner(err));
                    }
                }
            }
        }
    }
}

/// Shorthand function to wrap any async operation with default settings.
pub async fn guard<F, Fut, T, E>(f: F) -> Result<T, Error<E>>
where
    F: FnMut(Option<Correction>) -> Fut,
    Fut: Future<Output = Result<T, E>>,
    T: std::fmt::Debug,
    E: std::fmt::Display,
{
    AgentGuard::default().run(f).await
}
