use crate::ai::schema::{ControversialityResponse, SummaryResponse};
use crate::ai::scoring::{ChunkScore, ScoringResult};
use crate::config::Config;
use crate::diff::{DiffResult, FileDiff};
use crate::error::CraiResult;
use crate::tui::event::{Action, Direction, StreamSortMode, SubagentAction};
use crate::tui::views::stream::calculate_stream_total_lines;
use std::collections::HashSet;

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

/// Navigation item for tree traversal
#[derive(Debug, Clone)]
pub enum TreeNavItem {
    File(usize),
    Highlight(usize, usize), // (file_index, highlight_index)
}

/// Apply a tree navigation selection
fn apply_tree_nav(item: &TreeNavItem, tree_selected: &mut usize, selected_highlight: &mut Option<usize>) {
    match item {
        TreeNavItem::File(idx) => {
            *tree_selected = *idx;
            *selected_highlight = None;
        }
        TreeNavItem::Highlight(file_idx, highlight_idx) => {
            *tree_selected = *file_idx;
            *selected_highlight = Some(*highlight_idx);
        }
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
        /// Sort mode for highlights stream
        sort_mode: StreamSortMode,
        /// Files that are expanded to show highlights
        expanded_files: HashSet<usize>,
        /// Selected highlight within the selected file (None = file itself is selected)
        selected_highlight: Option<usize>,
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
                let expanded = self.compute_smart_expanded();
                self.view = View::Review {
                    tree_selected: 0,
                    tree_scroll_offset: 0,
                    tree_focused: true,
                    stream_scroll_offset: 0,
                    show_analysis: true,
                    sort_mode: StreamSortMode::default(),
                    expanded_files: expanded,
                    selected_highlight: None,
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
            Action::NextHighlight => {
                self.navigate_highlight(1);
            }
            Action::PrevHighlight => {
                self.navigate_highlight(-1);
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
                        let expanded = self.compute_smart_expanded();
                        self.view = View::Review {
                            tree_selected: 0,
                            tree_scroll_offset: 0,
                            tree_focused: true,
                            stream_scroll_offset: 0,
                            show_analysis: true,
                            sort_mode: StreamSortMode::default(),
                            expanded_files: expanded,
                            selected_highlight: None,
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
                        let expanded = self.compute_smart_expanded();
                        self.view = View::Review {
                            tree_selected: 0,
                            tree_scroll_offset: 0,
                            tree_focused: false,
                            stream_scroll_offset: 0,
                            show_analysis: true,
                            sort_mode: StreamSortMode::default(),
                            expanded_files: expanded,
                            selected_highlight: None,
                        };
                    }
                }
            }
            Action::ToggleSortMode => {
                if let View::Review { sort_mode, .. } = &mut self.view {
                    *sort_mode = match sort_mode {
                        StreamSortMode::ByScore => StreamSortMode::ByFile,
                        StreamSortMode::ByFile => StreamSortMode::ByScore,
                    };
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
        // Extract state needed for navigation before mutable borrow
        let (tree_nav_data, is_tree_focused, stream_max_scroll) = if let View::Review {
            tree_selected,
            expanded_files,
            selected_highlight,
            tree_focused,
            sort_mode,
            ..
        } = &self.view
        {
            // Calculate max scroll for stream based on actual highlights
            let max_scroll = calculate_stream_total_lines(self, *sort_mode).saturating_sub(1);

            if *tree_focused {
                let tree_items = self.build_tree_item_list(expanded_files);
                let current_sel = (*tree_selected, *selected_highlight);
                let has_highlights_for_selected = !self.highlights_for_file(*tree_selected).is_empty();
                (Some((tree_items, current_sel, has_highlights_for_selected)), true, max_scroll)
            } else {
                (None, false, max_scroll)
            }
        } else {
            (None, false, 0)
        };

        match &mut self.view {
            View::Review {
                tree_selected,
                tree_scroll_offset,
                tree_focused,
                stream_scroll_offset,
                expanded_files,
                selected_highlight,
                ..
            } => {
                if is_tree_focused {
                    if let Some((tree_items, (curr_file, curr_highlight), has_highlights)) = tree_nav_data {
                        let total_items = tree_items.len();

                        // Find current position
                        let current_pos = tree_items
                            .iter()
                            .enumerate()
                            .find(|(_, item)| match item {
                                TreeNavItem::File(idx) => *idx == curr_file && curr_highlight.is_none(),
                                TreeNavItem::Highlight(f_idx, h_idx) => {
                                    *f_idx == curr_file && Some(*h_idx) == curr_highlight
                                }
                            })
                            .map(|(idx, _)| idx)
                            .unwrap_or(0);

                        match dir {
                            Direction::Up => {
                                if current_pos > 0 {
                                    apply_tree_nav(&tree_items[current_pos - 1], tree_selected, selected_highlight);
                                }
                            }
                            Direction::Down => {
                                if current_pos + 1 < total_items {
                                    apply_tree_nav(&tree_items[current_pos + 1], tree_selected, selected_highlight);
                                }
                            }
                            Direction::Home => {
                                if !tree_items.is_empty() {
                                    apply_tree_nav(&tree_items[0], tree_selected, selected_highlight);
                                    *tree_scroll_offset = 0;
                                }
                            }
                            Direction::End => {
                                if !tree_items.is_empty() {
                                    apply_tree_nav(&tree_items[total_items - 1], tree_selected, selected_highlight);
                                }
                            }
                            Direction::PageUp => {
                                let new_pos = current_pos.saturating_sub(10);
                                if !tree_items.is_empty() {
                                    apply_tree_nav(&tree_items[new_pos], tree_selected, selected_highlight);
                                }
                            }
                            Direction::PageDown => {
                                let new_pos = (current_pos + 10).min(total_items.saturating_sub(1));
                                if !tree_items.is_empty() {
                                    apply_tree_nav(&tree_items[new_pos], tree_selected, selected_highlight);
                                }
                            }
                            Direction::Left => {
                                if selected_highlight.is_some() {
                                    *selected_highlight = None;
                                } else if expanded_files.contains(tree_selected) {
                                    expanded_files.remove(tree_selected);
                                }
                            }
                            Direction::Right => {
                                if selected_highlight.is_none() {
                                    if has_highlights {
                                        if !expanded_files.contains(tree_selected) {
                                            expanded_files.insert(*tree_selected);
                                        } else {
                                            *selected_highlight = Some(0);
                                        }
                                    } else {
                                        *tree_focused = false;
                                    }
                                } else {
                                    *tree_focused = false;
                                }
                            }
                        }
                    }
                } else {
                    // Navigate stream using pre-calculated max scroll
                    match dir {
                        Direction::Up => {
                            *stream_scroll_offset = stream_scroll_offset.saturating_sub(1);
                        }
                        Direction::Down => {
                            *stream_scroll_offset = (*stream_scroll_offset + 1).min(stream_max_scroll);
                        }
                        Direction::PageUp => {
                            *stream_scroll_offset = stream_scroll_offset.saturating_sub(20);
                        }
                        Direction::PageDown => {
                            *stream_scroll_offset = (*stream_scroll_offset + 20).min(stream_max_scroll);
                        }
                        Direction::Home => {
                            *stream_scroll_offset = 0;
                        }
                        Direction::End => {
                            *stream_scroll_offset = stream_max_scroll;
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
                let expanded = self.compute_smart_expanded();
                self.view = View::Review {
                    tree_selected: 0,
                    tree_scroll_offset: 0,
                    tree_focused: false, // Start with stream focused
                    stream_scroll_offset: 0,
                    show_analysis: true,
                    sort_mode: StreamSortMode::default(),
                    expanded_files: expanded,
                    selected_highlight: None,
                };
            }
            View::Review {
                tree_selected,
                tree_focused,
                expanded_files,
                selected_highlight,
                ..
            } => {
                if *tree_focused {
                    if selected_highlight.is_some() {
                        // A highlight is selected - jump to it in stream and focus stream
                        // TODO: Jump to specific highlight
                        *tree_focused = false;
                    } else {
                        // A file is selected - toggle expand/collapse
                        if expanded_files.contains(tree_selected) {
                            expanded_files.remove(tree_selected);
                        } else {
                            expanded_files.insert(*tree_selected);
                        }
                    }
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

    fn navigate_highlight(&mut self, delta: i32) {
        // Get highlight positions first (before mutable borrow)
        let highlight_starts = self.get_highlight_starts();
        if highlight_starts.is_empty() {
            return;
        }

        if let View::Review {
            stream_scroll_offset,
            tree_focused,
            ..
        } = &mut self.view
        {
            // Find current highlight index based on scroll position
            let current_idx = highlight_starts
                .iter()
                .enumerate()
                .rev()
                .find(|(_, &start)| *stream_scroll_offset >= start)
                .map(|(idx, _)| idx)
                .unwrap_or(0);

            // Calculate new index
            let new_idx = if delta > 0 {
                (current_idx + delta as usize).min(highlight_starts.len().saturating_sub(1))
            } else {
                current_idx.saturating_sub((-delta) as usize)
            };

            // Jump to new highlight
            if let Some(&new_offset) = highlight_starts.get(new_idx) {
                *stream_scroll_offset = new_offset;
                *tree_focused = false; // Ensure stream is focused
            }
        }
    }

    /// Get the starting line positions for each highlight in the stream
    fn get_highlight_starts(&self) -> Vec<usize> {
        let Some(scoring_result) = &self.scoring_result else {
            return Vec::new();
        };

        let highlights: Vec<_> = scoring_result
            .scores
            .iter()
            .filter(|s| !s.is_filtered() && s.response.is_some())
            .collect();

        let mut starts = Vec::with_capacity(highlights.len());
        let mut current_line = 0;

        for score in highlights {
            starts.push(current_line);
            current_line += self.calculate_highlight_height(score);
        }

        starts
    }

    /// Calculate how many lines a highlight block needs (mirrors stream.rs logic)
    fn calculate_highlight_height(&self, score: &ChunkScore) -> usize {
        let file = match self.diff_result.files.get(score.file_index) {
            Some(f) => f,
            None => return 0,
        };
        let chunk = match file.chunks.get(score.chunk_index) {
            Some(c) => c,
            None => return 0,
        };

        let mut height = 0;

        // Header: 3 lines (title + separator + blank)
        height += 3;

        // Side-by-side diff: chunk lines + 2 for borders
        height += chunk.lines.len() + 2;

        // Analysis section
        if let Some(resp) = &score.response {
            height += 1; // "Analysis" header
            height += 1; // Classification/Score line
            height += 1; // Blank
            height += 1; // Reasoning

            if !resp.concerns.is_empty() {
                height += 1; // Blank
                height += 1; // "Concerns:" header
                height += resp.concerns.len();
            }
        }

        // Separator: 2 lines
        height += 2;

        height
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

    /// Compute which files should be auto-expanded (smart expand)
    /// Files are expanded if they have at least one highlight with score >= 0.5
    pub fn compute_smart_expanded(&self) -> HashSet<usize> {
        let mut expanded = HashSet::new();

        if let Some(sr) = &self.scoring_result {
            for score in &sr.scores {
                if let Some(resp) = &score.response {
                    if resp.score >= 0.5 {
                        expanded.insert(score.file_index);
                    }
                }
            }
        }

        expanded
    }

    /// Get highlights for a specific file (non-heuristic-filtered chunks with responses)
    pub fn highlights_for_file(&self, file_index: usize) -> Vec<&ChunkScore> {
        self.scoring_result
            .as_ref()
            .map(|sr| {
                sr.scores
                    .iter()
                    .filter(|s| {
                        s.file_index == file_index
                            && s.response.is_some()
                            && !s.is_heuristic_filtered()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Build flat list of tree items for navigation
    fn build_tree_item_list(&self, expanded_files: &HashSet<usize>) -> Vec<TreeNavItem> {
        let mut items = Vec::new();
        for (file_idx, _) in self.diff_result.files.iter().enumerate() {
            items.push(TreeNavItem::File(file_idx));
            if expanded_files.contains(&file_idx) {
                let highlights = self.highlights_for_file(file_idx);
                for (h_idx, _) in highlights.iter().enumerate() {
                    items.push(TreeNavItem::Highlight(file_idx, h_idx));
                }
            }
        }
        items
    }

}
