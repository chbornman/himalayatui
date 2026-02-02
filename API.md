# Himalaya API Usage

This document tracks which parts of the himalaya CLI API are used by himalayatui.

## Summary

himalayatui uses roughly **30% of himalaya's CLI capabilities**, focusing on the core read/search/send workflow while omitting organization features (folders, move, delete), advanced composition (templates, MML), and email management (forward, copy, archive).

## Features Being Used

| Feature | Command | Location |
|---------|---------|----------|
| Account list | `himalaya account list --output json` | `src/himalaya/client.rs:6-13` |
| Envelope list | `himalaya envelope list --output json --page-size 500` | `src/himalaya/client.rs:42-56` |
| Envelope search | `himalaya envelope list <query>` | `src/himalaya/client.rs:198-229` |
| Message read | `himalaya message read <id>` | `src/himalaya/client.rs:58-76` |
| Message send | `himalaya message send` (stdin) | `src/main.rs:615-621` |
| Flag add | `himalaya flag add <id> seen` | `src/himalaya/client.rs:154-160` |
| Flag remove | `himalaya flag remove <id> seen` | `src/himalaya/client.rs:163-168` |
| Attachment download | `himalaya attachment download <id>` | `src/main.rs:640-668` |
| Folder list | `himalaya folder list --output json` | `src/himalaya/client.rs:179-196` |

Note: Folder list exists in code but is not exposed in the UI.

## Features NOT Being Used

### Folder Management

| Feature | Command | Notes |
|---------|---------|-------|
| Folder switching | `--folder` flag | Hardcoded to Inbox only |
| Folder add/create | `himalaya folder add` | Not implemented |
| Folder expunge | `himalaya folder expunge` | Not implemented |
| Folder purge | `himalaya folder purge` | Not implemented |
| Folder delete | `himalaya folder delete` | Not implemented |

### Message Operations

| Feature | Command | Notes |
|---------|---------|-------|
| Message delete | `himalaya message delete` | No deletion support |
| Message move | `himalaya message move` | No moving/archiving |
| Message copy | `himalaya message copy` | Not implemented |
| Message forward | `himalaya message forward` | No forwarding |
| Message save | `himalaya message save` | No draft saving |
| Message thread | `himalaya message thread` | No threading view |
| Message export | `himalaya message export` | Not implemented |
| Message edit | `himalaya message edit` | Not implemented |
| Message mailto | `himalaya message mailto` | Not implemented |

### Flag Operations

| Feature | Command | Notes |
|---------|---------|-------|
| Flag set | `himalaya flag set` | Uses add/remove instead |
| Flagged | `flagged` flag | Not implemented (starred) |
| Answered | `answered` flag | Not implemented |
| Deleted | `deleted` flag | Not implemented |
| Draft | `draft` flag | Not implemented |

Only the `seen` flag is currently used.

### Template System

| Feature | Command | Notes |
|---------|---------|-------|
| Template write | `himalaya template write` | Not using template system |
| Template reply | `himalaya template reply` | Not using template system |
| Template forward | `himalaya template forward` | Not using template system |
| Template save | `himalaya template save` | Not using template system |
| Template send | `himalaya template send` | Not using template system |

### Other Features

| Feature | Notes |
|---------|-------|
| Multiple accounts | Only loads default account |
| PGP encryption | Not implemented |
| MML (MIME Meta Language) | Not using for attachments in compose |
| Envelope thread | Not using threaded view |
| Account configure | Assumes pre-configured |
| Account doctor | Not used |

## Integration Notes

### Architecture

- himalayatui wraps himalaya CLI via `std::process::Command` subprocess calls
- Uses JSON output format (`--output json`) for structured data parsing
- Parses JSON responses into Rust structs using serde

### Hybrid Approach

- For deep body search, uses `ripgrep` directly on maildir files, then looks up himalaya IDs
- For reading messages with non-numeric IDs (from deep search), reads maildir files directly
- HTML rendering uses `w3m` for conversion to text

### Configuration

- Has its own config file (`~/.config/himalayatui/config.toml`) for UI settings
- Reads himalaya's config (`~/.config/himalaya/config.toml`) to extract email address

### Limitations

1. Single folder (Inbox) only - no folder navigation
2. No message deletion, moving, or archiving
3. No draft saving
4. No message forwarding
5. No threading view
6. No flagging (starred) support
7. Compose doesn't use himalaya's template/MML system
8. Attachment downloads only work with numeric himalaya IDs (not maildir IDs)
