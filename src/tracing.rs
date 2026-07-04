/// Holds token consumption statistics for LLM calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenUsageInfo {
    /// Number of tokens in the input prompt.
    pub prompt_tokens: u32,
    /// Number of tokens in the generated response.
    pub completion_tokens: u32,
    /// Optional field for total tokens (usually prompt_tokens + completion_tokens).
    pub total_tokens: Option<u32>,
}

/// A trait that response types can implement to allow `agentrs` to automatically
/// record token usage statistics in the tracing spans.
pub trait TokenUsage {
    /// Extracts the token usage details, if available.
    fn token_usage(&self) -> Option<TokenUsageInfo> {
        None
    }
}

impl TokenUsage for String {}
impl TokenUsage for serde_json::Value {}
impl TokenUsage for () {}

impl<T, E> TokenUsage for Result<T, E>
where
    T: TokenUsage,
{
    fn token_usage(&self) -> Option<TokenUsageInfo> {
        self.as_ref().ok().and_then(|t| t.token_usage())
    }
}
