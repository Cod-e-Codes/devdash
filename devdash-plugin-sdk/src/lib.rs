pub use devdash_core::{Event, EventBus, EventResult, Size, Widget};

pub const PLUGIN_API_VERSION: u32 = 1;

#[repr(C)]
pub struct PluginMetadata {
    pub api_version: u32,
    pub name: *const u8,
    pub name_len: usize,
}

/// FFI-safe representation of a fat pointer (trait object)
#[repr(C)]
#[derive(Copy, Clone)]
pub struct FatPointer {
    pub data: *mut std::ffi::c_void,
    pub vtable: *mut std::ffi::c_void,
}

/// FFI-safe plugin API with proper memory management
#[repr(C)]
pub struct PluginApi {
    /// Create a new widget instance (returns fat pointer components)
    pub create: extern "C" fn() -> FatPointer,
    /// Destroy a widget instance (called by plugin, not host)
    pub destroy: extern "C" fn(FatPointer),
    /// Metadata about the plugin
    pub metadata: PluginMetadata,
}

#[macro_export]
macro_rules! export_plugin {
    ($widget_type:ty, $name:expr) => {
        // Metadata function
        #[unsafe(no_mangle)]
        pub extern "C" fn devdash_plugin_metadata() -> $crate::PluginMetadata {
            $crate::PluginMetadata {
                api_version: $crate::PLUGIN_API_VERSION,
                name: $name.as_ptr(),
                name_len: $name.len(),
            }
        }

        // Create function - allocates with plugin's allocator
        #[unsafe(no_mangle)]
        pub extern "C" fn devdash_plugin_create() -> $crate::FatPointer {
            let widget: Box<dyn $crate::Widget> = Box::new(<$widget_type>::default());
            let ptr = Box::into_raw(widget);
            // Split fat pointer into data and vtable components
            unsafe {
                let parts: [*mut std::ffi::c_void; 2] = std::mem::transmute(ptr);
                $crate::FatPointer {
                    data: parts[0],
                    vtable: parts[1],
                }
            }
        }

        // Destroy function - deallocates with plugin's allocator
        #[unsafe(no_mangle)]
        pub extern "C" fn devdash_plugin_destroy(ptr: $crate::FatPointer) {
            if !ptr.data.is_null() {
                unsafe {
                    // Reconstruct fat pointer from components
                    let fat_ptr: *mut dyn $crate::Widget =
                        std::mem::transmute([ptr.data, ptr.vtable]);
                    let _ = Box::from_raw(fat_ptr);
                }
            }
        }
    };
}
