use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
    Frame,
};

use super::Pane;
use crate::config::ThemeConfig;
use crate::mail::Envelope;

pub fn render_envelopes(
    f: &mut Frame,
    area: Rect,
    envelopes: &[&Envelope],
    state: &mut ListState,
    title: &str,
    focused: bool,
    theme: &ThemeConfig,
    date_width: usize,
    from_width: usize,
) {
    // Available width: area minus borders (2) minus highlight symbol (2)
    let avail_width = area.width.saturating_sub(4) as usize;
    // Account for tree prefix (max ~9 chars for "│  └─ ") and sent indicator (~7 chars for " ┤sent├")
    let tree_prefix_reserve = 10;
    let sent_indicator_reserve = 8;
    let content_width = avail_width.saturating_sub(tree_prefix_reserve + sent_indicator_reserve);
    let from_w = from_width.min(content_width.saturating_sub(date_width + 4) / 3);
    let subject_width = content_width.saturating_sub(date_width + from_w + 4);

    let items: Vec<ListItem> = envelopes
        .iter()
        .map(|e| {
            let is_unread = !e.flags.contains(&"Seen".to_string());
            let has_attach = e.has_attachment;
            let has_images = e.has_inline_images;

            let unread_marker = if is_unread { "*" } else { " " };
            let attach_marker = if has_attach {
                "@"
            } else if has_images {
                "🖼"
            } else {
                " "
            };
            let from = e.from_display();
            let subject = e.subject.as_deref().unwrap_or("(no subject)");
            let date = format_date(e.date.as_deref().unwrap_or(""));

            // Build styled spans
            let mut spans = vec![];

            // Unread marker with color
            if is_unread {
                spans.push(Span::styled(
                    unread_marker,
                    Style::default().fg(theme.unread()),
                ));
            } else {
                spans.push(Span::raw(unread_marker));
            }

            // Attachment/image marker with color
            if has_attach {
                spans.push(Span::styled(
                    attach_marker,
                    Style::default().fg(theme.attachment()),
                ));
            } else if has_images {
                spans.push(Span::styled(
                    attach_marker,
                    Style::default().fg(theme.primary()),
                ));
            } else {
                spans.push(Span::raw(attach_marker));
            }

            // Tree prefix for threading (indentation)
            if !e.tree_prefix.is_empty() {
                spans.push(Span::styled(
                    e.tree_prefix.clone(),
                    Style::default().fg(theme.fg_subtle()),
                ));
            }

            // Main content: date, from, subject
            let main_content = format!(
                " {:dw$} {:fw$} {}",
                truncate(&date, date_width),
                truncate(&from, from_w),
                truncate(subject, subject_width),
                dw = date_width,
                fw = from_w,
            );

            // Thread replies (depth > 0) get more muted colors
            let is_thread_reply = e.thread_depth > 0;

            let text_color = if is_unread {
                if is_thread_reply {
                    theme.fg_muted() // Unread reply: muted but not as dim
                } else {
                    theme.fg() // Unread root: bright white
                }
            } else if is_thread_reply {
                theme.fg_subtle() // Read reply: quite dim
            } else {
                theme.fg_muted() // Read root: normal muted
            };

            let style = if is_unread && !is_thread_reply {
                Style::default().fg(text_color).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(text_color)
            };

            spans.push(Span::styled(main_content, style));

            // Sent indicator with box-breaking style
            if e.is_sent {
                spans.push(Span::styled(" ┤sent├", Style::default().fg(theme.sent())));
            }

            ListItem::new(Line::from(spans))
        })
        .collect();

    let pane = Pane::new(title, focused, theme);

    let list = List::new(items)
        .block(pane.block())
        .highlight_style(
            Style::default()
                .bg(theme.selected_bg())
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ")
        .scroll_padding(0);

    f.render_stateful_widget(list, area, state);
}

fn truncate(s: &str, max: usize) -> String {
    if max < 4 {
        return s.chars().take(max).collect();
    }
    let char_count = s.chars().count();
    if char_count <= max {
        format!("{:width$}", s, width = max)
    } else {
        let truncated: String = s.chars().take(max - 3).collect();
        format!("{}...", truncated)
    }
}

/// Format date from "2026-02-02 04:11+00:00" to "Feb 02 4:11"
fn format_date(date: &str) -> String {
    // Handle notmuch relative dates like "today", "yesterday", "2 days ago"
    if !date.contains('-') || date.contains("ago") {
        return date.to_string();
    }

    // Parse "2026-02-02 04:11+00:00" or similar
    let parts: Vec<&str> = date.split_whitespace().collect();
    if parts.is_empty() {
        return date.to_string();
    }

    let date_part = parts[0];
    let time_part = parts.get(1).unwrap_or(&"");

    // Parse date
    let date_parts: Vec<&str> = date_part.split('-').collect();
    if date_parts.len() < 3 {
        return date.to_string();
    }

    let month = match date_parts[1] {
        "01" => "Jan",
        "02" => "Feb",
        "03" => "Mar",
        "04" => "Apr",
        "05" => "May",
        "06" => "Jun",
        "07" => "Jul",
        "08" => "Aug",
        "09" => "Sep",
        "10" => "Oct",
        "11" => "Nov",
        "12" => "Dec",
        _ => return date.to_string(),
    };
    let day = date_parts[2];

    // Parse time - take just HH:MM
    let time = time_part
        .split('+')
        .next()
        .unwrap_or("")
        .split('-')
        .next()
        .unwrap_or("");
    let time_short = if time.len() >= 5 { &time[..5] } else { time };

    format!("{} {} {}", month, day, time_short)
}
