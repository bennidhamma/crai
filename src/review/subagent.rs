use crate::ai::provider::{AiProvider, SubagentType};
use crate::ai::schema::SubagentReviewResponse;
use crate::config::SubagentConfig;
use crate::diff::chunk::LineKind;
use crate::diff::FileDiff;
use crate::error::CraiResult;
use std::sync::Arc;

pub struct SubagentRunner {
    provider: Arc<dyn AiProvider>,
    config: SubagentConfig,
}

impl SubagentRunner {
    pub fn new(provider: Arc<dyn AiProvider>, config: SubagentConfig) -> Self {
        Self { provider, config }
    }

    pub async fn run_security_review(
        &self,
        files: &[FileDiff],
    ) -> CraiResult<Option<SubagentReviewResponse>> {
        if !self.config.security.enabled {
            return Ok(None);
        }

        self.run_review(SubagentType::Security, files, self.config.security.custom_prompt.as_deref())
            .await
            .map(Some)
    }

    pub async fn run_performance_review(
        &self,
        files: &[FileDiff],
    ) -> CraiResult<Option<SubagentReviewResponse>> {
        if !self.config.performance.enabled {
            return Ok(None);
        }

        self.run_review(SubagentType::Performance, files, self.config.performance.custom_prompt.as_deref())
            .await
            .map(Some)
    }

    pub async fn run_usability_review(
        &self,
        files: &[FileDiff],
    ) -> CraiResult<Option<SubagentReviewResponse>> {
        if !self.config.usability.enabled {
            return Ok(None);
        }

        self.run_review(SubagentType::Usability, files, self.config.usability.custom_prompt.as_deref())
            .await
            .map(Some)
    }

    async fn run_review(
        &self,
        subagent: SubagentType,
        files: &[FileDiff],
        custom_prompt: Option<&str>,
    ) -> CraiResult<SubagentReviewResponse> {
        // Build unified diff text
        let diff_text = build_diff_text(files);

        // Get file references
        let file_refs: Vec<&FileDiff> = files.iter().collect();

        self.provider
            .run_subagent_review(subagent, &diff_text, &file_refs, custom_prompt)
            .await
    }

    pub fn is_security_enabled(&self) -> bool {
        self.config.security.enabled
    }

    pub fn is_performance_enabled(&self) -> bool {
        self.config.performance.enabled
    }

    pub fn is_usability_enabled(&self) -> bool {
        self.config.usability.enabled
    }
}

fn build_diff_text(files: &[FileDiff]) -> String {
    let mut result = String::new();

    for file in files {
        result.push_str(&format!("=== {} ===\n", file.path.display()));

        for chunk in &file.chunks {
            result.push_str(&format!(
                "@@ -{},{} +{},{} @@ {}\n",
                chunk.old_range.start,
                chunk.old_range.count,
                chunk.new_range.start,
                chunk.new_range.count,
                chunk.header
            ));

            for line in &chunk.lines {
                let prefix = match line.kind {
                    LineKind::Context => ' ',
                    LineKind::Add => '+',
                    LineKind::Remove => '-',
                };
                result.push_str(&format!("{}{}\n", prefix, line.content));
            }
        }

        result.push('\n');
    }

    result
}
