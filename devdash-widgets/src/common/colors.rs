// devdash-widgets/src/common/colors.rs
use ratatui::style::Color;

/// Threshold constants for usage-based coloring
pub const LOW_THRESHOLD: f64 = 60.0;
pub const HIGH_THRESHOLD: f64 = 80.0;

/// Get color based on usage percentage
///
/// # Arguments
/// * `percentage` - Usage percentage (0.0 - 100.0)
///
/// # Returns
/// Color based on thresholds:
/// - Green: < 60%
/// - Yellow: 60% - 80%
/// - Red: >= 80%
///
/// # Example
/// ```rust
/// assert_eq!(usage_color(45.0), Color::Green);
/// assert_eq!(usage_color(70.0), Color::Yellow);
/// assert_eq!(usage_color(85.0), Color::Red);
/// ```
pub fn usage_color(percentage: f64) -> Color {
    if percentage < LOW_THRESHOLD {
        Color::Green
    } else if percentage < HIGH_THRESHOLD {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Get color for focus state
///
/// # Arguments
/// * `focused` - Whether the widget is currently focused
///
/// # Returns
/// Yellow if focused, DarkGray if not focused
///
/// # Example
/// ```rust
/// assert_eq!(focus_color(true), Color::Yellow);
/// assert_eq!(focus_color(false), Color::DarkGray);
/// ```
pub fn focus_color(focused: bool) -> Color {
    if focused {
        Color::Yellow
    } else {
        Color::DarkGray
    }
}

/// Common color palette for consistent theming across widgets
#[derive(Debug, Clone, Copy)]
pub struct ColorPalette {
    /// Color for focused widgets
    pub focus: Color,
    /// Color for unfocused widgets
    pub unfocus: Color,
    /// Color for good/low usage states
    pub good: Color,
    /// Color for warning/medium usage states
    pub warning: Color,
    /// Color for critical/high usage states
    pub critical: Color,
    /// Color for informational content
    pub info: Color,
}

/// Default color palette used across all widgets
pub const DEFAULT_PALETTE: ColorPalette = ColorPalette {
    focus: Color::Yellow,
    unfocus: Color::DarkGray,
    good: Color::Green,
    warning: Color::Yellow,
    critical: Color::Red,
    info: Color::Cyan,
};

/// Get color from palette based on usage percentage
///
/// # Arguments
/// * `percentage` - Usage percentage (0.0 - 100.0)
/// * `palette` - Color palette to use
///
/// # Returns
/// Color from palette based on usage thresholds
pub fn usage_color_palette(percentage: f64, palette: ColorPalette) -> Color {
    if percentage < LOW_THRESHOLD {
        palette.good
    } else if percentage < HIGH_THRESHOLD {
        palette.warning
    } else {
        palette.critical
    }
}

/// Get focus color from palette
///
/// # Arguments
/// * `focused` - Whether the widget is currently focused
/// * `palette` - Color palette to use
///
/// # Returns
/// Focus or unfocus color from palette
pub fn focus_color_palette(focused: bool, palette: ColorPalette) -> Color {
    if focused {
        palette.focus
    } else {
        palette.unfocus
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_usage_color() {
        assert_eq!(usage_color(0.0), Color::Green);
        assert_eq!(usage_color(59.9), Color::Green);
        assert_eq!(usage_color(60.0), Color::Yellow);
        assert_eq!(usage_color(79.9), Color::Yellow);
        assert_eq!(usage_color(80.0), Color::Red);
        assert_eq!(usage_color(100.0), Color::Red);
    }

    #[test]
    fn test_focus_color() {
        assert_eq!(focus_color(true), Color::Yellow);
        assert_eq!(focus_color(false), Color::DarkGray);
    }

    #[test]
    fn test_usage_color_palette() {
        let palette = DEFAULT_PALETTE;
        assert_eq!(usage_color_palette(50.0, palette), Color::Green);
        assert_eq!(usage_color_palette(70.0, palette), Color::Yellow);
        assert_eq!(usage_color_palette(90.0, palette), Color::Red);
    }

    #[test]
    fn test_focus_color_palette() {
        let palette = DEFAULT_PALETTE;
        assert_eq!(focus_color_palette(true, palette), Color::Yellow);
        assert_eq!(focus_color_palette(false, palette), Color::DarkGray);
    }
}
