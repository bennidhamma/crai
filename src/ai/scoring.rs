use crate::ai::provider::{AiProvider, ScoringContext};
use crate::ai::schema::ControversialityResponse;
use crate::diff::chunk::{ChunkId, DiffChunk, FileDiff, LineKind};
use crate::diff::filter::{ChunkFilter, FilterReason, FilterResult, FilterStats};
use crate::error::CraiResult;
use futures::stream::{self, StreamExt};
use std::sync::Arc;

pub struct ScoringOrchestrator {
    provider: Arc<dyn AiProvider>,
    filter: ChunkFilter,
    concurrent_requests: usize,
}

impl ScoringOrchestrator {
    pub fn new(
        provider: Arc<dyn AiProvider>,
        filter: ChunkFilter,
        concurrent_requests: usize,
    ) -> Self {
        Self {
            provider,
            filter,
            concurrent_requests,
        }
    }

    /// Score all chunks in the diff result
    pub async fn score_all<F>(
        &self,
        files: &[FileDiff],
        context: &ScoringContext,
        mut progress_callback: F,
    ) -> CraiResult<ScoringResult>
    where
        F: FnMut(ScoringUpdate) + Send,
    {
        let mut all_scores = Vec::new();
        let mut chunks_to_score = Vec::new();
        let mut stats = FilterStats::default();

        // First pass: apply heuristic filters
        for (file_idx, file) in files.iter().enumerate() {
            for (chunk_idx, chunk) in file.chunks.iter().enumerate() {
                let line_count = chunk.lines.len() as u32;
                stats.total_chunks += 1;
                stats.total_lines += line_count;

                let filter_result = self.filter.filter_chunk(chunk, file);

                if filter_result.is_filtered {
                    if let Some(reason) = filter_result.reason {
                        stats.add_filtered(reason, line_count);
                    }
                    all_scores.push(ChunkScore {
                        file_index: file_idx,
                        chunk_index: chunk_idx,
                        chunk_id: chunk.id,
                        response: None,
                        filter_result: Some(filter_result),
                    });
                } else {
                    chunks_to_score.push((file_idx, chunk_idx, file, chunk));
                }
            }
        }

        let total = chunks_to_score.len();
        let mut completed = 0;

        // Second pass: AI scoring for non-filtered chunks
        // Process results as they stream in for real-time feedback
        let mut score_stream = stream::iter(chunks_to_score)
            .map(|(file_idx, chunk_idx, file, chunk)| {
                let provider = Arc::clone(&self.provider);
                let ctx = context.clone();
                let file_path = file.path.to_string_lossy().to_string();
                async move {
                    let diff_text = chunk_to_diff_text(chunk);
                    let language = file
                        .language
                        .map(|l| l.name())
                        .unwrap_or("unknown");

                    let response = provider
                        .score_controversiality(
                            &diff_text,
                            &file_path,
                            language,
                            &ctx,
                        )
                        .await;

                    (file_idx, chunk_idx, chunk.id, file_path, response)
                }
            })
            .buffer_unordered(self.concurrent_requests);

        // Process each result as it completes
        while let Some((file_idx, chunk_idx, chunk_id, file_path, response)) = score_stream.next().await {
            completed += 1;

            let (finding, chunk_score) = match response {
                Ok(resp) => {
                    let filter_result = self.filter.filter_by_score(resp.score);
                    let is_filtered = filter_result.is_filtered;

                    if is_filtered {
                        if let Some(reason) = filter_result.reason {
                            let chunk = &files[file_idx].chunks[chunk_idx];
                            stats.add_filtered(reason, chunk.lines.len() as u32);
                        }
                    }

                    let finding = ScoringFinding {
                        file_path,
                        file_index: file_idx,
                        chunk_index: chunk_idx,
                        score: resp.score,
                        classification: format!("{}", resp.classification),
                        reasoning: resp.reasoning.clone(),
                        is_filtered,
                    };

                    let score = ChunkScore {
                        file_index: file_idx,
                        chunk_index: chunk_idx,
                        chunk_id,
                        response: Some(resp),
                        filter_result: if is_filtered {
                            Some(filter_result)
                        } else {
                            None
                        },
                    };

                    (Some(finding), score)
                }
                Err(e) => {
                    tracing::warn!("Failed to score chunk {}: {}", chunk_id, e);
                    let score = ChunkScore {
                        file_index: file_idx,
                        chunk_index: chunk_idx,
                        chunk_id,
                        response: None,
                        filter_result: None,
                    };
                    (None, score)
                }
            };

            all_scores.push(chunk_score);

            // Send update with finding details
            progress_callback(ScoringUpdate {
                progress: ScoringProgress { completed, total },
                finding,
            });
        }

        stats.filtered_chunks = all_scores
            .iter()
            .filter(|s| s.filter_result.as_ref().map(|f| f.is_filtered).unwrap_or(false))
            .count() as u32;

        Ok(ScoringResult {
            scores: all_scores,
            stats,
        })
    }

