use crate::diff::chunk::FileStatus;
use crate::tui::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

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
