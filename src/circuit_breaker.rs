use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Represents the state of the circuit breaker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Requests are allowed to pass through normally.
    Closed,
    /// Requests are rejected immediately.
    Open,
    /// A trial request is allowed to pass to test if the remote service has recovered.
    HalfOpen,
}

/// A circuit breaker implementation to prevent cascading failures.
pub struct CircuitBreaker {
    failure_threshold: u32,
    reset_timeout: Duration,
    state: Arc<Mutex<CircuitBreakerState>>,
}

struct CircuitBreakerState {
    state: CircuitState,
    failures: u32,
    last_failure_time: Option<Instant>,
    on_open: Option<Arc<dyn Fn() + Send + Sync>>,
    on_close: Option<Arc<dyn Fn() + Send + Sync>>,
    on_half_open: Option<Arc<dyn Fn() + Send + Sync>>,
}

impl CircuitBreaker {
    /// Creates a new `CircuitBreaker` with the given failure threshold and reset timeout.
    pub fn new(failure_threshold: u32, reset_timeout: Duration) -> Self {
        Self {
            failure_threshold,
            reset_timeout,
            state: Arc::new(Mutex::new(CircuitBreakerState {
                state: CircuitState::Closed,
                failures: 0,
                last_failure_time: None,
                on_open: None,
                on_close: None,
                on_half_open: None,
            })),
        }
    }

    /// Registers a callback to be run when the circuit breaker enters the Open state.
    pub fn on_open<F>(&self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.state.lock().unwrap().on_open = Some(Arc::new(f));
    }

    /// Registers a callback to be run when the circuit breaker enters the Closed state.
    pub fn on_close<F>(&self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.state.lock().unwrap().on_close = Some(Arc::new(f));
    }

    /// Registers a callback to be run when the circuit breaker enters the HalfOpen state.
    pub fn on_half_open<F>(&self, f: F)
    where
        F: Fn() + Send + Sync + 'static,
    {
        self.state.lock().unwrap().on_half_open = Some(Arc::new(f));
    }

    /// Returns the current state of the circuit breaker.
    pub fn state(&self) -> CircuitState {
        let mut state_lock = self.state.lock().unwrap();
        self.check_state(&mut state_lock)
    }

    /// Checks the state and transitions from Open to HalfOpen if the cooldown period has expired.
    fn check_state(&self, state: &mut CircuitBreakerState) -> CircuitState {
        if state.state == CircuitState::Open {
            if let Some(last_failure) = state.last_failure_time {
                if last_failure.elapsed() >= self.reset_timeout {
                    state.state = CircuitState::HalfOpen;
                    if let Some(callback) = &state.on_half_open {
                        callback();
                    }
                }
            }
        }
        state.state
    }

    /// Checks if a call is allowed through the circuit breaker.
    /// Returns `true` if allowed, `false` if rejected (circuit is Open).
    pub fn check_call_allowed(&self) -> bool {
        let mut state_lock = self.state.lock().unwrap();
        let current_state = self.check_state(&mut state_lock);
        current_state != CircuitState::Open
    }

    /// Records a successful execution, resetting the failure count and closing the circuit.
    pub fn record_success(&self) {
        let mut state_lock = self.state.lock().unwrap();
        let old_state = state_lock.state;

        state_lock.failures = 0;
        state_lock.last_failure_time = None;
        state_lock.state = CircuitState::Closed;

        if old_state != CircuitState::Closed {
            if let Some(callback) = &state_lock.on_close {
                callback();
            }
        }
    }

    /// Records a failure. Tripping the circuit to Open if the threshold is met or exceeded.
    pub fn record_failure(&self) {
        let mut state_lock = self.state.lock().unwrap();
        state_lock.failures += 1;
        state_lock.last_failure_time = Some(Instant::now());

        if state_lock.state == CircuitState::HalfOpen
            || state_lock.failures >= self.failure_threshold
        {
            let old_state = state_lock.state;
            state_lock.state = CircuitState::Open;
            if old_state != CircuitState::Open {
                if let Some(callback) = &state_lock.on_open {
                    callback();
                }
            }
        }
    }
}
