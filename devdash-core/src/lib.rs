pub mod config;
pub mod event;
pub mod layout;
pub mod plugin;
pub mod registry;
pub mod widget;

pub use config::{ConfigError, ConfigFile, flatten_layout_items};
pub use event::{
    Event as BusEvent, EventBus, EventPayload, GitBranchChange, ProcessUpdate, SystemMetrics,
};
pub use layout::{Constraint, Layout, LayoutItem};
pub use plugin::{PluginError, PluginManager, PluginWidget};
pub use registry::{WidgetFactory, WidgetRegistry};
pub use widget::{Event, EventResult, Size, Widget, WidgetContainer};
