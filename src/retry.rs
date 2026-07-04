use std::time::Duration;

/// A policy configuration for retrying failed operations.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// The maximum number of attempts (including the initial call) before giving up.
    pub max_attempts: usize,
    /// The initial delay before retrying.
    pub initial_delay: Duration,
    /// The maximum delay between retries.
    pub max_delay: Duration,
    /// The factor by which the delay is multiplied on each subsequent failure.
    pub backoff_factor: f64,
    /// Whether to apply random jitter to the delay to prevent thundering herd problems.
    pub jitter: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(10),
            backoff_factor: 2.0,
            jitter: true,
        }
    }
}

impl RetryPolicy {
    /// Creates a new custom `RetryPolicy`.
    pub fn new(
        max_attempts: usize,
        initial_delay: Duration,
        max_delay: Duration,
        backoff_factor: f64,
        jitter: bool,
    ) -> Self {
        Self {
            max_attempts,
            initial_delay,
            max_delay,
            backoff_factor,
            jitter,
        }
    }

    /// Calculate the delay for a given attempt.
    /// `attempt` is 1-indexed (first retry is attempt 1).
    pub fn calculate_delay(&self, attempt: usize) -> Duration {
        if attempt == 0 {
            return Duration::ZERO;
        }

        let factor = self.backoff_factor.powi(attempt as i32 - 1);
        let delay_secs = self.initial_delay.as_secs_f64() * factor;
        let mut delay = Duration::from_secs_f64(delay_secs);

        if delay > self.max_delay {
            delay = self.max_delay;
        }

        if self.jitter {
            let rng_val = rand::random::<f64>();
            // Full jitter: select a random duration between 0 and the calculated delay
            delay = Duration::from_secs_f64(delay.as_secs_f64() * rng_val);
        }

        delay
    }
}

/// A trait used to determine if an error is transient (should be retried)
/// or permanent (should not be retried).
pub trait ClassifyError<E>: Send + Sync {
    /// Returns true if the error is transient, false if it is permanent.
    fn is_transient(&self, err: &E) -> bool;
}

// Implement ClassifyError for any closure Fn(&E) -> bool
impl<E, F> ClassifyError<E> for F
where
    F: Fn(&E) -> bool + Send + Sync,
{
    fn is_transient(&self, err: &E) -> bool {
        self(err)
    }
}

/// A default classifier that treats all errors as transient.
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultClassifier;

impl<E> ClassifyError<E> for DefaultClassifier {
    fn is_transient(&self, _err: &E) -> bool {
        true
    }
}
