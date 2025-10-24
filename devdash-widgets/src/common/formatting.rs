// devdash-widgets/src/common/formatting.rs
/// Unit options for byte formatting
///
/// Controls how byte values are displayed in widgets.
/// Auto automatically selects the most appropriate unit based on the value size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    /// Automatically select unit (Bytes/KB/MB/GB/TB) based on value size
    Auto,
    /// Always display in bytes
    Bytes,
    /// Always display in kilobytes
    KB,
    /// Always display in megabytes
    MB,
    /// Always display in gigabytes
    GB,
    /// Always display in terabytes
    TB,
}

impl Unit {
    /// Cycle to the next display unit
    pub fn next(self) -> Self {
        match self {
            Self::Auto => Self::Bytes,
            Self::Bytes => Self::KB,
            Self::KB => Self::MB,
            Self::MB => Self::GB,
            Self::GB => Self::TB,
            Self::TB => Self::Auto,
        }
    }
}

/// Format bytes to human-readable string with automatic unit selection
///
/// # Arguments
/// * `bytes` - Number of bytes to format
///
/// # Returns
/// Formatted string with appropriate unit (e.g., "1.5 GB", "512 MB")
///
/// # Example
/// ```rust
/// assert_eq!(format_bytes(1024), "1.0 KB");
/// assert_eq!(format_bytes(1536 * 1024 * 1024), "1.5 GB");
/// ```
pub fn format_bytes(bytes: u64) -> String {
    format_bytes_unit(bytes, Unit::Auto)
}

/// Format bytes with specific unit
///
/// # Arguments
/// * `bytes` - Number of bytes to format
/// * `unit` - Unit to use for formatting
///
/// # Returns
/// Formatted string with specified unit
///
/// # Example
/// ```rust
/// assert_eq!(format_bytes_unit(1024, Unit::KB), "1.0 KB");
/// assert_eq!(format_bytes_unit(1024, Unit::MB), "0.0 MB");
/// ```
pub fn format_bytes_unit(bytes: u64, unit: Unit) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    const TB: u64 = GB * 1024;

    match unit {
        Unit::Auto => {
            if bytes >= TB {
                format!("{:.1} TB", bytes as f64 / TB as f64)
            } else if bytes >= GB {
                format!("{:.1} GB", bytes as f64 / GB as f64)
            } else if bytes >= MB {
                format!("{:.1} MB", bytes as f64 / MB as f64)
            } else if bytes >= KB {
                format!("{:.1} KB", bytes as f64 / KB as f64)
            } else {
                format!("{} B", bytes)
            }
        }
        Unit::Bytes => format!("{} B", bytes),
        Unit::KB => format!("{:.1} KB", bytes as f64 / KB as f64),
        Unit::MB => format!("{:.1} MB", bytes as f64 / MB as f64),
        Unit::GB => format!("{:.1} GB", bytes as f64 / GB as f64),
        Unit::TB => format!("{:.1} TB", bytes as f64 / TB as f64),
    }
}

/// Format rate (bytes per second) to human-readable string
///
/// # Arguments
/// * `bytes_per_sec` - Rate in bytes per second
///
/// # Returns
/// Formatted rate string (e.g., "15.2 MB/s", "1.5 GB/s")
///
/// # Example
/// ```rust
/// assert_eq!(format_rate(1024.0), "1.0 KB/s");
/// assert_eq!(format_rate(15.2 * 1024.0 * 1024.0), "15.2 MB/s");
/// ```
pub fn format_rate(bytes_per_sec: f64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    const TB: f64 = GB * 1024.0;

    if bytes_per_sec >= TB {
        format!("{:.1} TB/s", bytes_per_sec / TB)
    } else if bytes_per_sec >= GB {
        format!("{:.1} GB/s", bytes_per_sec / GB)
    } else if bytes_per_sec >= MB {
        format!("{:.1} MB/s", bytes_per_sec / MB)
    } else if bytes_per_sec >= KB {
        format!("{:.1} KB/s", bytes_per_sec / KB)
    } else {
        format!("{:.0} B/s", bytes_per_sec)
    }
}

/// Format percentage with 1 decimal place
///
/// # Arguments
/// * `value` - Percentage value (0.0 - 100.0)
///
/// # Returns
/// Formatted percentage string (e.g., "45.2%", "100.0%")
///
/// # Example
/// ```rust
/// assert_eq!(format_percentage(45.2), "45.2%");
/// assert_eq!(format_percentage(100.0), "100.0%");
/// ```
pub fn format_percentage(value: f64) -> String {
    format!("{:.1}%", value)
}

/// Format large numbers with separators
///
/// # Arguments
/// * `value` - Number to format
///
/// # Returns
/// Formatted number with comma separators (e.g., "1,234,567")
///
/// # Example
/// ```rust
/// assert_eq!(format_number(1234567), "1,234,567");
/// assert_eq!(format_number(123), "123");
/// ```
pub fn format_number(value: u64) -> String {
    let mut result = String::new();
    let s = value.to_string();
    let chars: Vec<char> = s.chars().collect();

    for (i, ch) in chars.iter().enumerate() {
        if i > 0 && (chars.len() - i) % 3 == 0 {
            result.push(',');
        }
        result.push(*ch);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(0), "0 B");
        assert_eq!(format_bytes(1023), "1023 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_bytes(1024_u64.pow(4)), "1.0 TB");
    }

    #[test]
    fn test_format_bytes_unit() {
        assert_eq!(format_bytes_unit(1024, Unit::KB), "1.0 KB");
        assert_eq!(format_bytes_unit(1024, Unit::MB), "0.0 MB");
        assert_eq!(format_bytes_unit(1024, Unit::Bytes), "1024 B");
    }

    #[test]
    fn test_format_rate() {
        assert_eq!(format_rate(0.0), "0 B/s");
        assert_eq!(format_rate(1024.0), "1.0 KB/s");
        assert_eq!(format_rate(15.2 * 1024.0 * 1024.0), "15.2 MB/s");
        assert_eq!(format_rate(1.5 * 1024.0 * 1024.0 * 1024.0), "1.5 GB/s");
    }

    #[test]
    fn test_format_percentage() {
        assert_eq!(format_percentage(0.0), "0.0%");
        assert_eq!(format_percentage(45.2), "45.2%");
        assert_eq!(format_percentage(100.0), "100.0%");
    }

    #[test]
    fn test_format_number() {
        assert_eq!(format_number(123), "123");
        assert_eq!(format_number(1234), "1,234");
        assert_eq!(format_number(1234567), "1,234,567");
        assert_eq!(format_number(1234567890), "1,234,567,890");
    }

    #[test]
    fn test_unit_cycle() {
        assert_eq!(Unit::Auto.next(), Unit::Bytes);
        assert_eq!(Unit::Bytes.next(), Unit::KB);
        assert_eq!(Unit::KB.next(), Unit::MB);
        assert_eq!(Unit::MB.next(), Unit::GB);
        assert_eq!(Unit::GB.next(), Unit::TB);
        assert_eq!(Unit::TB.next(), Unit::Auto);
    }
}
