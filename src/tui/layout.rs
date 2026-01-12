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
            View::FileTree { selected, scroll_offset } => {
                views::file_tree::render(frame, area, app, *selected, *scroll_offset)
            }
            View::DiffView {
                file_index,
                chunk_index,
                scroll_offset,
                show_analysis,
            } => views::diff::render(frame, area, app, *file_index, *chunk_index, *scroll_offset, *show_analysis),
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
                View::Summary => "[Enter] Files [s] Stats [?] Help [q] Quit",
                View::FileTree { .. } => "[j/k] Navigate [Enter] Open [Esc] Back [q] Quit",
                View::DiffView { .. } => "[j/k] Scroll [h/l] Chunks []/[] Files [Tab] Analysis [Esc] Back",
                View::Stats => "[Esc] Back [q] Quit",
                View::Help => "[Esc] Back [q] Quit",
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
