use anyhow::Result;
use std::process::Command;

use super::types::{Account, Envelope};

pub fn list_accounts() -> Result<Vec<Account>> {
    let output = Command::new("himalaya")
        .args(["account", "list", "--output", "json"])
        .output()?;

    let accounts: Vec<Account> = serde_json::from_slice(&output.stdout)?;
    Ok(accounts)
}

pub fn list_envelopes(account: Option<&str>, folder: Option<&str>) -> Result<Vec<Envelope>> {
    let mut cmd = Command::new("himalaya");
    cmd.args(["envelope", "list", "--output", "json", "--page-size", "500"]);

    if let Some(acc) = account {
        cmd.args(["--account", acc]);
    }
    if let Some(f) = folder {
        cmd.args(["--folder", f]);
    }

    let output = cmd.output()?;
    let envelopes: Vec<Envelope> = serde_json::from_slice(&output.stdout)?;
    Ok(envelopes)
}

pub fn read_message(id: &str, account: Option<&str>) -> Result<String> {
    // Check if this is a numeric ID (himalaya) or a file path/maildir ID
    if id.parse::<u64>().is_ok() {
        // Numeric ID - use himalaya
        let mut cmd = Command::new("himalaya");
        cmd.args(["message", "read", id]);

        if let Some(acc) = account {
            cmd.args(["--account", acc]);
        }

        let output = cmd.output()?;
        let raw_content = String::from_utf8_lossy(&output.stdout).to_string();
        process_email_content(&raw_content)
    } else {
        // Maildir ID - find and read file directly
        read_message_by_maildir_id(id)
    }
}

fn read_message_by_maildir_id(id: &str) -> Result<String> {
    // Search for the file in maildir
    let mail_dir = "/home/caleb/Mail/gmail";

    let output = Command::new("find")
        .args([mail_dir, "-name", &format!("{}*", id), "-type", "f"])
        .output()?;

    let file_path = String::from_utf8_lossy(&output.stdout);
    let file_path = file_path.lines().next().unwrap_or("").trim();

    if file_path.is_empty() {
        return Ok(format!("Message not found: {}", id));
    }

    // Read and parse the email file
    let content = std::fs::read_to_string(file_path)?;

    // Extract body (after blank line following headers)
    let mut in_body = false;
    let mut body_lines = Vec::new();
    let mut is_html = false;

    for line in content.lines() {
        if !in_body {
            if line.is_empty() {
                in_body = true;
            } else if line.to_lowercase().contains("content-type: text/html") {
                is_html = true;
            }
        } else {
            body_lines.push(line);
        }
    }

    let body = body_lines.join("\n");

    if is_html || body.contains("<html") || body.contains("<div") {
        render_html(&body)
    } else {
        Ok(body)
    }
}

fn process_email_content(raw_content: &str) -> Result<String> {
    // Check if content looks like HTML
    if raw_content.contains("<html")
        || raw_content.contains("<HTML")
        || raw_content.contains("<div")
        || raw_content.contains("<p>")
    {
        // Render HTML to text using w3m
        render_html(raw_content)
    } else {
        Ok(raw_content.to_string())
    }
}

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

/// Mark a message as read (add Seen flag)
pub fn mark_as_read(id: &str) -> Result<()> {
    if id.parse::<u64>().is_ok() {
        // Himalaya numeric ID
        Command::new("himalaya")
            .args(["flag", "add", id, "seen"])
            .output()?;
    } else {
        // Notmuch maildir ID - remove unread tag
        Command::new("notmuch")
            .args(["tag", "-unread", &format!("id:{}", id)])
            .output()?;
    }
    Ok(())
}

/// Mark a message as unread (remove Seen flag)
pub fn mark_as_unread(id: &str) -> Result<()> {
    if id.parse::<u64>().is_ok() {
        // Himalaya numeric ID
        Command::new("himalaya")
            .args(["flag", "remove", id, "seen"])
            .output()?;
    } else {
        // Notmuch maildir ID - add unread tag
        Command::new("notmuch")
            .args(["tag", "+unread", &format!("id:{}", id)])
            .output()?;
    }
    Ok(())
}

/// Toggle read/unread status
pub fn toggle_read(id: &str, currently_read: bool) -> Result<()> {
    if currently_read {
        mark_as_unread(id)
    } else {
        mark_as_read(id)
    }
}

