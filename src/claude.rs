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
pub struct LogEntry<'a> {
    #[serde(rename = "type", borrow)]
    pub kind: Cow<'a, str>,
    #[serde(default)]
    pub message: Option<Message>,
    #[serde(default, borrow)]
    pub timestamp: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub cwd: Option<Cow<'a, str>>,
    #[serde(default, borrow)]
    pub summary: Option<Cow<'a, str>>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub content: Option<Content>,
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
        d.deserialize_any(BlockVisitor)
    }
}

struct BlockVisitor;

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
    fn visit_str<E: de::Error>(self, _: &str) -> Result<Block, E> {
        Ok(Block(None))
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
