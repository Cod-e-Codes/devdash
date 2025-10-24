// devdash-widgets/src/disk.rs
use devdash_core::{
    EventBus, EventResult, Widget,
    event::{Event, Subscription},
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    prelude::Widget as RatatuiWidget,
    style::{Color, Style},
    widgets::{Block, Borders},
};
use std::time::Duration;
use sysinfo::{Disks, System};

use crate::common::{focus_color, format_bytes, format_rate, usage_color};

/// View mode for the DiskWidget
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Show I/O statistics with read/write rates and sparklines
    IOStats,
    /// Show disk usage per mount point with usage bars
    Usage,
}

/// Information about a disk mount point
#[derive(Debug, Clone)]
pub struct DiskInfo {
    /// Device name (e.g., "/dev/sda1")
    pub name: String,
    /// Mount point path (e.g., "/", "/mnt/data")
    pub mount_point: String,
    /// Total disk space in bytes
    pub total_space: u64,
    /// Available disk space in bytes
    pub available_space: u64,
}

impl DiskInfo {
    /// Get used space in bytes
    pub fn used_space(&self) -> u64 {
        self.total_space.saturating_sub(self.available_space)
    }

    /// Get usage percentage (0.0 - 100.0)
    pub fn usage_percent(&self) -> f64 {
        if self.total_space > 0 {
            (self.used_space() as f64 / self.total_space as f64) * 100.0
        } else {
            0.0
        }
    }
}

/// Disk I/O metrics published to the event bus
#[derive(Debug, Clone)]
pub struct DiskIOMetrics {
    /// Current read rate in bytes per second
    pub read_rate: u64,
    /// Current write rate in bytes per second
    pub write_rate: u64,
    /// Total bytes read since boot
    pub total_read: u64,
    /// Total bytes written since boot
    pub total_write: u64,
}

/// Disk usage metrics published to the event bus
#[derive(Debug, Clone)]
pub struct DiskUsageMetrics {
    /// Mount point path
    pub mount_point: String,
    /// Total space in bytes
    pub total: u64,
    /// Used space in bytes
    pub used: u64,
    /// Available space in bytes
    pub available: u64,
    /// Usage percentage (0.0 - 100.0)
    pub percentage: f64,
}

/// Disk monitoring widget with I/O statistics and usage display
///
/// Displays system disk I/O rates with sparklines and disk usage per mount point.
/// Supports interactive controls for view switching and disk navigation.
///
/// # Keyboard Shortcuts
/// - `t` - Toggle between I/O Stats and Usage views
/// - `d` - Cycle through disks in Usage view
/// - `r` - Reset I/O history
/// - `h` - Toggle history length (30 → 60 → 120)
/// - `j`/`k` or `↓`/`↑` - Navigate disk list in Usage view
///
/// # Event Publishing
/// - Publishes `system.disk.io` events on each poll with current I/O metrics
/// - Publishes `system.disk.usage` events when disk usage data updates
/// - Publishes `system.disk.full` events when any disk exceeds 90% usage
pub struct DiskWidget {
    system: System,
    disks: Disks,

    // Disk I/O state
    read_bytes: u64,
    write_bytes: u64,
    prev_read_bytes: u64,
    prev_write_bytes: u64,
    read_history: Vec<u64>,  // Last N read rates
    write_history: Vec<u64>, // Last N write rates

    // Disk usage state
    disk_info: Vec<DiskInfo>,
    selected_disk_idx: usize,

    // View mode
    view_mode: ViewMode,

    // UI state
    history_size: usize,

    // Polling
    poll_interval: Duration,
    time_since_poll: Duration,

    // Event bus
    event_bus: EventBus,
    _subscription: Option<Subscription>,
}

