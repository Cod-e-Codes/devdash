use crate::{Constraint, Layout, LayoutItem};
use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("Config directory not found")]
    NoConfigDir,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub dashboard: Vec<Dashboard>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Dashboard {
    pub name: String,
    pub layout: ConfigLayout,
    #[serde(default)]
    pub widgets: Vec<WidgetConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ConfigLayout {
    Layout {
        direction: Direction,
        items: Vec<ConfigLayoutItem>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ConfigLayoutItem {
    Widget {
        name: String,
        #[serde(flatten)]
        constraint: ConfigConstraint,
    },
    Layout {
        direction: Direction,
        items: Vec<ConfigLayoutItem>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ConfigConstraint {
    #[serde(default)]
    pub flex: Option<u16>,
    pub fixed: Option<u16>,
    pub percentage: Option<u16>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct WidgetConfig {
    pub name: String,
    #[serde(flatten)]
    pub settings: toml::Value,
}

impl Default for ConfigFile {
    fn default() -> Self {
        Self {
            dashboard: vec![Dashboard {
                name: "default".to_string(),
                layout: ConfigLayout::Layout {
                    direction: Direction::Horizontal,
                    items: vec![
                        ConfigLayoutItem::Widget {
                            name: "process".to_string(),
                            constraint: ConfigConstraint {
                                flex: Some(1),
                                fixed: None,
                                percentage: None,
                            },
                        },
                        ConfigLayoutItem::Layout {
                            direction: Direction::Vertical,
                            items: vec![
                                ConfigLayoutItem::Widget {
                                    name: "cpu".to_string(),
                                    constraint: ConfigConstraint {
                                        flex: Some(1),
                                        fixed: None,
                                        percentage: None,
                                    },
                                },
                                ConfigLayoutItem::Widget {
                                    name: "memory".to_string(),
                                    constraint: ConfigConstraint {
                                        flex: Some(1),
                                        fixed: None,
                                        percentage: None,
                                    },
                                },
                                ConfigLayoutItem::Widget {
                                    name: "disk".to_string(),
                                    constraint: ConfigConstraint {
                                        flex: Some(1),
                                        fixed: None,
                                        percentage: None,
                                    },
                                },
                            ],
                        },
                    ],
                },
                widgets: vec![],
            }],
        }
    }
}

impl ConfigFile {
    pub fn load() -> Result<Self, ConfigError> {
        // Priority: ./devdash.toml -> ~/.config/devdash/devdash.toml -> default
        let paths = [
            std::env::current_dir()?.join("devdash.toml"),
            dirs::config_dir()
                .ok_or(ConfigError::NoConfigDir)?
                .join("devdash/devdash.toml"),
        ];

        for path in paths {
            if path.exists() {
                let content = std::fs::read_to_string(path)?;
                return toml::from_str(&content).map_err(ConfigError::Parse);
            }
        }

        Ok(Self::default())
    }

    pub fn get_dashboard(&self, name: &str) -> Option<&Dashboard> {
        self.dashboard.iter().find(|d| d.name == name)
    }
}

impl ConfigLayout {
    pub fn to_layout(&self) -> Layout {
        match self {
            ConfigLayout::Layout { direction, items } => {
                let layout_items: Vec<_> = items.iter().map(|item| item.to_layout_item()).collect();

                match direction {
                    Direction::Horizontal => Layout::horizontal(layout_items),
                    Direction::Vertical => Layout::vertical(layout_items),
                }
            }
        }
    }
}

impl ConfigLayoutItem {
    pub fn to_layout_item(&self) -> LayoutItem {
        match self {
            ConfigLayoutItem::Widget { constraint, .. } => {
                LayoutItem::Constraint(constraint.to_constraint())
            }
            ConfigLayoutItem::Layout { direction, items } => {
                let layout_items: Vec<_> = items.iter().map(|item| item.to_layout_item()).collect();

                let layout = match direction {
                    Direction::Horizontal => Layout::horizontal(layout_items),
                    Direction::Vertical => Layout::vertical(layout_items),
                };

                LayoutItem::Nested(layout)
            }
        }
    }
}

impl ConfigConstraint {
    pub fn to_constraint(&self) -> Constraint {
        if let Some(flex) = self.flex {
            Constraint::Flex(flex)
        } else if let Some(fixed) = self.fixed {
            Constraint::Fixed(fixed)
        } else if let Some(pct) = self.percentage {
            Constraint::Percentage(pct)
        } else {
            Constraint::Flex(1) // default
        }
    }
}

/// Flatten a config layout to extract all widget names in order
pub fn flatten_layout_items(layout: &ConfigLayout) -> Vec<&ConfigLayoutItem> {
    let mut result = Vec::new();
    match layout {
        ConfigLayout::Layout { items, .. } => {
            flatten_items_recursive(items, &mut result);
        }
    }
    result
}

fn flatten_items_recursive<'a>(
    items: &'a [ConfigLayoutItem],
    result: &mut Vec<&'a ConfigLayoutItem>,
) {
    for item in items {
        match item {
            ConfigLayoutItem::Widget { .. } => {
                result.push(item);
            }
            ConfigLayoutItem::Layout {
                items: nested_items,
                ..
            } => {
                flatten_items_recursive(nested_items, result);
            }
        }
    }
}
