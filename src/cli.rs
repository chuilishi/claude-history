//! Non-interactive `search` and `show`, meant for scripts and for Claude
//! itself (see `skill/`): plain text out, read from the same cache the TUI uses.

use crate::error::{AppError, Result};
use crate::history::{self, Conversation, LoaderMessage, hidden};
use crate::search::{self, matches_word_start, word_start_matches};

/// Characters of context shown on each side of a match
const SNIPPET_CONTEXT: usize = 50;

pub struct SearchOptions {
    pub words: Vec<String>,
    pub any: bool,
    pub project: Option<String>,
    pub limit: usize,
    pub snippets: usize,
}

pub fn search(opts: SearchOptions) -> Result<()> {
    let words = search::query_words(&opts.words.join(" "));
    if words.is_empty() {
        return Err(AppError::Usage("give at least one search word".into()));
    }
    let project = opts.project.as_deref().map(|p| p.to_lowercase());

    // (distinct words matched, conversation); newest first within a score
    let mut hits: Vec<(usize, Conversation)> = load_sorted()?
        .into_iter()
        .filter(|c| {
            project
                .as_deref()
                .is_none_or(|p| c.project_path.to_string_lossy().to_lowercase().contains(p))
        })
        .filter_map(|c| {
            let score = words
                .iter()
                .filter(|w| matches_word_start(&c.search_text, w))
                .count();
            let ok = if opts.any {
                score > 0
            } else {
                score == words.len()
            };
            ok.then_some((score, c))
        })
        .collect();
    hits.sort_by_key(|(score, _)| std::cmp::Reverse(*score));

    let total = hits.len();
    for (score, conv) in hits.iter().take(opts.limit) {
        print_header(conv);
        if opts.any {
            println!("  matched {}/{} words", score, words.len());
        }
        for snippet in snippets(&conv.search_text, &words, opts.snippets) {
            println!("  … {} …", snippet);
        }
        println!();
    }
    if total > opts.limit {
        println!(
            "({} more not shown; add words, use --project, or raise --limit)",
            total - opts.limit
        );
    } else if total == 0 {
        println!("no matches");
    }
    Ok(())
}

pub fn show(id: &str, words: &[String], snippet_count: usize) -> Result<()> {
    let convs = load_sorted()?;
    let found: Vec<&Conversation> = convs
        .iter()
        .filter(|c| hidden::session_id(&c.path).is_some_and(|s| s.starts_with(id)))
        .collect();
    let conv = match found.as_slice() {
        [one] => *one,
        [] => return Err(AppError::Usage(format!("no conversation with id {id}"))),
        _ => return Err(AppError::Usage(format!("id prefix {id} is ambiguous"))),
    };

    print_header(conv);
    println!("  file:     {}", conv.path.display());
    if let Some(m) = &conv.model {
        println!("  model:    {m}");
    }
    if let Some(d) = conv.duration_minutes {
        println!("  duration: {d} min");
    }
    println!("  start:    {}", one_line(&conv.preview));
    if let Some(p) = &conv.last_prompt {
        println!("  last ask: {}", one_line(p));
    }
    if let Some(r) = &conv.recap {
        println!("  recap:    {}", one_line(r));
    }
    let words = search::query_words(&words.join(" "));
    for snippet in snippets(&conv.search_text, &words, snippet_count) {
        println!("  … {} …", snippet);
    }
    println!(
        "  resume:   cd \"{}\" && claude --resume {}",
        conv.project_path.display(),
        hidden::session_id(&conv.path).unwrap_or_default()
    );
    Ok(())
}

fn print_header(conv: &Conversation) {
    let id = hidden::session_id(&conv.path).unwrap_or_default();
    println!(
        "{}  {}  {}",
        id,
        conv.timestamp.format("%Y-%m-%d %H:%M"),
        conv.project_path.display()
    );
    let title = conv
        .title
        .as_deref()
        .map(one_line)
        .unwrap_or_else(|| one_line(&conv.preview));
    let branch = conv
        .git_branch
        .as_deref()
        .map(|b| format!("  [{b}]"))
        .unwrap_or_default();
    println!("  {}  ({} msgs){}", title, conv.message_count, branch);
}

/// Text around the first matches, one word after another, without overlap
fn snippets(text: &str, words: &[String], max: usize) -> Vec<String> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    'words: for word in words.iter().cycle().take(words.len() * max) {
        if spans.len() >= max {
            break;
        }
        for start in word_start_matches(text, word) {
            if spans.iter().any(|&(s, e)| start >= s && start < e) {
                continue;
            }
            let from = back_chars(text, start, SNIPPET_CONTEXT);
            let to = forward_chars(text, start + word.len(), SNIPPET_CONTEXT);
            spans.push((from, to));
            continue 'words;
        }
    }
    spans.sort();
    spans
        .into_iter()
        .map(|(s, e)| text[s..e].to_string())
        .collect()
}

fn back_chars(text: &str, at: usize, n: usize) -> usize {
    text[..at]
        .char_indices()
        .rev()
        .nth(n.saturating_sub(1))
        .map_or(0, |(i, _)| i)
}

fn forward_chars(text: &str, at: usize, n: usize) -> usize {
    text[at..]
        .char_indices()
        .nth(n)
        .map_or(text.len(), |(i, _)| at + i)
}

fn one_line(s: &str) -> String {
    let s: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    match s.char_indices().nth(160) {
        Some((i, _)) => format!("{}…", &s[..i]),
        None => s,
    }
}

fn load_sorted() -> Result<Vec<Conversation>> {
    let rx = history::load_all_conversations_streaming();
    let mut convs = Vec::new();
    for msg in rx {
        match msg {
            LoaderMessage::Batch(batch) => convs.extend(batch),
            LoaderMessage::Fatal(e) => return Err(e),
            LoaderMessage::Done => break,
        }
    }
    convs.sort_by_key(|c| std::cmp::Reverse(c.timestamp));
    Ok(convs)
}
