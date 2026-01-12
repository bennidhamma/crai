use crate::ai::schema::{ChangeClassification, Severity};
use crate::tui::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

pub fn render_compact(frame: &mut Frame, area: Rect, app: &App, file_index: usize, chunk_index: usize) {
    let score = app.scoring_result.as_ref().and_then(|sr| {
        sr.scores
            .iter()
            .find(|s| s.file_index == file_index && s.chunk_index == chunk_index)
    });

    let response = score.and_then(|s| s.response.as_ref());

    let content = if let Some(resp) = response {
        let classification_style = classification_color(&resp.classification);
        let score_display = format!("{:.0}%", resp.score * 100.0);

        let mut lines = vec![
            Line::from(vec![
                Span::raw("Classification: "),
                Span::styled(format!("{}", resp.classification), classification_style),
                Span::raw("  Score: "),
                Span::styled(score_display, score_style(resp.score)),
                Span::raw("  Depth: "),
                Span::raw(format!("{}", resp.review_depth)),
            ]),
            Line::from(""),
            Line::from(resp.reasoning.clone()),
        ];

        if !resp.concerns.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                "Concerns:",
                Style::default().add_modifier(Modifier::BOLD),
            )));

            for concern in &resp.concerns {
                let severity_style = severity_color(&concern.severity);
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(format!("[{}]", concern.severity), severity_style),
                    Span::raw(format!(" {}: {}", concern.category, concern.description)),
                ]));
            }
        }

        lines
    } else if let Some(s) = score {
        if let Some(ref filter) = s.filter_result {
            if filter.is_filtered {
                vec![
                    Line::from(Span::styled(
                        "Filtered",
                        Style::default().fg(Color::DarkGray),
                    )),
                    Line::from(""),
                    Line::from(format!(
                        "Reason: {}",
                        filter
                            .reason
                            .map(|r| r.description())
                            .unwrap_or("Unknown")
                    )),
                ]
            } else {
                vec![Line::from("Analysis pending...")]
            }
        } else {
            vec![Line::from("No analysis available")]
        }
    } else {
        vec![Line::from("Run AI analysis to see insights...")]
    };

    let title = " AI Analysis ";

    let paragraph = Paragraph::new(content)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Magenta)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

pub fn render_full(frame: &mut Frame, area: Rect, app: &App, file_index: usize) {
    let file = match app.diff_result.files.get(file_index) {
        Some(f) => f,
        None => return,
    };

    let items: Vec<ListItem> = file
        .chunks
        .iter()
        .enumerate()
        .filter_map(|(chunk_idx, chunk)| {
            let score = app.scoring_result.as_ref().and_then(|sr| {
                sr.scores
                    .iter()
                    .find(|s| s.file_index == file_index && s.chunk_index == chunk_idx)
            })?;

            let resp = score.response.as_ref()?;

            let style = score_style(resp.score);
            let text = format!(
                "Chunk {} (lines {}-{}): {:.0}% - {}",
                chunk_idx + 1,
                chunk.new_range.start,
                chunk.new_range.end(),
                resp.score * 100.0,
                resp.classification
            );

            Some(ListItem::new(text).style(style))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .title(format!(" Analysis: {} ", file.path.display()))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta)),
    );

    frame.render_widget(list, area);
}

fn classification_color(classification: &ChangeClassification) -> Style {
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

fn severity_color(severity: &Severity) -> Style {
    match severity {
        Severity::Low => Style::default().fg(Color::DarkGray),
        Severity::Medium => Style::default().fg(Color::Yellow),
        Severity::High => Style::default().fg(Color::LightRed),
        Severity::Critical => Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD),
    }
}

fn score_style(score: f64) -> Style {
    if score >= 0.9 {
        Style::default()
            .fg(Color::Red)
            .add_modifier(Modifier::BOLD)
    } else if score >= 0.7 {
        Style::default().fg(Color::LightRed)
    } else if score >= 0.5 {
        Style::default().fg(Color::Yellow)
    } else if score >= 0.3 {
        Style::default().fg(Color::Green)
    } else {
        Style::default().fg(Color::DarkGray)
    }
}
