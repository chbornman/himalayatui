mod app;
mod config;
mod himalaya;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, MouseEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;
use std::io;
use std::process::Command;

use app::{App, Pane, View};
use config::Config;
use himalaya::{
    get_account_info, list_accounts, list_envelopes, mark_as_read, read_message, search_deep,
    search_envelopes, toggle_read, Envelope,
};
use ui::{render_compose, render_compose_help, render_envelopes, render_help, render_reader};

use std::sync::Arc;

fn main() -> Result<()> {
    // Load config
    let config = Arc::new(Config::load());

    // Get all accounts and find default
    let accounts: Vec<String> = list_accounts()
        .unwrap_or_default()
        .into_iter()
        .map(|a| a.name)
        .collect();
    let account = accounts.first().cloned();
    let account_info = get_account_info(account.as_deref());

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Load envelopes
    let envelopes = list_envelopes(account.as_deref(), None).unwrap_or_default();
    let mut app = App::new(
        envelopes,
        config,
        account,
        account_info.email,
        account_info.signature,
        account_info.signature_delim,
        accounts,
    );

    // Load initial preview and mark as read
    load_and_mark_read(&mut app);

    // Main loop
    loop {
        terminal.draw(|f| render(&mut app, f))?;

        // Process any pending debounced read marks
        process_pending_read_marks(&mut app);

        // Poll with timeout so we redraw on resize even without focus
        if !event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }

        match event::read()? {
            Event::Key(key) => {
                app.clear_status();
                match app.view {
                    View::List => match key.code {
                        KeyCode::Char('q') => app.should_quit = true,
                        KeyCode::Esc => {
                            if app.is_search_results {
                                app.cancel_search();
                                app.reload_preview(|id| {
                                    read_message(id, None)
                                        .unwrap_or_else(|e| format!("Error: {}", e))
                                });
                            } else {
                                app.focused_pane = Pane::List;
                            }
                        }
                        KeyCode::Char('h') | KeyCode::Left => {
                            app.focused_pane = Pane::List;
                        }
                        KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                            app.focused_pane = Pane::Preview;
                        }
                        KeyCode::Char('j') | KeyCode::Down => match app.focused_pane {
                            Pane::List => {
                                app.next();
                                load_and_mark_read(&mut app);
                            }
                            Pane::Preview => app.preview_scroll_down(),
                        },
                        KeyCode::Char('k') | KeyCode::Up => match app.focused_pane {
                            Pane::List => {
                                app.previous();
                                load_and_mark_read(&mut app);
                            }
                            Pane::Preview => app.preview_scroll_up(),
                        },
                        KeyCode::Char('u') => {
                            // Toggle read/unread
                            if let Some((id, is_read)) = app.toggle_current_read() {
                                let _ = toggle_read(&id, !is_read);
                                app.set_status(if is_read {
                                    "Marked read"
                                } else {
                                    "Marked unread"
                                });
                            }
                        }
                        KeyCode::Char('U') => {
                            // Toggle unread-only filter
                            app.toggle_unread_filter();
                            app.reload_preview(|id| {
                                read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                            });
                        }
                        KeyCode::Char('o') => {
                            if let Some(env) = app.selected_envelope() {
                                let subject = env.subject.clone();
                                let from = env.from.as_ref().map(|a| a.addr.clone());
                                open_in_browser_search(subject.as_deref(), from.as_deref());
                                app.set_status("Opened in browser");
                            }
                        }
                        KeyCode::Char('a') => {
                            if let Some(env) = app.selected_envelope() {
                                let id = env.id.clone();
                                // himalaya attachment download only works with numeric IDs
                                if id.parse::<u64>().is_err() {
                                    app.set_status("Press R to refresh, then try again");
                                } else {
                                    match download_attachments(&id) {
                                        Ok(files) => {
                                            if files.is_empty() {
                                                app.set_status("No attachments");
                                            } else {
                                                app.set_status(&format!("{} file(s)", files.len()));
                                                // Open yazi at the first file
                                                open_yazi(&files[0], &mut terminal)?;
                                            }
                                        }
                                        Err(e) => app.set_status(&format!("Error: {}", e)),
                                    }
                                }
                            }
                        }
                        KeyCode::Char('R') => {
                            let envelopes =
                                list_envelopes(app.account.as_deref(), None).unwrap_or_default();
                            app.refresh(envelopes);
                            app.preview_id = None; // Force reload
                            load_and_mark_read(&mut app);
                        }
                        KeyCode::Char('S') => {
                            // Edit himalaya config (for signature, etc.)
                            if let Some(config_path) = dirs::config_dir() {
                                let himalaya_config = config_path.join("himalaya/config.toml");
                                if himalaya_config.exists() {
                                    disable_raw_mode()?;
                                    execute!(std::io::stdout(), LeaveAlternateScreen)?;

                                    let editor = std::env::var("EDITOR")
                                        .unwrap_or_else(|_| "nvim".to_string());
                                    let _ = Command::new(&editor).arg(&himalaya_config).status();

                                    enable_raw_mode()?;
                                    execute!(std::io::stdout(), EnterAlternateScreen)?;
                                    terminal.clear()?;

                                    // Reload account info after editing
                                    let info = get_account_info(app.account.as_deref());
                                    app.account_email = info.email;
                                    app.account_signature = info.signature;
                                    app.account_signature_delim = info.signature_delim;
                                    app.set_status("Config reloaded");
                                }
                            }
                        }
                        KeyCode::Tab => {
                            // Switch account
                            if app.next_account() {
                                let info = get_account_info(app.account.as_deref());
                                app.account_email = info.email;
                                app.account_signature = info.signature;
                                app.account_signature_delim = info.signature_delim;
                                let envelopes = list_envelopes(app.account.as_deref(), None)
                                    .unwrap_or_default();
                                app.refresh(envelopes);
                                app.preview_id = None;
                                load_and_mark_read(&mut app);
                                if let Some(ref acc) = app.account {
                                    app.set_status(&format!("Switched to {}", acc));
                                }
                            }
                        }
                        KeyCode::Char('c') => {
                            app.start_compose(None);
                            // Open editor
                            let sig = SignatureInfo {
                                signature: app.account_signature.as_deref(),
                                delimiter: &app.account_signature_delim,
                                include: true,
                            };
                            let draft =
                                edit_message(&app.compose, app.account_email.as_deref(), sig)?;
                            if let Some((to, subject, body)) = draft {
                                app.compose.to = to;
                                app.compose.subject = subject;
                                app.compose.body = body;
                                app.view = View::Compose;
                            }
                        }
                        KeyCode::Char('C') => {
                            app.start_compose(None);
                            // Pick attachments first
                            if let Some(files) = pick_files()? {
                                for file in files {
                                    app.add_attachment(file);
                                }
                            }
                            // Then open editor
                            let sig = SignatureInfo {
                                signature: app.account_signature.as_deref(),
                                delimiter: &app.account_signature_delim,
                                include: true,
                            };
                            let draft =
                                edit_message(&app.compose, app.account_email.as_deref(), sig)?;
                            if let Some((to, subject, body)) = draft {
                                app.compose.to = to;
                                app.compose.subject = subject;
                                app.compose.body = body;
                                app.view = View::Compose;
                            }
                        }
                        KeyCode::Char('r') => {
                            // Reply to selected message
                            if let Some(env) = app.selected_envelope() {
                                let id = env.id.clone();
                                let to = env
                                    .from
                                    .as_ref()
                                    .map(|a| a.addr.clone())
                                    .unwrap_or_default();
                                let subject = env.subject.clone().unwrap_or_default();
                                app.start_compose(Some((&id, &to, &subject)));
                                let sig = SignatureInfo {
                                    signature: app.account_signature.as_deref(),
                                    delimiter: &app.account_signature_delim,
                                    include: app.config.compose.signature_on_reply,
                                };
                                let draft =
                                    edit_message(&app.compose, app.account_email.as_deref(), sig)?;
                                if let Some((to, subject, body)) = draft {
                                    app.compose.to = to;
                                    app.compose.subject = subject;
                                    app.compose.body = body;
                                    app.view = View::Compose;
                                }
                            }
                        }
                        KeyCode::Char('/') => {
                            app.start_search();
                        }
                        KeyCode::Char('?') => {
                            app.search_query.clear();
                            app.view = View::DeepSearch;
                        }
                        _ => {}
                    },
                    View::Reader => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => app.view = View::List,
                        _ => {}
                    },
                    View::Search => match key.code {
                        KeyCode::Esc => {
                            app.cancel_search();
                            app.reload_preview(|id| {
                                read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                            });
                        }
                        KeyCode::Enter => {
                            app.view = View::List;
                            app.load_preview_if_needed(|id| {
                                read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                            });
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                            run_search(&mut app);
                            app.reload_preview(|id| {
                                read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                            });
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                            run_search(&mut app);
                            app.reload_preview(|id| {
                                read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                            });
                        }
                        KeyCode::Down | KeyCode::Tab => {
                            app.next();
                            app.load_preview_if_needed(|id| {
                                read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                            });
                        }
                        KeyCode::Up => {
                            app.previous();
                            app.load_preview_if_needed(|id| {
                                read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                            });
                        }
                        _ => {}
                    },
                    View::DeepSearch => match key.code {
                        KeyCode::Esc => {
                            app.cancel_search();
                            app.reload_preview(|id| {
                                read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                            });
                        }
                        KeyCode::Enter => {
                            // Run deep search on Enter (it's slow so don't run on every keystroke)
                            if !app.search_query.is_empty() {
                                app.set_status("Deep searching...");
                                match search_deep(&app.search_query, "/home/caleb/Mail/gmail") {
                                    Ok(results) => {
                                        let count = results.len();
                                        app.set_search_results(results);
                                        app.set_status(&format!("Found {} results (deep)", count));
                                    }
                                    Err(e) => {
                                        app.set_status(&format!("Search error: {}", e));
                                    }
                                }
                            }
                            app.view = View::List;
                            app.reload_preview(|id| {
                                read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                            });
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                        }
                        _ => {}
                    },
                    View::Compose => match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            app.view = View::List;
                            app.set_status("Draft discarded");
                        }
                        KeyCode::Char('e') => {
                            // When re-editing, don't add signature again (it's already in body)
                            let sig = SignatureInfo {
                                signature: None,
                                delimiter: "",
                                include: false,
                            };
                            let draft =
                                edit_message(&app.compose, app.account_email.as_deref(), sig)?;
                            if let Some((to, subject, body)) = draft {
                                app.compose.to = to;
                                app.compose.subject = subject;
                                app.compose.body = body;
                            }
                        }
                        KeyCode::Char('a') => {
                            if let Some(files) = pick_files()? {
                                for file in files {
                                    app.add_attachment(file);
                                }
                            }
                        }
                        KeyCode::Char('d') => {
                            app.remove_selected_attachment();
                        }
                        KeyCode::Char('j') | KeyCode::Down => app.next_attachment(),
                        KeyCode::Char('k') | KeyCode::Up => app.prev_attachment(),
                        KeyCode::Char('s') => {
                            if send_message(&app.compose, app.account_email.as_deref())? {
                                app.view = View::List;
                                app.set_status("Message sent!");
                            } else {
                                app.set_status("Failed to send");
                            }
                        }
                        _ => {}
                    },
                    View::Loading => {}
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(_) => {
                    if app.handle_click(mouse.column, mouse.row) {
                        app.load_preview_if_needed(|id| {
                            read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                        });
                    }
                }
                MouseEventKind::ScrollDown => match app.focused_pane {
                    Pane::List => {
                        app.next();
                        app.load_preview_if_needed(|id| {
                            read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                        });
                    }
                    Pane::Preview => app.preview_scroll_down(),
                },
                MouseEventKind::ScrollUp => match app.focused_pane {
                    Pane::List => {
                        app.previous();
                        app.load_preview_if_needed(|id| {
                            read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
                        });
                    }
                    Pane::Preview => app.preview_scroll_up(),
                },
                _ => {}
            },
            Event::Resize(_, _) => {
                // Terminal resized - just redraw on next loop iteration
            }
            _ => {}
        }

        if app.should_quit {
            break;
        }
    }

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}

