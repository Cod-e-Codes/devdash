// devdash-widgets/src/memory.rs
use devdash_core::{
    EventBus, EventResult, Widget,
    event::{Event, Subscription},
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    prelude::Widget as RatatuiWidget,
    style::Style,
    widgets::{Block, Borders, Gauge},
};
use std::time::Duration;
use sysinfo::System;

use crate::common::{Unit, focus_color, format_bytes_unit, usage_color};

/// Memory usage information published to the event bus
///
/// Contains current memory and swap usage statistics that can be consumed
/// by other widgets or external components for system monitoring.
#[derive(Debug, Clone)]
pub struct MemoryMetrics {
    /// Currently used memory in bytes
    pub used: u64,
    /// Total available memory in bytes
    pub total: u64,
    /// Currently used swap in bytes
    pub swap_used: u64,
    /// Total available swap in bytes
    pub swap_total: u64,
    /// Memory usage percentage (0.0 - 100.0)
    pub usage_percent: f32,
    /// Swap usage percentage (0.0 - 100.0)
    pub swap_percent: f32,
}

/// Memory monitoring widget with visual bars and interactive controls
///
/// Displays system memory and swap usage with color-coded bars and percentage indicators.
/// Supports interactive controls for unit switching and swap visibility toggling.
///
/// # Keyboard Shortcuts
/// - `u` - Cycle through display units (Auto → Bytes → KB → MB → GB)
/// - `s` - Toggle swap visibility on/off
/// - `r` - Force immediate memory refresh
///
/// # Event Publishing
/// - Publishes `system.memory` events on each poll with current memory metrics
/// - Publishes `system.memory.pressure` events when memory usage exceeds 80%
pub struct MemoryWidget {
    system: System,

    // Memory state
    used_memory: u64,
    total_memory: u64,
    swap_used: u64,
    swap_total: u64,

    // UI state
    show_swap: bool,
    display_unit: Unit,

    // Polling
    poll_interval: Duration,
    time_since_poll: Duration,

    // Event bus
    event_bus: EventBus,
    _subscription: Option<Subscription>,
}

impl MemoryWidget {
    /// Create a new MemoryWidget with specified poll interval
    ///
    /// # Arguments
    /// * `event_bus` - Event bus for publishing memory metrics and pressure events
    /// * `poll_interval` - How often to refresh memory statistics from the system
    ///
    /// # Example
    /// ```rust
    /// let event_bus = EventBus::new();
    /// let memory_widget = MemoryWidget::new(
    ///     event_bus,
    ///     Duration::from_secs(2)
    /// );
    /// ```
    pub fn new(event_bus: EventBus, poll_interval: Duration) -> Self {
        let mut system = System::new_all();
        system.refresh_memory();

        Self {
            system,
            used_memory: 0,
            total_memory: 0,
            swap_used: 0,
            swap_total: 0,
            show_swap: true,
            display_unit: Unit::Auto,
            poll_interval,
            time_since_poll: Duration::ZERO,
            event_bus,
            _subscription: None,
        }
    }

    /// Poll system for current memory information
    fn poll_memory(&mut self) {
        self.system.refresh_memory();

        self.used_memory = self.system.used_memory();
        self.total_memory = self.system.total_memory();
        self.swap_used = self.system.used_swap();
        self.swap_total = self.system.total_swap();

        // Publish memory metrics event
        let metrics = MemoryMetrics {
            used: self.used_memory,
            total: self.total_memory,
            swap_used: self.swap_used,
            swap_total: self.swap_total,
            usage_percent: if self.total_memory > 0 {
                (self.used_memory as f32 / self.total_memory as f32) * 100.0
            } else {
                0.0
            },
            swap_percent: if self.swap_total > 0 {
                (self.swap_used as f32 / self.swap_total as f32) * 100.0
            } else {
                0.0
            },
        };

        self.event_bus
            .publish(Event::new("system.memory", metrics.clone()));

        // Publish pressure event if memory usage is high
        if metrics.usage_percent >= 80.0 {
            self.event_bus
                .publish(Event::new("system.memory.pressure", metrics));
        }
    }

