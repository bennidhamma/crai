use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn render(frame: &mut Frame, area: Rect) {
    let help_text = r#"CRAI - Code Review AI Help

NAVIGATION
──────────
j/k, Up/Down     Move selection up/down
h/l, Left/Right  Navigate chunks (in diff view)
Enter, Space     Select / Open
Esc              Go back
Tab              Toggle analysis pane
[/]              Previous/Next file
PgUp/PgDn        Page up/down
Home/End         Jump to start/end

VIEWS
─────
1                Summary view
f                File tree view
s                Statistics view
?                This help screen

REVIEW ACTIONS
──────────────
a                Approve current chunk
d                Mark for discussion
r                Request changes
n                Add note
t                Toggle filtered chunks

SPECIALIZED REVIEWS
───────────────────
Shift+S          Run security review
Shift+P          Run performance review
Shift+U          Run usability review

GENERAL
───────
q                Quit / Back
Ctrl+q, Ctrl+c   Force quit

TIPS
────
- High-score chunks (>70%) are highlighted in yellow/red
- Filtered chunks are hidden by default (press 't' to show)
- Use Tab to toggle the AI analysis pane in diff view
- Statistics show how many lines were filtered and why

Press Esc or ? to close this help"#;

    let paragraph = Paragraph::new(help_text)
        .block(
            Block::default()
                .title(" Help ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Cyan)),
        )
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}
