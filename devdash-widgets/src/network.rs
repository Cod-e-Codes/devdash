// devdash-widgets/src/network.rs
use devdash_core::{EventBus, EventResult, Widget, event::Subscription};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    prelude::Widget as RatatuiWidget,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Sparkline},
};
use std::time::Duration;
use sysinfo::Networks;

use crate::common::{focus_color, format_bytes, format_rate};

/// View mode for NetworkWidget
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    IOStats,
    InterfaceUsage,
}

/// Interface information with session totals
#[derive(Debug, Clone)]
pub struct InterfaceInfo {
    pub name: String,
    pub total_rx: u64,
    pub total_tx: u64,
    pub max_speed: Option<u64>, // Mbps, if known
}

pub struct NetworkWidget {
    networks: Networks,

    // Interface management
    interfaces: Vec<String>,
    current_idx: usize,

    // I/O state
    rx_history: Vec<u64>,
    tx_history: Vec<u64>,
    last_rx: u64,
    last_tx: u64,

    // Interface usage state
    interface_info: Vec<InterfaceInfo>,
    selected_interface_idx: usize,

    // View mode
    view_mode: ViewMode,

    // Configuration
    max_history: usize,
    poll_interval: Duration,
    time_since_poll: Duration,

    // Event bus
    event_bus: EventBus,
    _subscription: Option<Subscription>,
}

impl NetworkWidget {
    pub fn new(event_bus: EventBus, poll_interval: Duration) -> Self {
        let networks = Networks::new_with_refreshed_list();
        let interfaces: Vec<String> = networks.keys().map(|s| s.to_string()).collect();

        Self {
            networks,
            interfaces: interfaces.clone(),
            current_idx: 0,
            rx_history: Vec::with_capacity(300),
            tx_history: Vec::with_capacity(300),
            last_rx: 0,
            last_tx: 0,
            interface_info: Vec::new(),
            selected_interface_idx: 0,
            view_mode: ViewMode::IOStats,
            max_history: 60,
            poll_interval,
            time_since_poll: Duration::ZERO,
            event_bus,
            _subscription: None,
        }
    }

    fn poll_network(&mut self) {
        self.networks.refresh(true);

        if self.interfaces.is_empty() {
            return;
        }

        // FIXED: Collapsed if with && let
        if let Some(name) = self.interfaces.get(self.current_idx)
            && let Some(data) = self.networks.get(name)
        {
            let current_rx = data.total_received();
            let current_tx = data.total_transmitted();

            if self.last_rx > 0 || self.last_tx > 0 {
                let delta_rx = current_rx.saturating_sub(self.last_rx);
                let delta_tx = current_tx.saturating_sub(self.last_tx);

                self.rx_history.push(delta_rx);
                self.tx_history.push(delta_tx);

                if self.rx_history.len() > self.max_history {
                    self.rx_history.remove(0);
                }
                if self.tx_history.len() > self.max_history {
                    self.tx_history.remove(0);
                }

                if let Some(info) = self.interface_info.iter_mut().find(|i| i.name == *name) {
                    info.total_rx += delta_rx;
                    info.total_tx += delta_tx;
                }
            }

            self.last_rx = current_rx;
            self.last_tx = current_tx;
        }

        self.update_interface_info();
    }

    fn update_interface_info(&mut self) {
        let mut new_info = Vec::new();

        for name in &self.interfaces {
            if let Some(_data) = self.networks.get(name) {
                let current_delta_rx = if name
                    == self
                        .interfaces
                        .get(self.current_idx)
                        .unwrap_or(&"".to_string())
                {
                    self.rx_history.last().copied().unwrap_or(0)
                } else {
                    0
                };
                let current_delta_tx = if name
                    == self
                        .interfaces
                        .get(self.current_idx)
                        .unwrap_or(&"".to_string())
                {
                    self.tx_history.last().copied().unwrap_or(0)
                } else {
                    0
                };

                let existing = self.interface_info.iter().find(|i| i.name == *name);
                let total_rx = existing.map(|i| i.total_rx).unwrap_or(0) + current_delta_rx;
                let total_tx = existing.map(|i| i.total_tx).unwrap_or(0) + current_delta_tx;

                new_info.push(InterfaceInfo {
                    name: name.clone(),
                    total_rx,
                    total_tx,
                    max_speed: None,
                });
            }
        }

        self.interface_info = new_info;
        if self.selected_interface_idx >= self.interface_info.len() {
            self.selected_interface_idx = 0;
        }
    }

    fn get_current_rx_rate(&self) -> u64 {
        self.rx_history.last().copied().unwrap_or(0)
    }

    fn get_current_tx_rate(&self) -> u64 {
        self.tx_history.last().copied().unwrap_or(0)
    }

    fn get_current_interface(&self) -> &str {
        self.interfaces
            .get(self.current_idx)
            .map(|s| s.as_str())
            .unwrap_or("Unknown")
    }

    fn next_interface(&mut self) {
        if !self.interfaces.is_empty() {
            self.current_idx = (self.current_idx + 1) % self.interfaces.len();
            self.reset_current_totals();
        }
    }

    fn prev_interface(&mut self) {
        if !self.interfaces.is_empty() {
            self.current_idx = if self.current_idx == 0 {
                self.interfaces.len() - 1
            } else {
                self.current_idx - 1
            };
            self.reset_current_totals();
        }
    }

