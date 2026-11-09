---
name: find-conversation
description: Find a past Claude Code conversation from a vague memory ("那次我们聊过…", "which session did we fix X in", "找一下之前那个对话"). Searches the local history index with `claude-history search` instead of reading raw session logs. Use when the user wants to locate, recall or resume an earlier conversation and does not remember exact keywords.
---

# Find a past conversation

`claude-history` keeps an index of every Claude Code session on this machine
(titles, recaps, every user/assistant message, tool names, branch). Query it;
never grep or read `~/.claude/projects/**/*.jsonl` directly — they are huge.

## Commands

```sh
claude-history search <words...> [--any] [-p <project-substring>] [-n 15] [--snippets 2]
claude-history show <id-prefix> [words...] [--snippets 6]
```

- Words are case-insensitive **prefix** matches (`cach` hits `cache`, `caching`).
  Chinese words match anywhere. Default is AND; `--any` is OR, ranked by how
  many words matched, then newest first.
- Each result: session id, last-activity time, project path, title, message
  count, branch, and snippets around the matches.
- `show` prints the opening, last prompt, recap and snippets for given words,
  plus the exact resume command.
- Searches take well under a second, so run many.

## Method

1. **Expand the memory into candidate terms** before searching. The user
   remembers the topic, not the wording. Think of:
   - Chinese and English forms (缓存 / cache, 代理 / proxy, 截图 / screenshot)
   - concrete artefacts that would appear in the log: file names, crate/package
     names, commands, error text, tool names (`mcp__figma`, `WebSearch`), URLs
   - the project or repo it was in (use `-p`)
2. **Start broad with `--any`** over 4-8 distinct terms, then narrow with AND
   on the terms that recur in good hits. If too many results, add a word or
   `-p`; if none, use shorter prefixes or other synonyms.
3. Use what the user remembers about **time** ("上周", "about a month ago") to
   discard hits by date; today's date is in your context.
4. **Ignore the current session** — it is usually the newest hit and contains
   your own search terms (its last prompt is the user's present request).
5. Confirm the best 1-3 candidates with `show <id> <words>` before answering.

## Answer

List the candidates (normally 1-3), each with title, date, project, one line
on why it matches, and the resume command from `show`:

```
cd "<project path>" && claude --resume <session-id>
```

Say how confident you are. If nothing fits after a few rounds, report the
terms you tried and ask for any other detail the user remembers.
