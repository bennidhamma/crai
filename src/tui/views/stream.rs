use crate::ai::schema::{ChangeClassification, Severity};
use crate::ai::scoring::ChunkScore;
use crate::diff::chunk::{FileStatus, LineKind};
use crate::tui::app::App;
use crate::tui::event::StreamSortMode;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Render the highlights stream - reviewable chunks with inline analysis
pub fn render(frame: &mut Frame, area: Rect, app: &App, scroll_offset: usize, sort_mode: StreamSortMode) {
    let visible_height = area.height.saturating_sub(2) as usize;
    let content_width = area.width.saturating_sub(2) as usize; // Account for borders

    // Get highlights based on sort mode
    let (highlights, divider_index) = get_sorted_highlights(app, sort_mode);

    if highlights.is_empty() {
        render_empty_state(frame, area, app);
        return;
    }

    // Generate visible lines with virtual scrolling
    let (lines, total_lines) =
        generate_highlight_lines(app, &highlights, scroll_offset, visible_height, content_width, divider_index);

    let mode_indicator = match sort_mode {
        StreamSortMode::ByScore => "by score",
        StreamSortMode::ByFile => "by file",
    };
    let scroll_indicator = format!(
        " [{}/{} lines] {} highlights ({}) ",
        (scroll_offset + 1).min(total_lines),
        total_lines,
        highlights.len(),
        mode_indicator
    );

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" (3) Review Highlights ")
            .title_bottom(Line::from(scroll_indicator).right_aligned())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(paragraph, area);
}