fn render(app: &mut App, f: &mut Frame) {
    let area = f.area();
    let config = app.config.clone();
    let theme = &config.theme;

    // Split into main area and help bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    match app.view {
        View::List | View::Search | View::DeepSearch | View::Reader => {
            // Two-pane layout: list on left, preview on right
            // Size depends on which pane is focused
            let (list_pct, preview_pct) = match app.focused_pane {
                Pane::List => (
                    config.layout.list_focused_width,
                    100 - config.layout.list_focused_width,
                ),
                Pane::Preview => (
                    100 - config.layout.preview_focused_width,
                    config.layout.preview_focused_width,
                ),
            };
            let panes = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(list_pct),
                    Constraint::Percentage(preview_pct),
                ])
                .split(chunks[0]);

            // Store pane areas for mouse handling
            app.set_pane_areas(panes[0], panes[1]);

            // Left pane: envelope list
            // Clone filtered envelopes to avoid borrow conflict with list_state
            let filtered: Vec<Envelope> = app
                .filtered_indices
                .iter()
                .filter_map(|&i| app.envelopes.get(i).cloned())
                .collect();
            let filtered_refs: Vec<&Envelope> = filtered.iter().collect();
            let account_prefix = app
                .account
                .as_ref()
                .map(|a| format!("[{}] ", a))
                .unwrap_or_default();
            let filter_suffix = if app.show_unread_only {
                " (Unread)"
            } else {
                ""
            };
            let title = if app.is_search_results {
                format!(
                    "{}Search: {} ({} results){}",
                    account_prefix,
                    app.search_query,
                    filtered.len(),
                    filter_suffix
                )
            } else if app.view == View::DeepSearch {
                format!(
                    "{}Deep Search: {}{}",
                    account_prefix, app.search_query, filter_suffix
                )
            } else if app.search_query.is_empty() {
                format!("{}Inbox{}", account_prefix, filter_suffix)
            } else {
                format!(
                    "{}Inbox ({} matches){}",
                    account_prefix,
                    filtered.len(),
                    filter_suffix
                )
            };
            render_envelopes(
                f,
                panes[0],
                &filtered_refs,
                &mut app.list_state,
                &title,
                app.focused_pane == Pane::List,
                theme,
                config.layout.date_width,
                config.layout.from_width,
            );

            // Right pane: message preview with clickable URLs
            let preview_title = app
                .selected_envelope()
                .and_then(|e| e.subject.clone())
                .unwrap_or_else(|| "Message".to_string());
            render_reader(
                f,
                panes[1],
                &app.preview_content,
                app.preview_scroll,
                app.focused_pane == Pane::Preview,
                &preview_title,
                theme,
            );
        }
        View::Compose => {
            render_compose(f, chunks[0], &app.compose);
            render_compose_help(f, chunks[1]);
            return;
        }
        View::Loading => {
            let loading =
                ratatui::widgets::Paragraph::new("Loading...").alignment(Alignment::Center);
            f.render_widget(loading, chunks[0]);
        }
    }

    let search_query = if app.view == View::Search || app.view == View::DeepSearch {
        Some(app.search_query.as_str())
    } else {
        None
    };
    render_help(
        f,
        chunks[1],
        app.view,
        app.status_message.as_deref(),
        search_query,
        theme,
    );
}

