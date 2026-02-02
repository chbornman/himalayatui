use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub layout: LayoutConfig,
    pub paths: PathsConfig,
    pub theme: ThemeConfig,
    pub compose: ComposeConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ComposeConfig {
    /// Include signature when replying to messages
    pub signature_on_reply: bool,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    /// Width percentage for list pane when focused (preview gets the rest)
    pub list_focused_width: u16,
    /// Width percentage for preview pane when focused (list gets the rest)
    pub preview_focused_width: u16,
    /// Date column width in characters
    pub date_width: usize,
    /// From column width in characters
    pub from_width: usize,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct PathsConfig {
    /// Mail directory for deep search
    pub mail_dir: String,
}

/// Semantic theme configuration using Capstan Cloud colors as defaults
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    // Base colors
    pub bg: String,
    pub bg_panel: String,
    pub bg_element: String,
    pub fg: String,
    pub fg_muted: String,
    pub fg_subtle: String,

    // Border colors
    pub border: String,
    pub border_subtle: String,
    pub border_active: String,

    // Accent colors
    pub primary: String,
    pub primary_light: String,
    pub secondary: String,
    pub secondary_light: String,

    // Semantic colors
    pub success: String,
    pub warning: String,
    pub error: String,
    pub info: String,

    // UI-specific mappings
    pub selected_bg: String,
    pub unread: String,
    pub url: String,
    pub attachment: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            layout: LayoutConfig::default(),
            paths: PathsConfig::default(),
            theme: ThemeConfig::default(),
            compose: ComposeConfig::default(),
        }
    }
}

impl Default for ComposeConfig {
    fn default() -> Self {
        Self {
            signature_on_reply: true,
        }
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            list_focused_width: 66,
            preview_focused_width: 67,
            date_width: 14,
            from_width: 18,
        }
    }
}

impl Default for PathsConfig {
    fn default() -> Self {
        Self {
            mail_dir: shellexpand::tilde("~/Mail/gmail").into_owned(),
        }
    }
}

/// Capstan Cloud theme - warm earth tones with gold accents
impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            // Base colors
            bg: "#1a1917".to_string(),
            bg_panel: "#262422".to_string(),
            bg_element: "#393634".to_string(),
            fg: "#f7f7f5".to_string(),
            fg_muted: "#8c8985".to_string(),
            fg_subtle: "#b8b5b0".to_string(),

            // Border colors
            border: "#524f4c".to_string(),
            border_subtle: "#393634".to_string(),
            border_active: "#d4a366".to_string(), // primary

            // Accent colors
            primary: "#d4a366".to_string(),
            primary_light: "#f8ce9b".to_string(),
            secondary: "#8fa5ae".to_string(), // blue
            secondary_light: "#b3c5cc".to_string(),

            // Semantic colors
            success: "#52c41a".to_string(),
            warning: "#faad14".to_string(),
            error: "#ff4d4f".to_string(),
            info: "#88c0d0".to_string(), // cyan

            // UI-specific mappings
            selected_bg: "#393634".to_string(), // bg_element
            unread: "#d4a366".to_string(),      // primary (gold)
            url: "#8fa5ae".to_string(),         // secondary (blue)
            attachment: "#b48ead".to_string(),  // magenta
        }
    }
}

impl Config {
    pub fn load() -> Self {
        let config_path = dirs::config_dir()
            .map(|p| p.join("himalayatui/config.toml"))
            .unwrap_or_else(|| PathBuf::from("~/.config/himalayatui/config.toml"));

        if config_path.exists() {
            match std::fs::read_to_string(&config_path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(config) => return config,
                    Err(e) => eprintln!("Config parse error: {}", e),
                },
                Err(e) => eprintln!("Config read error: {}", e),
            }
        }

        Self::default()
    }
}

impl ThemeConfig {
    /// Parse a color string to ratatui Color
    pub fn parse_color(&self, color: &str) -> ratatui::style::Color {
        parse_color(color)
    }

    // Convenience methods for common colors
    pub fn bg(&self) -> ratatui::style::Color {
        parse_color(&self.bg)
    }
    pub fn bg_panel(&self) -> ratatui::style::Color {
        parse_color(&self.bg_panel)
    }
    pub fn bg_element(&self) -> ratatui::style::Color {
        parse_color(&self.bg_element)
    }
    pub fn fg(&self) -> ratatui::style::Color {
        parse_color(&self.fg)
    }
    pub fn fg_muted(&self) -> ratatui::style::Color {
        parse_color(&self.fg_muted)
    }
    pub fn fg_subtle(&self) -> ratatui::style::Color {
        parse_color(&self.fg_subtle)
    }
    pub fn border(&self) -> ratatui::style::Color {
        parse_color(&self.border)
    }
    pub fn border_subtle(&self) -> ratatui::style::Color {
        parse_color(&self.border_subtle)
    }
    pub fn border_active(&self) -> ratatui::style::Color {
        parse_color(&self.border_active)
    }
    pub fn primary(&self) -> ratatui::style::Color {
        parse_color(&self.primary)
    }
    pub fn primary_light(&self) -> ratatui::style::Color {
        parse_color(&self.primary_light)
    }
    pub fn secondary(&self) -> ratatui::style::Color {
        parse_color(&self.secondary)
    }
    pub fn secondary_light(&self) -> ratatui::style::Color {
        parse_color(&self.secondary_light)
    }
    pub fn success(&self) -> ratatui::style::Color {
        parse_color(&self.success)
    }
    pub fn warning(&self) -> ratatui::style::Color {
        parse_color(&self.warning)
    }
    pub fn error(&self) -> ratatui::style::Color {
        parse_color(&self.error)
    }
    pub fn info(&self) -> ratatui::style::Color {
        parse_color(&self.info)
    }
    pub fn selected_bg(&self) -> ratatui::style::Color {
        parse_color(&self.selected_bg)
    }
    pub fn unread(&self) -> ratatui::style::Color {
        parse_color(&self.unread)
    }
    pub fn url(&self) -> ratatui::style::Color {
        parse_color(&self.url)
    }
    pub fn attachment(&self) -> ratatui::style::Color {
        parse_color(&self.attachment)
    }
}

/// Parse color string to ratatui Color
pub fn parse_color(s: &str) -> ratatui::style::Color {
    use ratatui::style::Color;

    // Try hex first (#RRGGBB)
    if s.starts_with('#') && s.len() == 7 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&s[1..3], 16),
            u8::from_str_radix(&s[3..5], 16),
            u8::from_str_radix(&s[5..7], 16),
        ) {
            return Color::Rgb(r, g, b);
        }
    }

    // Named colors
    match s.to_lowercase().as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "gray" | "grey" => Color::Gray,
        "darkgray" | "darkgrey" => Color::DarkGray,
        "lightred" => Color::LightRed,
        "lightgreen" => Color::LightGreen,
        "lightyellow" => Color::LightYellow,
        "lightblue" => Color::LightBlue,
        "lightmagenta" => Color::LightMagenta,
        "lightcyan" => Color::LightCyan,
        "white" => Color::White,
        _ => Color::White,
    }
}
