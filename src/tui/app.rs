use crate::ai::schema::{ControversialityResponse, SummaryResponse};
use crate::ai::scoring::{ChunkScore, ScoringResult};
use crate::config::Config;
use crate::diff::{DiffResult, FileDiff};
use crate::error::CraiResult;
use crate::tui::event::{Action, Direction, SubagentAction};

pub struct App {
    pub config: Config,
    pub diff_result: DiffResult,
    pub scoring_result: Option<ScoringResult>,
    pub summary: Option<SummaryResponse>,
    pub view: View,
    pub should_quit: bool,
    pub status_message: Option<StatusMessage>,
    pub progress: Option<Progress>,
}

#[derive(Debug, Clone)]
pub enum View {
    Summary,
    FileTree {
        selected: usize,
        scroll_offset: usize,
    },
    DiffView {
        file_index: usize,
        chunk_index: usize,
        scroll_offset: usize,
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
        Self {
            config,
            diff_result,
            scoring_result: None,
            summary: None,
            view: View::Summary,
            should_quit: false,
            status_message: None,
            progress: None,
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
                self.view = View::FileTree {
                    selected: 0,
                    scroll_offset: 0,
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
            View::FileTree { .. } => View::Summary,
            View::DiffView { file_index, .. } => View::FileTree {
                selected: *file_index,
                scroll_offset: 0,
            },
            View::Summary | View::QuitConfirm => {
                self.view = View::QuitConfirm;
                return;
            }
        };
    }

    fn handle_navigation(&mut self, dir: Direction) {
        match &mut self.view {
            View::FileTree { selected, scroll_offset } => {
                let file_count = self.diff_result.files.len();
                match dir {
                    Direction::Up => {
                        *selected = selected.saturating_sub(1);
                        if *selected < *scroll_offset {
                            *scroll_offset = *selected;
                        }
                    }
                    Direction::Down => {
                        *selected = (*selected + 1).min(file_count.saturating_sub(1));
                    }
                    Direction::Home => {
                        *selected = 0;
                        *scroll_offset = 0;
                    }
                    Direction::End => {
                        *selected = file_count.saturating_sub(1);
                    }
                    Direction::PageUp => {
                        *selected = selected.saturating_sub(10);
                        if *selected < *scroll_offset {
                            *scroll_offset = *selected;
                        }
                    }
                    Direction::PageDown => {
                        *selected = (*selected + 10).min(file_count.saturating_sub(1));
                    }
                    _ => {}
                }
            }
            View::DiffView {
                file_index,
                chunk_index,
                scroll_offset,
                ..
            } => {
                let file = &self.diff_result.files[*file_index];
                let chunk_count = file.chunks.len();

                match dir {
                    Direction::Up => {
                        if *scroll_offset > 0 {
                            *scroll_offset -= 1;
                        } else if *chunk_index > 0 {
                            *chunk_index -= 1;
                            *scroll_offset = 0;
                        }
                    }
                    Direction::Down => {
                        *scroll_offset += 1;
                    }
                    Direction::Left => {
                        if *chunk_index > 0 {
                            *chunk_index -= 1;
                            *scroll_offset = 0;
                        }
                    }
                    Direction::Right => {
                        if *chunk_index + 1 < chunk_count {
                            *chunk_index += 1;
                            *scroll_offset = 0;
                        }
                    }
                    Direction::PageUp => {
                        *scroll_offset = scroll_offset.saturating_sub(20);
                    }
                    Direction::PageDown => {
                        *scroll_offset += 20;
                    }
                    Direction::Home => {
                        *chunk_index = 0;
                        *scroll_offset = 0;
                    }
                    Direction::End => {
                        *chunk_index = chunk_count.saturating_sub(1);
                        *scroll_offset = 0;
                    }
                }
            }
            _ => {}
        }
    }

    fn handle_select(&mut self) {
        match &self.view {
            View::Summary => {
                self.view = View::FileTree {
                    selected: 0,
                    scroll_offset: 0,
                };
            }
            View::FileTree { selected, .. } => {
                if *selected < self.diff_result.files.len() {
                    self.view = View::DiffView {
                        file_index: *selected,
                        chunk_index: 0,
                        scroll_offset: 0,
                        show_analysis: true,
                    };
                }
            }
            View::Help => {
                self.view = View::Summary;
            }
            _ => {}
        }
    }

    fn handle_tab(&mut self) {
        if let View::DiffView { show_analysis, .. } = &mut self.view {
            *show_analysis = !*show_analysis;
        }
    }

    fn navigate_file(&mut self, delta: i32) {
        if let View::DiffView { file_index, chunk_index, scroll_offset, show_analysis: _ } = &mut self.view {
            let file_count = self.diff_result.files.len();
            let new_index = if delta > 0 {
                (*file_index + delta as usize).min(file_count.saturating_sub(1))
            } else {
                file_index.saturating_sub((-delta) as usize)
            };

            if new_index != *file_index {
                *file_index = new_index;
                *chunk_index = 0;
                *scroll_offset = 0;
            }
        }
    }

    // Accessor methods for views

    pub fn files(&self) -> &[FileDiff] {
        &self.diff_result.files
    }

    pub fn current_file(&self) -> Option<&FileDiff> {
        match &self.view {
            View::DiffView { file_index, .. } => self.diff_result.files.get(*file_index),
            View::FileTree { selected, .. } => self.diff_result.files.get(*selected),
            _ => None,
        }
    }

    pub fn current_chunk_score(&self) -> Option<&ChunkScore> {
        match &self.view {
            View::DiffView { file_index, chunk_index, .. } => {
                self.scoring_result.as_ref().and_then(|sr| {
                    sr.scores.iter().find(|s| {
                        s.file_index == *file_index && s.chunk_index == *chunk_index
                    })
                })
            }
            _ => None,
        }
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
