use crate::diff::chunk::{FileStatus, LineKind};
use crate::tui::app::{App, StreamIndex};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

/// Render the continuous diff stream view
pub fn render(frame: &mut Frame, area: Rect, app: &App, scroll_offset: usize) {
    let visible_height = area.height.saturating_sub(2) as usize; // Account for borders

    // Generate only the visible lines (virtual scrolling)
    let lines = generate_visible_lines(
        &app.diff_result.files,
        &app.stream_index,
        scroll_offset,
        visible_height,
        app.config.tui.show_line_numbers,
    );

    let total_lines = app.stream_index.total_lines;
    let scroll_indicator = format!(
        " [{}/{} lines] ",
        (scroll_offset + 1).min(total_lines),
        total_lines
    );

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(" Diff Stream ")
            .title_bottom(Line::from(scroll_indicator).right_aligned())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );

    frame.render_widget(paragraph, area);
}

fn generate_visible_lines<'a>(
    files: &'a [crate::diff::FileDiff],
    index: &StreamIndex,
    scroll_offset: usize,
    visible_count: usize,
    show_line_numbers: bool,
) -> Vec<Line<'a>> {
    let mut lines = Vec::with_capacity(visible_count);
    let end_offset = scroll_offset + visible_count;

    // Track current position in virtual stream
    let mut current_line = 0;

    for (file_idx, file) in files.iter().enumerate() {
        // Skip files entirely before scroll offset
        let file_end = index
            .file_starts
            .get(file_idx + 1)
            .copied()
            .unwrap_or(index.total_lines);

        if file_end <= scroll_offset {
            current_line = file_end;
            continue;
        }

        // Check if we've passed visible range
        if current_line >= end_offset {
            break;
        }

        // Render file header if visible
        if current_line >= scroll_offset && current_line < end_offset {
            lines.push(render_file_header(file));
        }
        current_line += 1;

        // File separator
        if current_line >= scroll_offset && current_line < end_offset {
            lines.push(render_separator());
        }
        current_line += 1;

        // Render chunks
        for chunk in &file.chunks {
            if current_line >= end_offset {
                break;
            }

            // Chunk header
            if current_line >= scroll_offset && current_line < end_offset {
                lines.push(render_chunk_header(chunk));
            }
            current_line += 1;

            // Chunk lines
            for diff_line in &chunk.lines {
                if current_line >= end_offset {
                    break;
                }
                if current_line >= scroll_offset {
                    lines.push(render_diff_line(diff_line, show_line_numbers));
                }
                current_line += 1;
            }

            // Chunk spacing
            if current_line >= scroll_offset && current_line < end_offset {
                lines.push(Line::from(""));
            }
            current_line += 1;
        }

        // File spacing
        if current_line >= scroll_offset && current_line < end_offset {
            lines.push(Line::from(""));
        }
        current_line += 1;
    }

    lines
}

fn render_file_header(file: &crate::diff::FileDiff) -> Line<'static> {
    let (status_char, status_style) = match file.status {
        FileStatus::Added => (
            'A',
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
        FileStatus::Deleted => (
            'D',
            Style::default()
                .fg(Color::Red)
                .add_modifier(Modifier::BOLD),
        ),
        FileStatus::Modified => (
            'M',
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        FileStatus::Renamed { .. } => (
            'R',
            Style::default()
                .fg(Color::Blue)
                .add_modifier(Modifier::BOLD),
        ),
        FileStatus::Copied => (
            'C',
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
    };

    Line::from(vec![
        Span::styled(format!(" {} ", status_char), status_style),
        Span::styled(
            file.path.display().to_string(),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ])
}

fn render_separator() -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(80),
        Style::default().fg(Color::DarkGray),
    ))
}

fn render_chunk_header(chunk: &crate::diff::DiffChunk) -> Line<'static> {
    let header_text = if chunk.header.is_empty() {
        format!(
            "@@ -{},{} +{},{} @@",
            chunk.old_range.start,
            chunk.old_range.count,
            chunk.new_range.start,
            chunk.new_range.count,
        )
    } else {
        format!(
            "@@ -{},{} +{},{} @@ {}",
            chunk.old_range.start,
            chunk.old_range.count,
            chunk.new_range.start,
            chunk.new_range.count,
            chunk.header.trim()
        )
    };

    Line::from(Span::styled(header_text, Style::default().fg(Color::Cyan)))
}

fn render_diff_line(line: &crate::diff::DiffLine, show_line_numbers: bool) -> Line<'static> {
    let (style, prefix) = match line.kind {
        LineKind::Context => (Style::default().fg(Color::DarkGray), ' '),
        LineKind::Add => (Style::default().fg(Color::Green), '+'),
        LineKind::Remove => (Style::default().fg(Color::Red), '-'),
    };

    let mut spans = Vec::new();

    if show_line_numbers {
        let old_num = line
            .old_line_num
            .map(|n| format!("{:4}", n))
            .unwrap_or_else(|| "    ".to_string());
        let new_num = line
            .new_line_num
            .map(|n| format!("{:4}", n))
            .unwrap_or_else(|| "    ".to_string());

        spans.push(Span::styled(old_num, Style::default().fg(Color::DarkGray)));
        spans.push(Span::raw(" "));
        spans.push(Span::styled(new_num, Style::default().fg(Color::DarkGray)));
        spans.push(Span::raw(" "));
    }

    spans.push(Span::styled(format!("{}", prefix), style));
    spans.push(Span::styled(line.content.clone(), style));

    Line::from(spans)
}
