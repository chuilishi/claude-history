//! Non-interactive `search` and `show`, meant for scripts and for Claude
//! itself (see `skill/`): plain text out, read from the same cache the TUI uses.

use crate::error::{AppError, Result};
use crate::history::{self, Conversation, LoaderMessage, hidden};
use crate::search::{self, TIER_NAMES, tier_at, word_start_matches};
use rayon::prelude::*;
use std::collections::HashMap;

/// Characters of context shown on each side of a match
const SNIPPET_CONTEXT: usize = 50;

/// Set by Claude Code for the commands it runs: the session doing the search
const SESSION_ENV: &str = "CLAUDE_CODE_SESSION_ID";

/// How much one match counts in each tier (title, user, reply, tool)
const TIER_WEIGHTS: [f64; 4] = [4.0, 2.0, 1.0, 0.5];
/// BM25 term-frequency saturation and length normalization
const BM25_K1: f64 = 1.2;
const BM25_B: f64 = 0.75;

/// Recap timeline entries, sites and tools listed by `show`
const SHOW_RECAPS: usize = 30;
const SHOW_SITES: usize = 10;
const SHOW_TOOLS: usize = 20;

pub struct SearchOptions {
    pub words: Vec<String>,
    pub any: bool,
    pub project: Option<String>,
    pub tool: Option<String>,
    pub include_current: bool,
    pub limit: usize,
    pub snippets: usize,
}

pub fn search(opts: SearchOptions) -> Result<()> {
    let words = search::query_words(&opts.words.join(" "));
    if words.is_empty() {
        return Err(AppError::Usage("give at least one search word".into()));
    }
    let project = opts.project.as_deref().map(str::to_lowercase);
    let tool = opts.tool.as_deref().map(str::to_lowercase);
    let current = std::env::var(SESSION_ENV)
        .ok()
        .filter(|_| !opts.include_current);

    let convs: Vec<Conversation> = load_sorted()?
        .into_iter()
        .filter(|c| current.as_deref() != hidden::session_id(&c.path))
        .filter(|c| {
            project
                .as_deref()
                .is_none_or(|p| c.project_path.to_string_lossy().to_lowercase().contains(p))
        })
        .filter(|c| {
            tool.as_deref()
                .is_none_or(|t| c.tools.iter().any(|name| name.to_lowercase().contains(t)))
        })
        .collect();

    // Weighted term frequency of every word in every conversation
    let tfs: Vec<Vec<f64>> = convs
        .par_iter()
        .map(|c| words.iter().map(|w| weighted_tf(c, w)).collect())
        .collect();
    let n = convs.len() as f64;
    let idf: Vec<f64> = (0..words.len())
        .map(|i| {
            let df = tfs.iter().filter(|tf| tf[i] > 0.0).count() as f64;
            (1.0 + (n - df + 0.5) / (df + 0.5)).ln()
        })
        .collect();
    let avg_len = convs.iter().map(|c| c.search_text.len()).sum::<usize>() as f64 / n.max(1.0);

    // (score, words matched, conversation); ties stay newest first
    let mut hits: Vec<(f64, usize, &Conversation)> = convs
        .iter()
        .zip(&tfs)
        .filter_map(|(c, tf)| {
            let matched = tf.iter().filter(|&&f| f > 0.0).count();
            let ok = if opts.any {
                matched > 0
            } else {
                matched == words.len()
            };
            if !ok {
                return None;
            }
            let norm = BM25_K1 * (1.0 - BM25_B + BM25_B * c.search_text.len() as f64 / avg_len);
            let score = tf
                .iter()
                .zip(&idf)
                .map(|(&f, idf)| idf * f * (BM25_K1 + 1.0) / (f + norm))
                .sum();
            Some((score, matched, c))
        })
        .collect();
    hits.sort_by(|a, b| b.0.total_cmp(&a.0));

    let total = hits.len();
    for &(_, matched, conv) in hits.iter().take(opts.limit) {
        print_header(conv);
        if opts.any {
            println!("  matched {}/{} words", matched, words.len());
        }
        for snippet in snippets(conv, &words, opts.snippets) {
            println!("  {snippet}");
        }
        println!();
    }
    if total > opts.limit {
        println!(
            "({} more not shown; add words, use --project/--tool, or raise --limit)",
            total - opts.limit
        );
    } else if total == 0 {
        println!("no matches");
    }
    Ok(())
}

