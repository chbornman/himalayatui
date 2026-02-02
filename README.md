# himalayatui

A terminal email client built with [ratatui](https://github.com/ratatui-org/ratatui), wrapping the [himalaya](https://github.com/pimalaya/himalaya) CLI.

![himalayatui screenshot](https://github.com/user-attachments/assets/placeholder)

## Features

- **Two-pane layout** - Inbox list on the left, message preview on the right
- **Vim-style navigation** - `h/l` to switch panes, `j/k` to navigate/scroll
- **Fast search** - `/` for notmuch search (instant), `?` for ripgrep deep search
- **Compose & reply** - `c` to compose, `r` to reply, `C` to compose with attachments
- **Attachments** - `a` to download and open in [yazi](https://github.com/sxyazi/yazi)
- **Mouse support** - Click to select, scroll wheel, clickable URLs
- **HTML rendering** - Rendered to text via w3m
- **Dynamic layout** - Panes resize based on focus

## Requirements

- [himalaya](https://github.com/pimalaya/himalaya) - Email CLI (configured with maildir backend)
- [mbsync](https://isync.sourceforge.io/) - For syncing Gmail to local Maildir
- [notmuch](https://notmuchmail.org/) - For fast email search
- [w3m](http://w3m.sourceforge.net/) - For HTML email rendering
- [yazi](https://github.com/sxyazi/yazi) - For attachment browsing (optional)

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
| `Enter` | Focus preview pane |
| `Esc` | Focus list pane |

### Actions
| Key | Action |
|-----|--------|
| `/` | Search (notmuch, live results) |
| `?` | Deep search (ripgrep, press Enter to search) |
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
                        |
                    notmuch (search index)
```

1. **mbsync** syncs Gmail to local Maildir
2. **notmuch** indexes mail for fast search  
3. **himalaya** reads from Maildir, sends via SMTP
4. **himalayatui** provides the TUI

## License

MIT
