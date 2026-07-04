use agenticrs::{
    AgentGuard, CircuitBreaker, CircuitState, Error, RateLimitInfo, RateLimitState, RetryPolicy,
};
use serde_json::json;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
enum TestError {
    Transient,
    Permanent,
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestError::Transient => write!(f, "Transient error"),
            TestError::Permanent => write!(f, "Permanent error"),
        }
    }
}

impl std::error::Error for TestError {}

#[tokio::test]
async fn test_retry_flaky_task() {
    let call_count = Arc::new(Mutex::new(0));
    let policy = RetryPolicy::new(
        3,
        Duration::from_millis(20),
        Duration::from_millis(100),
        2.0,
        false, // disable jitter for predictable test execution
    );

    let cc = call_count.clone();
    let result: Result<String, Error<TestError>> = AgentGuard::new()
        .with_retry(policy)
        .with_classifier(|err: &TestError| match err {
            TestError::Transient => true,
            TestError::Permanent => false,
        })
        .run(|_correction| {
            let mut count = cc.lock().unwrap();
            *count += 1;
            let current = *count;
            async move {
                if current < 3 {
                    Err(TestError::Transient)
                } else {
                    Ok("success".to_string())
                }
            }
        })
        .await;

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
    assert_eq!(*call_count.lock().unwrap(), 3);
}

#[tokio::test]
async fn test_no_retry_on_permanent_error() {
    let call_count = Arc::new(Mutex::new(0));
    let policy = RetryPolicy::new(
        3,
        Duration::from_millis(20),
        Duration::from_millis(100),
        2.0,
        false,
    );

    let cc = call_count.clone();
    let result: Result<String, Error<TestError>> = AgentGuard::new()
        .with_retry(policy)
        .with_classifier(|err: &TestError| match err {
            TestError::Transient => true,
            TestError::Permanent => false,
        })
        .run(|_correction| {
            let mut count = cc.lock().unwrap();
            *count += 1;
            async move { Err(TestError::Permanent) }
        })
        .await;

    assert!(matches!(result, Err(Error::Inner(TestError::Permanent))));
    assert_eq!(*call_count.lock().unwrap(), 1);
}

#[tokio::test]
async fn test_circuit_breaker() {
    let cb = Arc::new(CircuitBreaker::new(2, Duration::from_millis(150)));

    let call = || {
        let cb_clone = cb.clone();
        async move {
            let res: Result<(), Error<TestError>> = AgentGuard::new()
                .with_circuit_breaker(cb_clone)
                .run(|_| async { Err(TestError::Permanent) })
                .await;
            res
        }
    };

    // First failure (circuit remains Closed)
    let res1 = call().await;
    assert!(matches!(res1, Err(Error::Inner(TestError::Permanent))));
    assert_eq!(cb.state(), CircuitState::Closed);

    // Second failure (trips circuit to Open)
    let res2 = call().await;
    assert!(matches!(res2, Err(Error::Inner(TestError::Permanent))));
    assert_eq!(cb.state(), CircuitState::Open);

    // Third call fails immediately without running because circuit is Open
    let res3 = call().await;
    assert!(matches!(res3, Err(Error::CircuitBreakerOpen)));

    // Sleep to exceed the reset timeout cooldown
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Check state has transitioned to HalfOpen
    assert_eq!(cb.state(), CircuitState::HalfOpen);

    // Next call succeeds, closing the circuit breaker
    let cb_clone = cb.clone();
    let res4: Result<String, Error<TestError>> = AgentGuard::new()
        .with_circuit_breaker(cb_clone)
        .run(|_| async { Ok("success".to_string()) })
        .await;

    assert!(res4.is_ok());
    assert_eq!(cb.state(), CircuitState::Closed);
}

#[tokio::test]
async fn test_validation_and_correction() {
    let schema = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "age": { "type": "integer", "minimum": 18 }
        },
        "required": ["name", "age"]
    });

    let validator = agenticrs::SchemaValidator::new(schema).unwrap();

    let call_count = Arc::new(Mutex::new(0));
    let cc = call_count.clone();

    let result: Result<String, Error<TestError>> = AgentGuard::new()
        .with_validator(move |res: &String| validator.validate(res))
        .with_correction(2)
        .run(|correction| {
            let mut count = cc.lock().unwrap();
            *count += 1;
            let current = *count;

            if current == 1 {
                assert!(correction.is_none());
            } else {
                let corr = correction.unwrap();
                assert!(corr.malformed_output.contains("15"));
                assert!(corr.error_description.contains("minimum"));
            }

            async move {
                if current == 1 {
                    Ok(r#"{"name": "Alice", "age": 15}"#.to_string())
                } else {
                    Ok(r#"{"name": "Alice", "age": 20}"#.to_string())
                }
            }
        })
        .await;

    assert!(result.is_ok());
    assert_eq!(*call_count.lock().unwrap(), 2);
}

#[tokio::test]
async fn test_proactive_rate_limiting() {
    let state = RateLimitState::new();

    // Simulate we have 0 remaining requests, and reset is in 100ms
    let info = RateLimitInfo {
        requests_remaining: Some(0),
        requests_reset: Some(Duration::from_millis(100)),
        tokens_remaining: None,
        tokens_reset: None,
    };
    state.update(&info);

    let start = Instant::now();
    let state_clone = state.clone();
    let res: Result<String, Error<TestError>> = AgentGuard::new()
        .with_rate_limits(state_clone)
        .run(|_| async { Ok("done".to_string()) })
        .await;

    assert!(res.is_ok());
    let elapsed = start.elapsed();
    // Proactive backoff should have delayed execution by about 100ms
    assert!(
        elapsed >= Duration::from_millis(80),
        "Should have delayed, elapsed={:?}",
        elapsed
    );
}