    /// Get memory usage percentage
    fn get_usage_percent(&self) -> f32 {
        if self.total_memory > 0 {
            (self.used_memory as f32 / self.total_memory as f32) * 100.0
        } else {
            0.0
        }
    }

    /// Get swap usage percentage
    fn get_swap_percent(&self) -> f32 {
        if self.swap_total > 0 {
            (self.swap_used as f32 / self.swap_total as f32) * 100.0
        } else {
            0.0
        }
    }

    /// Check if swap is available
    fn has_swap(&self) -> bool {
        self.swap_total > 0
    }
}

impl Widget for MemoryWidget {
    fn on_mount(&mut self) {
        self.poll_memory(); // Initial poll

        // Subscribe to memory refresh events (for future use)
        let (sub, _rx) = self.event_bus.subscribe("system.memory.refresh");
        self._subscription = Some(sub);
    }

    fn on_update(&mut self, delta: Duration) {
        self.time_since_poll += delta;

        if self.time_since_poll >= self.poll_interval {
            self.poll_memory();
            self.time_since_poll = Duration::ZERO;
        }
    }

    fn on_event(&mut self, event: devdash_core::Event) -> EventResult {
        use crossterm::event::KeyCode;

        if let devdash_core::Event::Key(key) = event {
            match key.code {
                KeyCode::Char('u') => {
                    // Cycle through display units
                    self.display_unit = self.display_unit.next();
                    return EventResult::Consumed;
                }
                KeyCode::Char('s') => {
                    // Toggle swap visibility
                    self.show_swap = !self.show_swap;
                    return EventResult::Consumed;
                }
                KeyCode::Char('r') => {
                    // Force refresh
                    self.time_since_poll = self.poll_interval;
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

        // Calculate usage percentages
        let usage_percent = self.get_usage_percent();
        let swap_percent = self.get_swap_percent();

        // Create title with memory info
        let title = format!(
            " Memory [{:.1}% - {}/{}] ",
            usage_percent,
            format_bytes_unit(self.used_memory, self.display_unit),
            format_bytes_unit(self.total_memory, self.display_unit)
        );

        // Create main block
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

        // Calculate inner area
        let inner_area = block.inner(area);

        if inner_area.height < 3 {
            // Not enough space, just show the block
            block.render(area, buf);
            return;
        }

        // Split area for memory and swap bars
        let chunks = if self.show_swap && self.has_swap() {
            Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    Constraint::Length(2), // Memory bar
                    Constraint::Length(2), // Swap bar
                    Constraint::Min(0),    // Remaining space
                ])
                .split(inner_area)
        } else {
            Layout::default()
                .direction(ratatui::layout::Direction::Vertical)
                .constraints([
                    Constraint::Length(2), // Memory bar only
                    Constraint::Min(0),    // Remaining space
                ])
                .split(inner_area)
        };

        // Render memory gauge
        let memory_color = usage_color(usage_percent as f64);
        let memory_gauge = Gauge::default()
            .block(Block::default().title("RAM"))
            .gauge_style(Style::default().fg(memory_color))
            .ratio(usage_percent as f64 / 100.0);

        RatatuiWidget::render(memory_gauge, chunks[0], buf);

        // Render swap gauge if enabled and available
        if self.show_swap && self.has_swap() && chunks.len() > 1 {
            let swap_color = usage_color(swap_percent as f64);
            let swap_gauge = Gauge::default()
                .block(Block::default().title("SWAP"))
                .gauge_style(Style::default().fg(swap_color))
                .ratio(swap_percent as f64 / 100.0);

            RatatuiWidget::render(swap_gauge, chunks[1], buf);
        }

        // Render the main block
        RatatuiWidget::render(block, area, buf);
    }

    fn needs_update(&self) -> bool {
        true // Always poll for updates
    }
}
