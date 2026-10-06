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
- **Hide conversations** — press Ctrl+D twice to hide a conversation from the list (Claude Code's files are untouched; remove its ID from `~/.claude-history/hidden` to restore)
- **Fast** — parallel loading with rayon, streaming results, precomputed search index
- **Cross-platform** — works on Windows, macOS, and Linux

## Install

```sh
cargo install --git https://github.com/chuilishi/claude-history
```

## Usage

```sh
claude-history
```

## Credits

Forked from [raine/claude-history](https://github.com/raine/claude-history) and stripped down to a single-purpose resume launcher.

## License

MIT
