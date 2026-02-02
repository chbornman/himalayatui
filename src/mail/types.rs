use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Address {
    pub name: Option<String>,
    pub addr: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct Envelope {
    pub id: String,
    #[serde(default)]
    pub flags: Vec<String>,
    pub subject: Option<String>,
    pub from: Option<Address>,
    pub to: Option<Address>,
    pub date: Option<String>,
    #[serde(default)]
    pub has_attachment: bool,
    #[serde(default)]
    pub has_inline_images: bool,

    // Threading fields (populated by maildir scan)
    #[serde(default)]
    pub message_id: Option<String>,
    #[serde(default)]
    pub in_reply_to: Option<String>,
    #[serde(default)]
    pub references: Vec<String>,
    #[serde(default)]
    pub is_sent: bool,
    #[serde(default)]
    pub file_path: Option<String>,

    // Display fields (computed by threading algorithm, not cached)
    #[serde(skip)]
    pub thread_depth: usize,
    #[serde(skip)]
    pub display_depth: usize,
    #[serde(skip)]
    pub is_last_in_thread: bool,
    #[serde(skip)]
    pub tree_prefix: String,
}

/// Cached envelope with file modification time for invalidation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CachedEnvelope {
    pub envelope: Envelope,
    pub mtime: u64, // File modification time in seconds since epoch
}

impl Envelope {
    pub fn from_display(&self) -> String {
        match &self.from {
            Some(addr) => addr.name.clone().unwrap_or_else(|| addr.addr.clone()),
            None => "(unknown)".to_string(),
        }
    }
}
