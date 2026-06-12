//! Error and result types shared across the data layer.

/// Errors surfaced by providers and the HTTP transport.
#[derive(Debug, thiserror::Error)]
pub enum DataError {
    /// A network/transport-level failure (DNS, TLS, connection, timeout).
    #[error("network error: {0}")]
    Transport(String),

    /// The upstream returned a status we treat as an error.
    #[error("upstream returned HTTP {status}: {message}")]
    Status {
        /// The HTTP status code.
        status: u16,
        /// A short description (reason phrase or body snippet).
        message: String,
    },

    /// The response body could not be parsed into the expected shape.
    #[error("failed to decode response: {0}")]
    Decode(String),

    /// The provider was rate limited; `retry_after` is the server hint, if any.
    #[error("rate limited by upstream")]
    RateLimited {
        /// Seconds to wait before retrying, if the server provided a hint.
        retry_after: Option<u64>,
    },

    /// A required API key for the selected provider was not configured.
    #[error("missing API key for the {0} provider")]
    MissingKey(&'static str),

    /// The selected provider does not support this operation (e.g. fine-grained
    /// live events on a limited free tier).
    #[error("operation not supported by the {provider} provider: {what}")]
    Unsupported {
        /// The provider name.
        provider: &'static str,
        /// What was requested.
        what: &'static str,
    },

    /// Any other error with a human-readable message.
    #[error("{0}")]
    Other(String),
}

/// Convenience result type for the data layer.
pub type Result<T> = std::result::Result<T, DataError>;
