# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.1.0] - 2026-07-04

### Added
- `AgentGuard` builder and `guard()` shorthand: wrap any `Future<Output = Result<T, E>>`
  with retry, circuit breaker, validation, rate-limit, and tracing behavior.
- `RetryPolicy`: exponential backoff with full jitter, configurable max attempts,
  initial/max delay, and backoff factor.
- `ClassifyError` trait (plus `DefaultClassifier`) to distinguish transient vs.
  permanent errors; any `Fn(&E) -> bool` closure implements it directly.
- `CircuitBreaker`: thread-safe three-state (Closed / Open / HalfOpen) breaker
  with `on_open` / `on_close` / `on_half_open` callbacks.
- `SchemaValidator`: JSON Schema validation with an optional correction-loop
  (`with_correction`) that feeds the malformed output and error description
  back into the next call attempt via `Correction`.
- `RateLimitState` and `parse_rate_limits()`: proactive backoff based on
  OpenAI-style and Anthropic-style rate-limit headers, applied before a call
  is attempted rather than after it fails.
- OTel-compatible `tracing` spans (`agenticrs.guard`) recording latency, retry
  count, failure reason/type, and token usage (via `TokenUsage` /
  `with_token_extractor`).
- Integration tests covering flaky-retry recovery, permanent-error
  short-circuiting, a full circuit-breaker state cycle, validation +
  correction, and proactive rate-limit delay.

[Unreleased]: https://github.com/mahmudsudo/agenticrs/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/mahmudsudo/agenticrs/releases/tag/v0.1.0
