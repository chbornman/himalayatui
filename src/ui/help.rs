use ratatui::{
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use crate::app::View;

pub fn render_help(
    f: &mut Frame,
    area: Rect,
    view: View,
    status: Option<&str>,
    search_query: Option<&str>,
) {
    let help_text = match view {
        View::Search => vec![
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
            Span::styled(
                search_query.unwrap_or(""),
                Style::default().fg(Color::White),
            ),
            Span::styled("_", Style::default().fg(Color::Yellow)),
            Span::raw("  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" confirm  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel"),
        ],
        View::List => vec![
            Span::styled("h/l", Style::default().fg(Color::Yellow)),
            Span::raw(" pane  "),
            Span::styled("j/k", Style::default().fg(Color::Yellow)),
            Span::raw(" nav  "),
            Span::styled("/", Style::default().fg(Color::Yellow)),
            Span::raw(" search  "),
            Span::styled("?", Style::default().fg(Color::Yellow)),
            Span::raw(" deep  "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(" reply  "),
            Span::styled("a", Style::default().fg(Color::Yellow)),
            Span::raw(" attach  "),
            Span::styled("o", Style::default().fg(Color::Yellow)),
            Span::raw(" browser  "),
            Span::styled("c", Style::default().fg(Color::Yellow)),
            Span::raw(" compose  "),
            Span::styled("R", Style::default().fg(Color::Yellow)),
            Span::raw(" refresh  "),
            Span::styled("q", Style::default().fg(Color::Yellow)),
            Span::raw(" quit"),
        ],
        View::Reader => vec![
            Span::styled("j/k", Style::default().fg(Color::Yellow)),
            Span::raw(" scroll  "),
            Span::styled("r", Style::default().fg(Color::Yellow)),
            Span::raw(" reply  "),
            Span::styled("o", Style::default().fg(Color::Yellow)),
            Span::raw(" browser  "),
            Span::styled("q/Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" back"),
        ],
        View::Loading => vec![Span::raw("Loading...")],
        View::DeepSearch => vec![
            Span::styled("?", Style::default().fg(Color::Magenta)),
            Span::raw(" "),
            Span::styled(
                search_query.unwrap_or(""),
                Style::default().fg(Color::White),
            ),
            Span::styled("_", Style::default().fg(Color::Magenta)),
            Span::raw("  "),
            Span::styled("Enter", Style::default().fg(Color::Yellow)),
            Span::raw(" search  "),
            Span::styled("Esc", Style::default().fg(Color::Yellow)),
            Span::raw(" cancel  "),
            Span::styled("(substring match)", Style::default().fg(Color::DarkGray)),
        ],
        View::Compose => vec![], // Compose has its own help bar
    };

    let mut line = Line::from(help_text);

    // Add status message if present
    if let Some(msg) = status {
        line.spans.push(Span::raw("  │  "));
        line.spans
            .push(Span::styled(msg, Style::default().fg(Color::Green)));
    }

    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::DarkGray));

    f.render_widget(paragraph, area);
}
