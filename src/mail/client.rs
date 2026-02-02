use anyhow::Result;
use rayon::prelude::*;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::cache::{get_files_to_parse, load_cache, save_cache};
use super::types::{Address, Envelope};

fn render_html(html: &str) -> Result<String> {
    use std::io::Write;
    use std::process::Stdio;

    let mut child = Command::new("w3m")
        .args(["-dump", "-T", "text/html", "-cols", "120"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(html.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Modify maildir flags in a filename
/// Maildir format: {unique}:2,{flags} where flags are sorted letters (DFPRST)
fn modify_maildir_flags(path: &str, add: Option<char>, remove: Option<char>) -> Result<String> {
    let path = std::path::Path::new(path);
    let filename = path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| anyhow::anyhow!("Invalid path"))?;

    // Parse flags from filename (after ":2,")
    let (base, flags) = if let Some(pos) = filename.rfind(":2,") {
        let base = &filename[..pos + 3]; // includes ":2,"
        let flags = &filename[pos + 3..];
        (base.to_string(), flags.to_string())
    } else {
        // No flags section, add one
        (format!("{}:2,", filename), String::new())
    };

    // Modify flags
    let mut flag_chars: Vec<char> = flags.chars().collect();
    if let Some(c) = remove {
        flag_chars.retain(|&x| x != c);
    }
    if let Some(c) = add {
        if !flag_chars.contains(&c) {
            flag_chars.push(c);
        }
    }
    flag_chars.sort(); // Maildir requires sorted flags

    let new_flags: String = flag_chars.into_iter().collect();
    let new_filename = format!("{}{}", base, new_flags);
    let new_path = path.with_file_name(&new_filename);

    // Rename the file
    std::fs::rename(path, &new_path)?;

    Ok(new_path.to_string_lossy().to_string())
}

/// Mark a message as read (add Seen flag) - operates on file path
pub fn mark_as_read(file_path: &str) -> Result<String> {
    modify_maildir_flags(file_path, Some('S'), None)
}

/// Mark a message as unread (remove Seen flag) - operates on file path
pub fn mark_as_unread(file_path: &str) -> Result<String> {
    modify_maildir_flags(file_path, None, Some('S'))
}

/// Toggle read/unread status - operates on file path, returns new path
pub fn toggle_read(file_path: &str, currently_read: bool) -> Result<String> {
    if currently_read {
        mark_as_unread(file_path)
    } else {
        mark_as_read(file_path)
    }
}

/// Scan all mail in maildir and parse threading headers
/// Returns envelopes with message_id, in_reply_to, references populated
/// Uses caching and Rayon for parallel file parsing
pub fn scan_all_mail<F>(mail_dir: &str, user_email: &str, progress: F) -> Result<Vec<Envelope>>
where
    F: Fn(usize, usize) + Sync, // (current, total)
{
    let all_mail_path = format!("{}/[Gmail]/All Mail", mail_dir);

    // Collect all file paths from cur/ and new/
    let mut file_paths: Vec<std::path::PathBuf> = Vec::new();

    for subdir in &["cur", "new"] {
        let dir_path = format!("{}/{}", all_mail_path, subdir);
        if let Ok(entries) = std::fs::read_dir(&dir_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file() {
                    file_paths.push(path);
                }
            }
        }
    }

    let total = file_paths.len();

    // Load cache and determine what needs parsing
    let cache = load_cache();
    let (to_parse, mut cached_envelopes) = get_files_to_parse(&file_paths, &cache);

    let cache_hits = cached_envelopes.len();
    let to_parse_count = to_parse.len();

    // Report initial progress (cache hits are "instant")
    progress(cache_hits, total);

    // Parse only new/modified files in parallel
    if !to_parse.is_empty() {
        let processed = AtomicUsize::new(0);

        let new_envelopes: Vec<Envelope> = to_parse
            .into_par_iter()
            .filter_map(|path| {
                let result = parse_mail_file(&path, user_email).ok();

                // Update progress atomically
                let current = processed.fetch_add(1, Ordering::Relaxed);
                if current % 100 == 0 || current == to_parse_count - 1 {
                    progress(cache_hits + current, total);
                }

                result
            })
            .collect();

        cached_envelopes.extend(new_envelopes);
    }

    progress(total, total);

    // Save updated cache
    let _ = save_cache(&cached_envelopes);

    Ok(cached_envelopes)
}

/// Parse a single maildir file and extract envelope with threading headers
fn parse_mail_file(path: &Path, user_email: &str) -> Result<Envelope> {
    use std::io::{BufRead, BufReader};

    let file = std::fs::File::open(path)?;
    let reader = BufReader::new(file);

    let mut message_id: Option<String> = None;
    let mut in_reply_to: Option<String> = None;
    let mut references: Vec<String> = Vec::new();
    let mut from: Option<String> = None;
    let mut to: Option<String> = None;
    let mut subject: Option<String> = None;
    let mut date: Option<String> = None;
    let mut content_type: Option<String> = None;

    let mut current_header: Option<String> = None;
    let mut current_value = String::new();

    for line in reader.lines() {
        let line = line?;

        // Empty line marks end of headers
        if line.is_empty() {
            // Save the last header
            if let Some(header) = current_header.take() {
                save_header(
                    &header,
                    &current_value,
                    &mut message_id,
                    &mut in_reply_to,
                    &mut references,
                    &mut from,
                    &mut to,
                    &mut subject,
                    &mut date,
                    &mut content_type,
                );
            }
            break;
        }

        // Check if this is a continuation line (starts with whitespace)
        if line.starts_with(' ') || line.starts_with('\t') {
            // Continuation of previous header
            current_value.push(' ');
            current_value.push_str(line.trim());
        } else {
            // New header - save the previous one first
            if let Some(header) = current_header.take() {
                save_header(
                    &header,
                    &current_value,
                    &mut message_id,
                    &mut in_reply_to,
                    &mut references,
                    &mut from,
                    &mut to,
                    &mut subject,
                    &mut date,
                    &mut content_type,
                );
            }

            // Parse new header
            if let Some(colon_pos) = line.find(':') {
                current_header = Some(line[..colon_pos].to_lowercase());
                current_value = line[colon_pos + 1..].trim().to_string();
            }
        }
    }

    // Parse flags from filename
    let flags = parse_flags_from_filename(path);

    // Check if this is a sent message
    let is_sent = from
        .as_ref()
        .map(|f| f.to_lowercase().contains(&user_email.to_lowercase()))
        .unwrap_or(false);

    // Check for attachments (simplified check via content-type)
    let has_attachment = content_type
        .as_ref()
        .map(|ct| ct.contains("multipart/mixed"))
        .unwrap_or(false);

    // Check for inline images (multipart/related often contains inline images)
    let has_inline_images = content_type
        .as_ref()
        .map(|ct| ct.contains("multipart/related"))
        .unwrap_or(false);

    // Parse From address
    let from_addr = from.as_ref().map(|f| parse_email_address(f));

    // Parse To address
    let to_addr = to.as_ref().map(|t| parse_email_address(t));

    // Use file path as ID (unique identifier)
    let id = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();

    Ok(Envelope {
        id,
        flags,
        subject,
        from: from_addr,
        to: to_addr,
        date,
        has_attachment,
        has_inline_images,
        message_id,
        in_reply_to,
        references,
        is_sent,
        file_path: Some(path.to_string_lossy().to_string()),
        // Display fields will be computed by threading algorithm
        thread_depth: 0,
        display_depth: 0,
        is_last_in_thread: false,
        tree_prefix: String::new(),
    })
}

fn save_header(
    header: &str,
    value: &str,
    message_id: &mut Option<String>,
    in_reply_to: &mut Option<String>,
    references: &mut Vec<String>,
    from: &mut Option<String>,
    to: &mut Option<String>,
    subject: &mut Option<String>,
    date: &mut Option<String>,
    content_type: &mut Option<String>,
) {
    match header {
        "message-id" => *message_id = Some(extract_message_id(value)),
        "in-reply-to" => *in_reply_to = Some(extract_message_id(value)),
        "references" => {
            *references = value
                .split_whitespace()
                .map(|s| extract_message_id(s))
                .filter(|s| !s.is_empty())
                .collect();
        }
        "from" => *from = Some(value.to_string()),
        "to" => *to = Some(value.to_string()),
        "subject" => *subject = Some(decode_header_value(value)),
        "date" => *date = Some(parse_date(value)),
        "content-type" => *content_type = Some(value.to_lowercase()),
        _ => {}
    }
}

/// Extract message ID from angle brackets: <foo@bar.com> -> foo@bar.com
fn extract_message_id(s: &str) -> String {
    let s = s.trim();
    if s.starts_with('<') && s.ends_with('>') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Parse email address from "Name <email@example.com>" or "email@example.com" format
fn parse_email_address(s: &str) -> Address {
    let s = s.trim();

    // Try to find angle brackets
    if let Some(start) = s.find('<') {
        if let Some(end) = s.find('>') {
            let addr = s[start + 1..end].trim().to_string();
            let name = s[..start].trim();
            // Remove surrounding quotes from name
            let name = name.trim_matches('"').trim();
            return Address {
                name: if name.is_empty() {
                    None
                } else {
                    Some(decode_header_value(name))
                },
                addr,
            };
        }
    }

    // No angle brackets, just an email address
    Address {
        name: None,
        addr: s.to_string(),
    }
}

/// Decode RFC 2047 encoded header values (=?UTF-8?Q?...?= or =?UTF-8?B?...?=)
fn decode_header_value(s: &str) -> String {
    // Simple decoder for common cases
    let mut result = s.to_string();

    // Handle =?charset?encoding?encoded_text?= format
    while let Some(start) = result.find("=?") {
        if let Some(end) = result[start..].find("?=") {
            let encoded = &result[start..start + end + 2];
            let parts: Vec<&str> = encoded[2..encoded.len() - 2].splitn(3, '?').collect();

            if parts.len() == 3 {
                let _charset = parts[0];
                let encoding = parts[1].to_uppercase();
                let text = parts[2];

                let decoded = match encoding.as_str() {
                    "Q" => decode_quoted_printable(text),
                    "B" => decode_base64(text),
                    _ => text.to_string(),
                };

                result = result.replace(encoded, &decoded);
            } else {
                break;
            }
        } else {
            break;
        }
    }

    // Remove leftover underscores from Q encoding in result
    result.replace('_', " ")
}

fn decode_quoted_printable(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '=' {
            // Read two hex characters
            let hex: String = chars.by_ref().take(2).collect();
            if let Ok(byte) = u8::from_str_radix(&hex, 16) {
                result.push(byte as char);
            }
        } else if c == '_' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

fn decode_base64(s: &str) -> String {
    // Simple base64 decode
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut output = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;

    for c in s.chars() {
        if let Some(val) = ALPHABET.iter().position(|&x| x == c as u8) {
            buffer = (buffer << 6) | val as u32;
            bits += 6;

            if bits >= 8 {
                bits -= 8;
                output.push((buffer >> bits) as u8);
                buffer &= (1 << bits) - 1;
            }
        }
    }

    String::from_utf8_lossy(&output).to_string()
}

/// Parse date string to ISO format (YYYY-MM-DD HH:MM) for sorting
fn parse_date(s: &str) -> String {
    // Email dates are like: "Mon, 15 Jan 2026 10:30:45 -0800"
    // We want: "2026-01-15 10:30" (ISO format for proper sorting)

    let month_to_num = |m: &str| -> &str {
        match m.to_lowercase().as_str() {
            "jan" => "01",
            "feb" => "02",
            "mar" => "03",
            "apr" => "04",
            "may" => "05",
            "jun" => "06",
            "jul" => "07",
            "aug" => "08",
            "sep" => "09",
            "oct" => "10",
            "nov" => "11",
            "dec" => "12",
            _ => "00",
        }
    };

    // Remove commas and clean up the string
    let cleaned = s.replace(',', " ");
    let parts: Vec<&str> = cleaned.split_whitespace().collect();

    // Try to extract day, month, year, time
    let day = parts
        .iter()
        .find(|p| p.parse::<u32>().map(|n| n >= 1 && n <= 31).unwrap_or(false));
    let month = parts.iter().find(|p| {
        matches!(
            p.to_lowercase().as_str(),
            "jan"
                | "feb"
                | "mar"
                | "apr"
                | "may"
                | "jun"
                | "jul"
                | "aug"
                | "sep"
                | "oct"
                | "nov"
                | "dec"
        )
    });
    let year = parts.iter().find(|p| {
        p.parse::<u32>()
            .map(|n| n >= 1990 && n <= 2100)
            .unwrap_or(false)
    });
    let time = parts.iter().find(|p| p.contains(':') && p.len() >= 4);

    if let (Some(day), Some(month), Some(year)) = (day, month, year) {
        let month_num = month_to_num(month);
        let day_padded = format!("{:02}", day.parse::<u32>().unwrap_or(1));
        let short_time: String = time
            .map(|t| t.chars().take(5).collect())
            .unwrap_or_else(|| "00:00".to_string());
        return format!("{}-{}-{} {}", year, month_num, day_padded, short_time);
    }

    // Fallback: return "0000" prefix so unparseable dates sort to bottom
    format!("0000-00-00 {}", s.chars().take(20).collect::<String>())
}

/// Parse flags from maildir filename suffix (e.g., ":2,RS" -> ["Replied", "Seen"])
fn parse_flags_from_filename(path: &Path) -> Vec<String> {
    let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    let mut flags = Vec::new();

    // Find the flags suffix after ":2,"
    if let Some(pos) = filename.find(":2,") {
        let flag_chars = &filename[pos + 3..];
        for c in flag_chars.chars() {
            match c {
                'S' => flags.push("Seen".to_string()),
                'R' => flags.push("Replied".to_string()),
                'F' => flags.push("Flagged".to_string()),
                'D' => flags.push("Draft".to_string()),
                'T' => flags.push("Trashed".to_string()),
                'P' => flags.push("Passed".to_string()),
                _ => {}
            }
        }
    }

    flags
}

/// Inline image data
#[derive(Clone)]
pub struct InlineImage {
    pub data: Vec<u8>,
    pub content_type: String,
    pub filename: Option<String>,
}

/// Attachment info (non-image)
#[derive(Clone)]
pub struct Attachment {
    pub filename: String,
    pub content_type: String,
    pub size: usize,
}

/// Message content with text, images, and attachments
pub struct MessageContent {
    pub text: String,
    pub images: Vec<InlineImage>,
    pub attachments: Vec<Attachment>,
}

/// Read message content directly from file path
pub fn read_message_by_path(file_path: &str) -> Result<String> {
    let content = read_message_content(file_path)?;

    let has_images = !content.images.is_empty();
    let has_attachments = !content.attachments.is_empty();

    // Append image and attachment info if present
    if !has_images && !has_attachments {
        Ok(content.text)
    } else {
        let mut text = content.text;
        text.push_str("\n\n───────────────────────────────────────\n");

        if has_images {
            text.push_str(&format!("Images ({})\n", content.images.len()));
            for img in &content.images {
                let name = img.filename.as_deref().unwrap_or("(unnamed)");
                text.push_str(&format!("  - {} ({})\n", name, img.content_type));
            }
        }

        if has_attachments {
            if has_images {
                text.push('\n');
            }
            text.push_str(&format!("Attachments ({})\n", content.attachments.len()));
            for att in &content.attachments {
                let size = if att.size < 1024 {
                    format!("{} B", att.size)
                } else if att.size < 1024 * 1024 {
                    format!("{:.1} KB", att.size as f64 / 1024.0)
                } else {
                    format!("{:.1} MB", att.size as f64 / (1024.0 * 1024.0))
                };
                text.push_str(&format!(
                    "  - {} ({}, {})\n",
                    att.filename, att.content_type, size
                ));
            }
        }

        Ok(text)
    }
}

/// Read message content with images
pub fn read_message_content(file_path: &str) -> Result<MessageContent> {
    use mail_parser::MimeHeaders;

    let raw = std::fs::read(file_path)?;

    let message = mail_parser::MessageParser::default()
        .parse(&raw)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse message"))?;

    // Extract inline images and attachments
    let mut images = Vec::new();
    let mut attachments = Vec::new();

    for part in message.parts.iter() {
        let content_type = part
            .content_type()
            .map(|ct| format!("{}/{}", ct.ctype(), ct.subtype().unwrap_or("octet-stream")))
            .unwrap_or_default();

        // Check if it's an image
        if content_type.starts_with("image/") {
            if let mail_parser::PartType::Binary(data) | mail_parser::PartType::InlineBinary(data) =
                &part.body
            {
                images.push(InlineImage {
                    data: data.to_vec(),
                    content_type: content_type.clone(),
                    filename: part.attachment_name().map(|s| s.to_string()),
                });
            }
        } else if let Some(filename) = part.attachment_name() {
            // Non-image attachment
            let size = match &part.body {
                mail_parser::PartType::Binary(data) | mail_parser::PartType::InlineBinary(data) => {
                    data.len()
                }
                mail_parser::PartType::Text(text) => text.len(),
                mail_parser::PartType::Html(html) => html.len(),
                mail_parser::PartType::Message(msg) => msg.raw_message.len(),
                mail_parser::PartType::Multipart(_) => 0,
            };
            attachments.push(Attachment {
                filename: filename.to_string(),
                content_type: content_type.clone(),
                size,
            });
        }
    }

    // Try to get text body first, then HTML
    if let Some(text_body) = message.body_text(0) {
        return Ok(MessageContent {
            text: text_body.to_string(),
            images,
            attachments,
        });
    }

    if let Some(html_body) = message.body_html(0) {
        return Ok(MessageContent {
            text: render_html(&html_body)?,
            images,
            attachments,
        });
    }

    // Fallback: try to extract any text parts
    let mut text_parts = Vec::new();
    for part in message.parts.iter() {
        if let mail_parser::PartType::Text(text) = &part.body {
            text_parts.push(text.as_ref());
        }
    }

    if !text_parts.is_empty() {
        return Ok(MessageContent {
            text: text_parts.join("\n\n"),
            images,
            attachments,
        });
    }

    // Last resort: show attachment info
    let mut info = String::from("(No readable text content)\n\nAttachments:\n");
    for part in message.parts.iter() {
        if let Some(filename) = part.attachment_name() {
            info.push_str(&format!("  - {}\n", filename));
        }
    }

    Ok(MessageContent {
        text: info,
        images,
        attachments,
    })
}

/// Save all attachments from an email to a directory
/// Returns list of saved file paths
pub fn save_attachments(file_path: &str, output_dir: &std::path::Path) -> Result<Vec<String>> {
    use mail_parser::MimeHeaders;

    let raw = std::fs::read(file_path)?;
    let message = mail_parser::MessageParser::default()
        .parse(&raw)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse message"))?;

    std::fs::create_dir_all(output_dir)?;

    let mut saved = Vec::new();

    for part in message.parts.iter() {
        // Get attachment filename
        let filename = match part.attachment_name() {
            Some(name) => name.to_string(),
            None => continue, // Skip parts without filenames
        };

        // Get the data
        let data: &[u8] = match &part.body {
            mail_parser::PartType::Binary(data) | mail_parser::PartType::InlineBinary(data) => data,
            mail_parser::PartType::Text(text) => text.as_bytes(),
            mail_parser::PartType::Html(html) => html.as_bytes(),
            _ => continue,
        };

        // Write to file
        let out_path = output_dir.join(&filename);
        std::fs::write(&out_path, data)?;
        saved.push(out_path.to_string_lossy().to_string());
    }

    Ok(saved)
}

/// Deep substring search using ripgrep to find matching files,
/// then parses the matching files directly
pub fn search_deep(query: &str, mail_dir: &str, user_email: &str) -> Result<Vec<Envelope>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Use ripgrep to find files containing the query (case insensitive)
    let output = Command::new("rg")
        .args([
            "-i",            // case insensitive
            "-l",            // only output filenames
            "--max-count=1", // stop after first match per file
            query,
            mail_dir,
        ])
        .output()?;

    let files: Vec<&str> = std::str::from_utf8(&output.stdout)
        .unwrap_or("")
        .lines()
        .take(100) // limit results for performance
        .collect();

    // Parse matched files directly
    let mut envelopes = Vec::new();

    for file_path in files {
        // Skip non-mail files
        if file_path.contains(".mbsync") || file_path.contains(".stringsvalidity") {
            continue;
        }

        let path = std::path::Path::new(file_path);
        if let Ok(env) = parse_mail_file(path, user_email) {
            envelopes.push(env);
        }
    }

    // Sort by date descending
    envelopes.sort_by(|a, b| {
        let date_a = a.date.as_deref().unwrap_or("");
        let date_b = b.date.as_deref().unwrap_or("");
        date_b.cmp(date_a)
    });

    Ok(envelopes)
}
