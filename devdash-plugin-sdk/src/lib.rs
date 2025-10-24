pub use devdash_core::{Event, EventBus, EventResult, Size, Widget};

pub const PLUGIN_API_VERSION: u32 = 1;

#[repr(C)]
pub struct PluginMetadata {
    pub api_version: u32,
    pub name: *const u8,
    pub name_len: usize,
}

// Use a trait object pointer as the FFI type
pub type WidgetPtr = *mut dyn Widget;

#[macro_export]
macro_rules! export_plugin {
    ($widget_type:ty, $name:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn devdash_plugin_metadata() -> $crate::PluginMetadata {
            $crate::PluginMetadata {
                api_version: $crate::PLUGIN_API_VERSION,
                name: $name.as_ptr(),
                name_len: $name.len(),
            }
        }

        #[unsafe(no_mangle)]
        pub extern "C" fn devdash_plugin_create() -> $crate::WidgetPtr {
            Box::into_raw(Box::new(<$widget_type>::default()))
        }
    };
}
