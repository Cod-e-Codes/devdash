// devdash-core/src/widget.rs
use ratatui::{buffer::Buffer, layout::Rect};
use std::time::Duration;
use sysinfo::System;

/// Core widget trait with lifecycle hooks
pub trait Widget: Send + Sync {
    /// Called once when widget is added to the dashboard
    fn on_mount(&mut self) {}

    /// Called every frame with delta time since last update
    fn on_update(&mut self, _delta: Duration) {}

    /// Handle input events (keyboard, mouse, custom events)
    fn on_event(&mut self, _event: Event) -> EventResult {
        EventResult::Ignored
    }

    /// Render the widget to the buffer
    fn render(&mut self, area: Rect, buf: &mut Buffer);

    /// Render the widget with focus awareness (default implementation calls render)
    fn render_focused(&mut self, area: Rect, buf: &mut Buffer, _focused: bool) {
        self.render(area, buf);
    }

    /// Widget's preferred size (None = flexible)
    fn preferred_size(&self) -> Option<Size> {
        None
    }

    /// Whether widget needs regular updates (for animations/polling)
    fn needs_update(&self) -> bool {
        false
    }

    /// Cleanup when widget is removed
    fn on_unmount(&mut self) {}
}

#[derive(Debug, Clone)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone)]
pub enum Event {
    Key(crossterm::event::KeyEvent),
    Mouse(crossterm::event::MouseEvent),
    Resize(u16, u16),
    Custom(String, Vec<u8>), // Plugin-defined events
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    Consumed, // Stop propagation
    Ignored,  // Continue to next widget
}

/// Container for managing widget lifecycle
pub struct WidgetContainer {
    widget: Box<dyn Widget>,
    last_update: std::time::Instant,
    mounted: bool,
    name: String,
}

impl WidgetContainer {
    pub fn new(name: String, widget: Box<dyn Widget>) -> Self {
        Self {
            widget,
            last_update: std::time::Instant::now(),
            mounted: false,
            name,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn mount(&mut self) {
        if !self.mounted {
            self.widget.on_mount();
            self.mounted = true;
        }
    }

    pub fn update(&mut self) {
        let now = std::time::Instant::now();
        let delta = now.duration_since(self.last_update);

        if self.widget.needs_update() {
            self.widget.on_update(delta);
        }

        self.last_update = now;
    }

    pub fn handle_event(&mut self, event: Event) -> EventResult {
        self.widget.on_event(event)
    }

    pub fn render(&mut self, area: Rect, buf: &mut Buffer) {
        self.widget.render(area, buf);
    }

    pub fn render_focused(&mut self, area: Rect, buf: &mut Buffer, focused: bool) {
        self.widget.render_focused(area, buf, focused);
    }

    pub fn unmount(&mut self) {
        if self.mounted {
            self.widget.on_unmount();
            self.mounted = false;
        }
    }
}

// Example widget implementation
pub struct CpuWidget {
    system: System,
    usage: f32,
    history: Vec<u64>,
    poll_interval: Duration,
    time_since_poll: Duration,
    max_history: usize,
    show_percentage: bool,
}

impl CpuWidget {
    pub fn new(poll_interval: Duration) -> Self {
        let mut system = System::new_all();
        system.refresh_cpu_all();

        Self {
            system,
            usage: 0.0,
            history: Vec::with_capacity(60),
            poll_interval,
            time_since_poll: Duration::ZERO,
            max_history: 60,
            show_percentage: true,
        }
    }

    fn poll_cpu(&mut self) {
        // Refresh CPU info and get global usage
        self.system.refresh_cpu_all();
        self.usage = self.system.global_cpu_usage();

        self.history.push(self.usage as u64);
        if self.history.len() > self.max_history {
            self.history.remove(0);
        }
    }
}

impl Widget for CpuWidget {
    fn on_mount(&mut self) {
        self.poll_cpu(); // Initial poll
    }

    fn on_update(&mut self, delta: Duration) {
        self.time_since_poll += delta;

        if self.time_since_poll >= self.poll_interval {
            self.poll_cpu();
            self.time_since_poll = Duration::ZERO;
        }
    }

    fn on_event(&mut self, event: Event) -> EventResult {
        use crossterm::event::KeyCode;

        if let Event::Key(key) = event {
            match key.code {
                KeyCode::Char('h') => {
                    // Toggle history length: 30, 60, 120, 300
                    self.max_history = match self.max_history {
                        30 => 60,
                        60 => 120,
                        120 => 300,
                        _ => 30,
                    };
                    // Trim history if needed
                    if self.history.len() > self.max_history {
                        self.history.drain(0..self.history.len() - self.max_history);
                    }
                    return EventResult::Consumed;
                }
                KeyCode::Char('p') => {
                    // Toggle percentage display
                    self.show_percentage = !self.show_percentage;
                    return EventResult::Consumed;
                }
                KeyCode::Char('r') => {
                    // Reset/clear history
                    self.history.clear();
                    return EventResult::Consumed;
                }
                KeyCode::Char('+') | KeyCode::Char('=') => {
                    // Increase poll frequency (faster updates)
                    self.poll_interval = self
                        .poll_interval
                        .saturating_sub(Duration::from_millis(100));
                    return EventResult::Consumed;
                }
                KeyCode::Char('-') => {
                    // Decrease poll frequency (slower updates)
                    self.poll_interval += Duration::from_millis(100);
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
        use ratatui::style::{Color, Style};
        use ratatui::widgets::{Block, Borders, Sparkline};

        let border_color = if focused {
            Color::Yellow
        } else {
            Color::DarkGray
        };

        // Generate data points to fill the available width
        // Account for borders (2 chars) and title space
        let available_width = area.width.saturating_sub(4).max(1) as usize;
        let display_data = if self.history.is_empty() {
            vec![0; available_width]
        } else if self.history.len() >= available_width {
            // If we have more data than width, take the most recent points
            self.history
                .iter()
                .rev()
                .take(available_width)
                .cloned()
                .collect()
        } else {
            // If we have less data than width, interpolate/stretch
            let mut display_data = Vec::with_capacity(available_width);
            let scale = self.history.len() as f32 / available_width as f32;

            for i in 0..available_width {
                let source_idx = (i as f32 * scale) as usize;
                let value = if source_idx < self.history.len() {
                    self.history[source_idx]
                } else {
                    *self.history.last().unwrap_or(&0)
                };
                display_data.push(value);
            }
            display_data
        };

        let title = if self.show_percentage {
            format!(" CPU {:.1}% [H:{}] ", self.usage, self.max_history)
        } else {
            format!(" CPU [H:{}] ", self.max_history)
        };

        let sparkline = Sparkline::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(border_color)),
            )
            .data(&display_data)
            .style(Style::default().fg(Color::Cyan));

        ratatui::widgets::Widget::render(sparkline, area, buf);
    }

    fn needs_update(&self) -> bool {
        true // Always poll for updates
    }
}
