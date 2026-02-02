# mailtui

A terminal email client built with [ratatui](https://github.com/ratatui-org/ratatui), reading directly from Maildir.

## Features

- **Two-pane layout** - Email list on the left, message preview on the right
- **Threaded view** - Emails grouped by conversation with tree prefixes
- **Vim-style navigation** - `h/l` to switch panes, `j/k` to navigate/scroll
- **Fast search** - `/` for in-memory search (from/to/subject), `?` for deep body search
- **Compose & reply** - `c` to compose, `r` to reply, `C` to compose with attachments
- **Inline images** - Renders images inline (Kitty graphics protocol)
- **Attachments** - `a` to download and open in [yazi](https://github.com/sxyazi/yazi)
- **Read/unread tracking** - Auto-marks read, `u` to toggle
- **Mouse support** - Click to select, scroll wheel, clickable URLs
- **HTML rendering** - Rendered to text via w3m
- **Multi-account** - Tab to switch between accounts
- **Configurable theming** - Semantic color system
- **Dynamic layout** - Panes resize based on focus

## Requirements

- [mbsync](https://isync.sourceforge.io/) - For syncing Gmail to local Maildir (runs via systemd timer)
- [msmtp](https://marlam.de/msmtp/) - For sending email
- [w3m](http://w3m.sourceforge.net/) - For HTML email rendering
- [yazi](https://github.com/sxyazi/yazi) - For attachment browsing (optional)
- [ripgrep](https://github.com/BurntSushi/ripgrep) - For deep body search (optional)

## Installation

```bash
git clone https://github.com/chbornman/mailtui.git
cd mailtui
cargo install --path .
```

## Configuration

Create `~/.config/mailtui/config.toml`:

```toml
default_account = "personal"

[accounts.personal]
email = "you@gmail.com"
maildir = "~/Mail/gmail"
signature = "Best,\nYour Name"
send_command = "msmtp -t"

[accounts.work]
email = "you@work.com"
maildir = "~/Mail/work"
send_command = "msmtp -a work -t"

[layout]
list_focused_width = 66
preview_focused_width = 67
date_width = 14
from_width = 18

[compose]
signature_on_reply = true

[theme]
# Warm earth tones with gold accents (default)
bg = "#1a1917"
bg_panel = "#262422"
bg_element = "#393634"
fg = "#f7f7f5"
fg_muted = "#8c8985"
fg_subtle = "#b8b5b0"
border = "#524f4c"
border_active = "#d4a366"
primary = "#d4a366"
secondary = "#8fa5ae"
unread = "#d4a366"
url = "#8fa5ae"
```

## Keybindings

### Navigation
| Key | Action |
|-----|--------|
| `h` / `l` | Switch pane focus (list / preview) |
| `j` / `k` | Navigate list or scroll preview |
| `Tab` | Switch account |
| `Enter` | Focus preview pane |
| `Esc` | Focus list pane / exit search |

### Actions
| Key | Action |
|-----|--------|
| `/` | Search (from/to/subject) |
| `?` | Deep search (body text via ripgrep) |
| `u` | Toggle read/unread |
| `U` | Toggle unread-only filter |
| `r` | Reply to message |
| `c` | Compose new message |
| `C` | Compose with attachments |
| `a` | Download attachments & open in yazi |
| `o` | Open in Gmail (browser) |
| `S` | Edit config |
| `R` | Reload from disk |
| `q` | Quit |

### Mouse
- Click to select/focus
- Scroll wheel to navigate
- Click URLs to open in browser

## Email Stack

```
Gmail <--mbsync--> ~/Mail/gmail <--mailtui
                                     |
                                     +--> msmtp --> Gmail SMTP
```

- **mbsync** syncs Gmail to local Maildir (systemd timer, every 5 min)
- **mailtui** reads directly from Maildir, renders with threading
- **msmtp** sends email via SMTP

## License

MIT
