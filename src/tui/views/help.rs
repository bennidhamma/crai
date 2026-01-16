use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

pub fn render(frame: &mut Frame, area: Rect) {
    let help_text = r#"CRAI - Code Review AI Help

QUICK ACCESS
────────────
1                Summary view
2                Focus file tree
3                Focus highlights stream
s                Statistics view
?                This help screen

REVIEW MODE
───────────
j/k, Up/Down     Scroll highlights (or navigate file tree)
h/l, Left/Right  Switch focus: files <-> stream
Tab              Toggle focus between panes
Enter            From file tree: jump to file in stream
n/N              Next/Previous highlight
G                Jump to end
g                Jump to start
Ctrl+F, PgDn     Page down
Ctrl+B, PgUp     Page up
Esc              Go back to summary

REVIEW ACTIONS
──────────────
a                Approve current highlight
d                Mark for discussion
r                Request changes
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

ABOUT THE STREAM
────────────────
The highlights stream shows only reviewable chunks:
- Each highlight includes side-by-side diff
- AI analysis is shown inline below each diff
- Filtered/low-score chunks are hidden
- Use j/k to scroll through all highlights

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