/// Matches of `word` in `conv`, each counted by the weight of its tier
fn weighted_tf(conv: &Conversation, word: &str) -> f64 {
    word_start_matches(&conv.search_text, word)
        .map(|pos| TIER_WEIGHTS[tier_at(conv, pos)])
        .sum()
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

    if !conv.recaps.is_empty() {
        println!("  timeline (recaps):");
        let skip = conv.recaps.len().saturating_sub(SHOW_RECAPS);
        if skip > 0 {
            println!("    ({skip} earlier not shown)");
        }
        for (time, recap) in &conv.recaps[skip..] {
            let time = time.map_or_else(
                || "           ".into(),
                |t| t.format("%m-%d %H:%M").to_string(),
            );
            println!("    {time}  {}", one_line(recap));
        }
    }

    let sites = sites(conv);
    if !sites.is_empty() {
        let list: Vec<String> = sites
            .iter()
            .take(SHOW_SITES)
            .map(|(host, n)| format!("{host} ({n})"))
            .collect();
        println!("  sites:    {}", list.join(", "));
    }
    if !conv.tools.is_empty() {
        let mut list = conv.tools[..conv.tools.len().min(SHOW_TOOLS)].join(", ");
        if conv.tools.len() > SHOW_TOOLS {
            list.push_str(&format!(", … ({} more)", conv.tools.len() - SHOW_TOOLS));
        }
        println!("  tools:    {list}");
    }

    let words = search::query_words(&words.join(" "));
    for snippet in snippets(conv, &words, snippet_count) {
        println!("  {snippet}");
    }
    println!(
        "  resume:   cd \"{}\" && claude --resume {}",
        conv.project_path.display(),
        hidden::session_id(&conv.path).unwrap_or_default()
    );
    Ok(())
}

/// Hosts of URLs in the tool tier (pages Claude opened, fetched or
/// searched), most frequent first
fn sites(conv: &Conversation) -> Vec<(String, usize)> {
    let tools = &conv.search_text[conv.tier_starts[2]..];
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for (i, _) in tools.match_indices("://") {
        if !tools[..i].ends_with("http") && !tools[..i].ends_with("https") {
            continue;
        }
        let rest = &tools[i + 3..];
        let end = rest
            .find(|c: char| !(c.is_alphanumeric() || c == '.' || c == '-'))
            .unwrap_or(rest.len());
        let host = rest[..end].trim_start_matches("www.").trim_end_matches('.');
        let is_local =
            host == "localhost" || host.starts_with("127.") || host.starts_with("192.168.");
        if host.contains('.') && !is_local {
            *counts.entry(host).or_default() += 1;
        }
    }
    let mut sites: Vec<(String, usize)> = counts
        .into_iter()
        .map(|(host, n)| (host.to_owned(), n))
        .collect();
    sites.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    sites
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

/// Text around the first (most important) matches, one word after another,
/// without overlap, each labeled with its tier
fn snippets(conv: &Conversation, words: &[String], max: usize) -> Vec<String> {
    let text = conv.search_text.as_str();
    let mut spans: Vec<(usize, usize, usize)> = Vec::new();
    'words: for word in words.iter().cycle().take(words.len() * max) {
        if spans.len() >= max {
            break;
        }
        for start in word_start_matches(text, word) {
            if spans.iter().any(|&(s, e, _)| start >= s && start < e) {
                continue;
            }
            // Context stays within the match's tier
            let tier = tier_at(conv, start);
            let tier_start = tier.checked_sub(1).map_or(0, |t| conv.tier_starts[t]);
            let tier_end = conv.tier_starts.get(tier).copied().unwrap_or(text.len());
            let from = back_chars(text, start, SNIPPET_CONTEXT).max(tier_start);
            let to = forward_chars(text, start + word.len(), SNIPPET_CONTEXT).min(tier_end);
            spans.push((from, to, tier));
            continue 'words;
        }
    }
    spans.sort();
    spans
        .into_iter()
        .map(|(s, e, tier)| format!("[{}] … {} …", TIER_NAMES[tier], text[s..e].trim()))
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
