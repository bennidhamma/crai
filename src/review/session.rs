use crate::ai::schema::{ControversialityResponse, SubagentReviewResponse, SummaryResponse};
use crate::ai::scoring::ScoringResult;
use crate::diff::chunk::ChunkId;
use crate::diff::DiffResult;
use std::collections::HashMap;
use std::time::Instant;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ReviewSession {
    pub id: SessionId,
    pub base_branch: String,
    pub compare_branch: String,
    pub started_at: Instant,
    pub diff_result: DiffResult,
    pub scoring_result: Option<ScoringResult>,
    pub summary: Option<SummaryResponse>,
    pub file_states: HashMap<usize, FileReviewState>,
    pub subagent_reviews: SubagentReviews,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId(pub Uuid);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ReviewSession {
    pub fn new(diff_result: DiffResult) -> Self {
        let file_states = diff_result
            .files
            .iter()
            .enumerate()
            .map(|(idx, _)| (idx, FileReviewState::default()))
            .collect();

        Self {
            id: SessionId::new(),
            base_branch: diff_result.base_branch.clone(),
            compare_branch: diff_result.compare_branch.clone(),
            started_at: Instant::now(),
            diff_result,
            scoring_result: None,
            summary: None,
            file_states,
            subagent_reviews: SubagentReviews::default(),
        }
    }

    pub fn set_scoring_result(&mut self, result: ScoringResult) {
        // Initialize chunk states from scoring
        for score in &result.scores {
            if let Some(file_state) = self.file_states.get_mut(&score.file_index) {
                file_state.chunk_states.insert(
                    score.chunk_id,
                    ChunkReviewState {
                        score: score.response.clone(),
                        user_status: UserChunkStatus::Unreviewed,
                        notes: Vec::new(),
                    },
                );
            }
        }

        self.scoring_result = Some(result);
    }

    pub fn set_summary(&mut self, summary: SummaryResponse) {
        self.summary = Some(summary);
    }

    pub fn mark_chunk_status(&mut self, file_idx: usize, chunk_id: ChunkId, status: UserChunkStatus) {
        if let Some(file_state) = self.file_states.get_mut(&file_idx) {
            if let Some(chunk_state) = file_state.chunk_states.get_mut(&chunk_id) {
                chunk_state.user_status = status;
            }
        }
    }

    pub fn add_note(&mut self, file_idx: usize, chunk_id: ChunkId, note: String) {
        if let Some(file_state) = self.file_states.get_mut(&file_idx) {
            if let Some(chunk_state) = file_state.chunk_states.get_mut(&chunk_id) {
                chunk_state.notes.push(UserNote {
                    text: note,
                    created_at: Instant::now(),
                });
            }
        }
    }

    pub fn file_status(&self, file_idx: usize) -> FileReviewStatus {
        self.file_states
            .get(&file_idx)
            .map(|s| s.status)
            .unwrap_or(FileReviewStatus::Pending)
    }

    pub fn set_file_status(&mut self, file_idx: usize, status: FileReviewStatus) {
        if let Some(file_state) = self.file_states.get_mut(&file_idx) {
            file_state.status = status;
        }
    }

    pub fn progress(&self) -> ReviewProgress {
        let total_files = self.diff_result.files.len();
        let completed_files = self
            .file_states
            .values()
            .filter(|s| s.status == FileReviewStatus::Completed)
            .count();

        let (total_chunks, reviewed_chunks) = if let Some(ref scoring) = self.scoring_result {
            let total = scoring.reviewable_count();
            let reviewed = self
                .file_states
                .values()
                .flat_map(|fs| fs.chunk_states.values())
                .filter(|cs| cs.user_status != UserChunkStatus::Unreviewed)
                .count();
            (total, reviewed)
        } else {
            (0, 0)
        };

        ReviewProgress {
            total_files,
            completed_files,
            total_chunks,
            reviewed_chunks,
        }
    }

    pub fn elapsed(&self) -> std::time::Duration {
        self.started_at.elapsed()
    }
}

#[derive(Debug, Clone, Default)]
pub struct FileReviewState {
    pub status: FileReviewStatus,
    pub chunk_states: HashMap<ChunkId, ChunkReviewState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileReviewStatus {
    #[default]
    Pending,
    InProgress,
    Completed,
    Skipped,
}

#[derive(Debug, Clone)]
pub struct ChunkReviewState {
    pub score: Option<ControversialityResponse>,
    pub user_status: UserChunkStatus,
    pub notes: Vec<UserNote>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UserChunkStatus {
    #[default]
    Unreviewed,
    Viewed,
    Approved,
    NeedsDiscussion,
    RequestedChanges,
}

impl UserChunkStatus {
    pub fn symbol(&self) -> char {
        match self {
            Self::Unreviewed => ' ',
            Self::Viewed => '.',
            Self::Approved => '+',
            Self::NeedsDiscussion => '?',
            Self::RequestedChanges => '!',
        }
    }
}

#[derive(Debug, Clone)]
pub struct UserNote {
    pub text: String,
    pub created_at: Instant,
}

#[derive(Debug, Clone, Default)]
pub struct SubagentReviews {
    pub security: Option<SubagentReviewResponse>,
    pub performance: Option<SubagentReviewResponse>,
    pub usability: Option<SubagentReviewResponse>,
}

#[derive(Debug, Clone, Copy)]
pub struct ReviewProgress {
    pub total_files: usize,
    pub completed_files: usize,
    pub total_chunks: usize,
    pub reviewed_chunks: usize,
}

impl ReviewProgress {
    pub fn file_percentage(&self) -> f64 {
        if self.total_files == 0 {
            100.0
        } else {
            (self.completed_files as f64 / self.total_files as f64) * 100.0
        }
    }

    pub fn chunk_percentage(&self) -> f64 {
        if self.total_chunks == 0 {
            100.0
        } else {
            (self.reviewed_chunks as f64 / self.total_chunks as f64) * 100.0
        }
    }
}
