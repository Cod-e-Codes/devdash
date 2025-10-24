use crate::{EventBus, Widget};
use std::collections::HashMap;
use std::time::Duration;

pub type WidgetFactory = Box<dyn Fn(&EventBus, Duration) -> Box<dyn Widget>>;

pub struct WidgetRegistry {
    factories: HashMap<String, WidgetFactory>,
    widgets: HashMap<String, Box<dyn Widget>>,
}

impl Default for WidgetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetRegistry {
    pub fn new() -> Self {
        Self {
            factories: HashMap::new(),
            widgets: HashMap::new(),
        }
    }

    pub fn register(&mut self, name: &str, factory: WidgetFactory) {
        self.factories.insert(name.to_string(), factory);
    }

    pub fn register_widget(&mut self, name: &str, widget: Box<dyn Widget>) {
        self.widgets.insert(name.to_string(), widget);
    }

    pub fn create(
        &mut self,
        name: &str,
        bus: &EventBus,
        interval: Duration,
    ) -> Option<Box<dyn Widget>> {
        // First check if it's a pre-registered widget
        if let Some(widget) = self.widgets.remove(name) {
            return Some(widget);
        }

        // Otherwise use factory
        self.factories.get(name).map(|f| f(bus, interval))
    }

    pub fn list_widgets(&self) -> Vec<&String> {
        self.factories.keys().collect()
    }

    pub fn clear_widgets(&mut self) {
        self.widgets.clear();
    }
}

#[macro_export]
macro_rules! register_widget {
    ($registry:expr, $name:expr, $widget_type:ty) => {
        $registry.register(
            $name,
            Box::new(|bus, interval| Box::new(<$widget_type>::new(bus.clone(), interval))),
        );
    };
}

#[macro_export]
macro_rules! register_widget_no_bus {
    ($registry:expr, $name:expr, $widget_type:ty) => {
        $registry.register(
            $name,
            Box::new(|_bus, interval| Box::new(<$widget_type>::new(interval))),
        );
    };
}
