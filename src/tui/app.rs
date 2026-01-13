use crate::ai::schema::{ControversialityResponse, SummaryResponse};
use crate::ai::scoring::{ChunkScore, ScoringResult};
use crate::config::Config;
use crate::diff::{DiffResult, FileDiff};
use crate::error::CraiResult;
use crate::tui::event::{Action, Direction, SubagentAction};

/// Precomputed index for efficient stream navigation
#[derive(Debug, Clone)]
pub struct StreamIndex {
    /// Total lines in the virtual stream
    pub total_lines: usize,
    /// Line offset where each file starts in the stream
    pub file_starts: Vec<usize>,
    /// Chunk start offsets within each file: chunk_starts[file_idx][chunk_idx]
    pub chunk_starts: Vec<Vec<usize>>,
}

impl StreamIndex {
    /// Build index from diff result
    pub fn build(diff_result: &DiffResult) -> Self {
        let mut total_lines = 0;
        let mut file_starts = Vec::with_capacity(diff_result.files.len());
        let mut chunk_starts = Vec::with_capacity(diff_result.files.len());

        for file in &diff_result.files {
            file_starts.push(total_lines);

            // File header: 2 lines (filename + separator)
            total_lines += 2;

            let mut file_chunk_starts = Vec::with_capacity(file.chunks.len());
            for chunk in &file.chunks {
                file_chunk_starts.push(total_lines - file_starts.last().copied().unwrap_or(0));

                // Chunk header: 1 line (@@ ... @@)
                total_lines += 1;

                // Chunk lines
                total_lines += chunk.lines.len();

                // Spacing after chunk: 1 line
                total_lines += 1;
            }
            chunk_starts.push(file_chunk_starts);

            // Spacing after file: 1 line
            total_lines += 1;
        }

        Self {
            total_lines,
            file_starts,
            chunk_starts,
        }
    }

    /// Get stream position for the start of a file
    pub fn file_to_position(&self, file_index: usize) -> usize {
        self.file_starts.get(file_index).copied().unwrap_or(0)
    }

    /// Find which file/chunk a stream position falls into
    pub fn position_to_context(&self, position: usize) -> Option<(usize, usize)> {
        // Find file via binary search
        let file_index = match self.file_starts.binary_search(&position) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };

        if file_index >= self.file_starts.len() {
            return None;
        }

        let file_start = self.file_starts[file_index];
        let relative_pos = position.saturating_sub(file_start);

        // Find chunk within file
        let chunk_index = match self.chunk_starts[file_index].binary_search(&relative_pos) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };

        Some((file_index, chunk_index.min(self.chunk_starts[file_index].len().saturating_sub(1))))
    }
}

pub struct App {
    pub config: Config,
    pub diff_result: DiffResult,
    pub scoring_result: Option<ScoringResult>,
    pub summary: Option<SummaryResponse>,
    pub view: View,
    pub should_quit: bool,
    pub status_message: Option<StatusMessage>,
    pub progress: Option<Progress>,
    pub stream_index: StreamIndex,
}

#[derive(Debug, Clone)]
pub enum View {
    Summary,
    /// Main review mode with file tree sidebar and diff stream
    Review {
        /// Selected file in tree
        tree_selected: usize,
        /// Scroll offset in tree
        tree_scroll_offset: usize,
        /// True if file tree has focus, false if stream has focus
        tree_focused: bool,
        /// Scroll position in the virtual diff stream
        stream_scroll_offset: usize,
        /// Show analysis pane
        show_analysis: bool,
    },
    Stats,
    Help,
    QuitConfirm,
}

impl Default for View {
    fn default() -> Self {
        Self::Summary
    }
}

#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub text: String,
    pub level: MessageLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Progress {
    pub operation: String,
    pub current: usize,
    pub total: usize,
}

impl Progress {
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.current as f64 / self.total as f64) * 100.0
        }
    }
}

impl App {
    pub fn new(config: Config, diff_result: DiffResult) -> Self {
        let stream_index = StreamIndex::build(&diff_result);
        Self {
            config,
            diff_result,
            scoring_result: None,
            summary: None,
            view: View::Summary,
            should_quit: false,
            status_message: None,
            progress: None,
            stream_index,
        }
    }

    pub fn set_scoring_result(&mut self, result: ScoringResult) {
        self.scoring_result = Some(result);
    }

    pub fn set_summary(&mut self, summary: SummaryResponse) {
        self.summary = Some(summary);
    }

    pub fn set_progress(&mut self, operation: &str, current: usize, total: usize) {
        self.progress = Some(Progress {
            operation: operation.to_string(),
            current,
            total,
        });
    }

    pub fn clear_progress(&mut self) {
        self.progress = None;
    }

