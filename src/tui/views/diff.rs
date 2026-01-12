use crate::diff::chunk::LineKind;
use crate::tui::app::App;
use crate::tui::views::analysis;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

pub fn render(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    file_index: usize,
    chunk_index: usize,
    scroll_offset: usize,
    show_analysis: bool,
) {
    let file = match app.diff_result.files.get(file_index) {
        Some(f) => f,
        None => return,
    };

    let chunk = match file.chunks.get(chunk_index) {
        Some(c) => c,
        None => return,
    };

    // Layout: either full diff or diff + analysis pane
    let layout = if show_analysis {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),    // Diff view
                Constraint::Length(12), // Analysis pane
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0)])
            .split(area)
    };

    // Render side-by-side diff
    let diff_layout = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(layout[0]);

    render_left_pane(frame, diff_layout[0], file, chunk, scroll_offset, app.config.tui.show_line_numbers);
    render_right_pane(frame, diff_layout[1], file, chunk, scroll_offset, app.config.tui.show_line_numbers);

    // Render analysis pane if visible
    if show_analysis && layout.len() > 1 {
        analysis::render_compact(frame, layout[1], app, file_index, chunk_index);
    }
}

fn render_left_pane(
    frame: &mut Frame,
    area: Rect,
    file: &crate::diff::FileDiff,
    chunk: &crate::diff::DiffChunk,
    scroll_offset: usize,
    show_line_numbers: bool,
) {
    let mut lines: Vec<Line> = Vec::new();

    for diff_line in chunk.lines.iter().skip(scroll_offset) {
        let (line_num, style, prefix) = match diff_line.kind {
            LineKind::Context => {
                let num = diff_line
                    .old_line_num
                    .map(|n| format!("{:4}", n))
                    .unwrap_or_else(|| "    ".to_string());
                (num, Style::default().fg(Color::DarkGray), ' ')
            }
            LineKind::Remove => {
                let num = diff_line
                    .old_line_num
                    .map(|n| format!("{:4}", n))
                    .unwrap_or_else(|| "    ".to_string());
                (num, Style::default().fg(Color::Red), '-')
            }
            LineKind::Add => {
                // Show placeholder on left side for additions
                ("    ".to_string(), Style::default().fg(Color::DarkGray), ' ')
            }
        };

        let content = if diff_line.kind == LineKind::Add {
            String::new()
        } else {
            diff_line.content.clone()
        };

        let spans = if show_line_numbers {
            vec![
                Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
                Span::styled(format!("{}", prefix), style),
                Span::styled(content, style),
            ]
        } else {
            vec![
                Span::styled(format!("{}", prefix), style),
                Span::styled(content, style),
            ]
        };

        lines.push(Line::from(spans));
    }

    let title = format!(
        " {} (old) @@ -{},{} ",
        file.path.display(),
        chunk.old_range.start,
        chunk.old_range.count
    );

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red)),
    );

    frame.render_widget(paragraph, area);
}

fn render_right_pane(
    frame: &mut Frame,
    area: Rect,
    file: &crate::diff::FileDiff,
    chunk: &crate::diff::DiffChunk,
    scroll_offset: usize,
    show_line_numbers: bool,
) {
    let mut lines: Vec<Line> = Vec::new();

    for diff_line in chunk.lines.iter().skip(scroll_offset) {
        let (line_num, style, prefix) = match diff_line.kind {
            LineKind::Context => {
                let num = diff_line
                    .new_line_num
                    .map(|n| format!("{:4}", n))
                    .unwrap_or_else(|| "    ".to_string());
                (num, Style::default().fg(Color::DarkGray), ' ')
            }
            LineKind::Add => {
                let num = diff_line
                    .new_line_num
                    .map(|n| format!("{:4}", n))
                    .unwrap_or_else(|| "    ".to_string());
                (num, Style::default().fg(Color::Green), '+')
            }
            LineKind::Remove => {
                // Show placeholder on right side for removals
                ("    ".to_string(), Style::default().fg(Color::DarkGray), ' ')
            }
        };

        let content = if diff_line.kind == LineKind::Remove {
            String::new()
        } else {
            diff_line.content.clone()
        };

        let spans = if show_line_numbers {
            vec![
                Span::styled(line_num, Style::default().fg(Color::DarkGray)),
                Span::raw(" "),
                Span::styled(format!("{}", prefix), style),
                Span::styled(content, style),
            ]
        } else {
            vec![
                Span::styled(format!("{}", prefix), style),
                Span::styled(content, style),
            ]
        };

        lines.push(Line::from(spans));
    }

    let title = format!(
        " {} (new) @@ +{},{} ",
        file.path.display(),
        chunk.new_range.start,
        chunk.new_range.count
    );

    let paragraph = Paragraph::new(lines).block(
        Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Green)),
    );

    frame.render_widget(paragraph, area);
}
