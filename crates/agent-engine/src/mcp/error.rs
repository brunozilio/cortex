#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum McpError {
    #[error("cancelled")]
    Cancelled,
    #[error("{server} answered HTTP 401: sign in with /mcp login {server}")]
    SignInNeeded { server: String },
    #[error("{server}: {message}")]
    Answered { server: String, message: String },
    #[error("{0}")]
    Failed(String),
}

impl McpError {
    pub fn needs_sign_in(&self) -> bool {
        matches!(self, Self::SignInNeeded { .. })
    }
}

impl From<String> for McpError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

impl From<&str> for McpError {
    fn from(message: &str) -> Self {
        Self::Failed(message.to_owned())
    }
}

impl From<McpError> for String {
    fn from(error: McpError) -> Self {
        error.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::McpError;

    #[test]
    fn each_failure_reads_as_the_text_it_always_had() {
        assert_eq!(McpError::Cancelled.to_string(), "cancelled");
        let refused = McpError::SignInNeeded { server: "linear".into() };
        assert_eq!(refused.to_string(), "linear answered HTTP 401: sign in with /mcp login linear");
        assert!(refused.needs_sign_in() && !McpError::Cancelled.needs_sign_in());
        let answered = McpError::Answered { server: "docs".into(), message: "no such tool".into() };
        assert_eq!(answered.to_string(), "docs: no such tool");
        assert_eq!(String::from(McpError::from("broken pipe")), "broken pipe");
    }
}
