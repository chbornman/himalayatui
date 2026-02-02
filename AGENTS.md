# AGENTS.md

This file provides context for AI agents working on this codebase.

## Project Overview

**mailtui** is a Terminal User Interface (TUI) email client built with Rust. It uses [ratatui](https://github.com/ratatui-org/ratatui) for terminal rendering and reads email directly from Maildir (synced from Gmail via mbsync).

## Project Structure

```
mailtui/
├── Cargo.toml           # Project dependencies (ratatui, crossterm, serde, etc.)
├── src/
│   ├── main.rs          # Entry point, event loop, key handling
│   ├── app.rs           # Application state management
│   ├── config.rs        # Configuration loading and theming
│   ├── mail/            # Email handling layer
│   │   ├── mod.rs       # Module exports
│   │   ├── client.rs    # Maildir parsing, flag manipulation, MIME parsing
│   │   ├── cache.rs     # Envelope caching for fast startup
│   │   ├── threading.rs # Thread building algorithm
│   │   └── types.rs     # Data types (Envelope, Address)
│   └── ui/              # UI rendering components
│       ├── mod.rs
│       ├── envelopes.rs # Email list rendering
│       ├── reader.rs    # Message preview rendering
│       ├── compose.rs   # Compose view rendering
│       ├── pane.rs      # Pane/Modal abstractions
│       └── help.rs      # Help bar rendering
└── AGENTS.md            # This file
```

## Key Concepts

### Maildir Native

This project reads email directly from Maildir format without any external email CLI. It uses:
- `mail-parser` crate for MIME parsing
- Direct maildir filename manipulation for flags (read/unread)
- `msmtp` or `sendmail` for sending

### Threading

Emails are threaded using Message-ID, In-Reply-To, and References headers. The threading algorithm:
- Builds thread trees from header relationships
- Collapses linear chains (single replies stay at depth 1)
- Branches create new depth levels (max depth 3)
- Threads sorted by most recent message

### Data Types

- `Envelope` - Email metadata (id, flags, subject, from, to, date, threading info)
- `Address` - Email address (name, addr)

These are defined in `src/mail/types.rs`.

## Configuration

Config file: `~/.config/mailtui/config.toml`

```toml
default_account = "personal"

[accounts.personal]
email = "you@example.com"
maildir = "~/Mail/gmail"
signature = "Best,\nYour Name"
send_command = "msmtp -t"

[accounts.work]
email = "you@work.com"
maildir = "~/Mail/work"
send_command = "msmtp -a work -t"

[layout]
list_focused_width = 66
date_width = 14
from_width = 18

[compose]
signature_on_reply = true
```

## External Dependencies

The application shells out to these external tools:

- `mbsync` - Email sync (optional, for R to resync)
- `msmtp` or `sendmail` - Sending email
- `ripgrep` (`rg`) - Deep body search
- `w3m` - HTML to text conversion for rendering
- `yazi` - File manager for attachments (optional)

## Development Notes

### Building

```bash
cargo build --release
```

### Running

```bash
cargo run
# or
./target/release/mailtui
```

### Key Bindings

Key handling is in `src/main.rs`. The app has multiple modes (normal, search, compose, etc.) with different key bindings per mode.

### Key Features

- Threaded email view with tree prefixes
- Inline image rendering (Kitty graphics protocol)
- Attachment display and download
- In-memory fast search + ripgrep deep search
- Multiple account support (Tab to switch)
- Compose with attachments
- Read/unread via maildir flags
