use crate::ai::provider::{AiProvider, ProviderHealth, ScoringContext, SubagentType, SummaryContext};
use crate::ai::schema::{
    controversiality_json_schema, subagent_review_json_schema, summary_json_schema,
    ControversialityResponse, SubagentReviewResponse, SummaryResponse,
};
use crate::config::{AiConfig, AiProviderType};
use crate::diff::FileDiff;
use crate::error::{CraiError, CraiResult};
use async_trait::async_trait;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub struct ClaudeProvider {
    cli_path: String,
    model: Option<String>,
    timeout: Duration,
    max_retries: u32,
}

impl ClaudeProvider {
    pub fn new(config: &AiConfig) -> CraiResult<Self> {
        let cli_path = config
            .custom_cli_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "claude".to_string());

        Ok(Self {
            cli_path,
            model: config.model.clone(),
            timeout: Duration::from_secs(config.timeout_seconds),
            max_retries: config.max_retries,
        })
    }

    async fn execute_with_schema<T: serde::de::DeserializeOwned>(
        &self,
        prompt: &str,
        json_schema: serde_json::Value,
        system_prompt: Option<&str>,
    ) -> CraiResult<T> {
        let schema_str = serde_json::to_string(&json_schema)
            .map_err(|e| CraiError::Serialization(e.to_string()))?;

        let mut attempt = 0;
        loop {
            let mut cmd = Command::new(&self.cli_path);
            cmd.args(["-p", prompt, "--output-format", "json", "--json-schema", &schema_str]);

            if let Some(model) = &self.model {
                cmd.args(["--model", model]);
            }

            if let Some(sys) = system_prompt {
                cmd.args(["--append-system-prompt", sys]);
            }

            let output = cmd
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|e| CraiError::CliExecution(format!("Failed to run claude: {}", e)))?;

            if output.status.success() {
                // Parse the response - it's a JSON array, find the result object with structured_output
                let events: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout)
                    .map_err(|e| CraiError::ResponseParse(format!("Failed to parse response array: {}", e)))?;

                // Find the result event (last item with type "result")
                let result_event = events
                    .iter()
                    .rev()
                    .find(|e| e.get("type").and_then(|t| t.as_str()) == Some("result"))
                    .ok_or_else(|| CraiError::ResponseParse("No result event found in response".to_string()))?;

                let structured_output = result_event
                    .get("structured_output")
                    .ok_or_else(|| CraiError::ResponseParse("No structured_output in result".to_string()))?;

                let result: T = serde_json::from_value(structured_output.clone())
                    .map_err(|e| CraiError::ResponseParse(format!("Failed to parse structured output: {}", e)))?;

                return Ok(result);
            }

            attempt += 1;
            if attempt >= self.max_retries {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(CraiError::CliExecution(format!(
                    "Claude CLI failed after {} attempts: {}",
                    self.max_retries, stderr
                )));
            }

            tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
        }
    }
}

#[async_trait]
impl AiProvider for ClaudeProvider {
    fn provider_type(&self) -> AiProviderType {
        AiProviderType::Claude
    }

    async fn score_controversiality(
        &self,
        diff_text: &str,
        file_path: &str,
        language: &str,
        context: &ScoringContext,
    ) -> CraiResult<ControversialityResponse> {
        let mut prompt = format!(
            r#"Analyze this code diff and score its controversiality.

## Diff Content
```{language}
{diff_text}
```

## Context
- File: {file_path}
- Language: {language}

Score from 0.0 (trivial, auto-approvable) to 1.0 (critical, needs deep review).
Consider: security implications, correctness risks, architectural impact, and maintainability."#
        );

        if let Some(ref pr_desc) = context.pr_description {
            prompt.push_str(&format!("\n\n## PR Description\n{}", pr_desc));
        }

        if !context.commit_messages.is_empty() {
            prompt.push_str("\n\n## Related Commits\n");
            for msg in &context.commit_messages {
                prompt.push_str(&format!("- {}\n", msg));
            }
        }

        self.execute_with_schema(&prompt, controversiality_json_schema(), None)
            .await
    }

    async fn run_subagent_review(
        &self,
        subagent: SubagentType,
        diff_text: &str,
        files: &[&FileDiff],
        custom_prompt: Option<&str>,
    ) -> CraiResult<SubagentReviewResponse> {
        let files_list = files
            .iter()
            .map(|f| format!("- {}", f.path.display()))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            r#"Review these code changes from a {} perspective.

## Files Changed
{files_list}

## Diff Content
```
{diff_text}
```

{}"#,
            subagent.name(),
            custom_prompt.unwrap_or("")
        );

        self.execute_with_schema(&prompt, subagent_review_json_schema(), Some(subagent.system_prompt()))
            .await
    }

    async fn generate_summary(
        &self,
        files: &[FileDiff],
        context: &SummaryContext,
    ) -> CraiResult<SummaryResponse> {
        let files_summary = files
            .iter()
            .map(|f| {
                let total_changes: usize = f.chunks.iter().map(|c| c.changes()).sum();
                format!("- {} ({} changes)", f.path.display(), total_changes)
            })
            .collect::<Vec<_>>()
            .join("\n");

        let mut prompt = format!(
            r#"Generate a summary of these code changes for a code review.

## Files Changed ({} files)
{files_summary}

Provide a high-level overview, identify key changes, and assess overall risk."#,
            files.len()
        );

        if let Some(ref pr_desc) = context.pr_description {
            prompt.push_str(&format!("\n\n## PR Description\n{}", pr_desc));
        }

        if !context.commit_messages.is_empty() {
            prompt.push_str("\n\n## Commit Messages\n");
            for msg in &context.commit_messages {
                prompt.push_str(&format!("- {}\n", msg));
            }
        }

        self.execute_with_schema(&prompt, summary_json_schema(), None)
            .await
    }

    async fn health_check(&self) -> CraiResult<ProviderHealth> {
        let start = std::time::Instant::now();

        let output = Command::new(&self.cli_path)
            .args(["--version"])
            .output()
            .await
            .map_err(|e| CraiError::CliExecution(e.to_string()))?;

        let latency = start.elapsed().as_millis() as u64;

        Ok(ProviderHealth {
            is_available: output.status.success(),
            cli_version: Some(String::from_utf8_lossy(&output.stdout).trim().to_string()),
            model_available: true,
            latency_ms: Some(latency),
        })
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }
}
