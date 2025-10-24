use crate::Widget;
use libloading::{Library, Symbol};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Result type for plugin loading operations
pub type PluginLoadResult = Result<Vec<(String, Box<dyn Widget>)>, PluginError>;

#[derive(Debug, thiserror::Error)]
pub enum PluginError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Library loading error: {0}")]
    Loading(#[from] libloading::Error),
    #[error("UTF-8 error: {0}")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("File watcher error: {0}")]
    Watcher(#[from] notify::Error),
    #[error("Plugin API version mismatch: expected {}, got {}", expected, got)]
    VersionMismatch { expected: u32, got: u32 },
    #[error("Plugin not found: {0}")]
    PluginNotFound(String),
}

#[repr(C)]
pub struct PluginMetadata {
    pub api_version: u32,
    pub name: *const u8,
    pub name_len: usize,
}

pub struct PluginManager {
    plugins: HashMap<String, LoadedPlugin>,
    plugin_dir: PathBuf,
    watcher: RecommendedWatcher,
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
}

struct LoadedPlugin {
    _lib: Library,
    _name: String,
}

impl Default for PluginManager {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginManager {
    pub fn new() -> Self {
        let plugin_dir = dirs::home_dir()
            .map(|h| h.join(".devdash/plugins"))
            .unwrap_or_else(|| PathBuf::from("./plugins"));

        // Create file watcher
        let (tx, rx) = mpsc::channel();
        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        )
        .unwrap_or_else(|_| {
            // Fallback to a dummy watcher if notify fails
            eprintln!("Warning: Failed to create file watcher. Hot-reload disabled.");
            RecommendedWatcher::new(|_| {}, notify::Config::default()).unwrap()
        });

        Self {
            plugins: HashMap::new(),
            plugin_dir,
            watcher,
            rx,
        }
    }

    pub fn load_all(&mut self) -> PluginLoadResult {
        let mut widgets = Vec::new();

        if !self.plugin_dir.exists() {
            return Ok(widgets);
        }

        for entry in std::fs::read_dir(&self.plugin_dir)? {
            let path = entry?.path();

            if path.extension().and_then(|s| s.to_str()) == Some(dll_extension())
                && let Ok((name, widget)) = unsafe { self.load_plugin(&path) }
            {
                widgets.push((name, widget));
            }
        }

        Ok(widgets)
    }

    pub fn watch(&mut self) -> Result<(), PluginError> {
        if self.plugin_dir.exists() {
            self.watcher
                .watch(&self.plugin_dir, RecursiveMode::NonRecursive)?;
        }
        Ok(())
    }

    pub fn check_for_changes(
        &mut self,
        widgets: &mut Vec<crate::WidgetContainer>,
    ) -> Result<(), PluginError> {
        while let Ok(event) = self.rx.try_recv() {
            if let Ok(event) = event
                && (event.kind.is_modify() || event.kind.is_create())
            {
                for path in event.paths {
                    if path
                        .extension()
                        .map(|s| s == dll_extension())
                        .unwrap_or(false)
                    {
                        let plugin_name = extract_plugin_name(&path);
                        if let Err(e) = self.reload_plugin(&path, &plugin_name, widgets) {
                            eprintln!("Failed to reload plugin {}: {}", plugin_name, e);
                        }
                    }
                }
            }
        }
        Ok(())
    }

    unsafe fn load_plugin(
        &mut self,
        path: &PathBuf,
    ) -> Result<(String, Box<dyn Widget>), PluginError> {
        let lib = unsafe { Library::new(path)? };

        // Check API version
        let metadata_fn: Symbol<extern "C" fn() -> PluginMetadata> =
            unsafe { lib.get(b"devdash_plugin_metadata")? };
        let metadata = metadata_fn();

        if metadata.api_version != 1 {
            return Err(PluginError::VersionMismatch {
                expected: 1,
                got: metadata.api_version,
            });
        }

        // Load widget
        let create_fn: Symbol<extern "C" fn() -> *mut dyn Widget> =
            unsafe { lib.get(b"devdash_plugin_create")? };
        let widget_ptr = create_fn();

        let name = std::str::from_utf8(unsafe {
            std::slice::from_raw_parts(metadata.name, metadata.name_len)
        })?
        .to_string();

        self.plugins.insert(
            name.clone(),
            LoadedPlugin {
                _lib: lib,
                _name: name.clone(),
            },
        );

        Ok((name, unsafe { Box::from_raw(widget_ptr) }))
    }

    fn reload_plugin(
        &mut self,
        path: &Path,
        plugin_name: &str,
        widgets: &mut Vec<crate::WidgetContainer>,
    ) -> Result<(), PluginError> {
        // Find widget in container list by name
        let widget_idx = widgets.iter().position(|w| w.name() == plugin_name);

        // Call on_unmount to preserve state
        if let Some(idx) = widget_idx {
            widgets[idx].unmount();
        }

        // Remove old plugin from HashMap (drops Library, unloads .so)
        self.plugins.remove(plugin_name);

        // Windows-specific retry logic
        #[cfg(target_os = "windows")]
        {
            for attempt in 0..3 {
                std::thread::sleep(Duration::from_millis(100));
                match unsafe { self.load_plugin(&path.to_path_buf()) } {
                    Ok((name, widget)) => {
                        let new_container = crate::WidgetContainer::new(name.clone(), widget);

                        // Replace or add widget
                        if let Some(idx) = widget_idx {
                            widgets[idx] = new_container;
                        } else {
                            widgets.push(new_container);
                        }

                        // Mount the new widget
                        if let Some(idx) = widgets.iter().position(|w| w.name() == name) {
                            widgets[idx].mount();
                        }

                        return Ok(());
                    }
                    Err(_e) if attempt < 2 => continue,
                    Err(e) => return Err(e),
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            let (name, widget) = unsafe { self.load_plugin(&path.to_path_buf()) }?;
            let new_container = crate::WidgetContainer::new(name.clone(), widget);

            // Replace or add widget
            if let Some(idx) = widget_idx {
                widgets[idx] = new_container;
            } else {
                widgets.push(new_container);
            }

            // Mount the new widget
            if let Some(idx) = widgets.iter().position(|w| w.name() == name) {
                widgets[idx].mount();
            }
        }

        Ok(())
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        // Explicitly stop watching before dropping
        if let Err(e) = self.watcher.unwatch(&self.plugin_dir) {
            eprintln!("Warning: Failed to stop file watcher: {}", e);
        }

        // Clear plugins to ensure libraries are unloaded
        self.plugins.clear();
    }
}

fn extract_plugin_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

fn dll_extension() -> &'static str {
    #[cfg(target_os = "windows")]
    return "dll";

    #[cfg(target_os = "macos")]
    return "dylib";

    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    return "so";
}
