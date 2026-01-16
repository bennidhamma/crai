use crate::error::{CraiError, CraiResult};
use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEvent, KeyModifiers};
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub enum Event {
    Key(KeyEvent),
    Tick,
    Resize(u16, u16),
}

pub struct EventHandler {
    tick_rate: Duration,
}

impl EventHandler {
    pub fn new(tick_rate_ms: u64) -> Self {
        Self {
            tick_rate: Duration::from_millis(tick_rate_ms),
        }
    }

    pub fn next(&self) -> CraiResult<Event> {
        if event::poll(self.tick_rate).map_err(|e| CraiError::Tui(e.to_string()))? {
            match event::read().map_err(|e| CraiError::Tui(e.to_string()))? {
                CrosstermEvent::Key(key) => Ok(Event::Key(key)),
                CrosstermEvent::Resize(w, h) => Ok(Event::Resize(w, h)),
                _ => Ok(Event::Tick),
            }
        } else {
            Ok(Event::Tick)
        }
    }
}

/// Key binding definitions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Quit,
    ForceQuit,
    ConfirmYes,
    Help,
    Navigate(Direction),
    Select,
    Back,
    Tab,
    Approve,
    Discuss,
    RequestChanges,
    AddNote,
    ToggleFilter,
    RunSubagent(SubagentAction),
    Stats,
    FileTree,
    FocusTree,
    FocusStream,
    Summary,
    NextHighlight,
    PrevHighlight,
    ToggleSortMode,
    None,
}

/// Sort mode for the highlights stream
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamSortMode {
    #[default]
    ByScore,
    ByFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubagentAction {
    Security,
    Performance,
    Usability,
}

impl Action {
    pub fn from_key(key: KeyEvent) -> Self {
        match key.code {
            KeyCode::Char('q') => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    Action::ForceQuit
                } else {
                    Action::Quit
                }
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => Action::ForceQuit,
            KeyCode::Char('?') => Action::Help,
            KeyCode::Char('j') | KeyCode::Down => Action::Navigate(Direction::Down),
            KeyCode::Char('k') | KeyCode::Up => Action::Navigate(Direction::Up),
            KeyCode::Char('h') | KeyCode::Left => Action::Navigate(Direction::Left),
            KeyCode::Char('l') | KeyCode::Right => Action::Navigate(Direction::Right),
            KeyCode::PageUp | KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Action::Navigate(Direction::PageUp)
            }
            KeyCode::PageUp => Action::Navigate(Direction::PageUp),
            KeyCode::PageDown | KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Action::Navigate(Direction::PageDown)
            }
            KeyCode::PageDown => Action::Navigate(Direction::PageDown),
            KeyCode::Home | KeyCode::Char('g') => Action::Navigate(Direction::Home), // g or gg for top
            KeyCode::End | KeyCode::Char('G') => Action::Navigate(Direction::End),   // G for bottom
            KeyCode::Enter | KeyCode::Char(' ') => Action::Select,
            KeyCode::Esc => Action::Back,
            KeyCode::Tab => Action::Tab,
            KeyCode::Char('a') => Action::Approve,
            KeyCode::Char('d') => Action::Discuss,
            KeyCode::Char('r') => Action::RequestChanges,
            KeyCode::Char('s') => Action::Stats,
            KeyCode::Char('S') => Action::RunSubagent(SubagentAction::Security),
            KeyCode::Char('P') => Action::RunSubagent(SubagentAction::Performance),
            KeyCode::Char('U') => Action::RunSubagent(SubagentAction::Usability),
            KeyCode::Char('n') => Action::NextHighlight,
            KeyCode::Char('N') => Action::PrevHighlight,
            KeyCode::Char('t') => Action::ToggleFilter,
            KeyCode::Char('o') => Action::ToggleSortMode,
            KeyCode::Char('y') => Action::ConfirmYes,
            KeyCode::Char('1') => Action::Summary,
            KeyCode::Char('2') => Action::FocusTree,
            KeyCode::Char('3') => Action::FocusStream,
            _ => Action::None,
        }
    }
}
