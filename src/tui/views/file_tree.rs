use crate::ai::scoring::ChunkScore;
use crate::diff::chunk::FileStatus;
use crate::tui::app::App;
use crate::tui::event::StreamSortMode;
use crate::tui::views::stream::get_sorted_highlights;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use std::collections::HashSet;

/// Tree item - either a file or a highlight
#[derive(Debug, Clone)]
enum TreeItem {
    File { index: usize },
    Highlight { file_index: usize, highlight_index: usize },
}

/// Render compact file tree sidebar for Review view with expandable highlights
pub fn render_sidebar(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    selected_file: usize,
    scroll_offset: usize,
    is_focused: bool,
    expanded_files: &HashSet<usize>,
    selected_highlight: Option<usize>,
    sort_mode: StreamSortMode,
) {
    match sort_mode {
        StreamSortMode::ByScore => {
            render_flat_highlights(frame, area, app, selected_highlight.unwrap_or(0), scroll_offset, is_focused);
        }
        StreamSortMode::ByFile => {
            render_file_tree(frame, area, app, selected_file, scroll_offset, is_focused, expanded_files, selected_highlight);
        }
    }
}

/// Render flat list of highlights sorted by score (for ByScore mode)
fn render_flat_highlights(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    selected_index: usize,
    scroll_offset: usize,
    is_focused: bool,
) {
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let visible_height = area.height.saturating_sub(2) as usize;
    let sidebar_width = area.width.saturating_sub(4) as usize;

    // Get highlights sorted by score
    let (highlights, divider_index) = get_sorted_highlights(app, StreamSortMode::ByScore);
    let threshold = app.config.filters.controversiality_threshold;

    // Render visible items
    let items: Vec<ListItem> = highlights
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(idx, score)| {
            render_highlight_item(app, score, idx, selected_index, sidebar_width, divider_index, threshold)
        })
        .collect();

    let total_items = highlights.len();
    let title = format!(" (2) Highlights ({}) ", total_items);

    let list = List::new(items).block(
        Block::default()
            .title(title)
            .title_bottom(Line::from(format!(" {}/{} ", selected_index + 1, total_items)).right_aligned())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)),
    );

    let mut state = ListState::default();
    if selected_index >= scroll_offset {
        state.select(Some(selected_index - scroll_offset));
    }

    frame.render_stateful_widget(list, area, &mut state);
}

/// Render a single highlight item for the sidebar
fn render_highlight_item<'a>(
    app: &'a App,
    score: &'a ChunkScore,
    idx: usize,
    selected_index: usize,
    sidebar_width: usize,
    divider_index: Option<usize>,
    _threshold: f64,
) -> ListItem<'a> {
    let file = &app.diff_result.files[score.file_index];
    let resp = score.response.as_ref();

    let score_val = resp.map(|r| r.score).unwrap_or(0.0);
    let reasoning = resp
        .map(|r| r.reasoning.replace('\n', " "))
        .unwrap_or_default();

    // Get filename for context
    let filename = file
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?");

    let score_style = if score_val >= 0.7 {
        Style::default().fg(Color::Red)
    } else if score_val >= 0.5 {
        Style::default().fg(Color::Yellow)
    } else if divider_index.map(|d| idx >= d).unwrap_or(false) {
        // Below threshold - dim
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default().fg(Color::Green)
    };

    // Format: [XX%] filename: reasoning preview...
    let prefix = format!("[{:>2.0}%] ", score_val * 100.0);
    let prefix_len = prefix.chars().count();

    // Calculate space for filename and reasoning
    let remaining_width = sidebar_width.saturating_sub(prefix_len);
    let filename_max = 12.min(remaining_width / 3);
    let truncated_filename = truncate_str(filename, filename_max);
    let filename_actual_len = truncated_filename.chars().count();

    // Space for reasoning: remaining - filename - ": "
    let reasoning_width = remaining_width.saturating_sub(filename_actual_len + 2);
    let truncated_reasoning = truncate_str(&reasoning, reasoning_width);

    let text_style = if divider_index.map(|d| idx >= d).unwrap_or(false) {
        Style::default().fg(Color::DarkGray)
    } else {
        Style::default()
    };

    let line = Line::from(vec![
        Span::styled(prefix, score_style),
        Span::styled(truncated_filename, Style::default().fg(Color::Blue)),
        Span::styled(": ", Style::default().fg(Color::DarkGray)),
        Span::styled(truncated_reasoning, text_style),
    ]);

    let is_selected = idx == selected_index;
    let item_style = if is_selected {
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    ListItem::new(line).style(item_style)
}