impl DiskWidget {
    /// Create a new DiskWidget with specified poll interval
    ///
    /// # Arguments
    /// * `event_bus` - Event bus for publishing disk metrics and alerts
    /// * `poll_interval` - How often to refresh disk statistics from the system
    ///
    /// # Example
    /// ```rust
    /// let event_bus = EventBus::new();
    /// let disk_widget = DiskWidget::new(
    ///     event_bus,
    ///     Duration::from_secs(2)
    /// );
    /// ```
    pub fn new(event_bus: EventBus, poll_interval: Duration) -> Self {
        let mut system = System::new_all();
        let mut disks = Disks::new_with_refreshed_list();

        system.refresh_all();
        disks.refresh(true);

        Self {
            system,
            disks,
            read_bytes: 0,
            write_bytes: 0,
            prev_read_bytes: 0,
            prev_write_bytes: 0,
            read_history: Vec::with_capacity(120),
            write_history: Vec::with_capacity(120),
            disk_info: Vec::new(),
            selected_disk_idx: 0,
            view_mode: ViewMode::IOStats,
            history_size: 30,
            poll_interval,
            time_since_poll: Duration::ZERO,
            event_bus,
            _subscription: None,
        }
    }

    /// Poll system for current disk I/O information
    fn poll_disk_io(&mut self) {
        self.system.refresh_all();
        self.disks.refresh(true);

        // Calculate total read/write bytes across all disks
        let mut total_read = 0u64;
        let mut total_write = 0u64;

        for disk in self.disks.iter() {
            let usage = disk.usage();
            total_read += usage.read_bytes;
            total_write += usage.written_bytes;
        }

        self.read_bytes = total_read;
        self.write_bytes = total_write;
    }

    /// Update disk usage information
    fn update_disk_info(&mut self) {
        self.disk_info.clear();

        for disk in self.disks.iter() {
            // Filter out virtual filesystems
            let mount_point = disk.mount_point().to_string_lossy().to_string();
            if !self.is_virtual_filesystem(&mount_point) {
                self.disk_info.push(DiskInfo {
                    name: disk.name().to_string_lossy().to_string(),
                    mount_point,
                    total_space: disk.total_space(),
                    available_space: disk.available_space(),
                });
            }
        }

        // Sort by mount point for consistent ordering
        self.disk_info
            .sort_by(|a, b| a.mount_point.cmp(&b.mount_point));

        // Ensure selected index is valid
        if !self.disk_info.is_empty() && self.selected_disk_idx >= self.disk_info.len() {
            self.selected_disk_idx = 0;
        }
    }

    /// Check if a mount point is a virtual filesystem
    fn is_virtual_filesystem(&self, mount_point: &str) -> bool {
        matches!(
            mount_point,
            "/proc" | "/sys" | "/dev" | "/run" | "/tmp" | "/var/run" | "/var/tmp"
        ) || mount_point.starts_with("/proc/")
            || mount_point.starts_with("/sys/")
            || mount_point.starts_with("/dev/")
            || mount_point.starts_with("/run/")
    }

    /// Calculate I/O rates and update history
    fn calculate_rates(&mut self, delta: Duration) {
        if self.prev_read_bytes > 0 || self.prev_write_bytes > 0 {
            let delta_secs = delta.as_secs_f64();
            if delta_secs > 0.0 {
                // Handle potential counter overflow or reset
                let read_rate = if self.read_bytes >= self.prev_read_bytes {
                    ((self.read_bytes - self.prev_read_bytes) as f64 / delta_secs) as u64
                } else {
                    // Counter reset or overflow, use current value as rate
                    (self.read_bytes as f64 / delta_secs) as u64
                };

                let write_rate = if self.write_bytes >= self.prev_write_bytes {
                    ((self.write_bytes - self.prev_write_bytes) as f64 / delta_secs) as u64
                } else {
                    // Counter reset or overflow, use current value as rate
                    (self.write_bytes as f64 / delta_secs) as u64
                };

                self.read_history.push(read_rate);
                self.write_history.push(write_rate);

                // Trim history to current size
                if self.read_history.len() > self.history_size {
                    self.read_history.remove(0);
                }
                if self.write_history.len() > self.history_size {
                    self.write_history.remove(0);
                }
            }
        }

        self.prev_read_bytes = self.read_bytes;
        self.prev_write_bytes = self.write_bytes;
    }

