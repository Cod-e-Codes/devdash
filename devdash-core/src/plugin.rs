use crate::Widget;
use libloading::{Library, Symbol};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

/// Result type for plugin loading operations
pub type PluginLoadResult = Result<Vec<(String, PluginWidget)>, PluginError>;

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

/// FFI-safe representation of a fat pointer (trait object)
#[repr(C)]
#[derive(Copy, Clone)]
struct FatPointer {
    data: *mut std::ffi::c_void,
    vtable: *mut std::ffi::c_void,
}

/// Wrapper around plugin widget that handles proper cleanup
pub struct PluginWidget {
    // Store as fat pointer to preserve vtable
    ptr: *mut dyn Widget,
    // Store components for destroy call
    fat_ptr: FatPointer,
    destroy: extern "C" fn(FatPointer),
    // Keep library alive for as long as the widget exists
    _lib: Library,
}

// Safety: The plugin system ensures that the pointer is valid and the
// Widget trait object is properly constructed. The Library keeps the
// code alive.
unsafe impl Send for PluginWidget {}
unsafe impl Sync for PluginWidget {}

impl PluginWidget {
    unsafe fn new(fat_ptr: FatPointer, destroy: extern "C" fn(FatPointer), lib: Library) -> Self {
        // Reconstruct the fat pointer from components
        let ptr: *mut dyn Widget = unsafe { std::mem::transmute([fat_ptr.data, fat_ptr.vtable]) };

        Self {
            ptr,
            fat_ptr,
            destroy,
            _lib: lib,
        }
    }

    fn as_widget(&mut self) -> &mut dyn Widget {
        // Safety: The pointer is valid and the library keeps the code alive
        unsafe { &mut *self.ptr }
    }

    fn as_widget_const(&self) -> &dyn Widget {
        // Safety: Same as above, but for const access
        unsafe { &*self.ptr }
    }
}

impl Drop for PluginWidget {
    fn drop(&mut self) {
        // Call plugin's destroy function to deallocate with correct allocator
        (self.destroy)(FatPointer {
            data: self.fat_ptr.data,
            vtable: self.fat_ptr.vtable,
        });
    }
}

// Forward Widget trait to the inner widget
impl Widget for PluginWidget {
    fn on_mount(&mut self) {
        self.as_widget().on_mount()
    }

    fn on_update(&mut self, delta: Duration) {
        self.as_widget().on_update(delta)
    }

    fn on_event(&mut self, event: crate::Event) -> crate::EventResult {
        self.as_widget().on_event(event)
    }

    fn render(&mut self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        self.as_widget().render(area, buf)
    }

    fn render_focused(
        &mut self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        focused: bool,
    ) {
        self.as_widget().render_focused(area, buf, focused)
    }

    fn preferred_size(&self) -> Option<crate::Size> {
        None
    }

    fn needs_update(&self) -> bool {
        self.as_widget_const().needs_update()
    }

    fn on_unmount(&mut self) {
        self.as_widget().on_unmount()
    }
}

pub struct PluginManager {
    plugins: HashMap<String, LoadedPlugin>,
    plugin_dir: PathBuf,
    temp_dir: PathBuf,
    watcher: RecommendedWatcher,
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
}

struct LoadedPlugin {
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

        // Create temp directory for plugin copies (Windows file locking workaround)
        let temp_dir = std::env::temp_dir().join("devdash_plugins");
        let _ = std::fs::create_dir_all(&temp_dir);

        // Create file watcher
        let (tx, rx) = mpsc::channel();
        let watcher = RecommendedWatcher::new(
            move |res| {
                let _ = tx.send(res);
            },
            notify::Config::default(),
        )
        .unwrap_or_else(|_| {
            eprintln!("Warning: Failed to create file watcher. Hot-reload disabled.");
            RecommendedWatcher::new(|_| {}, notify::Config::default()).unwrap()
        });

        Self {
            plugins: HashMap::new(),
            plugin_dir,
            temp_dir,
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

            if path.extension().and_then(|s| s.to_str()) == Some(dll_extension()) {
                match unsafe { self.load_plugin(&path) } {
                    Ok((name, widget)) => widgets.push((name, widget)),
                    Err(e) => eprintln!("Warning: Failed to load plugin {:?}: {}", path, e),
                }
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

    unsafe fn load_plugin(&mut self, path: &Path) -> Result<(String, PluginWidget), PluginError> {
        // FIX: Use temp copy to avoid Windows file locking
        let temp_path = self.copy_to_temp(path)?;
        let lib = unsafe { Library::new(&temp_path)? };

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

        // Load create and destroy functions
        let create_fn: Symbol<extern "C" fn() -> FatPointer> =
            unsafe { lib.get(b"devdash_plugin_create")? };
        let destroy_fn: Symbol<extern "C" fn(FatPointer)> =
            unsafe { lib.get(b"devdash_plugin_destroy")? };

        let fat_ptr = create_fn();
        let destroy = *destroy_fn;

        let name = std::str::from_utf8(unsafe {
            std::slice::from_raw_parts(metadata.name, metadata.name_len)
        })?
        .to_string();

        self.plugins.insert(
            name.clone(),
            LoadedPlugin {
                _name: name.clone(),
            },
        );

        let plugin_widget = unsafe { PluginWidget::new(fat_ptr, destroy, lib) };

        Ok((name, plugin_widget))
    }

    fn copy_to_temp(&self, path: &Path) -> Result<PathBuf, PluginError> {
        let file_name = path.file_name().ok_or_else(|| {
            PluginError::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Invalid plugin path",
            ))
        })?;

        // Create unique temp file name to avoid collisions during hot-reload
        let temp_name = format!(
            "{}_{}.{}",
            file_name
                .to_string_lossy()
                .trim_end_matches(&format!(".{}", dll_extension())),
            rand::random::<u32>(),
            dll_extension()
        );

        let temp_path = self.temp_dir.join(temp_name);
        std::fs::copy(path, &temp_path)?;

        Ok(temp_path)
    }

    fn reload_plugin(
        &mut self,
        path: &Path,
        plugin_name: &str,
        widgets: &mut Vec<crate::WidgetContainer>,
    ) -> Result<(), PluginError> {
        // Find widget in container list by name
        let widget_idx = widgets.iter().position(|w| w.name() == plugin_name);

        // FIX: Unmount and drop old widget BEFORE unloading library
        if let Some(idx) = widget_idx {
            let mut old_widget = std::mem::replace(
                &mut widgets[idx],
                crate::WidgetContainer::new(
                    "placeholder".to_string(),
                    Box::new(crate::widget::CpuWidget::new(Duration::from_secs(1))),
                ),
            );
            old_widget.unmount();
            // old_widget is dropped here, which calls PluginWidget::drop
        }

        // NOW remove plugin from HashMap (this drops Library)
        self.plugins.remove(plugin_name);

        // Small delay to ensure library is fully unloaded (especially on Windows)
        std::thread::sleep(Duration::from_millis(100));

        // Load new plugin
        let (name, widget) = unsafe { self.load_plugin(path) }?;

        let new_container = crate::WidgetContainer::new(name.clone(), Box::new(widget));

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

        Ok(())
    }
}

impl Drop for PluginManager {
    fn drop(&mut self) {
        // Explicitly stop watching before dropping
        let _ = self.watcher.unwatch(&self.plugin_dir);

        // Clear plugins to ensure libraries are unloaded
        self.plugins.clear();

        // Clean up temp directory
        let _ = std::fs::remove_dir_all(&self.temp_dir);
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
