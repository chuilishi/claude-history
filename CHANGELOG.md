# Changelog

## Unreleased

- Search covers the start of every tool result (command output, fetched
  pages, files read), not just failed ones, and the full transcripts of
  subagents, which count toward their parent conversation. Boilerplate
  results (file edits, tool lookups) are left out. All of it ranks in the
  lowest tier, so the TUI order barely changes
- The cache roughly doubles (about 55 MB for 4.5 GB of logs) and a full
  rebuild takes a few seconds longer

## v0.1.32 (2026-11-09)

- `show` lists a timeline of the conversation's recaps, the sites it visited
  and the tools it used, so a long conversation shows more than its opening
- `search` ranks by relevance (BM25): rare words weigh more, and long
  conversations no longer win just by containing more words
- `search --tool <name>` keeps conversations that called a matching tool
- `search` leaves out the Claude Code session running it
  (`$CLAUDE_CODE_SESSION_ID`); `--include-current` brings it back
- Every recap is searchable at title rank, not just the latest
- Tool lookups (`ToolSearch`) are no longer indexed as tool use

## v0.1.31 (2026-11-09)

- Search also covers messages sent while Claude was working (they were
  missing), answers to Claude's questions, commands run, file paths, search
  queries, URLs, subagent reports, background task results, errors and plans
- Results are ranked by where the words match: title and recap first, then
  what you typed, then Claude's replies and pasted text, then tool activity;
  newest first within each. `search` labels each snippet with its tier
- Background task notifications no longer count as messages or show in the
  preview
- `install.ps1` retries each download up to three times, since proxies and
  GitHub's CDN fail transiently (504)

## v0.1.30 (2026-11-09)

- `claude-history search <words...>` and `claude-history show <id>` print
  matching conversations and their details as plain text, for scripts and
  for Claude itself. `skill/find-conversation` is a Claude Code skill that
  uses them to find a past conversation from a vague description
- Added `claude-history search` and `claude-history show`, which print
  matches as plain text, and a `find-conversation` skill (in `skill/`) that
  lets Claude use them to find a conversation from a vague description
- Added `--version`. The install script calls it to check the installed
  binary, so installing through `install.sh` used to end with "binary exists
  but failed to run", and upgrades showed the old version as `unknown`
- Windows install script: `irm .../scripts/install.ps1 | iex` downloads the
  binary, verifies its checksum, adds it to the user PATH and upgrades an
  existing install. Works in Windows PowerShell 5.1 and PowerShell 7
- Both install scripts download from `releases/latest/download` instead of
  asking the GitHub API for the latest version, whose 60 requests/hour limit
  per IP is easily used up behind shared proxies

## v0.1.29 (2026-11-09)

- The session cost is no longer shown in the header
- The recap on the preview line is told apart by its color only; the `↻`
  marker is gone

## v0.1.28 (2026-11-09)

- Conversations show their title again: the `/rename` title, else the title
  Claude Code generates (`ai-title`), else an old-style `summary`. Newer
  Claude Code versions stopped writing `summary`, so titles had gone blank
- The header shows the git branch (`project@branch`), the model that
  answered last, the session cost, lines added/removed, and tags for
  sessions continued elsewhere (`↪ continued`) or started via the SDK
  (`sdk`). Less important parts are dropped first on narrow terminals
- The preview line shows Claude's "while you were away" recap when there is
  one, otherwise the first messages followed by `» latest prompt`
- Search also matches the title, branch, model, recap, tools and subagents
  used (e.g. `figma`, `bash`)
- Resumed sessions keep their last permission mode (`--permission-mode`)
- Context injected by Claude Code (`isMeta`) and compaction summaries are no
  longer shown or counted as messages
- Bulky log lines (API request records, file history, queue operations) are
  skipped before JSON parsing
- Layout and the search cursor account for double-width CJK characters

## v0.1.27 (2026-11-09)

- Resumed conversations are launched with `--allow-dangerously-skip-permissions`,
  so bypass permissions mode is always available in the `Shift+Tab` cycle
