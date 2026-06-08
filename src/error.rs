use serde_json::Value;
use std::error::Error;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    General,
    NotFound,
    Auth,
    RateLimited,
    InvalidInput,
}

impl ErrorKind {
    pub fn exit_code(self) -> u8 {
        match self {
            Self::General => 1,
            Self::NotFound => 2,
            Self::Auth => 3,
            Self::RateLimited => 4,
            Self::InvalidInput => 5,
        }
    }
}

#[derive(Debug)]
pub struct CliError {
    pub kind: ErrorKind,
    pub message: String,
    pub details: Option<Value>,
    pub retry_after: Option<u64>,
}

impl CliError {
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            details: None,
            retry_after: None,
        }
    }

    pub fn general(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::General, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::NotFound, message)
    }

    pub fn auth(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::Auth, message)
    }

    pub fn rate_limited(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::RateLimited, message)
    }

    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(ErrorKind::InvalidInput, message)
    }

    pub fn code(&self) -> u8 {
        self.kind.exit_code()
    }

    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_retry_after(mut self, retry_after: Option<u64>) -> Self {
        self.retry_after = retry_after;
        self
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)?;

        if let Some(details) = &self.details {
            if let Some(message) = details.get("message").and_then(|value| value.as_str()) {
                write!(f, ": {message}")?;
            } else if let Some(error) = details.get("error").and_then(|value| value.as_str()) {
                write!(f, ": {error}")?;
            }
        }

        Ok(())
    }
}

impl Error for CliError {}

impl From<reqwest::Error> for CliError {
    fn from(error: reqwest::Error) -> Self {
        Self::general(error.to_string())
    }
}

impl From<serde_json::Error> for CliError {
    fn from(error: serde_json::Error) -> Self {
        Self::general(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_codes_are_stable() {
        assert_eq!(ErrorKind::General.exit_code(), 1);
        assert_eq!(ErrorKind::NotFound.exit_code(), 2);
        assert_eq!(ErrorKind::Auth.exit_code(), 3);
        assert_eq!(ErrorKind::RateLimited.exit_code(), 4);
        assert_eq!(ErrorKind::InvalidInput.exit_code(), 5);
    }
}
