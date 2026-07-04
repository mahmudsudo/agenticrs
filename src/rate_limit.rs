use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Rate limit information extracted from HTTP headers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitInfo {
    /// The number of remaining requests allowed in the current time window.
    pub requests_remaining: Option<u32>,
    /// The duration after which the request rate limit resets.
    pub requests_reset: Option<Duration>,
    /// The number of remaining tokens allowed in the current time window.
    pub tokens_remaining: Option<u32>,
    /// The duration after which the token rate limit resets.
    pub tokens_reset: Option<Duration>,
}

/// Thread-safe shared state storing parsed rate-limit limits.
#[derive(Debug, Clone, Default)]
pub struct RateLimitState {
    inner: Arc<Mutex<RateLimitStateInner>>,
}

#[derive(Debug, Default)]
struct RateLimitStateInner {
    requests_remaining: Option<u32>,
    requests_reset_at: Option<Instant>,
    tokens_remaining: Option<u32>,
    tokens_reset_at: Option<Instant>,
}

impl RateLimitState {
    /// Creates a new, empty `RateLimitState`.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(RateLimitStateInner::default())),
        }
    }

    /// Checks the rate limit state and sleep/delay if we are close to hitting a limit.
    pub async fn check_and_delay(&self) {
        let delay = {
            let inner = self.inner.lock().unwrap();
            let now = Instant::now();
            let mut max_delay = Duration::ZERO;

            if let Some(rem) = inner.requests_remaining {
                if rem <= 1 {
                    if let Some(reset_at) = inner.requests_reset_at {
                        if reset_at > now {
                            max_delay = max_delay.max(reset_at - now);
                        }
                    }
                }
            }

            if let Some(rem) = inner.tokens_remaining {
                if rem <= 100 {
                    if let Some(reset_at) = inner.tokens_reset_at {
                        if reset_at > now {
                            max_delay = max_delay.max(reset_at - now);
                        }
                    }
                }
            }

            max_delay
        };

        if delay > Duration::ZERO {
            tokio::time::sleep(delay).await;
        }
    }

    /// Update the rate limit state with new information.
    pub fn update(&self, info: &RateLimitInfo) {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();

        if let Some(rem) = info.requests_remaining {
            inner.requests_remaining = Some(rem);
        }
        if let Some(reset) = info.requests_reset {
            inner.requests_reset_at = Some(now + reset);
        }
        if let Some(rem) = info.tokens_remaining {
            inner.tokens_remaining = Some(rem);
        }
        if let Some(reset) = info.tokens_reset {
            inner.tokens_reset_at = Some(now + reset);
        }
    }
}

/// Parses OpenAI-style and Anthropic-style rate-limit headers.
/// Case-insensitive.
pub fn parse_rate_limits<I, K, V>(headers: I) -> Option<RateLimitInfo>
where
    I: IntoIterator<Item = (K, V)>,
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut info = RateLimitInfo {
        requests_remaining: None,
        requests_reset: None,
        tokens_remaining: None,
        tokens_reset: None,
    };

    let mut found = false;

    for (key, val) in headers {
        let k = key.as_ref().to_lowercase();
        let v = val.as_ref().trim();

        match k.as_str() {
            // OpenAI and general requests remaining
            "x-ratelimit-remaining-requests" | "anthropic-ratelimit-requests-remaining" | "ratelimit-remaining" => {
                if let Ok(rem) = v.parse::<u32>() {
                    info.requests_remaining = Some(rem);
                    found = true;
                }
            }
            // OpenAI and general requests reset
            "x-ratelimit-reset-requests" | "anthropic-ratelimit-requests-reset" | "ratelimit-reset" => {
                if let Some(dur) = parse_duration_string(v) {
                    info.requests_reset = Some(dur);
                    found = true;
                }
            }
            // OpenAI and general tokens remaining
            "x-ratelimit-remaining-tokens" | "anthropic-ratelimit-tokens-remaining" => {
                if let Ok(rem) = v.parse::<u32>() {
                    info.tokens_remaining = Some(rem);
                    found = true;
                }
            }
            // OpenAI and general tokens reset
            "x-ratelimit-reset-tokens" | "anthropic-ratelimit-tokens-reset" => {
                if let Some(dur) = parse_duration_string(v) {
                    info.tokens_reset = Some(dur);
                    found = true;
                }
            }
            _ => {}
        }
    }

    if found { Some(info) } else { None }
}

/// Helper function to parse duration string (e.g. "6s", "20ms", "1m30s", or float seconds "1.5")
fn parse_duration_string(s: &str) -> Option<Duration> {
    if s.is_empty() {
        return None;
    }

    // Try parsing as float seconds first (common in standard rate limit headers)
    if let Ok(secs) = s.parse::<f64>() {
        return Some(Duration::from_secs_f64(secs));
    }

    let mut duration = Duration::ZERO;
    let mut current_num = String::new();

    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_ascii_digit() || c == '.' {
            current_num.push(c);
            i += 1;
        } else {
            let mut unit = String::new();
            while i < chars.len() && !chars[i].is_ascii_digit() && chars[i] != '.' {
                unit.push(chars[i]);
                i += 1;
            }
            let unit = unit.trim();
            if let Ok(val) = current_num.parse::<f64>() {
                match unit {
                    "ms" => duration += Duration::from_secs_f64(val / 1000.0),
                    "s" => duration += Duration::from_secs_f64(val),
                    "m" => duration += Duration::from_secs_f64(val * 60.0),
                    "h" => duration += Duration::from_secs_f64(val * 3600.0),
                    _ => {}
                }
            }
            current_num.clear();
        }
    }

    if duration == Duration::ZERO {
        None
    } else {
        Some(duration)
    }
}