fn run_search(app: &mut App) {
    if app.search_query.is_empty() {
        // Restore original inbox
        app.envelopes = app.original_envelopes.clone();
        app.filtered_indices = (0..app.envelopes.len()).collect();
        app.is_search_results = false;
        if !app.envelopes.is_empty() {
            app.list_state.select(Some(0));
        }
    } else {
        match search_envelopes(&app.search_query) {
            Ok(results) => {
                app.set_search_results(results);
            }
            Err(_) => {
                // Ignore errors during typing
            }
        }
    }
}

/// Signature info for compose
struct SignatureInfo<'a> {
    signature: Option<&'a str>,
    delimiter: &'a str,
    include: bool,
}

fn edit_message(
    compose: &app::ComposeState,
    from_email: Option<&str>,
    sig_info: SignatureInfo,
) -> Result<Option<(String, String, String)>> {
    use std::io::Write;

    // Create temp file with email template
    let mut temp_file = tempfile::NamedTempFile::new()?;
    if let Some(email) = from_email {
        writeln!(temp_file, "From: {}", email)?;
    }
    writeln!(temp_file, "To: {}", compose.to)?;
    writeln!(temp_file, "Subject: {}", compose.subject)?;
    writeln!(temp_file)?;
    write!(temp_file, "{}", compose.body)?;

    // Add signature if configured
    if sig_info.include {
        if let Some(sig) = sig_info.signature {
            write!(temp_file, "\n{}{}", sig_info.delimiter, sig)?;
        }
    }
    temp_file.flush()?;

    let path = temp_file.path().to_owned();

    // Open editor
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
    let status = Command::new(&editor).arg(&path).status()?;

    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen)?;

    if !status.success() {
        return Ok(None);
    }

    // Parse the edited file
    let content = std::fs::read_to_string(&path)?;
    let mut lines = content.lines();

    let mut to = String::new();
    let mut subject = String::new();
    let mut in_headers = true;
    let mut body_lines = Vec::new();

    for line in lines.by_ref() {
        if in_headers {
            if line.is_empty() {
                in_headers = false;
            } else if let Some(val) = line.strip_prefix("To: ") {
                to = val.to_string();
            } else if let Some(val) = line.strip_prefix("Subject: ") {
                subject = val.to_string();
            }
        } else {
            body_lines.push(line);
        }
    }

    let body = body_lines.join("\n");

    if to.is_empty() {
        return Ok(None);
    }

    Ok(Some((to, subject, body)))
}