- Much faster startup: parse results are cached in `~/.claude-history/cache.bin`,
  so unchanged conversations are not read again and growing ones are only
  parsed from where the last run stopped. The cache is only rewritten when
  something changed, and a log file that was rewritten rather than appended to
  is detected and parsed from scratch. Parsing itself also skips tool
  inputs/outputs without allocating
- Search works for Chinese/Japanese/Korean text: a CJK query word matches
  anywhere, and a word right after CJK text or punctuation counts as a word
  start (e.g. `cache` now matches `src/history/cache.rs`). Highlighting
  follows the same rules
- Conversations are sorted by the time of their last message instead of the
  file's modification time, which changes when files are copied or synced
- Windows: `claude` installed via npm (`claude.cmd`) is now found on `PATH`
- Git worktree names (`project/worktree`) are shown on Windows too
- User messages that merely mention tags like `<command-args>` are no longer
  dropped; only slash-command wrappers and their output are skipped
- The screen is only redrawn when something changes

## v0.1.26 (2026-10-06)

- Added `Ctrl+D` (press twice) to hide a conversation from the list. Hidden
  session IDs are stored in `~/.claude-history/hidden`; Claude Code's own files
  are never modified
- Added mouse support: click to select, double-click to resume, scroll wheel to
  move the selection

## v0.1.25 (2026-02-11)

- Added `--show-id` (`-i`) flag to print the selected conversation's session ID,
  useful for resuming with custom shell aliases (e.g.,
  `claude --resume $(claude-history --show-id)`)
- Added `I` keybinding in the viewer to copy the session ID to clipboard

## v0.1.24 (2026-02-11)

- Tool calls now default to **truncated** mode, showing the header and first few
  lines with a "(N more lines...)" indicator — a middle ground between hidden
  and full output. Press `t` to cycle through modes: off, truncated, full
- Added `--no-tools` flag to start with tools hidden (complements `--show-tools`
  for full mode)
- Tables in conversation output are now rendered with proper box-drawing borders
  instead of being collapsed into plain text

## v0.1.23 (2026-02-08)

- Fixed blank or empty message blocks occasionally appearing in conversation
  output

## v0.1.22 (2026-02-07)

- Added multi-word search support in the viewer — search for phrases like "add
  feature" to find matches containing both words
- Timestamps now display on tool calls and results in ledger view (when timing
  is enabled with `i`)
- Fixed a crash that could occur when highlighting search matches containing
  certain Unicode characters

## v0.1.21 (2026-02-05)

- Fixed timestamp alignment for subagent messages and empty messages in ledger
  view
- Fixed double blank lines appearing after tool calls with empty output
- `/clear` commands are no longer shown in conversation rendering

## v0.1.20 (2026-02-05)

- Added toggleable timing display in conversation viewer — press `i` to show
  timestamps next to each message
- Show conversation duration and model/token count in the viewer header
- Show conversation duration in the conversation list
- Added keyboard shortcuts help overlay — press `?` in any view
- Added keyboard shortcuts bar at the bottom of the conversation list
- Added `Ctrl+R` (resume) and `Ctrl+X` (delete) shortcuts to the viewer status
  bar
- Added `Ctrl+C` to quit from viewer mode
- Exports now include thinking blocks and tool calls when their display is
  toggled on
- Long bash commands in tool calls are now wrapped for readability
- Improved search highlight color for better visibility

## v0.1.19 (2026-02-04)

- Added syntax highlighting for code blocks in conversation output
- Improved tool call display with human-readable formatting instead of raw JSON
- Added Vim-style half-page navigation (Ctrl-D/Ctrl-U) in the viewer
- Added Ctrl-W to delete word before cursor in the search field
- Show conversation summary in the viewer header and search results
- Display subagent conversations in ledger view
- Added direct JSONL file input support (pass a file path as argument)
- Added `--render` flag for debugging ledger output
- Improved header layout: combined into single line when terminal width allows
- Tool/thinking toggle settings now persist within session

