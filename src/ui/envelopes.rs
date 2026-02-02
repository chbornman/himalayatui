use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::Line,
    widgets::{Block, Borders, List, ListItem, ListState},
    Frame,
};

use crate::himalaya::Envelope;

pub fn render_envelopes(
    f: &mut Frame,
    area: Rect,
    envelopes: &[&Envelope],
    state: &mut ListState,
    title: &str,
    focused: bool,
) {
    // Available width: area minus borders (2) minus highlight symbol (2)
    let avail_width = area.width.saturating_sub(4) as usize;

    // Fixed widths: flags (2) + spacing
    // Date: "Feb 02 04:11" = 12 chars
    // From: flexible
    // Subject: rest
    let date_width = 14; // "Feb 02 04:11" + padding
    let from_width = 18.min(avail_width.saturating_sub(date_width + 4) / 3);
    let subject_width = avail_width.saturating_sub(date_width + from_width + 4);

    let items: Vec<ListItem> = envelopes
        .iter()
        .map(|e| {
            let unread = if e.flags.contains(&"Seen".to_string()) {
                " "
            } else {
                "*"
            };
            let attach = if e.has_attachment { "@" } else { " " };
            let from = e.from_display();
            let subject = e.subject.as_deref().unwrap_or("(no subject)");
            let date = format_date(e.date.as_deref().unwrap_or(""));
            let line = format!(
                "{}{} {:dw$} {:fw$} {}",
                unread,
                attach,
                truncate(&date, date_width),
                truncate(&from, from_width),
                truncate(subject, subject_width),
                dw = date_width,
                fw = from_width,
            );
            ListItem::new(Line::raw(line))
        })
        .collect();

    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(border_style)
                .title(title.to_string()),
        )
        .highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("> ");

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
