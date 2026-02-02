use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Address {
    pub name: Option<String>,
    pub addr: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Envelope {
    pub id: String,
    pub flags: Vec<String>,
    pub subject: Option<String>,
    pub from: Option<Address>,
    pub to: Option<Address>,
    pub date: Option<String>,
    #[serde(default)]
    pub has_attachment: bool,
}

impl Envelope {
    pub fn from_display(&self) -> String {
        match &self.from {
            Some(addr) => addr.name.clone().unwrap_or_else(|| addr.addr.clone()),
            None => "(unknown)".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Account {
    pub name: String,
    pub backend: String,
    pub default: bool,
}