fn render_empty_state(frame: &mut Frame, area: Rect, app: &App) {
    let message = if app.scoring_result.is_none() {
        "AI analysis not yet run. Press Enter from Summary to start review."
    } else {
        "No highlights found. All chunks were filtered or scored below threshold."
    };

    let paragraph = Paragraph::new(message)
        .block(
            Block::default()
                .title(" (3) Review Highlights ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

/// Get highlights sorted according to mode, returning (highlights, divider_index)
/// divider_index is the index of the first item below threshold (only relevant for ByScore mode)
fn get_sorted_highlights(app: &App, sort_mode: StreamSortMode) -> (Vec<&ChunkScore>, Option<usize>) {
    let Some(scoring_result) = &app.scoring_result else {
        return (Vec::new(), None);
    };

    let threshold = app.config.filters.controversiality_threshold;

    // Get all chunks with AI responses (including those below threshold, but excluding heuristic-filtered)
    let mut highlights: Vec<&ChunkScore> = scoring_result
        .scores
        .iter()
        .filter(|s| {
            s.response.is_some() && !s.is_heuristic_filtered()
        })
        .collect();

    let divider_index = match sort_mode {
        StreamSortMode::ByScore => {
            // Sort by score descending
            highlights.sort_by(|a, b| {
                let score_a = a.score().unwrap_or(0.0);
                let score_b = b.score().unwrap_or(0.0);
                score_b.partial_cmp(&score_a).unwrap_or(std::cmp::Ordering::Equal)
            });

            // Find the divider position (first item below threshold)
            highlights.iter().position(|s| {
                s.score().map(|score| score < threshold).unwrap_or(true)
            })
        }
        StreamSortMode::ByFile => {
            // Sort by file index, then chunk index (original diff order)
            highlights.sort_by(|a, b| {
                a.file_index.cmp(&b.file_index)
                    .then(a.chunk_index.cmp(&b.chunk_index))
            });
            None // No divider in file order mode
        }
    };

    (highlights, divider_index)
}

/// Height of the threshold divider block
const DIVIDER_HEIGHT: usize = 5;

/// Generate visible lines for highlights with virtual scrolling
fn generate_highlight_lines<'a>(
    app: &'a App,
    highlights: &[&'a ChunkScore],
    scroll_offset: usize,
    visible_count: usize,
    content_width: usize,
    divider_index: Option<usize>,
) -> (Vec<Line<'a>>, usize) {
    let threshold = app.config.filters.controversiality_threshold;

    // First, calculate total lines needed (including divider if present)
    let mut total_lines = 0;
    let mut highlight_starts = Vec::with_capacity(highlights.len());
    let mut divider_line_start: Option<usize> = None;

    for (idx, score) in highlights.iter().enumerate() {
        // Insert divider before the first below-threshold item
        if divider_index == Some(idx) && divider_line_start.is_none() {
            divider_line_start = Some(total_lines);
            total_lines += DIVIDER_HEIGHT;
        }
        highlight_starts.push(total_lines);
        total_lines += calculate_highlight_height(app, score, content_width);
    }

    // Now generate only visible lines
    let mut lines = Vec::with_capacity(visible_count);
    let end_offset = scroll_offset + visible_count;
    let mut current_line = 0;

    // Count items below threshold for divider message
    let below_threshold_count = divider_index
        .map(|idx| highlights.len().saturating_sub(idx))
        .unwrap_or(0);

    for (idx, score) in highlights.iter().enumerate() {
        // Render divider before the first below-threshold item
        if divider_index == Some(idx) && divider_line_start.is_some() {
            let divider_start = divider_line_start.unwrap();

            // Only render if divider is visible
            if current_line < end_offset && divider_start + DIVIDER_HEIGHT > scroll_offset {
                let divider_lines = render_threshold_divider(content_width, threshold, below_threshold_count);
                for line in divider_lines {
                    if current_line >= scroll_offset && current_line < end_offset {
                        lines.push(line);
                    }
                    current_line += 1;
                    if current_line >= end_offset {
                        break;
                    }
                }
            } else {
                current_line = divider_start + DIVIDER_HEIGHT;
            }
        }

        let highlight_start = highlight_starts[idx];
        let highlight_height = if idx + 1 < highlights.len() {
            highlight_starts[idx + 1] - highlight_start
        } else {
            total_lines - highlight_start
        };

        // Skip highlights entirely before scroll offset
        if highlight_start + highlight_height <= scroll_offset {
            current_line = highlight_start + highlight_height;
            continue;
        }

        // Stop if we've passed visible range
        if current_line >= end_offset {
            break;
        }

        // Render this highlight
        let highlight_lines = render_highlight_block(app, score, idx + 1, highlights.len(), content_width);

        for line in highlight_lines {
            if current_line >= scroll_offset && current_line < end_offset {
                lines.push(line);
            }
            current_line += 1;
            if current_line >= end_offset {
                break;
            }
        }
    }

    (lines, total_lines)
}

/// Render the threshold divider
fn render_threshold_divider<'a>(content_width: usize, threshold: f64, remaining_count: usize) -> Vec<Line<'a>> {
    let threshold_pct = (threshold * 100.0) as u32;
    let message = format!(
        "Below review threshold (score < {}%) - {} remaining items",
        threshold_pct,
        remaining_count
    );

    vec![
        Line::from(""),
        Line::from(Span::styled(
            "━".repeat(content_width),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(Span::styled(
            format!("  ▼ {} ▼ ", message),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "━".repeat(content_width),
            Style::default().fg(Color::Yellow),
        )),
        Line::from(""),
    ]
}

/// Calculate how many lines a highlight block needs
fn calculate_highlight_height(app: &App, score: &ChunkScore, content_width: usize) -> usize {
    let file = match app.diff_result.files.get(score.file_index) {
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

        // Reasoning - calculate wrapped lines
        let reasoning_width = content_width.saturating_sub(2);
        height += wrap_text(&resp.reasoning, reasoning_width).len();

        if !resp.concerns.is_empty() {
            height += 1; // Blank
            height += 1; // "Concerns:" header

            // Each concern may wrap
            for concern in &resp.concerns {
                let prefix_len = format!("[{}] {}: ", concern.severity, concern.category).chars().count();
                let first_line_width = content_width.saturating_sub(4 + prefix_len);
                let continuation_width = content_width.saturating_sub(6);

                let wrapped = wrap_text(&concern.description, first_line_width);
                height += 1; // First line
                for cont_line in wrapped.iter().skip(1) {
                    height += wrap_text(cont_line, continuation_width).len();
                }
            }
        }
    }

    // Separator: 2 lines
    height += 2;

    height
}

/// Render a complete highlight block
fn render_highlight_block<'a>(
    app: &'a App,
    score: &'a ChunkScore,
    highlight_num: usize,
    total_highlights: usize,
    content_width: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    let file = match app.diff_result.files.get(score.file_index) {
        Some(f) => f,
        None => return lines,
    };
    let chunk = match file.chunks.get(score.chunk_index) {
        Some(c) => c,
        None => return lines,
    };

    let resp = match &score.response {
        Some(r) => r,
        None => return lines,
    };

    // === HEADER ===
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

    let score_style = if resp.score >= 0.7 {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else if resp.score >= 0.5 {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    };

    lines.push(Line::from(vec![
        Span::styled(
            format!("━━━ Highlight {}/{} ", highlight_num, total_highlights),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("{} ", status_char), status_style),
        Span::styled(
            file.path.display().to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            " @@ {}-{} ",
            chunk.new_range.start,
            chunk.new_range.start + chunk.new_range.count
        )),
        Span::styled(format!("[{:.0}%]", resp.score * 100.0), score_style),
        Span::raw(" "),
        Span::styled(
            format!("{}", resp.classification),
            classification_style(&resp.classification),
        ),
    ]));

    lines.push(Line::from(Span::styled(
        "─".repeat(content_width),
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(""));

    // === SIDE-BY-SIDE DIFF ===
    lines.extend(render_side_by_side_diff(chunk, app.config.tui.show_line_numbers, content_width));

    lines.push(Line::from(""));

    // === ANALYSIS ===
    lines.push(Line::from(Span::styled(
        "Analysis:",
        Style::default()
            .fg(Color::Magenta)
            .add_modifier(Modifier::BOLD),
    )));

    lines.push(Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("{}", resp.classification),
            classification_style(&resp.classification),
        ),
        Span::raw(format!(" • Score: {:.0}%", resp.score * 100.0)),
        Span::raw(format!(" • Depth: {}", resp.review_depth)),
    ]));

    lines.push(Line::from(""));

    // Reasoning - wrap to fit width
    let reasoning_indent = 2;
    let reasoning_width = content_width.saturating_sub(reasoning_indent);
    for wrapped_line in wrap_text(&resp.reasoning, reasoning_width) {
        lines.push(Line::from(format!("{}{}", " ".repeat(reasoning_indent), wrapped_line)));
    }

    // Concerns
    if !resp.concerns.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Concerns:",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        for concern in &resp.concerns {
            let prefix = format!("[{}] {}: ", concern.severity, concern.category);
            let concern_indent = 4;
            let first_line_width = content_width.saturating_sub(concern_indent + prefix.chars().count());
            let continuation_width = content_width.saturating_sub(concern_indent + 2);

            let wrapped = wrap_text(&concern.description, first_line_width);
            if let Some((first, rest)) = wrapped.split_first() {
                // First line with severity badge
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::styled(
                        format!("[{}]", concern.severity),
                        severity_style(&concern.severity),
                    ),
                    Span::raw(format!(" {}: {}", concern.category, first)),
                ]));
                // Continuation lines
                for cont_line in rest {
                    // Re-wrap continuation if needed
                    for rewrapped in wrap_text(cont_line, continuation_width) {
                        lines.push(Line::from(format!("      {}", rewrapped)));
                    }
                }
            }
        }
    }

    // === SEPARATOR ===
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "━".repeat(content_width),
        Style::default().fg(Color::DarkGray),
    )));

    lines
}

