use devdash_plugin_sdk::*;
use ratatui::{
    layout::Rect,
    buffer::Buffer,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    prelude::Widget as RatatuiWidget,
};
use std::time::Duration;

/// Get color for focus state - matches the built-in widgets
fn focus_color(focused: bool) -> Color {
    if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    }
}

struct ExampleWidget {
    counter: u32,
    paused: bool,
    speed: u32, // How fast counter increments (1-5)
    message: String,
}

impl Default for ExampleWidget {
    fn default() -> Self {
        Self { 
            counter: 0,
            paused: false,
            speed: 1,
            message: "This is an example plugin!".to_string(),
        }
    }
}

impl Widget for ExampleWidget {
    fn on_mount(&mut self) {
        // Plugin mounted successfully
    }
    
    fn on_update(&mut self, _delta: Duration) {
        if !self.paused {
            self.counter += self.speed;
        }
    }
    
    fn on_event(&mut self, event: Event) -> EventResult { 
        use crossterm::event::KeyCode;
        
        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char(' ') => {
                    // Space: Toggle pause/resume
                    self.paused = !self.paused;
                    return EventResult::Consumed;
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    // + or =: Increase speed
                    if self.speed < 5 {
                        self.speed += 1;
                    }
                    return EventResult::Consumed;
                }
                KeyCode::Char('-') => {
                    // -: Decrease speed
                    if self.speed > 1 {
                        self.speed -= 1;
                    }
                    return EventResult::Consumed;
                }
                KeyCode::Char('r') => {
                    // R: Reset counter
                    self.counter = 0;
                    return EventResult::Consumed;
                }
                KeyCode::Char('m') => {
                    // M: Toggle message
                    if self.message == "This is an example plugin!" {
                        self.message = "Plugin is interactive! Try: space, +/-, r, m".to_string();
                    } else {
                        self.message = "This is an example plugin!".to_string();
                    }
                    return EventResult::Consumed;
                }
                _ => {}
            }
        }
        
        EventResult::Ignored 
    }
    
    fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.render_focused(area, buf, true);
    }
    
    fn render_focused(&mut self, area: Rect, buf: &mut Buffer, focused: bool) {
        let border_color = focus_color(focused);
        
        let title = if self.paused {
            "Example Plugin [PAUSED]"
        } else {
            "Example Plugin"
        };
        
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        
        let status = if self.paused { "PAUSED" } else { "RUNNING" };
        let text = format!(
            "Counter: {}\nStatus: {}\nSpeed: {}\n\n{}\n\nControls:\n[Space] Pause/Resume\n[+/-] Speed\n[R] Reset\n[M] Toggle message",
            self.counter, status, self.speed, self.message
        );
        
        let paragraph = Paragraph::new(text)
            .block(block)
            .style(Style::default().fg(Color::White));
        
        RatatuiWidget::render(paragraph, area, buf);
    }
    
    fn needs_update(&self) -> bool {
        true // Always update to increment counter
    }
    
    fn on_unmount(&mut self) {
        // Plugin unmounted successfully
    }
}

export_plugin!(ExampleWidget, "example");
