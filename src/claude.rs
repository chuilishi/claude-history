//! Minimal, borrow-friendly view of a Claude Code JSONL log line.
//!
//! Only the fields the history list needs are deserialized. Everything else
//! (tool inputs, tool results, attachments, ...) is skipped by serde without
//! allocating, which matters because those payloads are most of the bytes.

use serde::Deserialize;
use serde::de::{self, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use std::borrow::Cow;
use std::fmt;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry<'a> {
    #[serde(rename = "type", borrow)]
    pub kind: Cow<'a, str>,
    #[serde(default, borrow)]
    pub subtype: Option<Cow<'a, str>>,
    #[serde(default)]
    pub message: Option<Message>,
    #[serde(default, borrow)]
    pub timestamp: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub cwd: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub git_branch: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub entrypoint: Option<Cow<'a, str>>,
    #[serde(default)]
    pub is_meta: Option<bool>,
    #[serde(default)]
    pub is_compact_summary: Option<bool>,
    /// `summary` records (older Claude Code versions)
    #[serde(default, borrow)]
    pub summary: Option<Cow<'a, str>>,
    /// Text of `system` records such as `away_summary`
    #[serde(default)]
    pub content: Option<LenientText>,
    #[serde(default, borrow)]
    pub ai_title: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub custom_title: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub last_prompt: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub permission_mode: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub agent_name: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub continued_in_session_id: Option<Cow<'a, str>>,
    /// `tools-used` records
    #[serde(default, borrow)]
    pub tools: Option<Vec<Cow<'a, str>>>,
    /// `cost-state` records
    #[serde(default)]
    pub total_lines_added: Option<u64>,
    #[serde(default)]
    pub total_lines_removed: Option<u64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub content: Option<Content>,
    #[serde(default)]
    pub model: Option<String>,
}

/// A string, or `None` for any other JSON value (which is skipped).
#[derive(Debug, Default)]
pub struct LenientText(pub Option<String>);

impl<'de> Deserialize<'de> for LenientText {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let block = d.deserialize_any(BlockVisitor { lenient_str: true })?;
        Ok(LenientText(block.0))
    }
}

/// Text extracted from `message.content`, which is either a plain string or
/// an array of blocks of which only `{"type":"text","text":...}` are kept.
#[derive(Debug, Default)]
pub struct Content(pub String);

impl<'de> Deserialize<'de> for Content {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(ContentVisitor)
    }
}

struct ContentVisitor;

impl<'de> Visitor<'de> for ContentVisitor {
    type Value = Content;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a string or an array of content blocks")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Content, E> {
        Ok(Content(v.to_owned()))
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Content, E> {
        Ok(Content(v))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Content, A::Error> {
        let mut out = String::new();
        while let Some(block) = seq.next_element::<Block>()? {
            if let Some(text) = block.0 {
                if !out.is_empty() {
                    out.push(' ');
                }
                out.push_str(&text);
            }
        }
        Ok(Content(out))
    }
}

/// A content block; holds its text only when it is a `text` block.
struct Block(Option<String>);

impl<'de> Deserialize<'de> for Block {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(BlockVisitor { lenient_str: false })
    }
}

/// Visits a content block, keeping its text only when it is a `text` block.
/// With `lenient_str` a bare string is kept too (see [`LenientText`]); maps
/// are then not expected but handled the same way.
struct BlockVisitor {
    lenient_str: bool,
}

impl<'de> Visitor<'de> for BlockVisitor {
    type Value = Block;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a content block")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Block, A::Error> {
        let mut is_text = false;
        let mut text: Option<String> = None;
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            match key.as_ref() {
                "type" => is_text = map.next_value::<Cow<'de, str>>()? == "text",
                "text" => text = map.next_value::<Option<String>>()?,
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(Block(if is_text { text } else { None }))
    }

    // Anything that isn't an object (shouldn't happen) is ignored.
    fn visit_str<E: de::Error>(self, v: &str) -> Result<Block, E> {
        Ok(Block(self.lenient_str.then(|| v.to_owned())))
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Block, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(Block(None))
    }
    fn visit_unit<E: de::Error>(self) -> Result<Block, E> {
        Ok(Block(None))
    }
    fn visit_bool<E: de::Error>(self, _: bool) -> Result<Block, E> {
        Ok(Block(None))
    }
    fn visit_i64<E: de::Error>(self, _: i64) -> Result<Block, E> {
        Ok(Block(None))
    }
    fn visit_u64<E: de::Error>(self, _: u64) -> Result<Block, E> {
        Ok(Block(None))
    }
    fn visit_f64<E: de::Error>(self, _: f64) -> Result<Block, E> {
        Ok(Block(None))
    }
}
