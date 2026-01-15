use crate::ai::provider::{AiProvider, ProviderHealth, ScoringContext, SubagentType, SummaryContext};
use crate::ai::schema::{ControversialityResponse, SubagentReviewResponse, SummaryResponse};
use crate::config::{AiConfig, AiProviderType};
use crate::diff::FileDiff;
use crate::error::{CraiError, CraiResult};
use async_trait::async_trait;
use regex::Regex;
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;

pub struct KiroProvider {
    cli_path: String,
    model: Option<String>,
    timeout: Duration,
    max_retries: u32,
}

impl KiroProvider {
    pub fn new(config: &AiConfig) -> CraiResult<Self> {
        let cli_path = config
            .custom_cli_path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "kiro-cli".to_string());

        Ok(Self {
            cli_path,
            model: config.model.clone(),
            timeout: Duration::from_secs(config.timeout_seconds),
            max_retries: config.max_retries,
        })
    }

    /// Strip ANSI escape codes from output
    fn strip_ansi_codes(text: &str) -> String {
        // Match ANSI escape sequences: ESC [ ... m (SGR) and other control sequences
        let ansi_re = Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]|\x1b\][^\x07]*\x07").unwrap();
        ansi_re.replace_all(text, "").to_string()
    }

    /// Extract JSON object from text (finds first complete {...} block)
    fn extract_json(text: &str) -> Option<String> {
        let clean = Self::strip_ansi_codes(text);

        // Find JSON object by tracking brace depth
        let mut start = None;
        let mut depth = 0;
        let mut in_string = false;
        let mut escape_next = false;

        for (i, c) in clean.char_indices() {
            if escape_next {
                escape_next = false;
                continue;
            }

            match c {
                '\\' if in_string => escape_next = true,
                '"' => in_string = !in_string,
                '{' if !in_string => {
                    if depth == 0 {
                        start = Some(i);
                    }
                    depth += 1;
                }
                '}' if !in_string => {
                    depth -= 1;
                    if depth == 0 {
                        if let Some(s) = start {
                            return Some(clean[s..=i].to_string());
                        }
                    }
                }
                _ => {}
            }
        }
        None
    }

    async fn execute_json_prompt<T: serde::de::DeserializeOwned>(
        &self,
        prompt: &str,
        json_format_hint: &str,
    ) -> CraiResult<T> {
        // Construct prompt that asks for JSON output
        let full_prompt = format!(
            "{}\n\nRespond with ONLY a JSON object in this exact format, no other text:\n{}",
            prompt, json_format_hint
        );

        let mut attempt = 0;
        loop {
            let mut cmd = Command::new(&self.cli_path);
            cmd.args(["chat", "--no-interactive", "--wrap", "never", &full_prompt]);

            if let Some(model) = &self.model {
                cmd.args(["--model", model]);
            }

            let output = cmd
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|e| CraiError::CliExecution(format!("Failed to run kiro-cli: {}", e)))?;

            let stdout = String::from_utf8_lossy(&output.stdout);

            // Extract JSON from the (possibly decorated) output
            if let Some(json_str) = Self::extract_json(&stdout) {
                match serde_json::from_str::<T>(&json_str) {
                    Ok(result) => return Ok(result),
                    Err(e) => {
                        // JSON found but didn't match expected structure
                        attempt += 1;
                        if attempt >= self.max_retries {
                            return Err(CraiError::ResponseParse(format!(
                                "Failed to parse JSON response after {} attempts: {}. JSON was: {}",
                                self.max_retries, e, json_str
                            )));
                        }
                    }
                }
            } else {
                attempt += 1;
                if attempt >= self.max_retries {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(CraiError::ResponseParse(format!(
                        "No JSON found in kiro-cli response after {} attempts. stdout: {}, stderr: {}",
                        self.max_retries,
                        stdout.chars().take(500).collect::<String>(),
                        stderr.chars().take(200).collect::<String>()
                    )));
                }
            }

            tokio::time::sleep(Duration::from_millis(500 * attempt as u64)).await;
        }
    }
}

#[async_trait]
impl AiProvider for KiroProvider {
    fn provider_type(&self) -> AiProviderType {
        AiProviderType::Kiro
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

        let json_hint = r#"{"score": 0.5, "classification": "routine", "reasoning": "Brief explanation", "concerns": [{"category": "correctness", "description": "Issue description", "severity": "low"}], "review_depth": "glance"}

IMPORTANT: All enum values MUST be lowercase.
- classification: trivial, routine, notable, significant, critical
- category: security, performance, correctness, maintainability, readability, testing, documentation, architecture
- severity: low, medium, high, critical
- review_depth: skip, glance, review, deep_dive
If there are no concerns, use an empty array: "concerns": []"#;

        self.execute_json_prompt(&prompt, json_hint).await
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
            r#"You are a {} specialist. Review these code changes.

## Files Changed
{files_list}

## Diff Content
```
{diff_text}
```

{}

{}"#,
            subagent.name(),
            custom_prompt.unwrap_or(""),
            subagent.system_prompt()
        );

        let json_hint = r#"{"findings": [{"id": "F1", "title": "Issue title", "description": "Detailed description", "location": {"file_path": "path/to/file", "line_start": 42, "line_end": 45}, "severity": "low", "category": "security", "code_snippet": "optional code"}], "overall_assessment": {"risk_level": "low", "summary": "Overall summary", "areas_of_concern": ["area1"]}, "recommendations": [{"priority": "suggested", "action": "What to do", "rationale": "Why", "affected_files": ["file.rs"]}]}

IMPORTANT: All enum values MUST be lowercase.
- severity: low, medium, high, critical
- category: security, performance, correctness, maintainability, readability, testing, documentation, architecture
- risk_level: low, medium, high, critical
- priority: optional, suggested, recommended, required
If no findings, use empty array: "findings": []"#;

        self.execute_json_prompt(&prompt, json_hint).await
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

        let json_hint = r#"{"overview": "High-level summary of changes", "key_changes": [{"description": "What changed", "affected_files": ["file.rs"], "impact_level": "low"}], "risk_assessment": {"overall_risk": "low", "factors": [{"factor": "Risk factor description", "contribution": 0.3}]}}

IMPORTANT: All enum values MUST be lowercase.
- impact_level: low, medium, high
- overall_risk: low, medium, high, critical"#;

        self.execute_json_prompt(&prompt, json_hint).await
    }

    async fn health_check(&self) -> CraiResult<ProviderHealth> {
        let start = std::time::Instant::now();

        let output = Command::new(&self.cli_path)
            .args(["--version"])
            .output()
            .await
            .map_err(|e| CraiError::CliExecution(e.to_string()))?;

        let latency = start.elapsed().as_millis() as u64;
        let version_output = String::from_utf8_lossy(&output.stdout);
        let clean_version = Self::strip_ansi_codes(&version_output).trim().to_string();

        Ok(ProviderHealth {
            is_available: output.status.success(),
            cli_version: Some(clean_version),
            model_available: true,
            latency_ms: Some(latency),
        })
    }

    fn timeout(&self) -> Duration {
        self.timeout
    }
}