/// Render hierarchical file tree with expandable highlights (for ByFile mode)
fn render_file_tree(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    selected_file: usize,
    scroll_offset: usize,
    is_focused: bool,
    expanded_files: &HashSet<usize>,
    selected_highlight: Option<usize>,
) {
    let border_color = if is_focused {
        Color::Cyan
    } else {
        Color::DarkGray
    };

    let visible_height = area.height.saturating_sub(2) as usize;
    let sidebar_width = area.width.saturating_sub(4) as usize;

    // Build flat list of visible tree items
    let mut tree_items: Vec<TreeItem> = Vec::new();
    for (file_idx, _file) in app.diff_result.files.iter().enumerate() {
        tree_items.push(TreeItem::File { index: file_idx });

        // If expanded, add highlight children
        if expanded_files.contains(&file_idx) {
            let highlights = app.highlights_for_file(file_idx);
            for (h_idx, _) in highlights.iter().enumerate() {
                tree_items.push(TreeItem::Highlight {
                    file_index: file_idx,
                    highlight_index: h_idx,
                });
            }
        }
    }

    // Find the visual index of the current selection
    let selected_visual_index = tree_items
        .iter()
        .enumerate()
        .find(|(_, item)| match item {
            TreeItem::File { index } => *index == selected_file && selected_highlight.is_none(),
            TreeItem::Highlight { file_index, highlight_index } => {
                *file_index == selected_file && Some(*highlight_index) == selected_highlight
            }
        })
        .map(|(idx, _)| idx)
        .unwrap_or(0);

    // Render visible items
    let items: Vec<ListItem> = tree_items
        .iter()
        .enumerate()
        .skip(scroll_offset)
        .take(visible_height)
        .map(|(visual_idx, item)| {
            match item {
                TreeItem::File { index } => {
                    let file = &app.diff_result.files[*index];
                    let is_expanded = expanded_files.contains(index);
                    let highlights = app.highlights_for_file(*index);

                    let status_char = match file.status {
                        FileStatus::Added => 'A',
                        FileStatus::Deleted => 'D',
                        FileStatus::Modified => 'M',
                        FileStatus::Renamed { .. } => 'R',
                        FileStatus::Copied => 'C',
                    };

                    let status_style = match file.status {
                        FileStatus::Added => Style::default().fg(Color::Green),
                        FileStatus::Deleted => Style::default().fg(Color::Red),
                        FileStatus::Modified => Style::default().fg(Color::Yellow),
                        FileStatus::Renamed { .. } => Style::default().fg(Color::Blue),
                        FileStatus::Copied => Style::default().fg(Color::Cyan),
                    };

                    let expand_char = if highlights.is_empty() {
                        ' '
                    } else if is_expanded {
                        '▼'
                    } else {
                        '▶'
                    };

                    let filename = file
                        .path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("?");

                    let highlight_count = if highlights.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", highlights.len())
                    };

                    let max_name_chars = sidebar_width.saturating_sub(6 + highlight_count.len());
                    let truncated = truncate_str(filename, max_name_chars);

                    let line = Line::from(vec![
                        Span::raw(format!("{} ", expand_char)),
                        Span::styled(format!("{} ", status_char), status_style),
                        Span::raw(truncated),
                        Span::styled(highlight_count, Style::default().fg(Color::DarkGray)),
                    ]);

                    let is_selected = visual_idx == selected_visual_index;
                    let item_style = if is_selected {
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    ListItem::new(line).style(item_style)
                }
                TreeItem::Highlight { file_index, highlight_index } => {
                    let highlights = app.highlights_for_file(*file_index);
                    let chunk_score = highlights.get(*highlight_index);
                    let score = chunk_score
                        .and_then(|s| s.response.as_ref())
                        .map(|r| r.score)
                        .unwrap_or(0.0);

                    // Get truncated reasoning instead of classification
                    let reasoning = chunk_score
                        .and_then(|s| s.response.as_ref())
                        .map(|r| r.reasoning.replace('\n', " "))
                        .unwrap_or_default();

                    let score_style = if score >= 0.7 {
                        Style::default().fg(Color::Red)
                    } else if score >= 0.5 {
                        Style::default().fg(Color::Yellow)
                    } else {
                        Style::default().fg(Color::Green)
                    };

                    let max_reason_chars = sidebar_width.saturating_sub(12);
                    let truncated_reason = truncate_str(&reasoning, max_reason_chars);

                    let line = Line::from(vec![
                        Span::raw("    "),
                        Span::styled(format!("[{:>2.0}%] ", score * 100.0), score_style),
                        Span::raw(truncated_reason),
                    ]);

                    let is_selected = visual_idx == selected_visual_index;
                    let item_style = if is_selected {
                        Style::default()
                            .bg(Color::DarkGray)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                    };

                    ListItem::new(line).style(item_style)
                }
            }
        })
        .collect();

    let total_items = tree_items.len();
    let list = List::new(items).block(
        Block::default()
            .title(format!(" (2) Files ({}) ", app.diff_result.files.len()))
            .title_bottom(Line::from(format!(" {}/{} ", selected_visual_index + 1, total_items)).right_aligned())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color)),
    );

    let mut state = ListState::default();
    if selected_visual_index >= scroll_offset {
        state.select(Some(selected_visual_index - scroll_offset));
    }

    frame.render_stateful_widget(list, area, &mut state);
}