    fn reset_current_totals(&mut self) {
        if let Some(info) = self.interface_info.get_mut(self.current_idx) {
            info.total_rx = 0;
            info.total_tx = 0;
        }
        self.rx_history.clear();
        self.tx_history.clear();
        self.last_rx = 0;
        self.last_tx = 0;
    }

    fn toggle_view(&mut self) {
        self.view_mode = match self.view_mode {
            ViewMode::IOStats => ViewMode::InterfaceUsage,
            ViewMode::InterfaceUsage => ViewMode::IOStats,
        };
    }
}

impl Widget for NetworkWidget {
    fn on_mount(&mut self) {
        self.poll_network();
        let (sub, _rx) = self.event_bus.subscribe("system.network.refresh");
        self._subscription = Some(sub);
    }

    fn on_update(&mut self, delta: Duration) {
        self.time_since_poll += delta;
        if self.time_since_poll >= self.poll_interval {
            self.poll_network();
            self.time_since_poll = Duration::ZERO;
        }
    }

    fn on_event(&mut self, event: devdash_core::Event) -> EventResult {
        use crossterm::event::KeyCode;

        if let devdash_core::Event::Key(key) = event {
            match key.code {
                KeyCode::Char('t') => {
                    self.toggle_view();
                    return EventResult::Consumed;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.view_mode == ViewMode::InterfaceUsage {
                        self.selected_interface_idx = if self.selected_interface_idx == 0 {
                            self.interface_info.len().saturating_sub(1)
                        } else {
                            self.selected_interface_idx - 1
                        };
                    } else {
                        self.prev_interface();
                    }
                    return EventResult::Consumed;
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if self.view_mode == ViewMode::InterfaceUsage {
                        self.selected_interface_idx =
                            (self.selected_interface_idx + 1) % self.interface_info.len();
                    } else {
                        self.next_interface();
                    }
                    return EventResult::Consumed;
                }
                KeyCode::Char('r') => {
                    self.reset_current_totals();
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
            ViewMode::IOStats => self.render_io_stats(area, buf, border_color),
            ViewMode::InterfaceUsage => self.render_usage_view(area, buf, border_color),
        }
    }

    fn needs_update(&self) -> bool {
        true
    }
}

impl NetworkWidget {
    fn render_io_stats(&mut self, area: Rect, buf: &mut Buffer, border_color: Color) {
        let rx_rate = self.get_current_rx_rate();
        let tx_rate = self.get_current_tx_rate();
        let interface = self.get_current_interface();

        let title = format!(
            " Network [{}] Down {} Up {} ",
            interface,
            format_rate(rx_rate as f64),
            format_rate(tx_rate as f64)
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        if inner.height < 4 {
            block.render(area, buf);
            return;
        }

        let chunks = Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(2),
                Constraint::Min(0),
            ])
            .split(inner);

        let available = chunks[0].width.saturating_sub(2).max(1) as usize;
        let prepare = |hist: &[u64]| -> Vec<u64> {
            if hist.is_empty() {
                vec![0; available]
            } else if hist.len() >= available {
                hist.iter().rev().take(available).cloned().collect()
            } else {
                let mut v = Vec::with_capacity(available);
                let scale = hist.len() as f32 / available as f32;
                for i in 0..available {
                    let idx = (i as f32 * scale) as usize;
                    v.push(if idx < hist.len() {
                        hist[idx]
                    } else {
                        *hist.last().unwrap_or(&0)
                    });
                }
                v
            }
        };

        let rx_data = prepare(&self.rx_history);
        let tx_data = prepare(&self.tx_history);

        Sparkline::default()
            .block(Block::default().title("Down Download"))
            .data(&rx_data)
            .style(Style::default().fg(Color::Green))
            .render(chunks[0], buf);

        Sparkline::default()
            .block(Block::default().title("Up Upload"))
            .data(&tx_data)
            .style(Style::default().fg(Color::Blue))
            .render(chunks[1], buf);

        block.render(area, buf);
    }

    fn render_usage_view(&mut self, area: Rect, buf: &mut Buffer, border_color: Color) {
        let title = format!(" Network [{} interfaces] ", self.interface_info.len());
        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(border_color));

        let inner = block.inner(area);
        if inner.height < 3 {
            block.render(area, buf);
            return;
        }

        let line_height = 1;
        let max_lines = inner.height as usize / line_height;
        let start = self
            .selected_interface_idx
            .saturating_sub(max_lines.saturating_sub(1));
        let end = (start + max_lines).min(self.interface_info.len());

        for (i, idx) in (start..end).enumerate() {
            if let Some(info) = self.interface_info.get(idx) {
                let y = inner.y + (i as u16);
                let selected = idx == self.selected_interface_idx;
                let prefix = if selected { ">> " } else { "   " };
                let line = format!(
                    "{}{}  RX: {}  TX: {}",
                    prefix,
                    info.name,
                    format_bytes(info.total_rx),
                    format_bytes(info.total_tx)
                );

                let style = if selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                for (x, ch) in line.chars().enumerate() {
                    if let Some(pos_x) = inner.x.checked_add(x as u16) {
                        if pos_x < inner.x + inner.width {
                            buf[(pos_x, y)].set_char(ch).set_style(style);
                        }
                    }
                }
            }
        }

        block.render(area, buf);
    }
}
