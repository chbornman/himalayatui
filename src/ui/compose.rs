use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::app::ComposeState;

pub fn render_compose(f: &mut Frame, area: Rect, compose: &ComposeState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // To/Subject
            Constraint::Min(5),    // Body preview
            Constraint::Length(6), // Attachments
        ])
        .split(area);

    // Header (To/Subject)
    let header_text = vec![
        Line::from(vec![
            Span::styled("To: ", Style::default().fg(Color::Yellow)),
            Span::raw(&compose.to),
        ]),
        Line::from(vec![
            Span::styled("Subject: ", Style::default().fg(Color::Yellow)),
            Span::raw(&compose.subject),
        ]),
    ];
    let header =
        Paragraph::new(header_text).block(Block::default().borders(Borders::ALL).title("Compose"));
    f.render_widget(header, chunks[0]);

    // Body preview
    let body = Paragraph::new(compose.body.as_str())
        .block(Block::default().borders(Borders::ALL).title("Body"))
        .wrap(Wrap { trim: false });
    f.render_widget(body, chunks[1]);

    // Attachments
    let attachment_items: Vec<ListItem> = if compose.attachments.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "(no attachments)",
            Style::default().fg(Color::DarkGray),
        )))]
    } else {
        compose
            .attachments
            .iter()
            .enumerate()
            .map(|(i, path)| {
                let filename = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path);
                let style = if i == compose.attachment_selection {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(Line::from(Span::styled(filename.to_string(), style)))
            })
            .collect()
    };

    let attachments = List::new(attachment_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!("Attachments ({})", compose.attachments.len())),
    );
    f.render_widget(attachments, chunks[2]);
}

pub fn render_compose_help(f: &mut Frame, area: Rect) {
    let help = Line::from(vec![
        Span::styled("e", Style::default().fg(Color::Yellow)),
        Span::raw(" edit  "),
        Span::styled("a", Style::default().fg(Color::Yellow)),
        Span::raw(" attach  "),
        Span::styled("d", Style::default().fg(Color::Yellow)),
        Span::raw(" remove  "),
        Span::styled("j/k", Style::default().fg(Color::Yellow)),
        Span::raw(" select  "),
        Span::styled("s", Style::default().fg(Color::Yellow)),
        Span::raw(" send  "),
        Span::styled("q", Style::default().fg(Color::Yellow)),
        Span::raw(" cancel"),
    ]);

    let paragraph = Paragraph::new(help).style(Style::default().bg(Color::DarkGray));
    f.render_widget(paragraph, area);
}