    /// Publish events to the event bus
    fn publish_events(&self) {
        // Publish I/O metrics
        let io_metrics = DiskIOMetrics {
            read_rate: self.read_history.last().copied().unwrap_or(0),
            write_rate: self.write_history.last().copied().unwrap_or(0),
            total_read: self.read_bytes,
            total_write: self.write_bytes,
        };

        self.event_bus
            .publish(Event::new("system.disk.io", io_metrics));

        // Publish usage metrics for each disk
        for disk in &self.disk_info {
            let usage_metrics = DiskUsageMetrics {
                mount_point: disk.mount_point.clone(),
                total: disk.total_space,
                used: disk.used_space(),
                available: disk.available_space,
                percentage: disk.usage_percent(),
            };

            self.event_bus
                .publish(Event::new("system.disk.usage", usage_metrics.clone()));

            // Publish full disk alert if usage > 90%
            if disk.usage_percent() > 90.0 {
                self.event_bus
                    .publish(Event::new("system.disk.full", usage_metrics));
            }
        }
    }

    /// Get current read rate in bytes per second
    fn get_read_rate(&self) -> u64 {
        self.read_history.last().copied().unwrap_or(0)
    }

    /// Get current write rate in bytes per second
    fn get_write_rate(&self) -> u64 {
        self.write_history.last().copied().unwrap_or(0)
    }
}

impl Widget for DiskWidget {
    fn on_mount(&mut self) {
        self.poll_disk_io();
        self.update_disk_info();

        // Subscribe to disk refresh events (for future use)
        let (sub, _rx) = self.event_bus.subscribe("system.disk.refresh");
        self._subscription = Some(sub);
    }

    fn on_update(&mut self, delta: Duration) {
        self.time_since_poll += delta;

        if self.time_since_poll >= self.poll_interval {
            self.poll_disk_io();
            self.update_disk_info();
            self.calculate_rates(delta);
            self.publish_events();
            self.time_since_poll = Duration::ZERO;
        }
    }

