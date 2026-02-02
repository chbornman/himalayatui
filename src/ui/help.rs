use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::View;
use crate::config::ThemeConfig;

pub fn render_help(
    f: &mut Frame,
    area: Rect,
    view: View,
    status: Option<&str>,
    search_query: Option<&str>,
    theme: &ThemeConfig,
) {
    let key_style = Style::default().fg(theme.primary());
    let text_style = Style::default().fg(theme.fg_subtle());
    let search_style = Style::default().fg(theme.fg());
    let cursor_style = Style::default().fg(theme.primary());
    let deep_key_style = Style::default().fg(theme.secondary());
    let muted_style = Style::default().fg(theme.fg_muted());

    let help_text = match view {
        View::Search => vec![
            Span::styled("/", key_style),
            Span::raw(" "),
            Span::styled(search_query.unwrap_or(""), search_style),
            Span::styled("_", cursor_style),
            Span::styled("  ", text_style),
            Span::styled("Enter", key_style),
            Span::styled(" confirm  ", text_style),
            Span::styled("Esc", key_style),
            Span::styled(" cancel", text_style),
        ],
        View::List => vec![
            Span::styled("h/l", key_style),
            Span::styled(" pane  ", text_style),
            Span::styled("j/k", key_style),
            Span::styled(" nav  ", text_style),
            Span::styled("Tab", key_style),
            Span::styled(" account  ", text_style),
            Span::styled("u", key_style),
            Span::styled(" unread  ", text_style),
            Span::styled("/", key_style),
            Span::styled(" search  ", text_style),
            Span::styled("?", key_style),
            Span::styled(" deep  ", text_style),
            Span::styled("r", key_style),
            Span::styled(" reply  ", text_style),
            Span::styled("c", key_style),
            Span::styled(" compose  ", text_style),
            Span::styled("S", key_style),
            Span::styled(" config  ", text_style),
            Span::styled("R", key_style),
            Span::styled(" refresh  ", text_style),
            Span::styled("q", key_style),
            Span::styled(" quit", text_style),
        ],
        View::Reader => vec![
            Span::styled("j/k", key_style),
            Span::styled(" scroll  ", text_style),
            Span::styled("r", key_style),
            Span::styled(" reply  ", text_style),
            Span::styled("o", key_style),
            Span::styled(" browser  ", text_style),
            Span::styled("q/Esc", key_style),
            Span::styled(" back", text_style),
        ],
        View::Loading => vec![Span::styled("Loading...", text_style)],
        View::DeepSearch => vec![
            Span::styled("?", deep_key_style),
            Span::raw(" "),
            Span::styled(search_query.unwrap_or(""), search_style),
            Span::styled("_", deep_key_style),
            Span::styled("  ", text_style),
            Span::styled("Enter", key_style),
            Span::styled(" search  ", text_style),
            Span::styled("Esc", key_style),
            Span::styled(" cancel  ", text_style),
            Span::styled("(substring match)", muted_style),
        ],
        View::Compose => vec![], // Compose has its own help bar
    };

    let mut line = Line::from(help_text);

    // Add status message if present
    if let Some(msg) = status {
        line.spans
            .push(Span::styled("  │  ", Style::default().fg(theme.border())));
        line.spans
            .push(Span::styled(msg, Style::default().fg(theme.success())));
    }

    let paragraph = Paragraph::new(line).style(Style::default().bg(theme.bg_panel()));

    f.render_widget(paragraph, area);
}
