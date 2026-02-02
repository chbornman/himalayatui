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

/// Get email address for an account from himalaya config
pub fn get_account_email(account_name: Option<&str>) -> Option<String> {
    let config_path = dirs::config_dir()?.join("himalaya/config.toml");
    let content = std::fs::read_to_string(config_path).ok()?;
    let config: toml::Value = content.parse().ok()?;

    let accounts = config.get("accounts")?.as_table()?;

    // If no account specified, find the default one
    let account = if let Some(name) = account_name {
        accounts.get(name)?
    } else {
        accounts
            .values()
            .find(|a| a.get("default").and_then(|v| v.as_bool()).unwrap_or(false))
            .or_else(|| accounts.values().next())?
    };

    account.get("email")?.as_str().map(|s| s.to_string())
}

/// Get default account name
pub fn get_default_account() -> Option<String> {
    let accounts = list_accounts().ok()?;
    accounts.into_iter().find(|a| a.default).map(|a| a.name)
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
    Command::new("himalaya")
        .args(["flag", "add", id, "seen"])
        .output()?;
    Ok(())
}

/// Mark a message as unread (remove Seen flag)
pub fn mark_as_unread(id: &str) -> Result<()> {
    Command::new("himalaya")
        .args(["flag", "remove", id, "seen"])
        .output()?;
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

/// Search envelopes using himalaya's built-in search
/// Query syntax: "from <pattern>", "to <pattern>", "subject <pattern>", or combine with "and"/"or"
pub fn search_envelopes(query: &str) -> Result<Vec<Envelope>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    // Build himalaya search query
    // For simple queries, search across from, to, and subject
    let search_query = if query.contains(" and ") || query.contains(" or ") {
        // Already a structured query
        query.to_string()
    } else {
        // Simple query - search in from, to, and subject
        let terms: Vec<String> = query
            .split_whitespace()
            .map(|word| format!("(from {} or to {} or subject {})", word, word, word))
            .collect();
        terms.join(" and ")
    };

    let output = Command::new("himalaya")
        .args(["envelope", "list", "--output", "json", &search_query])
        .output()?;

    if !output.status.success() {
        return Ok(vec![]);
    }

    let envelopes: Vec<Envelope> = serde_json::from_slice(&output.stdout).unwrap_or_default();
    Ok(envelopes)
}

/// Deep substring search using ripgrep to find matching files,
/// then looks up himalaya IDs by matching subjects
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
        .take(50) // limit results for performance
        .collect();

    // Extract subjects from matched files
    let mut subjects: Vec<String> = Vec::new();
    for file_path in files {
        // Skip non-mail files
        if file_path.contains(".mbsync") || file_path.contains(".stringsvalidity") {
            continue;
        }

        if let Ok(content) = std::fs::read_to_string(file_path) {
            for line in content.lines() {
                if line.is_empty() {
                    break; // End of headers
                }
                if let Some(val) = line.strip_prefix("Subject: ") {
                    // Clean subject for search (take first few words)
                    let clean: String = val
                        .chars()
                        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                        .collect();
                    let words: Vec<&str> = clean.split_whitespace().take(4).collect();
                    if !words.is_empty() {
                        subjects.push(words.join(" "));
                    }
                    break;
                }
            }
        }
    }

    // Look up each subject in himalaya to get proper IDs
    let mut seen_ids = std::collections::HashSet::new();
    let mut envelopes = Vec::new();

    for subject in subjects.iter().take(20) {
        // Search himalaya for this subject
        let output = Command::new("himalaya")
            .args([
                "envelope",
                "list",
                "--output",
                "json",
                &format!("subject {}", subject),
            ])
            .output()?;

        if output.status.success() {
            let results: Vec<Envelope> = serde_json::from_slice(&output.stdout).unwrap_or_default();
            for env in results {
                if seen_ids.insert(env.id.clone()) {
                    envelopes.push(env);
                }
            }
        }
    }

    Ok(envelopes)
}
