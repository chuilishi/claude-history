---
name: find-conversation
description: Find a past Claude Code conversation from a vague memory ("那次我们聊过…", "which session did we fix X in", "找一下之前那个对话"). Searches the local history index with `claude-history search` instead of reading raw session logs. Use when the user wants to locate, recall or resume an earlier conversation and does not remember exact keywords.
---

# Find a past conversation

`claude-history` keeps an index of every Claude Code session on this machine.
Query it; never grep or read `~/.claude/projects/**/*.jsonl` directly — they
are huge. The index has four tiers, and each snippet is labeled with its tier:

- `[title]` title, every recap (Claude's "while you were away" summaries),
  branch, model
- `[user]` what the user typed, including messages sent mid-task and answers
  to AskUserQuestion
- `[reply]` Claude's replies and text the user pasted
- `[tool]` commands run, file paths read/written, search queries, URLs,
  subagent tasks and reports, background task results, errors, plan files

Not indexed: file contents and successful tool output. If the user remembers
only something that appeared there, say the index can't see it.

## Commands

```sh
claude-history search <words...> [--any] [-p <project>] [-t <tool>] [-n 15] [--snippets 2]
claude-history show <id-prefix> [words...] [--snippets 6]
```

- Words are case-insensitive **prefix** matches (`cach` hits `cache`, `caching`).
  Chinese words match anywhere. Default is AND; `--any` is OR.
- Results rank by relevance (BM25): rare words count more than common ones,
  matches in higher tiers count more, and long conversations get no bonus for
  merely containing more text.
- `-t chrome-devtools` keeps only conversations that actually called a tool
  whose name contains that text; `-p` does the same for the project path.
- The session running the search is left out automatically.
- A file name, command or URL the user remembers is a strong term: it hits
  the `[tool]` tier even when nobody wrote it in a message.
- Each result: session id, last-activity time, project path, title, message
  count, branch, and snippets around the matches.
- `show` prints the opening, the last prompt, a **timeline of recaps** (what
  a long conversation moved through, with times), the **sites** it visited
  (domains, most visited first), the tools it used, snippets for the given
  words, and the exact resume command. A long conversation's title only
  describes its start; the timeline and sites show the rest.
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
4. Confirm the best 1-3 candidates with `show <id> <words>` before answering,
   passing the details the user remembers as `words`.

## Answer

List the candidates (normally 1-3), each with title, date, project and the
resume command from `show`. For each, match it against the **specific details
the user described**: say which part of the conversation (a timeline entry, a
site, a snippet) corresponds to each detail. A long conversation's title and
opening are often about something else, so a bare title makes a right answer
look wrong.

```
cd "<project path>" && claude --resume <session-id>
```

Say how confident you are. If nothing fits after a few rounds, report the
terms you tried and ask for any other detail the user remembers.
