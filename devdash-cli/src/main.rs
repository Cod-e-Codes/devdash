// devdash-cli/src/main.rs
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event as CEvent, KeyCode},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};
use std::{
    io,
    time::{Duration, Instant},
};

use devdash_core::{
    ConfigFile, EventBus, PluginManager, WidgetContainer, WidgetRegistry, flatten_layout_items,
    register_widget, register_widget_no_bus, widget::CpuWidget,
};
use devdash_widgets::{DiskWidget, GitWidget, MemoryWidget, NetworkWidget, ProcessWidget};

fn reload_dashboard(
    dashboard_name: &str,
    registry: &mut WidgetRegistry,
    event_bus: &EventBus,
    _plugin_manager: &mut PluginManager,
) -> Result<(Vec<WidgetContainer>, devdash_core::Layout), Box<dyn std::error::Error>> {
    // Re-load config
    let config = ConfigFile::load()?;

    // Get specified dashboard by name
    let dashboard = config
        .get_dashboard(dashboard_name)
        .ok_or_else(|| format!("Dashboard '{}' not found", dashboard_name))?;

    // Flatten layout items to get widget list
    let layout_items = flatten_layout_items(&dashboard.layout);

    // Create new widgets from config
    let mut new_widgets = Vec::new();
    for item in layout_items {
        if let devdash_core::config::ConfigLayoutItem::Widget { name, .. } = item {
            if let Some(widget) = registry.create(name, event_bus, Duration::from_secs(1)) {
                new_widgets.push(WidgetContainer::new(name.clone(), widget));
            } else {
                eprintln!("Warning: Unknown widget '{}' in config", name);
            }
        }
    }

    // Convert config layout to runtime layout
    let new_layout = dashboard.layout.to_layout();

    Ok((new_widgets, new_layout))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load config
    let config = ConfigFile::load().unwrap_or_else(|e| {
        eprintln!("Warning: Failed to load config: {}. Using default.", e);
        ConfigFile::default()
    });

    // Parse CLI args for dashboard selection
    let dashboard_name = std::env::args()
        .nth(1)
        .filter(|arg| arg.starts_with("--dashboard="))
        .and_then(|arg| arg.strip_prefix("--dashboard=").map(String::from))
        .unwrap_or_else(|| "default".to_string());

    let dashboard = config.get_dashboard(&dashboard_name).ok_or_else(|| {
        format!(
            "Dashboard '{}' not found. Available: {}",
            dashboard_name,
            config
                .dashboard
                .iter()
                .map(|d| d.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create event bus
    let event_bus = EventBus::new();

    // Build widget registry
    let mut registry = WidgetRegistry::new();
    register_widget!(registry, "process", ProcessWidget);
    register_widget_no_bus!(registry, "cpu", CpuWidget);
    register_widget!(registry, "memory", MemoryWidget);
    register_widget!(registry, "disk", DiskWidget);
    register_widget!(registry, "network", NetworkWidget);
    register_widget!(registry, "git", GitWidget);

    // Register plugin widgets (they'll be loaded dynamically)
    // The plugin system will handle creating these widgets

    // Load plugins and register them
    let mut plugin_manager = PluginManager::new();
    let plugin_widgets = plugin_manager.load_all().unwrap_or_else(|e| {
        eprintln!(
            "Warning: Failed to load plugins: {}. Continuing without plugins.",
            e
        );
        Vec::new()
    });

    // Start watching for plugin changes
    if let Err(e) = plugin_manager.watch() {
        eprintln!(
            "Warning: Failed to start plugin watcher: {}. Hot-reload disabled.",
            e
        );
    }

    // Register plugin widgets in the registry
    for (name, widget) in plugin_widgets {
        registry.register_widget(&name, widget);
    }

    // Create widgets from config
    let mut widgets = Vec::new();

    for item in flatten_layout_items(&dashboard.layout) {
        if let devdash_core::config::ConfigLayoutItem::Widget { name, .. } = item {
            if let Some(widget) = registry.create(name, &event_bus, Duration::from_secs(1)) {
                widgets.push(WidgetContainer::new(name.clone(), widget));
            } else {
                eprintln!("Warning: Unknown widget '{}' in config", name);
            }
        }
    }

    // Convert config layout to runtime layout
    let mut layout = dashboard.layout.to_layout();

    // Focus management
    let mut focused_widget = 0;

    // Mount all widgets
    for widget in widgets.iter_mut() {
        widget.mount();
    }

    // Main loop
    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    loop {
        // Render
        terminal.draw(|f| {
            let area = f.area();
            let buf = f.buffer_mut();

            // Calculate layout areas
            let areas = layout.calculate(area);

            // Render each widget in its allocated area
            for (i, (widget, widget_area)) in widgets.iter_mut().zip(areas).enumerate() {
                let is_focused = i == focused_widget;
                widget.render_focused(widget_area, buf, is_focused);
            }
        })?;

        // Handle input with timeout
        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)?
            && let CEvent::Key(key) = event::read()?
        {
            // Only handle key press events, not key release
            if key.kind == crossterm::event::KeyEventKind::Press {
                // Quit on 'q'
                if key.code == KeyCode::Char('q') {
                    break;
                }

                // Reload config on Ctrl+r
                if key.code == KeyCode::Char('r')
                    && key
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL)
                {
                    match reload_dashboard(
                        &dashboard_name,
                        &mut registry,
                        &event_bus,
                        &mut plugin_manager,
                    ) {
                        Ok((new_widgets, new_layout)) => {
                            // Unmount old widgets
                            for w in widgets.iter_mut() {
                                w.unmount();
                            }

                            // Replace with new
                            widgets = new_widgets;
                            layout = new_layout;

                            // Mount new widgets
                            for w in widgets.iter_mut() {
                                w.mount();
                            }

                            // Reset focus
                            focused_widget = 0;
                        }
                        Err(e) => {
                            eprintln!("Config reload failed: {}. Keeping old config.", e);
                        }
                    }
                    continue;
                }

                // Handle focus management
                if key.code == KeyCode::Tab {
                    focused_widget = (focused_widget + 1) % widgets.len();
                    continue;
                }

                // Pass event only to focused widget
                let widget_event = devdash_core::Event::Key(key);
                if let Some(focused) = widgets.get_mut(focused_widget) {
                    focused.handle_event(widget_event);
                }
            }
        }

        // Check for plugin changes (hot-reload)
        if let Err(e) = plugin_manager.check_for_changes(&mut widgets) {
            eprintln!("Plugin reload error: {}", e);
        }

        // Update widgets on tick
        if last_tick.elapsed() >= tick_rate {
            for widget in widgets.iter_mut() {
                widget.update();
            }
            last_tick = Instant::now();
        }
    }

    // Cleanup
    for widget in widgets.iter_mut() {
        widget.unmount();
    }

    // Explicitly drop plugin manager to ensure proper cleanup
    drop(plugin_manager);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