/// Render side-by-side diff for a chunk
fn render_side_by_side_diff<'a>(
    chunk: &'a crate::diff::DiffChunk,
    show_line_numbers: bool,
    content_width: usize,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    // Calculate column width dynamically
    // Layout: [line_num (5)] [space] [left_col] [ │ ] [line_num (5)] [space] [right_col]
    // Separator " │ " = 3 chars, line numbers = 5 + 1 space each side = 12 total
    let overhead = if show_line_numbers { 3 + 12 } else { 3 };
    let col_width = content_width.saturating_sub(overhead) / 2;

    for diff_line in &chunk.lines {
        let (left_content, right_content, left_style, right_style) = match diff_line.kind {
            LineKind::Context => {
                let content = truncate_line(&diff_line.content, col_width);
                (
                    content.clone(),
                    content,
                    Style::default().fg(Color::DarkGray),
                    Style::default().fg(Color::DarkGray),
                )
            }
            LineKind::Remove => {
                let content = truncate_line(&diff_line.content, col_width);
                (
                    format!("-{}", content),
                    String::new(),
                    Style::default().fg(Color::Red),
                    Style::default(),
                )
            }
            LineKind::Add => {
                let content = truncate_line(&diff_line.content, col_width);
                (
                    String::new(),
                    format!("+{}", content),
                    Style::default(),
                    Style::default().fg(Color::Green),
                )
            }
        };

        let mut spans = Vec::new();

        if show_line_numbers {
            let old_num = diff_line
                .old_line_num
                .map(|n| format!("{:4}", n))
                .unwrap_or_else(|| "    ".to_string());

            spans.push(Span::styled(old_num, Style::default().fg(Color::DarkGray)));
            spans.push(Span::raw(" "));
        }

        // Left side (old)
        spans.push(Span::styled(format!("{:<width$}", left_content, width = col_width), left_style));
        spans.push(Span::styled(" │ ", Style::default().fg(Color::DarkGray)));

        if show_line_numbers {
            let new_num = diff_line
                .new_line_num
                .map(|n| format!("{:4}", n))
                .unwrap_or_else(|| "    ".to_string());
            spans.push(Span::styled(new_num, Style::default().fg(Color::DarkGray)));
            spans.push(Span::raw(" "));
        }

        // Right side (new)
        spans.push(Span::styled(right_content, right_style));

        lines.push(Line::from(spans));
    }

    lines
}

