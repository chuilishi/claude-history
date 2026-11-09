# claude-history

[![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)](#)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

[**中文文档**](README_CN.md)

A minimal, fast TUI for browsing and resuming [Claude Code](https://docs.anthropic.com/en/docs/claude-code) conversations.

Run `claude-history` anywhere → fuzzy-search across **all** projects → press Enter to resume right where you left off.

## Features

- **Global history** — searches every Claude Code conversation across all projects
- **Instant fuzzy search** — case-insensitive, prefix-matching, multi-word AND logic
- **One-key resume** — press Enter to jump straight back into Claude Code
- **Mouse support** — click to select, double-click to resume, scroll wheel to move
- **Hide conversations** — press Ctrl+D twice to hide a conversation from the list (Claude Code's files are untouched; remove its ID from `~/.claude-history/hidden` to restore)
- **Fast** — parallel loading with rayon, streaming results, precomputed search index
- **Cross-platform** — works on Windows, macOS, and Linux

## Install

macOS / Linux:

```sh
curl -fsSL https://raw.githubusercontent.com/chuilishi/claude-history/main/scripts/install.sh | bash
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/chuilishi/claude-history/main/scripts/install.ps1 | iex
```

Both download the prebuilt binary for your platform from the [releases page](https://github.com/chuilishi/claude-history/releases) and verify its checksum. Run the same command again to upgrade. Set `CLAUDE_HISTORY_VERSION` (e.g. `v0.1.30`) to pin a version and `CLAUDE_HISTORY_INSTALL_DIR` to choose where it goes.

With Rust installed you can also build from source:

```sh
cargo install claude-history-resume
```

The crate is named `claude-history-resume` (the upstream project owns `claude-history`); the installed command is `claude-history`.

## Usage

```sh
claude-history
```

## Ask Claude to find a conversation

`claude-history search <words>` and `claude-history show <id>` print results as
plain text. The `find-conversation` skill tells Claude to search with them
instead of reading raw logs, so you can ask "which session did we fix the proxy
thing in?" from any Claude Code session. Install it with:

```sh
mkdir -p ~/.claude/skills/find-conversation
cp skill/find-conversation/SKILL.md ~/.claude/skills/find-conversation/
```

## Credits

Forked from [raine/claude-history](https://github.com/raine/claude-history) and stripped down to a single-purpose resume launcher.

## License

MIT
