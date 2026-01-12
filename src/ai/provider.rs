use crate::ai::schema::{ControversialityResponse, SubagentReviewResponse, SummaryResponse};
use crate::config::{AiConfig, AiProviderType};
use crate::diff::FileDiff;
use crate::error::{CraiError, CraiResult};
use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;

/// Core trait for AI provider implementations
#[async_trait]
pub trait AiProvider: Send + Sync {
    /// Get the provider type identifier
    fn provider_type(&self) -> AiProviderType;

    /// Score a single chunk for controversiality
    async fn score_controversiality(
        &self,
        diff_text: &str,
        file_path: &str,
        language: &str,
        context: &ScoringContext,
    ) -> CraiResult<ControversialityResponse>;

    /// Run a specialized subagent review
    async fn run_subagent_review(
        &self,
        subagent: SubagentType,
        diff_text: &str,
        files: &[&FileDiff],
        custom_prompt: Option<&str>,
    ) -> CraiResult<SubagentReviewResponse>;

    /// Generate a summary of the entire diff
    async fn generate_summary(
        &self,
        files: &[FileDiff],
        context: &SummaryContext,
    ) -> CraiResult<SummaryResponse>;

    /// Check if the provider is available and configured
    async fn health_check(&self) -> CraiResult<ProviderHealth>;

    /// Get the configured timeout
    fn timeout(&self) -> Duration;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentType {
    Security,
    Performance,
    Usability,
}

impl SubagentType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Security => "Security",
            Self::Performance => "Performance",
            Self::Usability => "Usability",
        }
    }

    pub fn system_prompt(&self) -> &'static str {
        match self {
            Self::Security => SECURITY_PROMPT,
            Self::Performance => PERFORMANCE_PROMPT,
            Self::Usability => USABILITY_PROMPT,
        }
    }
}

const SECURITY_PROMPT: &str = r#"You are a security-focused code reviewer. Analyze the provided code changes for:
- Authentication and authorization vulnerabilities
- Injection vulnerabilities (SQL, command, XSS)
- Data exposure and privacy issues
- Cryptographic weaknesses
- Input validation gaps
- Security misconfigurations
Focus only on security-relevant findings."#;

const PERFORMANCE_PROMPT: &str = r#"You are a performance-focused code reviewer. Analyze the provided code changes for:
- Algorithm complexity issues (O(n^2) or worse in hot paths)
- Unnecessary allocations or copies
- Missing caching opportunities
- I/O inefficiencies
- Database query patterns
- Memory leaks or resource exhaustion
Focus only on performance-relevant findings."#;

const USABILITY_PROMPT: &str = r#"You are a usability-focused code reviewer. Analyze the provided code changes for:
- API design clarity and consistency
- Error message quality and helpfulness
- Documentation completeness
- Breaking changes impact
- Developer experience concerns
- Configuration complexity
Focus only on usability and developer experience findings."#;

#[derive(Debug, Clone, Default)]
pub struct ScoringContext {
    pub pr_description: Option<String>,
    pub commit_messages: Vec<String>,
    pub surrounding_code: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SummaryContext {
    pub pr_description: Option<String>,
    pub commit_messages: Vec<String>,
    pub repository_context: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ProviderHealth {
    pub is_available: bool,
    pub cli_version: Option<String>,
    pub model_available: bool,
    pub latency_ms: Option<u64>,
}

/// Factory for creating AI providers
pub struct AiProviderFactory;

impl AiProviderFactory {
    pub fn create(config: &AiConfig) -> CraiResult<Arc<dyn AiProvider>> {
        match config.provider {
            AiProviderType::Claude => {
                Ok(Arc::new(crate::ai::claude::ClaudeProvider::new(config)?))
            }
            AiProviderType::Kiro => {
                Err(CraiError::AiProvider("Kiro provider not yet implemented".to_string()))
            }
            AiProviderType::OpenAi => {
                Err(CraiError::AiProvider("OpenAI provider not yet implemented".to_string()))
            }
            AiProviderType::Custom => {
                let path = config
                    .custom_cli_path
                    .as_ref()
                    .ok_or_else(|| {
                        CraiError::Config("Custom provider requires custom_cli_path".to_string())
                    })?;
                Err(CraiError::AiProvider(format!(
                    "Custom provider at {} not yet implemented",
                    path.display()
                )))
            }
        }
    }
}