    /// Get chunks that need review (not filtered)
    pub fn reviewable_chunks<'a>(&self, result: &'a ScoringResult) -> Vec<&'a ChunkScore> {
        result
            .scores
            .iter()
            .filter(|s| !s.is_filtered())
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct ChunkScore {
    pub file_index: usize,
    pub chunk_index: usize,
    pub chunk_id: ChunkId,
    pub response: Option<ControversialityResponse>,
    pub filter_result: Option<FilterResult>,
}

impl ChunkScore {
    pub fn is_filtered(&self) -> bool {
        self.filter_result
            .as_ref()
            .map(|f| f.is_filtered)
            .unwrap_or(false)
    }

    /// Returns true if filtered by heuristics (whitespace, imports, etc.) but NOT by score threshold
    pub fn is_heuristic_filtered(&self) -> bool {
        self.filter_result
            .as_ref()
            .map(|f| {
                f.is_filtered && f.reason != Some(FilterReason::BelowThreshold)
            })
            .unwrap_or(false)
    }

    pub fn score(&self) -> Option<f64> {
        self.response.as_ref().map(|r| r.score)
    }
}

#[derive(Debug, Clone)]
pub struct ScoringResult {
    pub scores: Vec<ChunkScore>,
    pub stats: FilterStats,
}

impl ScoringResult {
    pub fn average_score(&self) -> Option<f64> {
        let scored: Vec<f64> = self
            .scores
            .iter()
            .filter_map(|s| s.response.as_ref().map(|r| r.score))
            .collect();

        if scored.is_empty() {
            None
        } else {
            Some(scored.iter().sum::<f64>() / scored.len() as f64)
        }
    }

    pub fn max_score(&self) -> Option<f64> {
        self.scores
            .iter()
            .filter_map(|s| s.response.as_ref().map(|r| r.score))
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn reviewable_count(&self) -> usize {
        self.scores.iter().filter(|s| !s.is_filtered()).count()
    }
}

#[derive(Debug, Clone)]
pub struct ScoringProgress {
    pub completed: usize,
    pub total: usize,
}

impl ScoringProgress {
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.completed as f64 / self.total as f64) * 100.0
        }
    }
}

/// Update sent during scoring - includes finding details for real-time display
#[derive(Debug, Clone)]
pub struct ScoringUpdate {
    pub progress: ScoringProgress,
    /// The finding that was just scored (if successful)
    pub finding: Option<ScoringFinding>,
}

/// A single finding from AI scoring
#[derive(Debug, Clone)]
pub struct ScoringFinding {
    pub file_path: String,
    pub file_index: usize,
    pub chunk_index: usize,
    pub score: f64,
    pub classification: String,
    pub reasoning: String,
    pub is_filtered: bool,
}

fn chunk_to_diff_text(chunk: &DiffChunk) -> String {
    let mut lines = Vec::new();

    for line in &chunk.lines {
        let prefix = match line.kind {
            LineKind::Context => ' ',
            LineKind::Add => '+',
            LineKind::Remove => '-',
        };
        lines.push(format!("{}{}", prefix, line.content));
    }

    lines.join("\n")
}
