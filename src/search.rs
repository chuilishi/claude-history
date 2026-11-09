//! Text matching, shared by indexing (parser), filtering and highlighting so
//! that all three agree on what a match is.
//!
//! A query word matches where it starts a word: at the start of the text,
//! after any character that is not a letter or digit, or after a CJK
//! character (CJK text has no spaces between words). A query word that itself
//! starts with a CJK character matches anywhere.

use crate::history::Conversation;
use rayon::prelude::*;

/// Fold a character for case-insensitive matching. Always one char out per
/// char in, so positions in folded and original text line up.
pub fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

/// Append the words of `s` to `out`, folded and separated by single spaces.
pub fn push_folded(out: &mut String, s: &str) {
    for word in s.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.extend(word.chars().map(fold));
    }
}

/// Separator for splitting the query and for word-wise editing
pub fn is_word_separator(c: char) -> bool {
    c.is_whitespace() || c == '_'
}

/// Folded words of a query
pub fn query_words(query: &str) -> Vec<String> {
    query
        .split(is_word_separator)
        .filter(|w| !w.is_empty())
        .map(|w| w.chars().map(fold).collect())
        .collect()
}

/// Names of the tiers of `Conversation::search_text`, most important first
pub const TIER_NAMES: [&str; 4] = ["title", "user", "reply", "tool"];

/// Tier of `conv.search_text` that byte offset `pos` falls in
pub fn tier_at(conv: &Conversation, pos: usize) -> usize {
    conv.tier_starts
        .iter()
        .filter(|&&start| pos >= start)
        .count()
}

/// Most important tier `word` matches in, or `None` if it does not match.
/// Matches come in text order and tiers are laid out in order of
/// importance, so the first match decides.
pub fn best_tier(conv: &Conversation, word: &str) -> Option<usize> {
    if word.is_empty() {
        return Some(0);
    }
    word_start_matches(&conv.search_text, word)
        .next()
        .map(|pos| tier_at(conv, pos))
}

/// Rank of a conversation for a query: the least important of the tiers
/// its words best match in, i.e. how deep one has to look to find them all.
/// `None` if some word does not match.
pub fn rank(conv: &Conversation, words: &[String]) -> Option<usize> {
    words
        .iter()
        .map(|w| best_tier(conv, w))
        .try_fold(0, |worst, tier| Some(worst.max(tier?)))
}

/// Indices of the conversations that match every query word, more important
/// matches first (see [`rank`]), list order within a rank.
pub fn search(conversations: &[Conversation], words: &[String]) -> Vec<usize> {
    let mut hits: Vec<(usize, usize)> = conversations
        .par_iter()
        .enumerate()
        .filter_map(|(idx, conv)| Some((rank(conv, words)?, idx)))
        .collect();
    hits.sort_by_key(|&(rank, _)| rank);
    hits.into_iter().map(|(_, idx)| idx).collect()
}

/// True if `word` occurs in folded `text` at the start of a word.
#[cfg(test)]
fn matches_word_start(text: &str, word: &str) -> bool {
    word.is_empty() || word_start_matches(text, word).next().is_some()
}

/// Byte offsets in folded `text` where `word` matches at the start of a word.
pub fn word_start_matches<'a>(text: &'a str, word: &'a str) -> impl Iterator<Item = usize> + 'a {
    let first = word.chars().next();
    text.match_indices(word)
        .map(|(i, _)| i)
        .filter(move |&i| first.is_some_and(|f| can_start_match(text[..i].chars().next_back(), f)))
}

/// Whether a query word starting with `first` may match right after `prev`
/// (`None` at the start of the text).
pub fn can_start_match(prev: Option<char>, first: char) -> bool {
    is_cjk(first) || prev.is_none_or(|p| !p.is_alphanumeric() || is_cjk(p))
}

/// Han, kana, Hangul and CJK punctuation
fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x2E80..=0x2FDF      // radicals
        | 0x3000..=0x30FF    // CJK punctuation, hiragana, katakana
        | 0x3100..=0x31FF    // bopomofo, katakana extensions
        | 0x3400..=0x4DBF    // ext A
        | 0x4E00..=0x9FFF    // unified ideographs
        | 0xAC00..=0xD7AF    // Hangul
        | 0xF900..=0xFAFF    // compatibility ideographs
        | 0xFF00..=0xFFEF    // fullwidth forms
        | 0x20000..=0x3FFFF) // ext B+
}

#[cfg(test)]
mod tests {
    use super::*;

    fn matches(text: &str, query: &str) -> bool {
        let mut folded = String::new();
        push_folded(&mut folded, text);
        query_words(query)
            .iter()
            .all(|w| matches_word_start(&folded, w))
    }

    #[test]
    fn matches_word_prefixes_in_any_case() {
        assert!(matches("Hardened Runtime enabled", "harden runtime"));
        assert!(!matches("hardened security", "harden runtime"));
    }

    #[test]
    fn underscores_separate_words() {
        assert!(matches("HARDENED_RUNTIME config", "harden runtime"));
        assert!(matches("hardened runtime enabled", "hardened_runtime"));
    }

    #[test]
    fn matches_cjk_mid_sentence() {
        assert!(matches("这个项目的整体思路是对的吗", "整体 思路"));
        assert!(!matches("这个项目的整体思路是对的吗", "整体 缓存"));
    }

    #[test]
    fn matches_latin_word_after_cjk() {
        assert!(matches("修一下claude的缓存，用postcard格式", "claude post"));
        // Still a prefix match: no hit in the middle of a latin word
        assert!(!matches("修一下claude的缓存", "laude"));
    }

    #[test]
    fn matches_after_punctuation() {
        assert!(matches("see src/history/cache.rs", "cache"));
    }
}
