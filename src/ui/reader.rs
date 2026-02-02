use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
    Frame,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, StatefulImage};

use super::Pane;
use crate::config::ThemeConfig;

/// Holds the stateful protocol for an image
pub type ImageState = StatefulProtocol;

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
    render_reader_with_images(f, area, content, &mut [], scroll, focused, title, theme);
}

/// Render reader with optional inline images
pub fn render_reader_with_images(
    f: &mut Frame,
    area: Rect,
    content: &str,
    image_states: &mut [ImageState],
    scroll: u16,
    focused: bool,
    title: &str,
    theme: &ThemeConfig,
) {
    let pane = Pane::new(title, focused, theme);
    let block = pane.block();
    let inner = block.inner(area);
    f.render_widget(block, area);

    if image_states.is_empty() {
        // Text only - simple case
        let lines = style_content(content, theme);
        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0));
        f.render_widget(paragraph, inner);
    } else {
        // Mixed content: text then images
        let num_images = image_states.len();

        // Calculate layout: text area + image areas
        let image_height = 12u16; // Lines per image
        let total_image_height = (num_images as u16) * image_height;
        let text_height = inner.height.saturating_sub(total_image_height);

        let mut constraints = vec![Constraint::Length(text_height)];
        for _ in 0..num_images {
            constraints.push(Constraint::Length(image_height));
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(inner);

        // Render text
        let lines = style_content(content, theme);
        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0));
        f.render_widget(paragraph, chunks[0]);

        // Render images
        for (i, state) in image_states.iter_mut().enumerate() {
            let image_widget = StatefulImage::default();
            f.render_stateful_widget(image_widget, chunks[i + 1], state);
        }
    }
}

/// Create image protocol states from images using the picker
pub fn create_image_states(images: &[image::DynamicImage], picker: &Picker) -> Vec<ImageState> {
    images
        .iter()
        .map(|img| picker.new_resize_protocol(img.clone()))
        .collect()
}
