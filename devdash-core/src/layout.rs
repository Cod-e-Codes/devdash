// devdash-core/src/layout.rs
use ratatui::layout::Rect;

/// Layout constraints for flexible widget sizing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Constraint {
    /// Fixed size in characters
    Fixed(u16),
    /// Proportional flex (weight)
    Flex(u16),
    /// Percentage of available space (0-100)
    Percentage(u16),
    /// Minimum size
    Min(u16),
    /// Maximum size
    Max(u16),
}

/// Layout item that can be either a widget constraint or a nested layout
#[derive(Debug, Clone)]
pub enum LayoutItem {
    /// Leaf: actual widget area with constraint
    Constraint(Constraint),
    /// Branch: nested layout for recursive arrangements
    Nested(Layout),
}

/// Layout configuration for widget arrangement
#[derive(Debug, Clone)]
pub enum Layout {
    /// Horizontal split with given layout items
    Horizontal(Vec<LayoutItem>),
    /// Vertical split with given layout items
    Vertical(Vec<LayoutItem>),
}

impl LayoutItem {
    /// Create a layout item for a widget with the given constraint
    pub fn widget(constraint: Constraint) -> Self {
        LayoutItem::Constraint(constraint)
    }

    /// Create a layout item for a nested layout
    pub fn nested(layout: Layout) -> Self {
        LayoutItem::Nested(layout)
    }
}

impl Layout {
    /// Create a horizontal layout with the given items
    pub fn horizontal(items: Vec<LayoutItem>) -> Self {
        Layout::Horizontal(items)
    }

    /// Create a vertical layout with the given items
    pub fn vertical(items: Vec<LayoutItem>) -> Self {
        Layout::Vertical(items)
    }

    /// Calculate the areas for each widget based on constraints
    pub fn calculate(&self, area: Rect) -> Vec<Rect> {
        let mut result = Vec::new();
        self.calculate_recursive(area, &mut result);
        result
    }

    /// Recursively calculate layout areas with depth-first traversal
    fn calculate_recursive(&self, area: Rect, output: &mut Vec<Rect>) {
        match self {
            Layout::Horizontal(items) => {
                let rects = Self::split_horizontal(area, items);
                for (rect, item) in rects.iter().zip(items) {
                    match item {
                        LayoutItem::Constraint(_) => output.push(*rect),
                        LayoutItem::Nested(nested) => nested.calculate_recursive(*rect, output),
                    }
                }
            }
            Layout::Vertical(items) => {
                let rects = Self::split_vertical(area, items);
                for (rect, item) in rects.iter().zip(items) {
                    match item {
                        LayoutItem::Constraint(_) => output.push(*rect),
                        LayoutItem::Nested(nested) => nested.calculate_recursive(*rect, output),
                    }
                }
            }
        }
    }

