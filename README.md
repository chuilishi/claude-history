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
- **Fast** — parallel loading with rayon, streaming results, precomputed search index
- **Cross-platform** — works on Windows, macOS, and Linux

## Install

```sh
cargo install --path .
```

Or, if cloned from the repo:

```sh
cargo install --git https://github.com/chuilishi/claude-history
```

## Usage

```sh
claude-history
```

That's it. No flags, no arguments. A TUI opens with all your conversations listed by recency. Type to search, then:

| Key | Action |
|---|---|
| `↑` / `↓` | Move selection |
| `Ctrl+N` / `Ctrl+P` | Move selection (alt) |
| `Ctrl+D` / `Ctrl+U` | Half-page down / up |
| `Page Up` / `Page Down` | Jump by page |
| `Home` / `End` | Jump to first / last |
| `Enter` | Resume selected conversation |
| `Ctrl+W` | Delete word before cursor |
| `Esc` / `Ctrl+C` | Quit |

### Search

- **Case-insensitive**: `config` matches `CONFIG`
- **Underscore as separator**: `api key` matches `API_KEY`
- **Prefix matching**: `auth` matches `authentication`
- **Multi-word**: all words must match (AND logic)

## Configuration

Optional config at `~/.config/claude-history/config.toml`:

```toml
[resume]
# Arguments passed to `claude` when resuming
default_args = ["--dangerously-skip-permissions"]
```

## Credits

Forked from [raine/claude-history](https://github.com/raine/claude-history) and stripped down to a single-purpose resume launcher.

## License

MIT