fn pick_files() -> Result<Option<Vec<String>>> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;

    // Use yazi in chooser mode
    let temp_file = tempfile::NamedTempFile::new()?;
    let temp_path = temp_file.path().to_owned();

    let status = Command::new("yazi")
        .args(["--chooser-file", temp_path.to_str().unwrap()])
        .status()?;

    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen)?;

    if !status.success() {
        return Ok(None);
    }

    let content = std::fs::read_to_string(&temp_path).unwrap_or_default();
    let files: Vec<String> = content.lines().map(|s| s.to_string()).collect();

    if files.is_empty() {
        Ok(None)
    } else {
        Ok(Some(files))
    }
}

fn send_message(compose: &app::ComposeState, from_email: Option<&str>) -> Result<bool> {
    use std::io::Write;

    // Build the message with headers
    let mut temp_file = tempfile::NamedTempFile::new()?;
    if let Some(email) = from_email {
        writeln!(temp_file, "From: {}", email)?;
    }
    writeln!(temp_file, "To: {}", compose.to)?;
    writeln!(temp_file, "Subject: {}", compose.subject)?;

    for attachment in &compose.attachments {
        writeln!(temp_file, "Attachment: {}", attachment)?;
    }

    writeln!(temp_file)?;
    write!(temp_file, "{}", compose.body)?;
    temp_file.flush()?;

    let path = temp_file.path();

    // Send via himalaya
    let status = Command::new("himalaya")
        .args(["message", "send"])
        .stdin(std::fs::File::open(path)?)
        .status()?;

    Ok(status.success())
}