    pub fn set_status(&mut self, text: &str, level: MessageLevel) {
        self.status_message = Some(StatusMessage {
            text: text.to_string(),
            level,
        });
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    pub fn handle_action(&mut self, action: Action) -> CraiResult<()> {
        // Handle quit confirmation specially
        if matches!(self.view, View::QuitConfirm) {
            match action {
                Action::Quit | Action::Select | Action::ConfirmYes => {
                    // 'q', 'y', or Enter confirms quit
                    self.should_quit = true;
                }
                _ => {
                    // Any other key cancels
                    self.view = View::Summary;
                }
            }
            return Ok(());
        }

        match action {
            Action::Quit => {
                if matches!(self.view, View::Summary) {
                    self.view = View::QuitConfirm;
                } else {
                    self.view = View::Summary;
                }
            }
            Action::ForceQuit => {
                self.should_quit = true;
            }
            Action::Help => {
                self.view = View::Help;
            }
            Action::Summary => {
                self.view = View::Summary;
            }
            Action::FileTree => {
                self.view = View::Review {
                    tree_selected: 0,
                    tree_scroll_offset: 0,
                    tree_focused: true,
                    stream_scroll_offset: 0,
                    show_analysis: true,
                };
            }
            Action::Stats => {
                self.view = View::Stats;
            }
            Action::Back => {
                self.handle_back();
            }
            Action::Navigate(dir) => {
                self.handle_navigation(dir);
            }
            Action::Select => {
                self.handle_select();
            }
            Action::Tab => {
                self.handle_tab();
            }
            Action::NextFile => {
                self.navigate_file(1);
            }
            Action::PrevFile => {
                self.navigate_file(-1);
            }
            Action::Approve | Action::Discuss | Action::RequestChanges | Action::AddNote => {
                // These would modify review state - stub for now
                self.set_status("Review actions not yet implemented", MessageLevel::Info);
            }
            Action::RunSubagent(subagent) => {
                self.set_status(
                    &format!("Running {} review...", match subagent {
                        SubagentAction::Security => "security",
                        SubagentAction::Performance => "performance",
                        SubagentAction::Usability => "usability",
                    }),
                    MessageLevel::Info,
                );
            }
            Action::ToggleFilter => {
                // Toggle showing filtered chunks
            }
            Action::FocusTree => {
                // '2' - go to review mode with tree focused
                match &mut self.view {
                    View::Review { tree_focused, .. } => {
                        *tree_focused = true;
                    }
                    _ => {
                        self.view = View::Review {
                            tree_selected: 0,
                            tree_scroll_offset: 0,
                            tree_focused: true,
                            stream_scroll_offset: 0,
                            show_analysis: true,
                        };
                    }
                }
            }
            Action::FocusStream => {
                // '3' - go to review mode with stream focused
                match &mut self.view {
                    View::Review { tree_focused, .. } => {
                        *tree_focused = false;
                    }
                    _ => {
                        self.view = View::Review {
                            tree_selected: 0,
                            tree_scroll_offset: 0,
                            tree_focused: false,
                            stream_scroll_offset: 0,
                            show_analysis: true,
                        };
                    }
                }
            }
            Action::ConfirmYes => {
                // Only used in QuitConfirm dialog, handled above
            }
            Action::None => {}
        }
        Ok(())
    }

    fn handle_back(&mut self) {
        self.view = match &self.view {
            View::Help => View::Summary,
            View::Stats => View::Summary,
            View::Review { .. } => View::Summary,
            View::Summary | View::QuitConfirm => {
                self.view = View::QuitConfirm;
                return;
            }
        };
    }

    fn handle_navigation(&mut self, dir: Direction) {
        match &mut self.view {
            View::Review {
                tree_selected,
                tree_scroll_offset,
                tree_focused,
                stream_scroll_offset,
                ..
            } => {
                if *tree_focused {
                    // Navigate file tree
                    let file_count = self.diff_result.files.len();
                    match dir {
                        Direction::Up => {
                            *tree_selected = tree_selected.saturating_sub(1);
                            if *tree_selected < *tree_scroll_offset {
                                *tree_scroll_offset = *tree_selected;
                            }
                        }
                        Direction::Down => {
                            *tree_selected = (*tree_selected + 1).min(file_count.saturating_sub(1));
                        }
                        Direction::Home => {
                            *tree_selected = 0;
                            *tree_scroll_offset = 0;
                        }
                        Direction::End => {
                            *tree_selected = file_count.saturating_sub(1);
                        }
                        Direction::PageUp => {
                            *tree_selected = tree_selected.saturating_sub(10);
                            if *tree_selected < *tree_scroll_offset {
                                *tree_scroll_offset = *tree_selected;
                            }
                        }
                        Direction::PageDown => {
                            *tree_selected = (*tree_selected + 10).min(file_count.saturating_sub(1));
                        }
                        Direction::Left => {
                            // Stay in tree, do nothing
                        }
                        Direction::Right => {
                            // Move focus to stream
                            *tree_focused = false;
                        }
                    }
                } else {
                    // Navigate stream
                    let max_scroll = self.stream_index.total_lines.saturating_sub(1);
                    match dir {
                        Direction::Up => {
                            *stream_scroll_offset = stream_scroll_offset.saturating_sub(1);
                        }
                        Direction::Down => {
                            *stream_scroll_offset = (*stream_scroll_offset + 1).min(max_scroll);
                        }
                        Direction::PageUp => {
                            *stream_scroll_offset = stream_scroll_offset.saturating_sub(20);
                        }
                        Direction::PageDown => {
                            *stream_scroll_offset = (*stream_scroll_offset + 20).min(max_scroll);
                        }
                        Direction::Home => {
                            *stream_scroll_offset = 0;
                        }
                        Direction::End => {
                            *stream_scroll_offset = max_scroll;
                        }
                        Direction::Left => {
                            // Move focus to tree
                            *tree_focused = true;
                        }
                        Direction::Right => {
                            // Stay in stream, do nothing
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_select(&mut self) {
        match &mut self.view {
            View::Summary => {
                // Enter review mode
                self.view = View::Review {
                    tree_selected: 0,
                    tree_scroll_offset: 0,
                    tree_focused: false, // Start with stream focused
                    stream_scroll_offset: 0,
                    show_analysis: true,
                };
            }
            View::Review {
                tree_selected,
                tree_focused,
                stream_scroll_offset,
                ..
            } => {
                if *tree_focused {
                    // Jump stream to selected file
                    *stream_scroll_offset = self.stream_index.file_to_position(*tree_selected);
                    *tree_focused = false; // Move focus to stream
                }
            }
            View::Help => {
                self.view = View::Summary;
            }
            _ => {}
        }
    }

    fn handle_tab(&mut self) {
        if let View::Review { tree_focused, show_analysis, .. } = &mut self.view {
            // Tab toggles focus between tree and stream
            *tree_focused = !*tree_focused;
            // Double-tap tab (when already on tree) could toggle analysis
            // For now, just toggle focus
            let _ = show_analysis; // Silence unused warning
        }
    }

    fn navigate_file(&mut self, delta: i32) {
        if let View::Review {
            tree_selected,
            stream_scroll_offset,
            ..
        } = &mut self.view
        {
            let file_count = self.diff_result.files.len();
            let new_index = if delta > 0 {
                (*tree_selected + delta as usize).min(file_count.saturating_sub(1))
            } else {
                tree_selected.saturating_sub((-delta) as usize)
            };

            if new_index != *tree_selected {
                *tree_selected = new_index;
                *stream_scroll_offset = self.stream_index.file_to_position(new_index);
            }
        }
    }

    // Accessor methods for views

    pub fn files(&self) -> &[FileDiff] {
        &self.diff_result.files
    }

    pub fn current_file(&self) -> Option<&FileDiff> {
        match &self.view {
            View::Review { tree_selected, .. } => self.diff_result.files.get(*tree_selected),
            _ => None,
        }
    }

    /// Get the current file and chunk index based on stream scroll position
    pub fn current_context(&self) -> Option<(usize, usize)> {
        match &self.view {
            View::Review { stream_scroll_offset, .. } => {
                self.stream_index.position_to_context(*stream_scroll_offset)
            }
            _ => None,
        }
    }

    pub fn current_chunk_score(&self) -> Option<&ChunkScore> {
        self.current_context().and_then(|(file_index, chunk_index)| {
            self.scoring_result.as_ref().and_then(|sr| {
                sr.scores.iter().find(|s| {
                    s.file_index == file_index && s.chunk_index == chunk_index
                })
            })
        })
    }

    pub fn current_analysis(&self) -> Option<&ControversialityResponse> {
        self.current_chunk_score().and_then(|cs| cs.response.as_ref())
    }

    pub fn reviewable_chunks_count(&self) -> usize {
        self.scoring_result
            .as_ref()
            .map(|sr| sr.reviewable_count())
            .unwrap_or(0)
    }

    pub fn total_chunks_count(&self) -> usize {
        self.diff_result.files.iter().map(|f| f.chunks.len()).sum()
    }

    pub fn filtered_lines_count(&self) -> u32 {
        self.scoring_result
            .as_ref()
            .map(|sr| sr.stats.filtered_lines)
            .unwrap_or(0)
    }

    pub fn total_lines_count(&self) -> u32 {
        self.scoring_result
            .as_ref()
            .map(|sr| sr.stats.total_lines)
            .unwrap_or(0)
    }
}
