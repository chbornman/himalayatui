# AGENTS.md

This file provides context for AI agents working on this codebase.

## Project Overview

**himalayatui** is a Terminal User Interface (TUI) email client built with Rust. It uses [ratatui](https://github.com/ratatui-org/ratatui) for terminal rendering and acts as a graphical wrapper around the [himalaya](https://github.com/pimalaya/himalaya) CLI tool.

## Project Structure

```
himalayatui/
├── Cargo.toml           # Project dependencies (ratatui, crossterm, serde, etc.)
├── src/
│   ├── main.rs          # Entry point, event loop, key handling
│   ├── app.rs           # Application state management
│   ├── config.rs        # Configuration loading and theming
│   ├── himalaya/        # Himalaya CLI integration layer
│   │   ├── mod.rs       # Module exports
│   │   ├── client.rs    # Functions that invoke himalaya CLI
│   │   └── types.rs     # Data types (Envelope, Account, Address)
│   └── ui/              # UI rendering components
│       ├── mod.rs
│       ├── envelopes.rs # Email list rendering
│       ├── reader.rs    # Message preview rendering
│       ├── compose.rs   # Compose view rendering
│       └── help.rs      # Help bar rendering
├── API.md               # Documents himalaya API usage
└── AGENTS.md            # This file
```

## Key Concepts

### Himalaya Integration

This project does NOT use himalaya as a Rust library. Instead, it invokes the `himalaya` CLI as a subprocess and parses JSON output. All himalaya interactions are in `src/himalaya/client.rs`.

See `API.md` for a complete breakdown of which himalaya features are used vs available.

### Maildir Backend

The project is designed for use with a Maildir backend (local email storage synced from Gmail via mbsync). Some features like deep search bypass himalaya entirely and use `ripgrep` directly on maildir files.

### Data Types

- `Envelope` - Email metadata (id, flags, subject, from, to, date, has_attachment)
- `Account` - Himalaya account info (name, backend, default)
- `Address` - Email address (name, addr)

These are defined in `src/himalaya/types.rs`.

## Configuration

Two config files are relevant:

1. **himalayatui config**: `~/.config/himalayatui/config.toml` - UI settings and theming
2. **himalaya config**: `~/.config/himalaya/config.toml` - Email account configuration (read-only access to get email address)

## External Dependencies

The application shells out to these external tools:

- `himalaya` - Email operations (required)
- `ripgrep` (`rg`) - Deep body search
- `w3m` - HTML to text conversion for rendering

## Development Notes

### Building

```bash
cargo build --release
```

### Running

```bash
cargo run
# or
./target/release/himalayatui
```

### Key Bindings

Key handling is in `src/main.rs`. The app has multiple modes (normal, search, compose, etc.) with different key bindings per mode.

### Adding New Himalaya Features

To add a new himalaya CLI feature:

1. Add the function in `src/himalaya/client.rs`
2. Add any new types to `src/himalaya/types.rs`
3. Call from `src/main.rs` or `src/app.rs` as appropriate
4. Update `API.md` to document the new usage

### Known Limitations

- Single folder (Inbox) only - no folder navigation
- No message deletion, moving, or archiving
- No draft saving
- No message forwarding
- No threading view
- Attachment downloads only work with numeric himalaya IDs
