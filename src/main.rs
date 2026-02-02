mod app;
mod config;
mod mail;
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
use std::sync::Arc;

use app::{App, Pane, View};
use config::Config;
use mail::{
    build_threaded_list, read_message_by_path, scan_all_mail, search_deep, toggle_read, Envelope,
};
use ratatui_image::picker::Picker;
use ui::{
    render_compose, render_compose_help, render_envelopes, render_help, render_loading,
    render_reader_with_images,
};

fn main() -> Result<()> {
    // Load config
    let config = Arc::new(Config::load());

    // Get default account
    let account_name = config
        .default_account_name()
        .ok_or_else(|| {
            anyhow::anyhow!("No accounts configured. Add accounts to ~/.config/mailtui/config.toml")
        })?
        .to_string();
    let account = config
        .get_account(&account_name)
        .ok_or_else(|| anyhow::anyhow!("Account '{}' not found", account_name))?;

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Get account info from our config
    let mail_dir = shellexpand::tilde(&account.maildir).to_string();
    let user_email = account.email.clone();

    // Setup image picker for Kitty protocol (falls back to halfblocks if query fails)
    let picker = Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks());

    // Load envelopes with progress
    let envelopes = load_envelopes_with_progress(&mut terminal, &mail_dir, &user_email, &config)?;

    let mut app = App::new(envelopes, config.clone(), account_name);

    // Load initial preview with images
    load_and_mark_read_with_images(&mut app, &picker);

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
                                app.reload_preview(read_message_from_path);
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
                                load_and_mark_read_with_images(&mut app, &picker);
                            }
                            Pane::Preview => app.preview_scroll_down(),
                        },
                        KeyCode::Char('k') | KeyCode::Up => match app.focused_pane {
                            Pane::List => {
                                app.previous();
                                load_and_mark_read_with_images(&mut app, &picker);
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
                            app.reload_preview(read_message_from_path);
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
                                if let Some(file_path) = env.file_path.as_deref() {
                                    match download_attachments(file_path) {
                                        Ok(files) => {
                                            if files.is_empty() {
                                                app.set_status("No attachments");
                                            } else {
                                                app.set_status(&format!(
                                                    "{} file(s) saved",
                                                    files.len()
                                                ));
                                                // Open yazi at the first file
                                                open_yazi(&files[0], &mut terminal)?;
                                            }
                                        }
                                        Err(e) => app.set_status(&format!("Error: {}", e)),
                                    }
                                } else {
                                    app.set_status("No file path for message");
                                }
                            }
                        }
                        KeyCode::Char('R') => {
                            // Reload envelopes from maildir (mbsync handled by systemd timer)
                            app.set_status("Reloading...");
                            terminal.draw(|f| render(&mut app, f))?;
                            let mail_dir = app
                                .maildir()
                                .map(|s| shellexpand::tilde(s).to_string())
                                .unwrap_or_default();
                            let user_email = app.email().unwrap_or_default().to_string();
                            match load_envelopes_with_progress(
                                &mut terminal,
                                &mail_dir,
                                &user_email,
                                &app.config,
                            ) {
                                Ok(envelopes) => {
                                    app.refresh(envelopes);
                                    app.preview_id = None;
                                    load_and_mark_read(&mut app);
                                    app.set_status("Reloaded");
                                }
                                Err(e) => {
                                    app.set_status(&format!("Reload error: {}", e));
                                }
                            }
                        }
                        KeyCode::Char('S') => {
                            // Edit mailtui config
                            if let Some(config_path) = dirs::config_dir() {
                                let mailtui_config = config_path.join("mailtui/config.toml");
                                disable_raw_mode()?;
                                execute!(std::io::stdout(), LeaveAlternateScreen)?;

                                let editor =
                                    std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
                                let _ = Command::new(&editor)
                                    .arg("-c")
                                    .arg("set wrap")
                                    .arg(&mailtui_config)
                                    .status();

                                enable_raw_mode()?;
                                execute!(std::io::stdout(), EnterAlternateScreen)?;
                                terminal.clear()?;

                                // Reload config
                                // Note: config is Arc, so we'd need to reload fully
                                // For now just notify user to restart
                                app.set_status("Config edited - restart to apply changes");
                            }
                        }
                        KeyCode::Tab => {
                            // Switch account
                            if let Some(new_account) = app.next_account() {
                                let status_msg = format!("Switched to {}", new_account);
                                // Reload envelopes from new account's maildir
                                let mail_dir = app
                                    .maildir()
                                    .map(|s| shellexpand::tilde(s).to_string())
                                    .unwrap_or_default();
                                let user_email = app.email().unwrap_or_default().to_string();
                                if let Ok(envelopes) = load_envelopes_with_progress(
                                    &mut terminal,
                                    &mail_dir,
                                    &user_email,
                                    &app.config,
                                ) {
                                    app.refresh(envelopes);
                                    app.preview_id = None;
                                    load_and_mark_read(&mut app);
                                }
                                app.set_status(&status_msg);
                            }
                        }
                        KeyCode::Char('c') => {
                            app.start_compose(None);
                            // Open editor
                            let sig = SignatureInfo {
                                signature: app.signature(),
                                delimiter: app.signature_delim(),
                                include: true,
                            };
                            let draft = edit_message(&app.compose, app.email(), sig)?;
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
                                signature: app.signature(),
                                delimiter: app.signature_delim(),
                                include: true,
                            };
                            let draft = edit_message(&app.compose, app.email(), sig)?;
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
                                    signature: app.signature(),
                                    delimiter: app.signature_delim(),
                                    include: app.config.compose.signature_on_reply,
                                };
                                let draft = edit_message(&app.compose, app.email(), sig)?;
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
                    View::Search => match key.code {
                        KeyCode::Esc => {
                            app.cancel_search();
                            app.reload_preview(|id| read_message_from_path(id));
                        }
                        KeyCode::Enter => {
                            app.view = View::List;
                            app.load_preview_if_needed(|id| read_message_from_path(id));
                        }
                        KeyCode::Backspace => {
                            app.search_query.pop();
                            run_search(&mut app);
                            app.reload_preview(|id| read_message_from_path(id));
                        }
                        KeyCode::Char(c) => {
                            app.search_query.push(c);
                            run_search(&mut app);
                            app.reload_preview(|id| read_message_from_path(id));
                        }
                        KeyCode::Down | KeyCode::Tab => {
                            app.next();
                            app.load_preview_if_needed(|id| read_message_from_path(id));
                        }
                        KeyCode::Up => {
                            app.previous();
                            app.load_preview_if_needed(|id| read_message_from_path(id));
                        }
                        _ => {}
                    },
                    View::DeepSearch => match key.code {
                        KeyCode::Esc => {
                            app.cancel_search();
                            app.reload_preview(|id| read_message_from_path(id));
                        }
                        KeyCode::Enter => {
                            // Run deep search on Enter (it's slow so don't run on every keystroke)
                            if !app.search_query.is_empty() {
                                app.set_status("Deep searching...");
                                let mail_dir = app
                                    .maildir()
                                    .map(|s| shellexpand::tilde(s).to_string())
                                    .unwrap_or_default();
                                let user_email = app.email().unwrap_or_default();
                                match search_deep(&app.search_query, &mail_dir, user_email) {
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
                            app.reload_preview(|id| read_message_from_path(id));
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
                        KeyCode::Char('q') => {
                            if app.confirm_send {
                                app.confirm_send = false;
                                app.set_status("Send cancelled");
                            } else {
                                app.view = View::List;
                                app.set_status("Draft discarded");
                            }
                        }
                        KeyCode::Char('e') => {
                            if app.confirm_send {
                                app.confirm_send = false;
                                app.set_status("Send cancelled");
                            } else {
                                // When re-editing, don't add signature again (it's already in body)
                                let sig = SignatureInfo {
                                    signature: None,
                                    delimiter: "",
                                    include: false,
                                };
                                let draft = edit_message(&app.compose, app.email(), sig)?;
                                if let Some((to, subject, body)) = draft {
                                    app.compose.to = to;
                                    app.compose.subject = subject;
                                    app.compose.body = body;
                                }
                            }
                        }
                        KeyCode::Char('a') => {
                            if app.confirm_send {
                                app.confirm_send = false;
                                app.set_status("Send cancelled");
                            } else if let Some(files) = pick_files()? {
                                for file in files {
                                    app.add_attachment(file);
                                }
                            }
                        }
                        KeyCode::Char('d') => {
                            if app.confirm_send {
                                app.confirm_send = false;
                                app.set_status("Send cancelled");
                            } else {
                                app.remove_selected_attachment();
                            }
                        }
                        KeyCode::Char('j') | KeyCode::Down => {
                            if app.confirm_send {
                                app.confirm_send = false;
                                app.set_status("Send cancelled");
                            } else {
                                app.next_attachment();
                            }
                        }
                        KeyCode::Char('k') | KeyCode::Up => {
                            if app.confirm_send {
                                app.confirm_send = false;
                                app.set_status("Send cancelled");
                            } else {
                                app.prev_attachment();
                            }
                        }
                        KeyCode::Char('s') => {
                            if app.confirm_send {
                                // Already confirming, 's' confirms the send
                                app.confirm_send = false;
                                if send_message(&app.compose, app.email(), app.send_command())? {
                                    app.view = View::List;
                                    app.set_status("Message sent!");
                                } else {
                                    app.set_status("Failed to send");
                                }
                            } else {
                                // First press - ask for confirmation
                                app.confirm_send = true;
                                app.set_status(
                                    "Press 's' again to confirm send, any other key to cancel",
                                );
                            }
                        }
                        KeyCode::Esc => {
                            if app.confirm_send {
                                app.confirm_send = false;
                                app.set_status("Send cancelled");
                            } else {
                                app.view = View::List;
                                app.set_status("Draft discarded");
                            }
                        }
                        _ => {}
                    },
                }
            }
            Event::Mouse(mouse) => match mouse.kind {
                MouseEventKind::Down(_) => {
                    if app.handle_click(mouse.column, mouse.row) {
                        app.load_preview_if_needed(|id| read_message_from_path(id));
                    }
                }
                MouseEventKind::ScrollDown => match app.focused_pane {
                    Pane::List => {
                        let h = app.list_visible_height();
                        if app.scroll_list_down(3, h) {
                            app.load_preview_if_needed(|id| read_message_from_path(id));
                        }
                    }
                    Pane::Preview => app.preview_scroll_down(),
                },
                MouseEventKind::ScrollUp => match app.focused_pane {
                    Pane::List => {
                        let h = app.list_visible_height();
                        if app.scroll_list_up(3, h) {
                            app.load_preview_if_needed(|id| read_message_from_path(id));
                        }
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
        View::List | View::Search | View::DeepSearch => {
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
            // Collect references to filtered envelopes (no cloning)
            let filtered_refs: Vec<&Envelope> = app
                .filtered_indices
                .iter()
                .filter_map(|&i| app.envelopes.get(i))
                .collect();
            let account_prefix = format!("[{}] ", app.current_account);
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
                    filtered_refs.len(),
                    filter_suffix
                )
            } else if app.view == View::DeepSearch {
                format!(
                    "{}Deep Search: {}{}",
                    account_prefix, app.search_query, filter_suffix
                )
            } else if app.search_query.is_empty() {
                format!("{}Mail{}", account_prefix, filter_suffix)
            } else {
                format!(
                    "{}Mail ({} matches){}",
                    account_prefix,
                    filtered_refs.len(),
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

            // Right pane: message preview with clickable URLs and images
            let preview_title = app
                .selected_envelope()
                .and_then(|e| e.subject.clone())
                .unwrap_or_else(|| "Message".to_string());
            render_reader_with_images(
                f,
                panes[1],
                &app.preview_content,
                &mut app.preview_image_states,
                app.preview_scroll,
                app.focused_pane == Pane::Preview,
                &preview_title,
                theme,
            );
        }
        View::Compose => {
            render_compose(f, chunks[0], &app.compose, app.confirm_send, theme);
            render_compose_help(f, chunks[1], theme);
            return;
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
        // Restore all indices
        app.filtered_indices = (0..app.envelopes.len()).collect();
        app.is_search_results = false;
    } else {
        // Filter in-memory by subject, from, to (case-insensitive)
        let query_lower = app.search_query.to_lowercase();
        app.filtered_indices = app
            .envelopes
            .iter()
            .enumerate()
            .filter(|(_, env)| {
                // Match subject
                if let Some(ref subj) = env.subject {
                    if subj.to_lowercase().contains(&query_lower) {
                        return true;
                    }
                }
                // Match from
                if let Some(ref from) = env.from {
                    if from.addr.to_lowercase().contains(&query_lower) {
                        return true;
                    }
                    if let Some(ref name) = from.name {
                        if name.to_lowercase().contains(&query_lower) {
                            return true;
                        }
                    }
                }
                false
            })
            .map(|(i, _)| i)
            .collect();
        app.is_search_results = true;
    }

    // Reset selection
    if !app.filtered_indices.is_empty() {
        app.list_state.select(Some(0));
    } else {
        app.list_state.select(None);
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
    let status = Command::new(&editor)
        .arg("-c")
        .arg("set wrap")
        .arg(&path)
        .status()?;

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

fn send_message(
    compose: &app::ComposeState,
    from_email: Option<&str>,
    send_command: &str,
) -> Result<bool> {
    use std::io::Write;
    use std::process::Stdio;

    // Build the message with headers
    let mut message = String::new();
    if let Some(email) = from_email {
        message.push_str(&format!("From: {}\n", email));
    }
    message.push_str(&format!("To: {}\n", compose.to));
    message.push_str(&format!("Subject: {}\n", compose.subject));
    message.push_str("MIME-Version: 1.0\n");

    if compose.attachments.is_empty() {
        // Simple text message
        message.push_str("Content-Type: text/plain; charset=utf-8\n\n");
        message.push_str(&compose.body);
    } else {
        // Multipart message with attachments
        let boundary = format!(
            "----=_Part_{:x}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );

        message.push_str(&format!(
            "Content-Type: multipart/mixed; boundary=\"{}\"\n\n",
            boundary
        ));

        // Text body part
        message.push_str(&format!("--{}\n", boundary));
        message.push_str("Content-Type: text/plain; charset=utf-8\n\n");
        message.push_str(&compose.body);
        message.push_str("\n");

        // Attachment parts
        for attachment_path in &compose.attachments {
            let path = std::path::Path::new(attachment_path);
            let filename = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("attachment");
            let data = std::fs::read(path)?;
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(&data);

            // Guess content type
            let content_type = match path.extension().and_then(|e| e.to_str()) {
                Some("pdf") => "application/pdf",
                Some("png") => "image/png",
                Some("jpg") | Some("jpeg") => "image/jpeg",
                Some("gif") => "image/gif",
                Some("txt") => "text/plain",
                Some("html") => "text/html",
                Some("zip") => "application/zip",
                _ => "application/octet-stream",
            };

            message.push_str(&format!("--{}\n", boundary));
            message.push_str(&format!(
                "Content-Type: {}; name=\"{}\"\n",
                content_type, filename
            ));
            message.push_str("Content-Transfer-Encoding: base64\n");
            message.push_str(&format!(
                "Content-Disposition: attachment; filename=\"{}\"\n\n",
                filename
            ));

            // Line-wrap base64 at 76 chars
            for chunk in encoded.as_bytes().chunks(76) {
                message.push_str(std::str::from_utf8(chunk).unwrap_or(""));
                message.push('\n');
            }
        }

        message.push_str(&format!("--{}--\n", boundary));
    }

    // Parse send command (e.g., "msmtp -t" -> ["msmtp", "-t"])
    let parts: Vec<&str> = send_command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(anyhow::anyhow!("Empty send command"));
    }

    let mut cmd = Command::new(parts[0]);
    for arg in &parts[1..] {
        cmd.arg(arg);
    }

    let mut child = cmd.stdin(Stdio::piped()).spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(message.as_bytes())?;
    }

    let status = child.wait()?;
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

fn download_attachments(file_path: &str) -> Result<Vec<String>> {
    // Download to ~/Downloads
    let download_dir = dirs::download_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    mail::save_attachments(file_path, &download_dir)
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

    app.load_preview_if_needed(|id| read_message_from_path(id));

    // Schedule read mark if message is unread (750ms debounce)
    if let Some(id) = id {
        if is_unread {
            app.schedule_read_mark(id);
        }
    }
}

/// Load preview for current selection with images and schedule read mark (debounced)
fn load_and_mark_read_with_images(app: &mut App, picker: &Picker) {
    // Cancel any pending read mark from previous selection
    app.cancel_pending_read_mark();

    // Get ID before loading
    let id = app.selected_envelope().map(|e| e.id.clone());
    let is_unread = app
        .selected_envelope()
        .map(|e| !e.flags.contains(&"Seen".to_string()))
        .unwrap_or(false);

    app.load_preview_with_images(|id| read_message_with_images(id), picker);

    // Schedule read mark if message is unread (750ms debounce)
    if let Some(id) = id {
        if is_unread {
            app.schedule_read_mark(id);
        }
    }
}

/// Process pending read marks (call in main loop)
fn process_pending_read_marks(app: &mut App) {
    if let Some(_id) = app.check_pending_read_mark() {
        // For now, skip marking as read since we're using maildir directly
        // TODO: Update maildir flags directly
        app.mark_current_read();
    }
}

/// Load envelopes from maildir with progress display
fn load_envelopes_with_progress(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    mail_dir: &str,
    user_email: &str,
    config: &Config,
) -> Result<Vec<Envelope>> {
    // Show initial loading screen
    terminal.draw(|f| {
        render_loading(f, f.area(), 0.0, 0, 0, "Scanning maildir...", &config.theme);
    })?;

    // Run scan_all_mail directly on main thread (Rayon will spawn worker threads)
    // Progress updates won't show smoothly but parallelism will work
    let envelopes = scan_all_mail(mail_dir, user_email, |_current, _total| {
        // Progress callback - we can't easily update UI from here
        // since we're on the main thread doing work
    })?;

    // Show threading progress
    terminal.draw(|f| {
        render_loading(
            f,
            f.area(),
            1.0,
            envelopes.len(),
            envelopes.len(),
            "Building threads...",
            &config.theme,
        );
    })?;

    let threaded = build_threaded_list(envelopes);
    Ok(threaded)
}

/// Read message content from path (used by load_preview_if_needed)
fn read_message_from_path(path: &str) -> String {
    read_message_by_path(path).unwrap_or_else(|e| format!("Error: {}", e))
}

/// Read message content with images from path
fn read_message_with_images(path: &str) -> (String, Vec<image::DynamicImage>) {
    use mail::read_message_content;

    match read_message_content(path) {
        Ok(content) => {
            let images: Vec<image::DynamicImage> = content
                .images
                .iter()
                .filter_map(|img| image::load_from_memory(&img.data).ok())
                .collect();
            (content.text, images)
        }
        Err(e) => (format!("Error: {}", e), Vec::new()),
    }
}
