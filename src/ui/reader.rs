use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::config::ThemeConfig;

/// Extract URLs from content - returns (row, col_start, col_end, url)
pub fn extract_urls(content: &str) -> Vec<(u16, u16, u16, String)> {
    let mut urls = Vec::new();

    for (row, line_str) in content.lines().enumerate() {
        let mut search_start = 0;
        while let Some(start) = line_str[search_start..]
            .find("http://")
            .or_else(|| line_str[search_start..].find("https://"))
        {
            let abs_start = search_start + start;

            // Find end of URL (whitespace or common delimiters)
            let url_end = line_str[abs_start..]
                .find(|c: char| c.is_whitespace() || c == '>' || c == ')' || c == ']' || c == '"')
                .map(|i| abs_start + i)
                .unwrap_or(line_str.len());

            let url = &line_str[abs_start..url_end];
            urls.push((
                row as u16,
                abs_start as u16,
                url_end as u16,
                url.to_string(),
            ));

            search_start = url_end;
        }
    }

    urls
}

/// Style content with underlined URLs
fn style_content(content: &str, theme: &ThemeConfig) -> Vec<Line<'static>> {
    let url_style = Style::default()
        .fg(theme.url())
        .add_modifier(Modifier::UNDERLINED);
    let text_style = Style::default().fg(theme.fg());

    content
        .lines()
        .map(|line_str| {
            let mut spans = Vec::new();
            let mut last_end = 0;
            let mut search_start = 0;

            while let Some(start) = line_str[search_start..]
                .find("http://")
                .or_else(|| line_str[search_start..].find("https://"))
            {
                let abs_start = search_start + start;
                let url_end = line_str[abs_start..]
                    .find(|c: char| {
                        c.is_whitespace() || c == '>' || c == ')' || c == ']' || c == '"'
                    })
                    .map(|i| abs_start + i)
                    .unwrap_or(line_str.len());

                if abs_start > last_end {
                    spans.push(Span::styled(
                        line_str[last_end..abs_start].to_string(),
                        text_style,
                    ));
                }
                spans.push(Span::styled(
                    line_str[abs_start..url_end].to_string(),
                    url_style,
                ));

                last_end = url_end;
                search_start = url_end;
            }

            if last_end < line_str.len() {
                spans.push(Span::styled(line_str[last_end..].to_string(), text_style));
            }
            if spans.is_empty() {
                spans.push(Span::styled(line_str.to_string(), text_style));
            }

            Line::from(spans)
        })
        .collect()
}

pub fn render_reader(
    f: &mut Frame,
    area: Rect,
    content: &str,
    scroll: u16,
    focused: bool,
    title: &str,
    theme: &ThemeConfig,
) {
    let border_color = if focused {
        theme.border_active()
    } else {
        theme.border_subtle()
    };

    let lines = style_content(content, theme);

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color))
                .title_style(Style::default().fg(theme.primary()))
                .title(title),
        )
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));

    f.render_widget(paragraph, area);
}