## v0.1.18 (2026-02-02)

- Added in-TUI conversation viewer. Press Enter to view conversations without
  leaving the TUI, with Vim-style navigation (j/k, d/u, g/G) and search (/)
- Added export and yank menus to the viewer. Press `e` to export to file or `y`
  to copy to clipboard in multiple formats (ledger, plain text, markdown, JSONL)
- Added `Y` hotkey to copy the conversation file path to clipboard
- Added `resume.default_args` config option to pass custom arguments when
  resuming conversations with `Ctrl+R`
- Improved markdown rendering: fixed spacing after numbered lists, styled
  headings with subtle color
- Fixed thinking blocks to render with italic and dimmed style
- Fixed user messages showing in wrong color in the viewer
- Improved search performance

## v0.1.17 (2026-02-01)

- Added `Ctrl+R` keybinding to resume the selected conversation directly from
  the TUI

## v0.1.16 (2026-02-01)

- Fixed a crash when using global search (`-g`) that could occur when deleting
  conversations

## v0.1.15 (2026-02-01)

- Added ability to delete conversations from the TUI (press `Ctrl+D`, confirm
  with `y`)
- Added cursor navigation in the search field with arrow keys

## v0.1.14 (2026-02-01)

- Added markdown rendering for conversation output with support for headings,
  lists, code blocks, tables, and inline formatting
- Added pager support—long conversations now open in `less` (or `$PAGER`)
- Added `--plain` flag for unformatted output
- Improved search to better match word variations (e.g., "config" now matches
  "configuration")
- Added `curl | bash` install script
- Hide caveat metadata from conversation previews

## v0.1.13 (2026-02-01)

- Replaced fzf with a built-in terminal UI

## v0.1.12 (2026-01-11)

- Fixed project path detection failing for usernames containing dots (e.g.,
  `my.user`) (Thanks @duke8585!)

## v0.1.11 (2025-12-20)

- Cleaned up fzf picker display by removing index numbers

## v0.1.10 (2025-12-15)

- Added a specific error message when fzf version is too old (requires 0.67.0+)

## v0.1.9 (2025-12-14)

- Added color highlighting to the fzf picker

## v0.1.8 (2025-12-14)

- Improved fzf UX: the timestamp stays visible when searching

## v0.1.7 (2025-12-14)

- Added `--global` (`-g`) flag to search conversations across all projects at
  once

## v0.1.6 (2025-11-29)

- Added `--all-projects` (`-a`) flag to browse conversations from any project
- Added `--show-path` (`-p`) flag to print the selected conversation's file path
- Improved fuzzy search to match against full conversation content
- Added Homebrew installation support

## v0.1.5 (2025-11-17)

- Added display of tool call inputs and results when viewing conversations
- Fixed project detection for paths containing dots or special characters

## v0.1.4 (2025-10-30)

- Added faster startup with parallel conversation loading

## v0.1.3 (2025-10-30)

- Added `--debug` flag to show diagnostic information about conversation loading
- Fixed conversations containing only `/clear` commands incorrectly appearing in
  the list
- Cleaned up `/clear` command metadata from conversation previews
- Used file modification time for more accurate conversation dates

## v0.1.2 (2025-10-29)

- Fixed display of tool results that contain structured content instead of plain
  text

## v0.1.1 (2025-10-29)

- Added configuration file support (`~/.config/claude-history/config.toml`) for
  persistent display preferences
- Added `--show-thinking` and `--hide-thinking` flags to control visibility of
  Claude's thinking blocks
- Hidden tool calls by default (use `--show-tools` or `-t` to show them)
- Added `--first` flag to show first messages in preview (inverse of `--last`)
- Added `--absolute-time` flag to explicitly use timestamps (inverse of
  `--relative-time`)
- Fixed message preview order when using `--last` flag

## v0.1.0 (2025-10-29)

- Initial release
