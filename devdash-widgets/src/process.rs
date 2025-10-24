// devdash-widgets/src/process.rs
use devdash_core::{
    EventBus, EventResult, Widget,
    event::{Event, Subscription},
};
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
};
use std::time::Duration;
use sysinfo::System;

use crate::common::{focus_color, format_bytes};

#[derive(Clone)]
pub struct ProcessInfo {
    pub pid: u32,
    pub name: String,
    pub cpu_percent: f32,
    pub memory_bytes: u64,
}

/// Process viewer widget with sorting and filtering
pub struct ProcessWidget {
    system: System,
    processes: Vec<ProcessInfo>,
    table_state: TableState,
    event_bus: EventBus,
    _subscription: Option<Subscription>,

    // Config
    poll_interval: Duration,
    time_since_poll: Duration,
    max_processes: usize,
    sort_by: SortBy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Cpu,
    Memory,
    Name,
}

impl ProcessWidget {
    pub fn new(event_bus: EventBus, poll_interval: Duration) -> Self {
        let mut sys = System::new_all();
        sys.refresh_all();

        Self {
            system: sys,
            processes: Vec::new(),
            table_state: TableState::default(),
            event_bus,
            _subscription: None,
            poll_interval,
            time_since_poll: Duration::ZERO,
            max_processes: 20,
            sort_by: SortBy::Cpu,
        }
    }

    fn refresh_processes(&mut self) {
        self.system
            .refresh_processes(sysinfo::ProcessesToUpdate::All, true);

        self.processes = self
            .system
            .processes()
            .iter()
            .map(|(pid, process)| ProcessInfo {
                pid: pid.as_u32(),
                name: process.name().to_string_lossy().to_string(),
                cpu_percent: process.cpu_usage(),
                memory_bytes: process.memory(),
            })
            .collect();

        // Sort
        match self.sort_by {
            SortBy::Cpu => self
                .processes
                .sort_by(|a, b| b.cpu_percent.partial_cmp(&a.cpu_percent).unwrap()),
            SortBy::Memory => self
                .processes
                .sort_by(|a, b| b.memory_bytes.cmp(&a.memory_bytes)),
            SortBy::Name => self.processes.sort_by(|a, b| a.name.cmp(&b.name)),
        }

        // Truncate to max
        self.processes.truncate(self.max_processes);

        // Publish top process update
        if let Some(top) = self.processes.first() {
            self.event_bus
                .publish(Event::new("system.process.top", top.clone()));
        }
    }
}

impl Widget for ProcessWidget {
    fn on_mount(&mut self) {
        self.refresh_processes();
        self.table_state.select(Some(0));

        // Subscribe to sort change events
        let (sub, _rx) = self.event_bus.subscribe("widget.process.sort");
        self._subscription = Some(sub);

        // Spawn task to handle events (in real impl, framework would handle this)
        // For now, just store the subscription to keep it alive
    }

    fn on_update(&mut self, delta: Duration) {
        self.time_since_poll += delta;

        if self.time_since_poll >= self.poll_interval {
            self.refresh_processes();
            self.time_since_poll = Duration::ZERO;
        }
    }

    fn on_event(&mut self, event: devdash_core::Event) -> EventResult {
        use crossterm::event::KeyCode;

        if let devdash_core::Event::Key(key) = event {
            match key.code {
                KeyCode::Down | KeyCode::Char('j') => {
                    let i = self.table_state.selected().unwrap_or(0);
                    if i < self.processes.len().saturating_sub(1) {
                        self.table_state.select(Some(i + 1));
                    }
                    return EventResult::Consumed;
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    let i = self.table_state.selected().unwrap_or(0);
                    if i > 0 {
                        self.table_state.select(Some(i - 1));
                    }
                    return EventResult::Consumed;
                }
                KeyCode::Char('c') => {
                    self.sort_by = SortBy::Cpu;
                    self.refresh_processes();
                    return EventResult::Consumed;
                }
                KeyCode::Char('m') => {
                    self.sort_by = SortBy::Memory;
                    self.refresh_processes();
                    return EventResult::Consumed;
                }
                KeyCode::Char('n') => {
                    self.sort_by = SortBy::Name;
                    self.refresh_processes();
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
        let sort_indicator = match self.sort_by {
            SortBy::Cpu => "↓CPU",
            SortBy::Memory => "↓MEM",
            SortBy::Name => "↓NAME",
        };

        let border_color = focus_color(focused);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Processes [{}] ", sort_indicator))
            .title_alignment(ratatui::layout::Alignment::Left)
            .border_style(Style::default().fg(border_color));

        let header_cells = ["PID", "Name", "CPU%", "Memory"]
            .iter()
            .map(|h| Cell::from(*h).style(Style::default().fg(Color::Yellow)));
        let header = Row::new(header_cells)
            .style(Style::default())
            .height(1)
            .bottom_margin(1);

        let rows = self.processes.iter().map(|proc| {
            let cells = vec![
                Cell::from(proc.pid.to_string()),
                Cell::from(proc.name.clone()),
                Cell::from(format!("{:.1}", proc.cpu_percent)),
                Cell::from(format_bytes(proc.memory_bytes)),
            ];
            Row::new(cells).height(1)
        });

        let widths = [
            Constraint::Length(8),
            Constraint::Min(20),
            Constraint::Length(8),
            Constraint::Length(12),
        ];

        let table = Table::new(rows, widths)
            .header(header)
            .block(block)
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> ");

        ratatui::widgets::StatefulWidget::render(table, area, buf, &mut self.table_state);
    }

    fn needs_update(&self) -> bool {
        true
    }
}
