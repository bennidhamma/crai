use crate::ai::schema::{ChangeClassification, Severity};
use crate::ai::scoring::ChunkScore;
use crate::diff::chunk::{FileStatus, LineKind};
use crate::tui::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Render the highlights stream - reviewable chunks with inline analysis
pub fn render(frame: &mut Frame, area: Rect, app: &App, scroll_offset: usize) {
    let visible_height = area.height.saturating_sub(2) as usize;

    // Get reviewable highlights
    let highlights = get_highlights(app);

    if highlights.is_empty() {
        render_empty_state(frame, area, app);
        return;
    }

    // Generate visible lines with virtual scrolling
    let (lines, total_lines) =
        generate_highlight_lines(app, &highlights, scroll_offset, visible_height);

    let scroll_indicator = format!(
        " [{}/{} lines] {} highlights ",
        (scroll_offset + 1).min(total_lines),
        total_lines,
        highlights.len()
    );

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" Review Highlights ")
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
                .title(" Review Highlights ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .style(Style::default().fg(Color::DarkGray))
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

/// Get reviewable highlights (non-filtered chunks with AI responses)
fn get_highlights(app: &App) -> Vec<&ChunkScore> {
    app.scoring_result
        .as_ref()
        .map(|sr| {
            sr.scores
                .iter()
                .filter(|s| !s.is_filtered() && s.response.is_some())
                .collect()
        })
        .unwrap_or_default()
}

/// Generate visible lines for highlights with virtual scrolling
fn generate_highlight_lines<'a>(
    app: &'a App,
    highlights: &[&'a ChunkScore],
    scroll_offset: usize,
    visible_count: usize,
) -> (Vec<Line<'a>>, usize) {
    // First, calculate total lines needed
    let mut total_lines = 0;
    let mut highlight_starts = Vec::with_capacity(highlights.len());

    for score in highlights {
        highlight_starts.push(total_lines);
        total_lines += calculate_highlight_height(app, score);
    }

    // Now generate only visible lines
    let mut lines = Vec::with_capacity(visible_count);
    let end_offset = scroll_offset + visible_count;
    let mut current_line = 0;

    for (idx, score) in highlights.iter().enumerate() {
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
        let highlight_lines = render_highlight_block(app, score, idx + 1, highlights.len());

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

/// Calculate how many lines a highlight block needs
fn calculate_highlight_height(app: &App, score: &ChunkScore) -> usize {
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
        height += 1; // Reasoning (could wrap, but estimate 1 line)

        if !resp.concerns.is_empty() {
            height += 1; // Blank
            height += 1; // "Concerns:" header
            height += resp.concerns.len(); // Each concern
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
        "─".repeat(80),
        Style::default().fg(Color::DarkGray),
    )));

    lines.push(Line::from(""));

    // === SIDE-BY-SIDE DIFF ===
    lines.extend(render_side_by_side_diff(chunk, app.config.tui.show_line_numbers));

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

    // Reasoning - wrap if needed
    let reasoning_prefix = "  ";
    lines.push(Line::from(format!("{}{}", reasoning_prefix, resp.reasoning)));

    // Concerns
    if !resp.concerns.is_empty() {
        lines.push(Line::from(""));
        lines.push(Line::from(Span::styled(
            "  Concerns:",
            Style::default().add_modifier(Modifier::BOLD),
        )));

        for concern in &resp.concerns {
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    format!("[{}]", concern.severity),
                    severity_style(&concern.severity),
                ),
                Span::raw(format!(" {}: {}", concern.category, concern.description)),
            ]));
        }
    }

    // === SEPARATOR ===
    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "━".repeat(80),
        Style::default().fg(Color::DarkGray),
    )));

    lines
}

/// Render side-by-side diff for a chunk
fn render_side_by_side_diff<'a>(
    chunk: &'a crate::diff::DiffChunk,
    show_line_numbers: bool,
) -> Vec<Line<'a>> {
    let mut lines = Vec::new();

    // Calculate column width (half of available width minus separators)
    let col_width = 38; // Approximate half width

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

fn truncate_line(line: &str, max_len: usize) -> String {
    if line.len() <= max_len {
        line.to_string()
    } else if max_len > 3 {
        format!("{}...", &line[..max_len - 3])
    } else {
        line[..max_len].to_string()
    }
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
