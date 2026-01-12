pub mod app;
pub mod event;
pub mod layout;
pub mod views;

pub use app::App;
pub use event::{Event, EventHandler};

use crate::error::{CraiError, CraiResult};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io::{self, Stdout};

pub type Tui = Terminal<CrosstermBackend<Stdout>>;

pub fn init_terminal() -> CraiResult<Tui> {
    enable_raw_mode().map_err(|e| CraiError::Terminal(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen).map_err(|e| CraiError::Terminal(e.to_string()))?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend).map_err(|e| CraiError::Terminal(e.to_string()))?;
    Ok(terminal)
}

pub fn restore_terminal() -> CraiResult<()> {
    disable_raw_mode().map_err(|e| CraiError::Terminal(e.to_string()))?;
    execute!(io::stdout(), LeaveAlternateScreen).map_err(|e| CraiError::Terminal(e.to_string()))?;
    Ok(())
}