    fn split_horizontal(area: Rect, items: &[LayoutItem]) -> Vec<Rect> {
        if items.is_empty() {
            return vec![];
        }

        let total_width = area.width;
        let mut areas = Vec::with_capacity(items.len());
        let mut remaining_width = total_width;
        let mut flex_total = 0u16;

        // First pass: allocate fixed and percentage constraints
        for item in items {
            match item {
                LayoutItem::Constraint(constraint) => {
                    match constraint {
                        Constraint::Fixed(size) => {
                            let allocated = (*size).min(remaining_width);
                            areas.push(Rect {
                                x: area.x + (total_width - remaining_width),
                                y: area.y,
                                width: allocated,
                                height: area.height,
                            });
                            remaining_width = remaining_width.saturating_sub(allocated);
                        }
                        Constraint::Percentage(pct) => {
                            let size = (total_width * pct.min(&100) / 100).min(remaining_width);
                            areas.push(Rect {
                                x: area.x + (total_width - remaining_width),
                                y: area.y,
                                width: size,
                                height: area.height,
                            });
                            remaining_width = remaining_width.saturating_sub(size);
                        }
                        Constraint::Flex(_) => {
                            // Placeholder for flex calculation
                            areas.push(Rect {
                                x: area.x + (total_width - remaining_width),
                                y: area.y,
                                width: 0,
                                height: area.height,
                            });
                            flex_total += 1; // Count flex items
                        }
                        Constraint::Min(_) | Constraint::Max(_) => {
                            // For now, treat Min/Max as Fixed(0) - will implement in Phase 2
                            areas.push(Rect {
                                x: area.x + (total_width - remaining_width),
                                y: area.y,
                                width: 0,
                                height: area.height,
                            });
                        }
                    }
                }
                LayoutItem::Nested(_) => {
                    // For nested layouts, we need to allocate space for the entire nested area
                    // We'll use a placeholder and let the recursive calculation handle it
                    areas.push(Rect {
                        x: area.x + (total_width - remaining_width),
                        y: area.y,
                        width: 0, // Will be calculated in second pass
                        height: area.height,
                    });
                    flex_total += 1; // Count nested items as flex for now
                }
            }
        }

        // Second pass: distribute remaining space among flex constraints and nested layouts
        if flex_total > 0 && remaining_width > 0 {
            let total_flex_weight: u16 = items
                .iter()
                .filter_map(|item| {
                    match item {
                        LayoutItem::Constraint(Constraint::Flex(w)) => Some(*w),
                        LayoutItem::Nested(_) => Some(1), // Default weight for nested layouts
                        _ => None,
                    }
                })
                .sum();

            if total_flex_weight > 0 {
                let mut distributed_width = 0u16;
                for (i, item) in items.iter().enumerate() {
                    match item {
                        LayoutItem::Constraint(Constraint::Flex(weight)) => {
                            let flex_width = (remaining_width * weight / total_flex_weight).max(1);
                            areas[i].width = flex_width;
                            distributed_width += flex_width;
                        }
                        LayoutItem::Nested(_) => {
                            let flex_width = (remaining_width / total_flex_weight).max(1);
                            areas[i].width = flex_width;
                            distributed_width += flex_width;
                        }
                        _ => {} // Already handled in first pass
                    }
                }

                // Handle rounding errors by giving any remaining space to the last flex item
                if distributed_width < remaining_width {
                    for (i, item) in items.iter().enumerate().rev() {
                        match item {
                            LayoutItem::Constraint(Constraint::Flex(_)) | LayoutItem::Nested(_) => {
                                areas[i].width += remaining_width - distributed_width;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Adjust x positions for proper layout
        let mut current_x = area.x;
        for rect in &mut areas {
            rect.x = current_x;
            current_x += rect.width;
        }

        areas
    }

    fn split_vertical(area: Rect, items: &[LayoutItem]) -> Vec<Rect> {
        if items.is_empty() {
            return vec![];
        }

        let total_height = area.height;
        let mut areas = Vec::with_capacity(items.len());
        let mut remaining_height = total_height;
        let mut flex_total = 0u16;

        // First pass: allocate fixed and percentage constraints
        for item in items {
            match item {
                LayoutItem::Constraint(constraint) => {
                    match constraint {
                        Constraint::Fixed(size) => {
                            let allocated = (*size).min(remaining_height);
                            areas.push(Rect {
                                x: area.x,
                                y: area.y + (total_height - remaining_height),
                                width: area.width,
                                height: allocated,
                            });
                            remaining_height = remaining_height.saturating_sub(allocated);
                        }
                        Constraint::Percentage(pct) => {
                            let size = (total_height * pct.min(&100) / 100).min(remaining_height);
                            areas.push(Rect {
                                x: area.x,
                                y: area.y + (total_height - remaining_height),
                                width: area.width,
                                height: size,
                            });
                            remaining_height = remaining_height.saturating_sub(size);
                        }
                        Constraint::Flex(_) => {
                            // Placeholder for flex calculation
                            areas.push(Rect {
                                x: area.x,
                                y: area.y + (total_height - remaining_height),
                                width: area.width,
                                height: 0,
                            });
                            flex_total += 1; // Count flex items
                        }
                        Constraint::Min(_) | Constraint::Max(_) => {
                            // For now, treat Min/Max as Fixed(0) - will implement in Phase 2
                            areas.push(Rect {
                                x: area.x,
                                y: area.y + (total_height - remaining_height),
                                width: area.width,
                                height: 0,
                            });
                        }
                    }
                }
                LayoutItem::Nested(_) => {
                    // For nested layouts, we need to allocate space for the entire nested area
                    // We'll use a placeholder and let the recursive calculation handle it
                    areas.push(Rect {
                        x: area.x,
                        y: area.y + (total_height - remaining_height),
                        width: area.width,
                        height: 0, // Will be calculated in second pass
                    });
                    flex_total += 1; // Count nested items as flex for now
                }
            }
        }

        // Second pass: distribute remaining space among flex constraints and nested layouts
        if flex_total > 0 && remaining_height > 0 {
            let total_flex_weight: u16 = items
                .iter()
                .filter_map(|item| {
                    match item {
                        LayoutItem::Constraint(Constraint::Flex(w)) => Some(*w),
                        LayoutItem::Nested(_) => Some(1), // Default weight for nested layouts
                        _ => None,
                    }
                })
                .sum();

            if total_flex_weight > 0 {
                let mut distributed_height = 0u16;
                for (i, item) in items.iter().enumerate() {
                    match item {
                        LayoutItem::Constraint(Constraint::Flex(weight)) => {
                            let flex_height =
                                (remaining_height * weight / total_flex_weight).max(1);
                            areas[i].height = flex_height;
                            distributed_height += flex_height;
                        }
                        LayoutItem::Nested(_) => {
                            let flex_height = (remaining_height / total_flex_weight).max(1);
                            areas[i].height = flex_height;
                            distributed_height += flex_height;
                        }
                        _ => {} // Already handled in first pass
                    }
                }

                // Handle rounding errors by giving any remaining space to the last flex item
                if distributed_height < remaining_height {
                    for (i, item) in items.iter().enumerate().rev() {
                        match item {
                            LayoutItem::Constraint(Constraint::Flex(_)) | LayoutItem::Nested(_) => {
                                areas[i].height += remaining_height - distributed_height;
                                break;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // Adjust y positions for proper layout
        let mut current_y = area.y;
        for rect in &mut areas {
            rect.y = current_y;
            current_y += rect.height;
        }

        areas
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_horizontal_flex_layout() {
        let area = Rect::new(0, 0, 100, 20);
        let layout = Layout::horizontal(vec![
            LayoutItem::widget(Constraint::Flex(2)),
            LayoutItem::widget(Constraint::Flex(1)),
        ]);
        let areas = layout.calculate(area);

        assert_eq!(areas.len(), 2);
        assert_eq!(areas[0].width, 66); // 2/3 of 100
        assert_eq!(areas[1].width, 34); // 1/3 of 100
        assert_eq!(areas[0].height, 20);
        assert_eq!(areas[1].height, 20);
    }

    #[test]
    fn test_horizontal_fixed_layout() {
        let area = Rect::new(0, 0, 100, 20);
        let layout = Layout::horizontal(vec![
            LayoutItem::widget(Constraint::Fixed(30)),
            LayoutItem::widget(Constraint::Fixed(20)),
        ]);
        let areas = layout.calculate(area);

        assert_eq!(areas.len(), 2);
        assert_eq!(areas[0].width, 30);
        assert_eq!(areas[1].width, 20);
        assert_eq!(areas[0].x, 0);
        assert_eq!(areas[1].x, 30);
    }

    #[test]
    fn test_vertical_percentage_layout() {
        let area = Rect::new(0, 0, 50, 100);
        let layout = Layout::vertical(vec![
            LayoutItem::widget(Constraint::Percentage(60)),
            LayoutItem::widget(Constraint::Percentage(40)),
        ]);
        let areas = layout.calculate(area);

        assert_eq!(areas.len(), 2);
        assert_eq!(areas[0].height, 60);
        assert_eq!(areas[1].height, 40);
        assert_eq!(areas[0].y, 0);
        assert_eq!(areas[1].y, 60);
    }

    #[test]
    fn test_empty_constraints() {
        let area = Rect::new(0, 0, 100, 20);
        let layout = Layout::horizontal(vec![]);
        let areas = layout.calculate(area);

        assert_eq!(areas.len(), 0);
    }

    #[test]
    fn test_nested_layout() {
        let area = Rect::new(0, 0, 100, 20);
        let layout = Layout::horizontal(vec![
            LayoutItem::widget(Constraint::Flex(1)), // Process widget
            LayoutItem::nested(Layout::vertical(vec![
                LayoutItem::widget(Constraint::Flex(1)), // CPU widget
                LayoutItem::widget(Constraint::Flex(1)), // Memory widget
                LayoutItem::widget(Constraint::Flex(1)), // Disk widget
            ])),
        ]);
        let areas = layout.calculate(area);

        assert_eq!(areas.len(), 4); // Process + CPU + Memory + Disk
        assert_eq!(areas[0].width, 50); // Process gets 50% (1/2)
        assert_eq!(areas[1].width, 50); // CPU gets 50% width
        assert_eq!(areas[1].height, 6); // CPU gets 1/3 of 20 height
        assert_eq!(areas[2].width, 50); // Memory gets 50% width
        assert_eq!(areas[2].height, 7); // Memory gets 1/3 of 20 height
        assert_eq!(areas[3].width, 50); // Disk gets 50% width
        assert_eq!(areas[3].height, 7); // Disk gets 1/3 of 20 height
    }

    #[test]
    fn test_deeply_nested_layout() {
        let area = Rect::new(0, 0, 100, 20);
        let layout = Layout::horizontal(vec![
            LayoutItem::widget(Constraint::Flex(1)),
            LayoutItem::nested(Layout::vertical(vec![
                LayoutItem::widget(Constraint::Flex(1)),
                LayoutItem::nested(Layout::horizontal(vec![
                    LayoutItem::widget(Constraint::Flex(1)),
                    LayoutItem::widget(Constraint::Flex(1)),
                ])),
            ])),
        ]);
        let areas = layout.calculate(area);

        assert_eq!(areas.len(), 4); // All widgets flattened
        assert_eq!(areas[0].width, 50); // First widget gets 50%
        assert_eq!(areas[1].width, 50); // Second widget gets 50% width
        assert_eq!(areas[2].width, 25); // Third widget gets 25% width (50% of 50%)
        assert_eq!(areas[3].width, 25); // Fourth widget gets 25% width (50% of 50%)
    }
}
