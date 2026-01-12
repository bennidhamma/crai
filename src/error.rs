use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CraiError {
    // Configuration errors
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Config file not found: {}", .0.display())]
    ConfigNotFound(PathBuf),

    #[error("Failed to parse config at {}: {source}", path.display())]
    ConfigParse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    // Git errors
    #[error("Git error: {0}")]
    Git(String),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Not a git repository: {}", .0.display())]
    NotAGitRepository(PathBuf),

    // Diff errors
    #[error("Diff error: {0}")]
    Diff(String),

    #[error("difftastic (difft) not found in PATH")]
    DifftasticNotFound,

    #[error("Failed to parse diff: {0}")]
    DiffParse(String),

    // AI provider errors
    #[error("AI provider error: {0}")]
    AiProvider(String),

    #[error("CLI execution failed: {0}")]
    CliExecution(String),

    #[error("CLI tool not found: {0}")]
    CliNotFound(String),

    #[error("Failed to parse AI response: {0}")]
    ResponseParse(String),

    #[error("Operation '{operation}' timed out after {duration:?}")]
    Timeout {
        operation: String,
        duration: std::time::Duration,
    },

    #[error("Rate limited{}", .retry_after.map(|d| format!(", retry after {:?}", d)).unwrap_or_default())]
    RateLimited {
        retry_after: Option<std::time::Duration>,
    },

    // Serialization errors
    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("JSON schema error: {0}")]
    JsonSchema(String),

    // IO errors
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("File not found: {}", .0.display())]
    FileNotFound(PathBuf),

    #[error("Permission denied: {}", .0.display())]
    PermissionDenied(PathBuf),

    // TUI errors
    #[error("TUI error: {0}")]
    Tui(String),

    #[error("Terminal error: {0}")]
    Terminal(String),

    // Review session errors
    #[error("Session error: {0}")]
    Session(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    // Regex errors
    #[error("Regex error: {0}")]
    Regex(#[from] regex::Error),
}

impl From<toml::de::Error> for CraiError {
    fn from(err: toml::de::Error) -> Self {
        Self::Config(err.to_string())
    }
}

impl From<serde_json::Error> for CraiError {
    fn from(err: serde_json::Error) -> Self {
        Self::Parse(err.to_string())
    }
}

pub type CraiResult<T> = Result<T, CraiError>;
