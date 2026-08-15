use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorKind {
    Validation,
    Busy,
    Authentication,
    InsufficientCredits,
    Network,
    Timeout,
    ProviderUnavailable,
    GeneratedCandidateBlocked,
    PromptBlocked,
    ProviderSafetyRefusal,
    UnsupportedParameter,
    Cancelled,
    InvalidResponse,
    File,
    Storage,
    Internal,
}

impl ErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Validation => "validation",
            Self::Busy => "busy",
            Self::Authentication => "authentication",
            Self::InsufficientCredits => "insufficient_credits",
            Self::Network => "network",
            Self::Timeout => "timeout",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::GeneratedCandidateBlocked => "generated_candidate_blocked",
            Self::PromptBlocked => "prompt_blocked",
            Self::ProviderSafetyRefusal => "provider_safety_refusal",
            Self::UnsupportedParameter => "unsupported_parameter",
            Self::Cancelled => "cancelled",
            Self::InvalidResponse => "invalid_response",
            Self::File => "file",
            Self::Storage => "storage",
            Self::Internal => "internal",
        }
    }
}

#[derive(Debug, Clone, Serialize, Error)]
#[error("{message}")]
pub struct AppError {
    pub kind: ErrorKind,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl AppError {
    pub fn new(kind: ErrorKind, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            kind,
            message: message.into(),
            retryable,
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }

    pub fn storage(error: impl std::fmt::Display) -> Self {
        Self::new(
            ErrorKind::Storage,
            "Eidos could not update its local library.",
            true,
        )
        .with_details(error.to_string())
    }

    pub fn file(error: impl std::fmt::Display) -> Self {
        Self::new(
            ErrorKind::File,
            "Eidos could not read or save that image.",
            true,
        )
        .with_details(error.to_string())
    }

    pub fn internal(error: impl std::fmt::Display) -> Self {
        Self::new(
            ErrorKind::Internal,
            "Eidos encountered an unexpected problem.",
            true,
        )
        .with_details(error.to_string())
    }
}