fn truncate_line(line: &str, max_chars: usize) -> String {
    let char_count = line.chars().count();
    if char_count <= max_chars {
        line.to_string()
    } else if max_chars > 3 {
        format!("{}...", line.chars().take(max_chars - 3).collect::<String>())
    } else {
        line.chars().take(max_chars).collect()
    }
}

/// Wrap text to fit within a given width, breaking at word boundaries
fn wrap_text(text: &str, max_width: usize) -> Vec<String> {
    if max_width == 0 {
        return vec![text.to_string()];
    }

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0;

    for word in text.split_whitespace() {
        let word_width = word.chars().count();

        if current_width == 0 {
            // First word on line
            if word_width <= max_width {
                current_line = word.to_string();
                current_width = word_width;
            } else {
                // Word is too long, force break it
                let mut remaining = word;
                while !remaining.is_empty() {
                    let take: String = remaining.chars().take(max_width).collect();
                    let taken_len = take.chars().count();
                    lines.push(take);
                    remaining = &remaining[remaining.char_indices().nth(taken_len).map(|(i, _)| i).unwrap_or(remaining.len())..];
                }
            }
        } else if current_width + 1 + word_width <= max_width {
            // Word fits on current line
            current_line.push(' ');
            current_line.push_str(word);
            current_width += 1 + word_width;
        } else {
            // Word doesn't fit, start new line
            lines.push(current_line);
            if word_width <= max_width {
                current_line = word.to_string();
                current_width = word_width;
            } else {
                // Word is too long, force break it
                current_line = String::new();
                current_width = 0;
                let mut remaining = word;
                while !remaining.is_empty() {
                    let take: String = remaining.chars().take(max_width).collect();
                    let taken_len = take.chars().count();
                    if taken_len < remaining.chars().count() {
                        lines.push(take);
                        remaining = &remaining[remaining.char_indices().nth(taken_len).map(|(i, _)| i).unwrap_or(remaining.len())..];
                    } else {
                        current_line = take;
                        current_width = taken_len;
                        break;
                    }
                }
            }
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

fn classification_style(classification: &ChangeClassification) -> Style {
    match classification {
        ChangeClassification::Trivial => Style::default().fg(Color::DarkGray),
        ChangeClassification::Routine => Style::default().fg(Color::Green),
        ChangeClassification::Notable => Style::default().fg(Color::Yellow),
        ChangeClassification::Significant => Style::default().fg(Color::LightRed),
        ChangeClassification::Critical => Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD),
    }
}

fn severity_style(severity: &Severity) -> Style {
    match severity {
        Severity::Low => Style::default().fg(Color::DarkGray),
        Severity::Medium => Style::default().fg(Color::Yellow),
        Severity::High => Style::default().fg(Color::LightRed),
        Severity::Critical => Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD),
    }
}

/// Calculate total lines for the highlights stream (for scroll bounds)
/// Uses an estimated content_width for text wrapping calculations
pub fn calculate_stream_total_lines(app: &App, sort_mode: StreamSortMode) -> usize {
    // Use a reasonable default width for calculating wrapped lines
    // This doesn't need to be exact - it's used for scroll bounds
    let estimated_width = 100;

    let (highlights, divider_index) = get_sorted_highlights(app, sort_mode);

    if highlights.is_empty() {
        return 0;
    }

    let mut total_lines = 0;

    for (idx, score) in highlights.iter().enumerate() {
        // Insert divider before the first below-threshold item
        if divider_index == Some(idx) {
            total_lines += DIVIDER_HEIGHT;
        }
        total_lines += calculate_highlight_height(app, score, estimated_width);
    }

    total_lines
}