    fn on_event(&mut self, event: devdash_core::Event) -> EventResult {
        use crossterm::event::KeyCode;

        if let devdash_core::Event::Key(key) = event {
            match key.code {
                KeyCode::Char('t') => {
                    // Toggle between I/O Stats and Usage views
                    self.view_mode = match self.view_mode {
                        ViewMode::IOStats => ViewMode::Usage,
                        ViewMode::Usage => ViewMode::IOStats,
                    };
                    return EventResult::Consumed;
                }
                KeyCode::Char('d') => {
                    // Cycle through disks in Usage view
                    if !self.disk_info.is_empty() {
                        self.selected_disk_idx =
                            (self.selected_disk_idx + 1) % self.disk_info.len();
                    }
                    return EventResult::Consumed;
                }
                KeyCode::Char('r') => {
                    // Reset I/O history
                    self.read_history.clear();
                    self.write_history.clear();
                    return EventResult::Consumed;
                }
                KeyCode::Char('h') => {
                    // Toggle history length: 30, 60, 120
                    self.history_size = match self.history_size {
                        30 => 60,
                        60 => 120,
                        _ => 30,
                    };
                    // Trim history if needed
                    if self.read_history.len() > self.history_size {
                        self.read_history
                            .drain(0..self.read_history.len() - self.history_size);
                    }
                    if self.write_history.len() > self.history_size {
                        self.write_history
                            .drain(0..self.write_history.len() - self.history_size);
                    }
                    return EventResult::Consumed;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    // Navigate down in disk list
                    if !self.disk_info.is_empty() {
                        self.selected_disk_idx =
                            (self.selected_disk_idx + 1) % self.disk_info.len();
                    }
                    return EventResult::Consumed;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    // Navigate up in disk list
                    if !self.disk_info.is_empty() {
                        self.selected_disk_idx = if self.selected_disk_idx > 0 {
                            self.selected_disk_idx - 1
                        } else {
                            self.disk_info.len() - 1
                        };
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

        match self.view_mode {
            ViewMode::IOStats => self.render_io_stats_view(area, buf, border_color),
            ViewMode::Usage => self.render_usage_view(area, buf, border_color),
        }
    }

    fn needs_update(&self) -> bool {
        true // Always poll for updates
    }
}

impl DiskWidget {
    /// Render I/O statistics view
    fn render_io_stats_view(&mut self, area: Rect, buf: &mut Buffer, border_color: Color) {
        let read_rate = self.get_read_rate();
        let write_rate = self.get_write_rate();

        let title = format!(
            " Disk I/O [R: {} | W: {}] ",
            format_rate(read_rate as f64),
            format_rate(write_rate as f64)
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

        let inner_area = block.inner(area);

        if inner_area.height < 8 {
            // Not enough space, just show the block
            block.render(area, buf);
            return;
        }

        // Split area for activity indicators and stats
        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(2), // Read activity bar
                Constraint::Length(2), // Write activity bar
                Constraint::Length(1), // Spacer
                Constraint::Length(1), // Current rates
                Constraint::Length(1), // Total read
                Constraint::Length(1), // Total write
                Constraint::Min(0),    // Remaining space
            ])
            .split(inner_area);

        // Calculate activity levels (0-100%)
        let max_rate = 100 * 1024 * 1024; // 100 MB/s as max for visualization
        let read_activity = ((read_rate as f64 / max_rate as f64) * 100.0).min(100.0) as u16;
        let write_activity = ((write_rate as f64 / max_rate as f64) * 100.0).min(100.0) as u16;

        // Render activity bars
        self.render_activity_bar(chunks[0], buf, "Read", read_activity, Color::Cyan);
        self.render_activity_bar(chunks[1], buf, "Write", write_activity, Color::Magenta);

        // Render current rates
        let rates_text = format!(
            "Current: R: {} | W: {}",
            format_rate(read_rate as f64),
            format_rate(write_rate as f64)
        );

        // Render totals
        let total_read_text = format!("Total Read:  {}", format_bytes(self.read_bytes));
        let total_write_text = format!("Total Write: {}", format_bytes(self.write_bytes));

        // Write text to buffer
        use ratatui::style::Style as RatatuiStyle;

        let rates_style = RatatuiStyle::default().fg(Color::White);
        let total_read_style = RatatuiStyle::default().fg(Color::Cyan);
        let total_write_style = RatatuiStyle::default().fg(Color::Magenta);

        // Write rates line
        for (i, ch) in rates_text.chars().enumerate() {
            if let Some(pos) = chunks[3].x.checked_add(i as u16)
                && pos < chunks[3].x + chunks[3].width
            {
                buf[(pos, chunks[3].y)].set_char(ch).set_style(rates_style);
            }
        }

        // Write totals
        for (i, ch) in total_read_text.chars().enumerate() {
            if let Some(pos) = chunks[4].x.checked_add(i as u16)
                && pos < chunks[4].x + chunks[4].width
            {
                buf[(pos, chunks[4].y)]
                    .set_char(ch)
                    .set_style(total_read_style);
            }
        }

        for (i, ch) in total_write_text.chars().enumerate() {
            if let Some(pos) = chunks[5].x.checked_add(i as u16)
                && pos < chunks[5].x + chunks[5].width
            {
                buf[(pos, chunks[5].y)]
                    .set_char(ch)
                    .set_style(total_write_style);
            }
        }

        // Render the main block
        RatatuiWidget::render(block, area, buf);
    }

    /// Render an activity bar showing current I/O activity level
    fn render_activity_bar(
        &self,
        area: Rect,
        buf: &mut Buffer,
        label: &str,
        activity: u16,
        color: Color,
    ) {
        let bar_width = area.width.saturating_sub(label.len() as u16 + 3).max(1);
        let filled_width = ((activity as f64 / 100.0) * bar_width as f64) as u16;

        // Create activity bar with different characters for different activity levels
        let mut bar = String::new();
        for i in 0..bar_width {
            if i < filled_width {
                // Use different characters based on activity level
                let char_index = (i as f64 / bar_width as f64 * 4.0) as usize;
                let chars = ['░', '▒', '▓', '█'];
                bar.push(chars[char_index.min(3)]);
            } else {
                bar.push(' ');
            }
        }

        // Write label and bar to buffer
        use ratatui::style::Style as RatatuiStyle;
        let style = RatatuiStyle::default().fg(color);

        // Write label
        for (i, ch) in label.chars().enumerate() {
            if let Some(pos) = area.x.checked_add(i as u16)
                && pos < area.x + area.width
            {
                buf[(pos, area.y)].set_char(ch).set_style(style);
            }
        }

        // Write bar
        let bar_start_x = area.x + label.len() as u16 + 1;
        for (i, ch) in bar.chars().enumerate() {
            if let Some(pos) = bar_start_x.checked_add(i as u16)
                && pos < area.x + area.width
            {
                buf[(pos, area.y)].set_char(ch).set_style(style);
            }
        }
    }

    /// Render disk usage view
    fn render_usage_view(&mut self, area: Rect, buf: &mut Buffer, border_color: Color) {
        let title = format!(" Disk Usage [{} disks] ", self.disk_info.len());

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

        let inner_area = block.inner(area);

        if inner_area.height < 3 {
            // Not enough space, just show the block
            block.render(area, buf);
            return;
        }

        // Calculate how many disks we can show
        let disk_height = 3; // Each disk takes 3 lines
        let max_disks = (inner_area.height / disk_height) as usize;
        let start_idx = if self.selected_disk_idx >= max_disks {
            self.selected_disk_idx - max_disks + 1
        } else {
            0
        };
        let end_idx = (start_idx + max_disks).min(self.disk_info.len());

        // Render each visible disk
        for (i, disk_idx) in (start_idx..end_idx).enumerate() {
            if let Some(disk) = self.disk_info.get(disk_idx) {
                let y_offset = (i * disk_height as usize) as u16;
                let disk_area = Rect {
                    x: inner_area.x,
                    y: inner_area.y + y_offset,
                    width: inner_area.width,
                    height: disk_height,
                };

                let is_selected = disk_idx == self.selected_disk_idx;
                let disk_clone = disk.clone();
                self.render_single_disk_info(disk_area, buf, &disk_clone, is_selected);
            }
        }

        // Render the main block
        RatatuiWidget::render(block, area, buf);
    }

    /// Render information for a single disk
    fn render_single_disk_info(
        &mut self,
        area: Rect,
        buf: &mut Buffer,
        disk: &DiskInfo,
        selected: bool,
    ) {
        let selection_indicator = if selected { ">> " } else { "   " };
        let usage_percent = disk.usage_percent();
        let usage_color = usage_color(usage_percent);

        // Disk name and mount point
        let disk_line = format!(
            "{}{} ({})",
            selection_indicator, disk.name, disk.mount_point
        );

        // Usage info
        let usage_line = format!(
            "   Used: {} / {} ({:.1}%)",
            format_bytes(disk.used_space()),
            format_bytes(disk.total_space),
            usage_percent
        );

        // Usage bar
        let bar_width = area.width.saturating_sub(2);
        let filled_width = ((usage_percent / 100.0) * bar_width as f64) as u16;

        let mut bar = String::new();
        for i in 0..bar_width {
            if i < filled_width {
                bar.push('█');
            } else {
                bar.push('░');
            }
        }

        // Write to buffer
        use ratatui::style::Style as RatatuiStyle;

        let disk_style = if selected {
            RatatuiStyle::default()
                .fg(Color::Yellow)
                .add_modifier(ratatui::style::Modifier::BOLD)
        } else {
            RatatuiStyle::default()
        };

        let usage_style = RatatuiStyle::default().fg(usage_color);

        // Write disk line
        for (i, ch) in disk_line.chars().enumerate() {
            if let Some(pos) = area.x.checked_add(i as u16)
                && pos < area.x + area.width
            {
                buf[(pos, area.y)].set_char(ch).set_style(disk_style);
            }
        }

        // Write usage line
        for (i, ch) in usage_line.chars().enumerate() {
            if let Some(pos) = area.x.checked_add(i as u16)
                && pos < area.x + area.width
            {
                buf[(pos, area.y + 1)].set_char(ch).set_style(usage_style);
            }
        }

        // Write bar
        for (i, ch) in bar.chars().enumerate() {
            if let Some(pos) = area.x.checked_add(i as u16)
                && pos < area.x + area.width
            {
                buf[(pos, area.y + 2)].set_char(ch).set_style(usage_style);
            }
        }
    }
}
