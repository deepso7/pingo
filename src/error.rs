//! Error types for pingo.

/// A boxed error type for convenience.
pub type Error = Box<dyn std::error::Error + Send + Sync>;

/// A Result type alias using our Error type.
pub type Result<T> = std::result::Result<T, Error>;
