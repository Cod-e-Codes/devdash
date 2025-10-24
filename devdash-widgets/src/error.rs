use devdash_core::{Event, EventResult, Widget};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    prelude::Widget as RatatuiWidget,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};
use std::time::Duration;

/// Widget that displays error messages in the TUI
#[derive(Debug)]
pub struct ErrorWidget {
    message: String,
    title: String,
    border_color: Color,
}

impl ErrorWidget {
    pub fn new(message: String) -> Self {
        Self {
            title: "Error".to_string(),
            border_color: Color::Red,
            message,
        }
    }

    pub fn plugin_error(plugin_name: &str) -> Self {
        Self {
            title: format!("Plugin Error: {}", plugin_name),
            border_color: Color::Red,
            message: format!("Plugin '{}' failed to load or is missing.", plugin_name),
        }
    }

    pub fn config_error(message: String) -> Self {
        Self {
            title: "Configuration Error".to_string(),
            border_color: Color::Yellow,
            message,
        }
    }
}

impl Default for ErrorWidget {
    fn default() -> Self {
        Self {
            title: "Error".to_string(),
            border_color: Color::Red,
            message: "An unknown error occurred.".to_string(),
        }
    }
}

impl Widget for ErrorWidget {
    fn on_mount(&mut self) {}

    fn on_update(&mut self, _delta: Duration) {}

    fn on_event(&mut self, _event: Event) -> EventResult {
        EventResult::Ignored
    }

    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.render_focused(area, buf, false);
    }

    fn render_focused(&mut self, area: Rect, buf: &mut Buffer, _focused: bool) {
        let block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.border_color));

        let paragraph = Paragraph::new(self.message.as_str())
            .block(block)
            .style(Style::default().fg(Color::White));

        RatatuiWidget::render(paragraph, area, buf);
    }

    fn needs_update(&self) -> bool {
        false
    }

    fn on_unmount(&mut self) {}
}