pub fn list_folders(account: Option<&str>) -> Result<Vec<String>> {
    let mut cmd = Command::new("himalaya");
    cmd.args(["folder", "list", "--output", "json"]);

    if let Some(acc) = account {
        cmd.args(["--account", acc]);
    }

    let output = cmd.output()?;

    #[derive(serde::Deserialize)]
    struct Folder {
        name: String,
    }

    let folders: Vec<Folder> = serde_json::from_slice(&output.stdout)?;
    Ok(folders.into_iter().map(|f| f.name).collect())
}

pub fn search_notmuch(query: &str) -> Result<Vec<Envelope>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Build search query - search in from, to, subject, and body
    // Add wildcards for prefix matching (notmuch is already case-insensitive)
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|word| {
            if word.contains(':') {
                // Already has a field prefix, leave it alone
                word.to_string()
            } else {
                // Search across multiple fields with wildcard
                let w = word.trim_end_matches('*');
                format!("(from:{}* or to:{}* or subject:{}* or {}*)", w, w, w, w)
            }
        })
        .collect();

    let search_query = terms.join(" and ");

    // Get matching files from notmuch
    let output = Command::new("notmuch")
        .args([
            "search",
            "--output=summary",
            "--format=json",
            "--limit=100",
            &search_query,
        ])
        .output()?;

    #[derive(serde::Deserialize)]
    struct NotmuchThread {
        thread: String,
        timestamp: i64,
        date_relative: String,
        matched: i32,
        total: i32,
        authors: String,
        subject: String,
        tags: Vec<String>,
    }

    let threads: Vec<NotmuchThread> = serde_json::from_slice(&output.stdout).unwrap_or_default();

    // Convert to our Envelope format
    // We need to get the actual message file to get the ID
    let mut envelopes = Vec::new();

    for thread in threads {
        // Get the first message file in this thread
        let file_output = Command::new("notmuch")
            .args([
                "search",
                "--output=files",
                "--limit=1",
                &format!("thread:{}", thread.thread),
            ])
            .output()?;

        let file_path = String::from_utf8_lossy(&file_output.stdout);
        let file_path = file_path.trim();

        // Extract message ID from filename (maildir format: unique_id:flags)
        let id = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .map(|s| s.split(':').next().unwrap_or(s))
            .unwrap_or(&thread.thread)
            .to_string();

        let flags = if thread.tags.contains(&"unread".to_string()) {
            vec![]
        } else {
            vec!["Seen".to_string()]
        };

        envelopes.push(Envelope {
            id,
            flags,
            subject: Some(thread.subject),
            from: Some(super::types::Address {
                name: Some(thread.authors.clone()),
                addr: thread.authors,
            }),
            to: None,
            date: Some(thread.date_relative),
            has_attachment: false,
        });
    }

    Ok(envelopes)
}

/// Deep substring search using ripgrep - slower but matches anywhere in text
pub fn search_deep(query: &str, mail_dir: &str) -> Result<Vec<Envelope>> {
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
        .take(100) // limit results
        .collect();

    let mut envelopes = Vec::new();

    for file_path in files {
        // Skip non-mail files
        if file_path.contains(".mbsync") || file_path.contains(".stringsvalidity") {
            continue;
        }

        // Parse email headers from file
        if let Ok(content) = std::fs::read_to_string(file_path) {
            let mut from = String::new();
            let mut subject = String::new();
            let mut date = String::new();

            for line in content.lines() {
                if line.is_empty() {
                    break; // End of headers
                }
                if let Some(val) = line.strip_prefix("From: ") {
                    from = val.to_string();
                } else if let Some(val) = line.strip_prefix("Subject: ") {
                    subject = val.to_string();
                } else if let Some(val) = line.strip_prefix("Date: ") {
                    date = val.to_string();
                }
            }

            let id = std::path::Path::new(file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .map(|s| s.split(':').next().unwrap_or(s))
                .unwrap_or("unknown")
                .to_string();

            let flags = if file_path.contains(":2,") && file_path.contains('S') {
                vec!["Seen".to_string()]
            } else {
                vec![]
            };

            envelopes.push(Envelope {
                id,
                flags,
                subject: Some(subject),
                from: Some(super::types::Address {
                    name: None,
                    addr: from,
                }),
                to: None,
                date: if date.is_empty() { None } else { Some(date) },
                has_attachment: false,
            });
        }
    }

    Ok(envelopes)
}