fn open_in_browser_search(subject: Option<&str>, from: Option<&str>) {
    // Build a Gmail search query to find the specific email
    let mut query_parts = Vec::new();
    if let Some(subj) = subject {
        // Escape quotes and limit length
        let clean = subj.replace('"', "").chars().take(50).collect::<String>();
        query_parts.push(format!("subject:\"{}\"", clean));
    }
    if let Some(f) = from {
        query_parts.push(format!("from:{}", f));
    }
    let query = query_parts.join(" ");
    let encoded = urlencoding::encode(&query);
    let url = format!("https://mail.google.com/mail/u/0/#search/{}", encoded);
    let _ = Command::new("xdg-open").arg(&url).spawn();
}

fn download_attachments(id: &str) -> Result<Vec<String>> {
    let output = Command::new("himalaya")
        .args(["attachment", "download", id])
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!("{}", stderr.trim()));
    }

    // Filenames are printed to stderr, not stdout
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Parse filenames from output like: Downloading "/home/caleb/Downloads/file.jpg"…
    let files: Vec<String> = stderr
        .lines()
        .filter(|line| line.starts_with("Downloading"))
        .filter_map(|line| {
            let start = line.find('"')?;
            let end = line.rfind('"')?;
            if start < end {
                Some(line[start + 1..end].to_string())
            } else {
                None
            }
        })
        .collect();

    Ok(files)
}

fn open_yazi(path: &str, terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen, DisableMouseCapture)?;

    let _ = Command::new("yazi").arg(path).status();

    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen, EnableMouseCapture)?;
    terminal.clear()?;
    Ok(())
}

/// Load preview for current selection and schedule read mark (debounced)
fn load_and_mark_read(app: &mut App) {
    // Cancel any pending read mark from previous selection
    app.cancel_pending_read_mark();

    // Get ID before loading
    let id = app.selected_envelope().map(|e| e.id.clone());
    let is_unread = app
        .selected_envelope()
        .map(|e| !e.flags.contains(&"Seen".to_string()))
        .unwrap_or(false);

    app.load_preview_if_needed(|id| {
        read_message(id, None).unwrap_or_else(|e| format!("Error: {}", e))
    });

    // Schedule read mark if message is unread (750ms debounce)
    // Works with both himalaya numeric IDs and notmuch maildir IDs
    if let Some(id) = id {
        if is_unread {
            app.schedule_read_mark(id);
        }
    }
}

/// Process pending read marks (call in main loop)
fn process_pending_read_marks(app: &mut App) {
    if let Some(id) = app.check_pending_read_mark() {
        let _ = mark_as_read(&id);
        app.mark_current_read();
    }
}
