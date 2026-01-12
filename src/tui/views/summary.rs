use crate::tui::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    // Adjust layout based on whether we have AI summary
    let has_summary = app.summary.is_some();

    let layout = if has_summary {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),  // Overview
                Constraint::Length(8),  // Key changes (from AI)
                Constraint::Length(8),  // Key concerns (from scoring)
                Constraint::Min(0),     // Statistics
            ])
            .margin(1)
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(8),  // Overview
                Constraint::Length(10), // Key concerns
                Constraint::Min(0),     // Statistics
            ])
            .margin(1)
            .split(area)
    };

    render_overview(frame, layout[0], app);

    if has_summary {
        render_key_changes(frame, layout[1], app);
        render_concerns(frame, layout[2], app);
        render_statistics(frame, layout[3], app);
    } else {
        render_concerns(frame, layout[1], app);
        render_statistics(frame, layout[2], app);
    }
}

fn render_overview(frame: &mut Frame, area: Rect, app: &App) {
    let overview_text = if let Some(ref summary) = app.summary {
        // Include risk assessment in the overview
        let risk_info = format!(
            "\n\nRisk: {} | {} files | {} reviewable chunks",
            summary.risk_assessment.overall_risk,
            app.diff_result.files.len(),
            app.reviewable_chunks_count()
        );
        format!("{}{}", summary.overview, risk_info)
    } else {
        format!(
            "Comparing {} -> {}\n\n\
             {} files changed with {} reviewable chunks.\n\n\
             Press [Enter] to start reviewing or [s] for detailed statistics.",
            app.diff_result.base_branch,
            app.diff_result.compare_branch,
            app.diff_result.files.len(),
            app.reviewable_chunks_count(),
        )
    };

    let paragraph = Paragraph::new(overview_text)
        .block(
            Block::default()
                .title(" Summary ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn render_key_changes(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = if let Some(ref summary) = app.summary {
        summary
            .key_changes
            .iter()
            .take(5)
            .map(|change| {
                let impact_style = match change.impact_level {
                    crate::ai::schema::ImpactLevel::High => {
                        Style::default().fg(Color::Red)
                    }
                    crate::ai::schema::ImpactLevel::Medium => {
                        Style::default().fg(Color::Yellow)
                    }
                    crate::ai::schema::ImpactLevel::Low => {
                        Style::default().fg(Color::Green)
                    }
                };

                let files_str = if change.affected_files.len() > 2 {
                    format!(
                        "{}, {} (+{} more)",
                        truncate_path(&change.affected_files[0]),
                        truncate_path(&change.affected_files[1]),
                        change.affected_files.len() - 2
                    )
                } else {
                    change
                        .affected_files
                        .iter()
                        .map(|f| truncate_path(f))
                        .collect::<Vec<_>>()
                        .join(", ")
                };

                let text = format!(
                    "[{:?}] {} ({})",
                    change.impact_level, change.description, files_str
                );

                ListItem::new(truncate(&text, area.width.saturating_sub(4) as usize))
                    .style(impact_style)
            })
            .collect()
    } else {
        vec![ListItem::new("No AI summary available")]
    };

    let list = List::new(items).block(
        Block::default()
            .title(" Key Changes ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Magenta)),
    );

    frame.render_widget(list, area);
}

fn truncate_path(path: &str) -> String {
    // Get just the filename or last path component
    path.rsplit('/').next().unwrap_or(path).to_string()
}

fn render_concerns(frame: &mut Frame, area: Rect, app: &App) {
    let items: Vec<ListItem> = if let Some(ref scoring) = app.scoring_result {
        // Find high-scoring chunks
        let mut concerns: Vec<_> = scoring
            .scores
            .iter()
            .filter_map(|s| {
                s.response.as_ref().and_then(|r| {
                    if r.score >= 0.7 {
                        let file = &app.diff_result.files[s.file_index];
                        Some((file.path.display().to_string(), r.score, &r.classification, &r.concerns))
                    } else {
                        None
                    }
                })
            })
            .collect();

        concerns.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        concerns
            .into_iter()
            .take(5)
            .map(|(path, score, classification, concerns)| {
                let severity_style = if score >= 0.9 {
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
                } else if score >= 0.7 {
                    Style::default().fg(Color::Yellow)
                } else {
                    Style::default().fg(Color::White)
                };

                let concern_summary = concerns
                    .first()
                    .map(|c| c.description.clone())
                    .unwrap_or_else(|| "No specific concerns".to_string());

                let text = format!(
                    "[{:.0}%] {} - {} - {}",
                    score * 100.0,
                    classification,
                    path,
                    truncate(&concern_summary, 50)
                );

                ListItem::new(text).style(severity_style)
            })
            .collect()
    } else {
        vec![ListItem::new("Run AI analysis to identify key concerns...")]
    };

    let list = List::new(items).block(
        Block::default()
            .title(" Key Concerns ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow)),
    );

    frame.render_widget(list, area);
}

fn render_statistics(frame: &mut Frame, area: Rect, app: &App) {
    let stats_text = if let Some(ref scoring) = app.scoring_result {
        let stats = &scoring.stats;
        let filter_pct = stats.filter_percentage();

        format!(
            "Files changed: {:<10} Chunks to review: {}\n\
             Lines added:   {:<10} Lines removed: {}\n\
             Lines filtered: {} ({:.1}%)\n\n\
             Filter breakdown:\n\
               Whitespace only:    {:<8} Import changes: {}\n\
               Auto-generated:     {:<8} Below threshold: {}\n\n\
             Average score: {:.2}    Max score: {:.2}",
            app.diff_result.files.len(),
            scoring.reviewable_count(),
            count_additions(app),
            count_deletions(app),
            stats.filtered_lines,
            filter_pct,
            stats.whitespace_lines,
            stats.import_lines,
            stats.generated_lines,
            stats.below_threshold_lines,
            scoring.average_score().unwrap_or(0.0),
            scoring.max_score().unwrap_or(0.0),
        )
    } else {
        format!(
            "Files changed: {}\n\
             Total chunks: {}\n\n\
             Run AI analysis to see detailed statistics...",
            app.diff_result.files.len(),
            app.total_chunks_count(),
        )
    };

    let paragraph = Paragraph::new(stats_text)
        .block(
            Block::default()
                .title(" Statistics ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn count_additions(app: &App) -> usize {
    app.diff_result
        .files
        .iter()
        .flat_map(|f| &f.chunks)
        .map(|c| c.additions())
        .sum()
}

fn count_deletions(app: &App) -> usize {
    app.diff_result
        .files
        .iter()
        .flat_map(|f| &f.chunks)
        .map(|c| c.deletions())
        .sum()
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len - 3])
    }
}
