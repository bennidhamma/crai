use crate::tui::app::{App, MessageLevel, View};
use crate::tui::views;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Paragraph};

pub struct LayoutManager;

impl LayoutManager {
    pub fn render(frame: &mut Frame, app: &App) {
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Header
                Constraint::Min(0),    // Main content
                Constraint::Length(1), // Status bar
            ])
            .split(frame.area());

        Self::render_header(frame, main_layout[0], app);
        Self::render_content(frame, main_layout[1], app);
        Self::render_status_bar(frame, main_layout[2], app);
    }

    fn render_header(frame: &mut Frame, area: Rect, app: &App) {
        let title = format!(
            " CRAI - Code Review AI | {} -> {} ",
            app.diff_result.base_branch, app.diff_result.compare_branch
        );

        let header = Paragraph::new(title)
            .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);

        frame.render_widget(header, area);
    }

    fn render_content(frame: &mut Frame, area: Rect, app: &App) {
        match &app.view {
            View::Summary => views::summary::render(frame, area, app),
            View::Review {
                tree_selected,
                tree_scroll_offset,
                tree_focused,
                stream_scroll_offset,
                show_analysis,
            } => {
                Self::render_review(
                    frame,
                    area,
                    app,
                    *tree_selected,
                    *tree_scroll_offset,
                    *tree_focused,
                    *stream_scroll_offset,
                    *show_analysis,
                );
            }
            View::Stats => views::stats::render(frame, area, app),
            View::Help => views::help::render(frame, area),
            View::QuitConfirm => {
                // Render summary in background
                views::summary::render(frame, area, app);
                // Render quit dialog on top
                Self::render_quit_dialog(frame, area);
            }
        }
    }

    fn render_review(
        frame: &mut Frame,
        area: Rect,
        app: &App,
        tree_selected: usize,
        tree_scroll_offset: usize,
        tree_focused: bool,
        stream_scroll_offset: usize,
        _show_analysis: bool, // Analysis is now inline in the stream
    ) {
        // Split into sidebar + main content
        let horizontal_split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(30), // Fixed-width sidebar
                Constraint::Min(0),     // Main content (highlights stream)
            ])
            .split(area);

        // Render file tree in left pane
        views::file_tree::render_sidebar(
            frame,
            horizontal_split[0],
            app,
            tree_selected,
            tree_scroll_offset,
            tree_focused,
        );

        // Render highlights stream (includes inline analysis)
        views::stream::render(frame, horizontal_split[1], app, stream_scroll_offset);
    }

    fn render_quit_dialog(frame: &mut Frame, area: Rect) {
        // Center a small dialog
        let dialog_width = 40;
        let dialog_height = 5;
        let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect::new(x, y, dialog_width, dialog_height);

        // Clear the area behind the dialog
        frame.render_widget(Clear, dialog_area);

        let text = "Quit CRAI?\n\n[q/y/Enter] Quit  [any other key] Cancel";
        let dialog = Paragraph::new(text)
            .block(
                Block::default()
                    .title(" Confirm ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow)),
            )
            .alignment(Alignment::Center)
            .style(Style::default().bg(Color::Black));

        frame.render_widget(dialog, dialog_area);
    }

    fn render_status_bar(frame: &mut Frame, area: Rect, app: &App) {
        let (text, style) = if let Some(ref progress) = app.progress {
            (
                format!(
                    " {} [{}/{}] {:.0}% ",
                    progress.operation, progress.current, progress.total, progress.percentage()
                ),
                Style::default().bg(Color::Blue).fg(Color::White),
            )
        } else if let Some(ref msg) = app.status_message {
            let bg = match msg.level {
                MessageLevel::Info => Color::DarkGray,
                MessageLevel::Warning => Color::Yellow,
                MessageLevel::Error => Color::Red,
            };
            let fg = match msg.level {
                MessageLevel::Warning => Color::Black,
                _ => Color::White,
            };
            (format!(" {} ", msg.text), Style::default().bg(bg).fg(fg))
        } else {
            let keybinds = match &app.view {
                View::Summary => "[Enter] Review [1] Summary [s] Stats [?] Help [q] Quit",
                View::Review { tree_focused, .. } => {
                    if *tree_focused {
                        "[j/k] Navigate [Enter] Jump [3/Tab] Stream [1] Summary [Esc] Back"
                    } else {
                        "[j/k] Scroll [G/g] End/Top [2/Tab] Files []/[] Prev/Next [1] Summary"
                    }
                }
                View::Stats => "[1] Summary [Esc] Back [q] Quit",
                View::Help => "[1] Summary [Esc] Back [q] Quit",
                View::QuitConfirm => "[q/y/Enter] Confirm quit [any key] Cancel",
            };

            let stats = format!(
                "Files: {} | Chunks: {}/{} | Filtered: {} lines ",
                app.diff_result.files.len(),
                app.reviewable_chunks_count(),
                app.total_chunks_count(),
                app.filtered_lines_count(),
            );

            (
                format!(" {} | {}", keybinds, stats),
                Style::default().bg(Color::DarkGray).fg(Color::White),
            )
        };

        let status = Paragraph::new(text).style(style);
        frame.render_widget(status, area);
    }
}
