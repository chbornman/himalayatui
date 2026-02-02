use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    widgets::{Block, Borders},
};

use crate::config::ThemeConfig;

/// A styled pane with consistent border and title treatment
pub struct Pane<'a> {
    title: &'a str,
    focused: bool,
    theme: &'a ThemeConfig,
}

impl<'a> Pane<'a> {
    pub fn new(title: &'a str, focused: bool, theme: &'a ThemeConfig) -> Self {
        Self {
            title,
            focused,
            theme,
        }
    }

    /// Get the styled block for this pane
    pub fn block(&self) -> Block<'a> {
        let border_color = if self.focused {
            self.theme.border_active()
        } else {
            self.theme.border_subtle()
        };

        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title_style(Style::default().fg(self.theme.primary()))
            .title(self.title)
    }

    /// Check if this pane is focused
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Get the theme
    pub fn theme(&self) -> &ThemeConfig {
        self.theme
    }
}

/// A centered modal dialog
pub struct Modal<'a> {
    title: &'a str,
    theme: &'a ThemeConfig,
}

impl<'a> Modal<'a> {
    pub fn new(title: &'a str, theme: &'a ThemeConfig) -> Self {
        Self { title, theme }
    }

    /// Calculate centered rect for the modal
    pub fn centered_rect(&self, width: u16, height: u16, area: Rect) -> Rect {
        let modal_width = width.min(area.width.saturating_sub(4));
        let modal_height = height.min(area.height.saturating_sub(4));
        let x = (area.width.saturating_sub(modal_width)) / 2 + area.x;
        let y = (area.height.saturating_sub(modal_height)) / 2 + area.y;
        Rect::new(x, y, modal_width, modal_height)
    }

    /// Get the styled block for this modal
    pub fn block(&self) -> Block<'a> {
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.theme.border_active()))
            .title(self.title)
            .title_alignment(Alignment::Center)
            .title_style(Style::default().fg(self.theme.primary()))
            .style(Style::default().bg(self.theme.bg_panel()))
    }

    /// Get the theme
    pub fn theme(&self) -> &ThemeConfig {
        self.theme
    }
}
