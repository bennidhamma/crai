use crate::tui::app::App;
use ratatui::prelude::*;
use ratatui::widgets::{Bar, BarChart, BarGroup, Block, Borders, Paragraph, Wrap};

pub fn render(frame: &mut Frame, area: Rect, app: &App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(12), // Filter breakdown
            Constraint::Min(0),     // Detailed stats
        ])
        .margin(1)
        .split(area);

    render_filter_breakdown(frame, layout[0], app);
    render_detailed_stats(frame, layout[1], app);
}

fn render_filter_breakdown(frame: &mut Frame, area: Rect, app: &App) {
    let stats = match app.scoring_result.as_ref() {
        Some(sr) => &sr.stats,
        None => {
            let placeholder = Paragraph::new("Run AI analysis to see filter statistics...")
                .block(
                    Block::default()
                        .title(" Filter Breakdown ")
                        .borders(Borders::ALL),
                );
            frame.render_widget(placeholder, area);
            return;
        }
    };

    let filter_pct = stats.filter_percentage();

    let bars = vec![
        Bar::default()
            .value(stats.whitespace_lines as u64)
            .label("Whitespace".into())
            .style(Style::default().fg(Color::Gray)),
        Bar::default()
            .value(stats.import_lines as u64)
            .label("Imports".into())
            .style(Style::default().fg(Color::Blue)),
        Bar::default()
            .value(stats.rename_lines as u64)
            .label("Renames".into())
            .style(Style::default().fg(Color::Cyan)),
        Bar::default()
            .value(stats.generated_lines as u64)
            .label("Generated".into())
            .style(Style::default().fg(Color::Magenta)),
        Bar::default()
            .value(stats.below_threshold_lines as u64)
            .label("Low Score".into())
            .style(Style::default().fg(Color::Green)),
    ];

    let bar_chart = BarChart::default()
        .block(
            Block::default()
                .title(format!(
                    " Filter Breakdown ({} lines filtered, {:.1}%) ",
                    stats.filtered_lines, filter_pct
                ))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .data(BarGroup::default().bars(&bars))
        .bar_width(10)
        .bar_gap(2)
        .direction(Direction::Vertical);

    frame.render_widget(bar_chart, area);
}

fn render_detailed_stats(frame: &mut Frame, area: Rect, app: &App) {
    let text = if let Some(ref scoring) = app.scoring_result {
        let stats = &scoring.stats;

        // Score distribution
        let mut trivial = 0;
        let mut routine = 0;
        let mut notable = 0;
        let mut significant = 0;
        let mut critical = 0;

        for score in &scoring.scores {
            if let Some(ref resp) = score.response {
                match resp.classification {
                    crate::ai::schema::ChangeClassification::Trivial => trivial += 1,
                    crate::ai::schema::ChangeClassification::Routine => routine += 1,
                    crate::ai::schema::ChangeClassification::Notable => notable += 1,
                    crate::ai::schema::ChangeClassification::Significant => significant += 1,
                    crate::ai::schema::ChangeClassification::Critical => critical += 1,
                }
            }
        }

        // Concern categories
        let mut security_concerns = 0;
        let mut performance_concerns = 0;
        let mut correctness_concerns = 0;
        let mut other_concerns = 0;

        for score in &scoring.scores {
            if let Some(ref resp) = score.response {
                for concern in &resp.concerns {
                    match concern.category {
                        crate::ai::schema::ConcernCategory::Security => security_concerns += 1,
                        crate::ai::schema::ConcernCategory::Performance => performance_concerns += 1,
                        crate::ai::schema::ConcernCategory::Correctness => correctness_concerns += 1,
                        _ => other_concerns += 1,
                    }
                }
            }
        }

        format!(
            "CLASSIFICATION DISTRIBUTION\n\
             ───────────────────────────\n\
             Trivial:      {:>4}    Routine:     {:>4}\n\
             Notable:      {:>4}    Significant: {:>4}\n\
             Critical:     {:>4}\n\n\
             CONCERN CATEGORIES\n\
             ──────────────────\n\
             Security:     {:>4}    Performance: {:>4}\n\
             Correctness:  {:>4}    Other:       {:>4}\n\n\
             TOTALS\n\
             ──────\n\
             Total chunks:     {:>4}    Filtered chunks: {:>4}\n\
             Total lines:      {:>4}    Filtered lines:  {:>4}\n\
             Reviewable chunks: {:>4}   Filter rate:     {:.1}%",
            trivial, routine, notable, significant, critical,
            security_concerns, performance_concerns, correctness_concerns, other_concerns,
            stats.total_chunks, stats.filtered_chunks,
            stats.total_lines, stats.filtered_lines,
            scoring.reviewable_count(),
            stats.filter_percentage()
        )
    } else {
        format!(
            "BASIC STATISTICS\n\
             ────────────────\n\
             Files changed: {}\n\
             Total chunks:  {}\n\n\
             Run AI analysis to see detailed statistics...",
            app.diff_result.files.len(),
            app.total_chunks_count()
        )
    };

    let paragraph = Paragraph::new(text)
        .block(
            Block::default()
                .title(" Detailed Statistics ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Green)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}
