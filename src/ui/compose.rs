use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

use super::Pane;
use crate::app::ComposeState;
use crate::config::ThemeConfig;

pub fn render_compose(
    f: &mut Frame,
    area: Rect,
    compose: &ComposeState,
    confirm_send: bool,
    theme: &ThemeConfig,
) {
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
            Span::styled("To: ", Style::default().fg(theme.primary())),
            Span::styled(&compose.to, Style::default().fg(theme.fg())),
        ]),
        Line::from(vec![
            Span::styled("Subject: ", Style::default().fg(theme.primary())),
            Span::styled(&compose.subject, Style::default().fg(theme.fg())),
        ]),
    ];
    let header_pane = Pane::new("Compose", true, theme);
    let header = Paragraph::new(header_text).block(header_pane.block());
    f.render_widget(header, chunks[0]);

    // Body preview
    let body_pane = Pane::new("Body", false, theme);
    let body = Paragraph::new(compose.body.as_str())
        .style(Style::default().fg(theme.fg()))
        .block(body_pane.block())
        .wrap(Wrap { trim: false });
    f.render_widget(body, chunks[1]);

    // Attachments
    let attachment_items: Vec<ListItem> = if compose.attachments.is_empty() {
        vec![ListItem::new(Line::from(Span::styled(
            "(no attachments)",
            Style::default().fg(theme.fg_muted()),
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
                        .fg(theme.attachment())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.fg())
                };
                ListItem::new(Line::from(Span::styled(filename.to_string(), style)))
            })
            .collect()
    };

    let attach_title = format!("Attachments ({})", compose.attachments.len());
    let attach_pane = Pane::new(&attach_title, false, theme);
    let attachments = List::new(attachment_items).block(attach_pane.block());
    f.render_widget(attachments, chunks[2]);

    // Render confirmation modal if needed
    if confirm_send {
        let modal = super::Modal::new(" Confirm ", theme);
        let modal_area = modal.centered_rect(40, 5, area);

        // Clear the area behind the modal
        f.render_widget(Clear, modal_area);

        let modal_text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Send this email?",
                Style::default()
                    .fg(theme.warning())
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Press 's' to confirm, any key to cancel",
                Style::default().fg(theme.fg_muted()),
            )),
        ];

        let content = Paragraph::new(modal_text)
            .alignment(Alignment::Center)
            .block(modal.block());

        f.render_widget(content, modal_area);
    }
}

pub fn render_compose_help(f: &mut Frame, area: Rect, theme: &ThemeConfig) {
    let key_style = Style::default().fg(theme.primary());
    let text_style = Style::default().fg(theme.fg_muted());
    let bg_style = Style::default().bg(theme.bg_panel());

    let help = Line::from(vec![
        Span::styled("e", key_style),
        Span::styled(" edit  ", text_style),
        Span::styled("a", key_style),
        Span::styled(" attach  ", text_style),
        Span::styled("d", key_style),
        Span::styled(" remove  ", text_style),
        Span::styled("j/k", key_style),
        Span::styled(" select  ", text_style),
        Span::styled("s", key_style),
        Span::styled(" send  ", text_style),
        Span::styled("q", key_style),
        Span::styled(" cancel", text_style),
    ]);

    let paragraph = Paragraph::new(help).style(bg_style);
    f.render_widget(paragraph, area);
}
