use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn render(frame: &mut Frame, area: Rect) {
    let help_text = r#"CRAI - Code Review AI Help

REVIEW MODE
───────────
j/k, Up/Down     Scroll diff stream (or navigate file tree)
h/l, Left/Right  Switch focus: files <-> stream
Enter, Space     From summary: enter review mode
                 From file tree: jump to file in stream
Tab              Toggle focus between file tree and stream
[/]              Previous/Next file (jumps in stream)
G                Jump to end of stream
g                Jump to start of stream
PgUp/PgDn        Page up/down
Esc              Go back to summary

VIEWS
─────
1                Summary view
f                Review mode (file tree + diff stream)
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
- The analysis pane shows AI insights for the current chunk
- Use Tab to switch between file tree and diff stream
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
