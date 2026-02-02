use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Clear, Gauge, Paragraph},
    Frame,
};

use super::Modal;
use crate::config::ThemeConfig;

/// Render a loading screen with progress bar
pub fn render_loading(
    f: &mut Frame,
    area: Rect,
    progress: f32,
    current: usize,
    total: usize,
    message: &str,
    theme: &ThemeConfig,
) {
    // Fill background
    let bg_block = Block::default().style(Style::default().bg(theme.bg()));
    f.render_widget(bg_block, area);

    // Centered modal
    let modal = Modal::new(" Loading ", theme);
    let modal_area = modal.centered_rect(50, 7, area);

    // Clear the modal area
    f.render_widget(Clear, modal_area);

    // Render modal block
    let block = modal.block();
    let inner_area = block.inner(modal_area);
    f.render_widget(block, modal_area);

    // Layout inside modal: message, progress bar, count
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // message
            Constraint::Length(1), // spacing
            Constraint::Length(1), // progress bar
            Constraint::Length(1), // count
        ])
        .split(inner_area);

    // Message
    let msg = Paragraph::new(Line::from(Span::styled(
        message,
        Style::default().fg(theme.fg()),
    )))
    .alignment(Alignment::Center);
    f.render_widget(msg, chunks[0]);

    // Progress bar using Gauge widget
    let gauge = Gauge::default()
        .ratio(progress.clamp(0.0, 1.0) as f64)
        .gauge_style(Style::default().fg(theme.primary()).bg(theme.bg_element()))
        .use_unicode(true);
    f.render_widget(gauge, chunks[2]);

    // Count
    let count_text = if total > 0 {
        format!("{} / {} messages", current, total)
    } else {
        "Scanning...".to_string()
    };
    let count = Paragraph::new(Line::from(Span::styled(
        count_text,
        Style::default().fg(theme.fg_muted()),
    )))
    .alignment(Alignment::Center);
    f.render_widget(count, chunks[3]);
}
