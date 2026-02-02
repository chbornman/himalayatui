use ratatui::{layout::Rect, widgets::ListState};
use std::sync::Arc;
use std::time::Instant;

use crate::config::Config;
use crate::himalaya::Envelope;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum View {
    List,
    Reader,
    Loading,
    Search,
    DeepSearch,
    Compose,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pane {
    List,
    Preview,
}

pub struct App {
    pub config: Arc<Config>,
    pub view: View,
    pub envelopes: Vec<Envelope>,
    pub original_envelopes: Vec<Envelope>, // Store original list for cancel
    pub filtered_indices: Vec<usize>,
    pub list_state: ListState,
    pub message_content: String,
    pub scroll: u16,
    pub should_quit: bool,
    pub status_message: Option<String>,
    pub current_message_id: Option<String>,
    pub current_subject: Option<String>,
    pub current_from: Option<String>,
    pub search_query: String,
    pub is_search_results: bool,
    // Compose state
    pub compose: ComposeState,
    // Preview pane state
    pub preview_content: String,
    pub preview_id: Option<String>,
    pub preview_scroll: u16,
    // Pane focus
    pub focused_pane: Pane,
    // Mouse tracking - pane areas
    pub list_area: Rect,
    pub preview_area: Rect,
    // Clickable URLs in preview: (row, col_start, col_end, url)
    pub preview_urls: Vec<(u16, u16, u16, String)>,
    // Debounced read marking: (message_id, opened_at)
    pub pending_read_mark: Option<(String, Instant)>,
    // Current account
    pub account: Option<String>,
    pub account_email: Option<String>,
    pub account_signature: Option<String>,
    pub account_signature_delim: String,
    pub accounts: Vec<String>,
    // Inbox filter
    pub show_unread_only: bool,
    // Send confirmation
    pub confirm_send: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ComposeState {
    pub to: String,
    pub subject: String,
    pub body: String,
    pub attachments: Vec<String>,
    pub attachment_selection: usize,
    pub reply_to_id: Option<String>,
}

impl App {
    pub fn new(
        envelopes: Vec<Envelope>,
        config: Arc<Config>,
        account: Option<String>,
        account_email: Option<String>,
        account_signature: Option<String>,
        account_signature_delim: String,
        accounts: Vec<String>,
    ) -> Self {
        let mut list_state = ListState::default();
        if !envelopes.is_empty() {
            list_state.select(Some(0));
        }

        let filtered_indices: Vec<usize> = (0..envelopes.len()).collect();

        Self {
            config,
            view: View::List,
            original_envelopes: envelopes.clone(),
            envelopes,
            filtered_indices,
            list_state,
            message_content: String::new(),
            scroll: 0,
            should_quit: false,
            status_message: None,
            current_message_id: None,
            current_subject: None,
            current_from: None,
            search_query: String::new(),
            is_search_results: false,
            compose: ComposeState::default(),
            preview_content: String::new(),
            preview_id: None,
            preview_scroll: 0,
            focused_pane: Pane::List,
            list_area: Rect::default(),
            preview_area: Rect::default(),
            preview_urls: Vec::new(),
            pending_read_mark: None,
            account,
            account_email,
            account_signature,
            account_signature_delim,
            accounts,
            show_unread_only: false,
            confirm_send: false,
        }
    }

    /// Switch to the next account in the list
    pub fn next_account(&mut self) -> bool {
        if self.accounts.len() <= 1 {
            return false;
        }
        let current_idx = self
            .account
            .as_ref()
            .and_then(|a| self.accounts.iter().position(|x| x == a))
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.accounts.len();
        self.account = Some(self.accounts[next_idx].clone());
        true
    }

    /// Schedule a message to be marked as read after delay
    pub fn schedule_read_mark(&mut self, id: String) {
        self.pending_read_mark = Some((id, Instant::now()));
    }

    /// Check if pending read mark is ready (750ms elapsed)
    /// Returns the message ID if ready to mark
    pub fn check_pending_read_mark(&mut self) -> Option<String> {
        if let Some((ref id, opened_at)) = self.pending_read_mark {
            if opened_at.elapsed().as_millis() >= 750 {
                let id = id.clone();
                self.pending_read_mark = None;
                return Some(id);
            }
        }
        None
    }

    /// Cancel pending read mark (e.g., when navigating away quickly)
    pub fn cancel_pending_read_mark(&mut self) {
        self.pending_read_mark = None;
    }

    pub fn refresh(&mut self, envelopes: Vec<Envelope>) {
        self.envelopes = envelopes.clone();
        self.original_envelopes = envelopes;
        self.is_search_results = false;
        self.search_query.clear();
        self.apply_filter();
        self.status_message = Some("Refreshed".to_string());
    }

    pub fn set_status(&mut self, msg: &str) {
        self.status_message = Some(msg.to_string());
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    pub fn selected_envelope(&self) -> Option<&Envelope> {
        self.list_state
            .selected()
            .and_then(|i| self.filtered_indices.get(i))
            .and_then(|&idx| self.envelopes.get(idx))
    }

    pub fn next(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => (i + 1).min(self.filtered_indices.len() - 1),
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn previous(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let i = match self.list_state.selected() {
            Some(i) => i.saturating_sub(1),
            None => 0,
        };
        self.list_state.select(Some(i));
    }

    pub fn start_search(&mut self) {
        self.search_query.clear();
        self.view = View::Search;
    }

    pub fn update_search(&mut self) {
        let query = self.search_query.to_lowercase();
        self.filtered_indices = self
            .envelopes
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                // Apply unread filter first
                if self.show_unread_only && e.flags.contains(&"Seen".to_string()) {
                    return false;
                }
                if query.is_empty() {
                    return true;
                }
                let subject = e.subject.as_deref().unwrap_or("").to_lowercase();
                let from = e.from_display().to_lowercase();
                // Fuzzy: check if all chars appear in order
                fuzzy_match(&subject, &query) || fuzzy_match(&from, &query)
            })
            .map(|(i, _)| i)
            .collect();

        // Reset selection
        if !self.filtered_indices.is_empty() {
            self.list_state.select(Some(0));
        } else {
            self.list_state.select(None);
        }
    }

    /// Toggle unread-only filter and recompute filtered_indices
    pub fn toggle_unread_filter(&mut self) {
        self.show_unread_only = !self.show_unread_only;
        self.apply_filter();
    }

    /// Recompute filtered_indices based on current filters (unread + search query)
    pub fn apply_filter(&mut self) {
        let query = self.search_query.to_lowercase();
        self.filtered_indices = self
            .envelopes
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                // Apply unread filter
                if self.show_unread_only && e.flags.contains(&"Seen".to_string()) {
                    return false;
                }
                // Apply search query if any
                if query.is_empty() {
                    return true;
                }
                let subject = e.subject.as_deref().unwrap_or("").to_lowercase();
                let from = e.from_display().to_lowercase();
                fuzzy_match(&subject, &query) || fuzzy_match(&from, &query)
            })
            .map(|(i, _)| i)
            .collect();

        // Preserve selection if possible, otherwise reset
        if let Some(selected) = self.list_state.selected() {
            if selected >= self.filtered_indices.len() {
                if !self.filtered_indices.is_empty() {
                    self.list_state.select(Some(0));
                } else {
                    self.list_state.select(None);
                }
            }
        } else if !self.filtered_indices.is_empty() {
            self.list_state.select(Some(0));
        }
    }

    pub fn confirm_search(&mut self) {
        self.view = View::List;
    }

    pub fn cancel_search(&mut self) {
        self.search_query.clear();
        // Restore original envelopes if we were showing search results
        if self.is_search_results {
            self.envelopes = self.original_envelopes.clone();
            self.is_search_results = false;
        }
        self.apply_filter();
        self.view = View::List;
    }

    pub fn set_search_results(&mut self, results: Vec<Envelope>) {
        self.envelopes = results;
        self.is_search_results = true;
        self.apply_filter();
    }

    pub fn scroll_down(&mut self) {
        self.scroll = self.scroll.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.scroll = self.scroll.saturating_sub(1);
    }

    pub fn preview_scroll_down(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_add(3);
    }

    pub fn preview_scroll_up(&mut self) {
        self.preview_scroll = self.preview_scroll.saturating_sub(3);
    }

    /// Load preview for currently selected envelope if not already loaded
    pub fn load_preview_if_needed(&mut self, loader: impl FnOnce(&str) -> String) {
        if let Some(env) = self.selected_envelope() {
            let id = env.id.clone();
            if self.preview_id.as_ref() != Some(&id) {
                self.preview_content = loader(&id);
                self.preview_id = Some(id);
                self.preview_scroll = 0;
                // Extract URLs for click handling
                self.preview_urls = crate::ui::extract_urls(&self.preview_content);
            }
        } else {
            self.preview_content.clear();
            self.preview_id = None;
            self.preview_scroll = 0;
            self.preview_urls.clear();
        }
    }

    /// Force reload preview (e.g., after navigation)
    pub fn reload_preview(&mut self, loader: impl FnOnce(&str) -> String) {
        self.preview_id = None;
        self.load_preview_if_needed(loader);
    }

    /// Mark current email as read in local state
    pub fn mark_current_read(&mut self) {
        if let Some(selected) = self.list_state.selected() {
            if let Some(&idx) = self.filtered_indices.get(selected) {
                if let Some(env) = self.envelopes.get_mut(idx) {
                    if !env.flags.contains(&"Seen".to_string()) {
                        env.flags.push("Seen".to_string());
                    }
                }
            }
        }
    }

    /// Toggle read/unread status in local state, returns (id, is_now_read)
    pub fn toggle_current_read(&mut self) -> Option<(String, bool)> {
        if let Some(selected) = self.list_state.selected() {
            if let Some(&idx) = self.filtered_indices.get(selected) {
                if let Some(env) = self.envelopes.get_mut(idx) {
                    let id = env.id.clone();
                    if env.flags.contains(&"Seen".to_string()) {
                        env.flags.retain(|f| f != "Seen");
                        return Some((id, false));
                    } else {
                        env.flags.push("Seen".to_string());
                        return Some((id, true));
                    }
                }
            }
        }
        None
    }

    /// Update pane areas (called during render)
    pub fn set_pane_areas(&mut self, list: Rect, preview: Rect) {
        self.list_area = list;
        self.preview_area = preview;
    }

    /// Handle click at (x, y) - returns true if email selection changed
    pub fn handle_click(&mut self, x: u16, y: u16) -> bool {
        // Check if click is in list pane
        if x >= self.list_area.x
            && x < self.list_area.x + self.list_area.width
            && y >= self.list_area.y
            && y < self.list_area.y + self.list_area.height
        {
            self.focused_pane = Pane::List;
            // Calculate which row was clicked (accounting for border and scroll offset)
            let visual_row = y.saturating_sub(self.list_area.y + 1) as usize; // +1 for top border
            let actual_row = visual_row + self.list_state.offset();
            if actual_row < self.filtered_indices.len() {
                self.list_state.select(Some(actual_row));
                return true;
            }
        }
        // Check if click is in preview pane
        else if x >= self.preview_area.x
            && x < self.preview_area.x + self.preview_area.width
            && y >= self.preview_area.y
            && y < self.preview_area.y + self.preview_area.height
        {
            self.focused_pane = Pane::Preview;
            // Check if click is on a URL
            if let Some(url) = self.get_url_at(x, y) {
                let _ = std::process::Command::new("xdg-open")
                    .arg(&url)
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
            }
        }
        false
    }

    /// Get URL at screen position if any
    fn get_url_at(&self, x: u16, y: u16) -> Option<String> {
        // Adjust for pane position and scroll
        let rel_x = x.saturating_sub(self.preview_area.x + 1); // +1 for border
        let rel_y = y.saturating_sub(self.preview_area.y + 1) + self.preview_scroll;

        for (row, col_start, col_end, url) in &self.preview_urls {
            if rel_y == *row && rel_x >= *col_start && rel_x < *col_end {
                return Some(url.clone());
            }
        }
        None
    }

    pub fn filtered_envelopes(&self) -> Vec<&Envelope> {
        self.filtered_indices
            .iter()
            .filter_map(|&i| self.envelopes.get(i))
            .collect()
    }

    pub fn start_compose(&mut self, reply_to: Option<(&str, &str, &str)>) {
        self.compose = ComposeState::default();
        if let Some((id, to, subject)) = reply_to {
            self.compose.reply_to_id = Some(id.to_string());
            self.compose.to = to.to_string();
            self.compose.subject = if subject.starts_with("Re:") {
                subject.to_string()
            } else {
                format!("Re: {}", subject)
            };
        }
    }

    pub fn add_attachment(&mut self, path: String) {
        if !self.compose.attachments.contains(&path) {
            self.compose.attachments.push(path);
        }
    }

    pub fn remove_selected_attachment(&mut self) {
        if !self.compose.attachments.is_empty() {
            self.compose
                .attachments
                .remove(self.compose.attachment_selection);
            if self.compose.attachment_selection >= self.compose.attachments.len()
                && self.compose.attachment_selection > 0
            {
                self.compose.attachment_selection -= 1;
            }
        }
    }

    pub fn next_attachment(&mut self) {
        if !self.compose.attachments.is_empty() {
            self.compose.attachment_selection =
                (self.compose.attachment_selection + 1) % self.compose.attachments.len();
        }
    }

    pub fn prev_attachment(&mut self) {
        if !self.compose.attachments.is_empty() {
            self.compose.attachment_selection = if self.compose.attachment_selection == 0 {
                self.compose.attachments.len() - 1
            } else {
                self.compose.attachment_selection - 1
            };
        }
    }
}

fn fuzzy_match(text: &str, pattern: &str) -> bool {
    let mut pattern_chars = pattern.chars().peekable();
    for c in text.chars() {
        if pattern_chars.peek() == Some(&c) {
            pattern_chars.next();
        }
        if pattern_chars.peek().is_none() {
            return true;
        }
    }
    pattern_chars.peek().is_none()
}
