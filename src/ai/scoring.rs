use crate::ai::provider::{AiProvider, ScoringContext};
use crate::ai::schema::ControversialityResponse;
use crate::diff::chunk::{ChunkId, DiffChunk, FileDiff, LineKind};
use crate::diff::filter::{ChunkFilter, FilterResult, FilterStats};
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
        F: FnMut(ScoringProgress) + Send,
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
        let scores: Vec<_> = stream::iter(chunks_to_score)
            .map(|(file_idx, chunk_idx, file, chunk)| {
                let provider = Arc::clone(&self.provider);
                let ctx = context.clone();
                async move {
                    let diff_text = chunk_to_diff_text(chunk);
                    let language = file
                        .language
                        .map(|l| l.name())
                        .unwrap_or("unknown");

                    let response = provider
                        .score_controversiality(
                            &diff_text,
                            &file.path.to_string_lossy(),
                            language,
                            &ctx,
                        )
                        .await;

                    (file_idx, chunk_idx, chunk.id, response)
                }
            })
            .buffer_unordered(self.concurrent_requests)
            .collect()
            .await;

        for (file_idx, chunk_idx, chunk_id, response) in scores {
            completed += 1;
            progress_callback(ScoringProgress { completed, total });

            match response {
                Ok(resp) => {
                    let filter_result = self.filter.filter_by_score(resp.score);

                    if filter_result.is_filtered {
                        if let Some(reason) = filter_result.reason {
                            let chunk = &files[file_idx].chunks[chunk_idx];
                            stats.add_filtered(reason, chunk.lines.len() as u32);
                        }
                    }

                    all_scores.push(ChunkScore {
                        file_index: file_idx,
                        chunk_index: chunk_idx,
                        chunk_id,
                        response: Some(resp),
                        filter_result: if filter_result.is_filtered {
                            Some(filter_result)
                        } else {
                            None
                        },
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to score chunk {}: {}", chunk_id, e);
                    all_scores.push(ChunkScore {
                        file_index: file_idx,
                        chunk_index: chunk_idx,
                        chunk_id,
                        response: None,
                        filter_result: None,
                    });
                }
            }
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

#[derive(Debug, Clone, Copy)]
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
