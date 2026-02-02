# himalayatui

A terminal email client built with [ratatui](https://github.com/ratatui-org/ratatui), wrapping the [himalaya](https://github.com/pimalaya/himalaya) CLI.

![himalayatui screenshot](https://github.com/user-attachments/assets/placeholder)

## Features

- **Two-pane layout** - Inbox list on the left, message preview on the right
- **Vim-style navigation** - `h/l` to switch panes, `j/k` to navigate/scroll
- **Fast search** - `/` for himalaya search (live results), `?` for deep body search
- **Compose & reply** - `c` to compose, `r` to reply, `C` to compose with attachments
- **Attachments** - `a` to download and open in [yazi](https://github.com/sxyazi/yazi)
- **Read/unread tracking** - Auto-marks read, `u` to toggle (syncs via mbsync)
- **Mouse support** - Click to select, scroll wheel, clickable URLs
- **HTML rendering** - Rendered to text via w3m
- **Configurable theming** - Semantic color system with Capstan Cloud defaults
- **Dynamic layout** - Panes resize based on focus

## Requirements

- [himalaya](https://github.com/pimalaya/himalaya) - Email CLI (configured with maildir backend)
- [mbsync](https://isync.sourceforge.io/) - For syncing Gmail to local Maildir
- [w3m](http://w3m.sourceforge.net/) - For HTML email rendering
- [yazi](https://github.com/sxyazi/yazi) - For attachment browsing (optional)
- [ripgrep](https://github.com/BurntSushi/ripgrep) - For deep body search (optional)

## Installation

```bash
# Clone and build
git clone https://github.com/chbornman/himalayatui.git
cd himalayatui
cargo build --release

# Install to ~/.local/bin
cp target/release/himalayatui ~/.local/bin/
```

## Configuration

### himalayatui config

Create `~/.config/himalayatui/config.toml`:

```toml
[layout]
# Pane width percentages when focused
list_focused_width = 66
preview_focused_width = 67

# Column widths in characters
date_width = 14
from_width = 18

[paths]
# Mail directory for deep search (ripgrep)
mail_dir = "~/Mail/gmail"

[theme]
# Capstan Cloud - warm earth tones with gold accents (default)

# Base colors
bg = "#1a1917"
bg_panel = "#262422"
bg_element = "#393634"
fg = "#f7f7f5"
fg_muted = "#8c8985"
fg_subtle = "#b8b5b0"

# Border colors
border = "#524f4c"
border_subtle = "#393634"
border_active = "#d4a366"

# Accent colors
primary = "#d4a366"
primary_light = "#f8ce9b"
secondary = "#8fa5ae"
secondary_light = "#b3c5cc"

# Semantic colors
success = "#52c41a"
warning = "#faad14"
error = "#ff4d4f"
info = "#88c0d0"

# UI-specific
selected_bg = "#393634"
unread = "#d4a366"
url = "#8fa5ae"
attachment = "#b48ead"
```

### himalaya config

himalayatui uses your existing himalaya configuration. Example `~/.config/himalaya/config.toml`:

```toml
[accounts.gmail]
default = true
email = "you@gmail.com"
downloads-dir = "/home/you/Downloads"
folder.aliases.inbox = "Inbox"
backend.type = "maildir"
backend.root-dir = "/home/you/Mail/gmail"

message.send.backend.type = "smtp"
message.send.backend.host = "smtp.gmail.com"
message.send.backend.port = 465
message.send.backend.encryption.type = "tls"
message.send.backend.login = "you@gmail.com"
message.send.backend.auth.type = "password"
message.send.backend.auth.command = "pass show gmail"
```

## Keybindings

### Navigation
| Key | Action |
|-----|--------|
| `h` / `l` | Switch pane focus (list / preview) |
| `j` / `k` | Navigate list or scroll preview |
| `Tab` | Switch account (cycles through himalaya accounts) |
| `Enter` | Focus preview pane |
| `Esc` | Focus list pane / exit search results |

### Actions
| Key | Action |
|-----|--------|
| `/` | Search (from/to/subject, live results) |
| `?` | Deep search (body text, press Enter to search) |
| `u` | Toggle read/unread |
| `r` | Reply to selected message |
| `c` | Compose new message |
| `C` | Compose with attachments |
| `a` | Download attachments & open in yazi |
| `o` | Open in Gmail (browser) |
| `R` | Refresh inbox |
| `q` | Quit |

### Mouse
- Click on list to select email
- Click on pane to focus
- Scroll wheel to navigate/scroll
- Click on URLs to open in browser

## Email Stack

This setup uses local Maildir storage synced from Gmail:

```
Gmail <--mbsync--> ~/Mail/gmail <--himalaya--> himalayatui
```

1. **mbsync** syncs Gmail to local Maildir
2. **himalaya** reads from Maildir, sends via SMTP, handles search
3. **himalayatui** provides the TUI

Read/unread status is stored in Maildir flags and syncs back to Gmail via mbsync.

## License

MIT