fn truncate_str(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else if max_chars > 3 {
        format!("{}...", s.chars().take(max_chars - 3).collect::<String>())
    } else {
        s.chars().take(max_chars).collect()
    }
}

/// Render full file tree view (original behavior)
pub fn render(frame: &mut Frame, area: Rect, app: &App, selected: usize, _scroll_offset: usize) {
    let items: Vec<ListItem> = app
        .diff_result
        .files
        .iter()
        .enumerate()
        .map(|(idx, file)| {
            let status_char = match file.status {
                FileStatus::Added => 'A',
                FileStatus::Deleted => 'D',
                FileStatus::Modified => 'M',
                FileStatus::Renamed { .. } => 'R',
                FileStatus::Copied => 'C',
            };

            let status_style = match file.status {
                FileStatus::Added => Style::default().fg(Color::Green),
                FileStatus::Deleted => Style::default().fg(Color::Red),
                FileStatus::Modified => Style::default().fg(Color::Yellow),
                FileStatus::Renamed { .. } => Style::default().fg(Color::Blue),
                FileStatus::Copied => Style::default().fg(Color::Cyan),
            };

            let chunk_count = file.chunks.len();
            let changes: usize = file.chunks.iter().map(|c| c.changes()).sum();

            // Get score info if available
            let score_info = app.scoring_result.as_ref().map(|sr| {
                let file_scores: Vec<f64> = sr
                    .scores
                    .iter()
                    .filter(|s| s.file_index == idx)
                    .filter_map(|s| s.response.as_ref().map(|r| r.score))
                    .collect();

                if file_scores.is_empty() {
                    String::new()
                } else {
                    let max = file_scores.iter().cloned().fold(0.0f64, f64::max);
                    format!(" [{:.0}%]", max * 100.0)
                }
            });

            let line = Line::from(vec![
                Span::styled(format!(" {} ", status_char), status_style),
                Span::raw(format!(
                    "{} ({} chunks, {} changes){}",
                    file.path.display(),
                    chunk_count,
                    changes,
                    score_info.unwrap_or_default(),
                )),
            ]);

            let item_style = if idx == selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            ListItem::new(line).style(item_style)
        })
        .collect();

    let title = format!(
        " Files ({}/{}) ",
        selected + 1,
        app.diff_result.files.len()
    );

    let list = List::new(items)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));

    let mut state = ListState::default();
    state.select(Some(selected));

    frame.render_stateful_widget(list, area, &mut state);
}
