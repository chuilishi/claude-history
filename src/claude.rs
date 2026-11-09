//! Minimal, borrow-friendly view of a Claude Code JSONL log line.
//!
//! Only the fields the history list needs are deserialized. Everything else
//! (file contents, tool output, attachments, ...) is skipped by serde without
//! allocating, which matters because those payloads are most of the bytes.
//! Of tool calls and results only short, searchable parts are kept.

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
    /// Who sent a `user` entry: `human`, `task-notification`, `peer`, ...
    #[serde(default)]
    pub origin: Option<Origin>,
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
    /// `attachment` records
    #[serde(default)]
    pub attachment: Option<Attachment>,
    /// Structured tool result on `user` entries; only AskUserQuestion answers are kept
    #[serde(default)]
    pub tool_use_result: Option<Answers>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Message {
    #[serde(default)]
    pub content: Option<Content>,
    #[serde(default)]
    pub model: Option<String>,
}

/// Characters kept of each tool input field and of each failed tool result
pub const TOOL_FIELD_CHARS: usize = 300;
/// Characters kept of results worth more (subagent reports)
pub const TOOL_RESULT_CHARS: usize = 2000;
/// Characters kept of a plan file
pub const PLAN_CHARS: usize = 4000;

/// Tool input fields worth searching: what was run, read, searched or
/// delegated. Bulky fields (file contents, edits, scripts) are left out.
const TOOL_INPUT_KEYS: [&str; 11] = [
    "command",
    "file_path",
    "notebook_path",
    "path",
    "pattern",
    "glob",
    "url",
    "query",
    "description",
    "prompt",
    "skill",
];

/// Scalars and anything unexpected are skipped rather than failing the line.
macro_rules! ignore_other {
    ($value:ty, $empty:expr) => {
        fn visit_unit<E: de::Error>(self) -> Result<$value, E> {
            Ok($empty)
        }
        fn visit_bool<E: de::Error>(self, _: bool) -> Result<$value, E> {
            Ok($empty)
        }
        fn visit_i64<E: de::Error>(self, _: i64) -> Result<$value, E> {
            Ok($empty)
        }
        fn visit_u64<E: de::Error>(self, _: u64) -> Result<$value, E> {
            Ok($empty)
        }
        fn visit_f64<E: de::Error>(self, _: f64) -> Result<$value, E> {
            Ok($empty)
        }
    };
}

macro_rules! ignore_seq {
    ($value:ty, $empty:expr) => {
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<$value, A::Error> {
            while seq.next_element::<IgnoredAny>()?.is_some() {}
            Ok($empty)
        }
    };
}

macro_rules! ignore_map {
    ($value:ty, $empty:expr) => {
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<$value, A::Error> {
            while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
            Ok($empty)
        }
    };
}

macro_rules! ignore_str {
    ($value:ty, $empty:expr) => {
        fn visit_str<E: de::Error>(self, _: &str) -> Result<$value, E> {
            Ok($empty)
        }
    };
}

fn push_joined(out: &mut String, text: &str) {
    if text.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push(' ');
    }
    out.push_str(text);
}

/// The first `max` characters of `s`
pub fn capped(s: &str, max: usize) -> &str {
    s.char_indices().nth(max).map_or(s, |(i, _)| &s[..i])
}

/// A string, or `None` for any other JSON value (which is skipped).
#[derive(Debug, Default)]
pub struct LenientText(pub Option<String>);

impl<'de> Deserialize<'de> for LenientText {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(LenientTextVisitor)
    }
}

struct LenientTextVisitor;

impl<'de> Visitor<'de> for LenientTextVisitor {
    type Value = LenientText;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a string")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<LenientText, E> {
        Ok(LenientText(Some(v.to_owned())))
    }

    ignore_seq!(LenientText, LenientText(None));
    ignore_map!(LenientText, LenientText(None));
    ignore_other!(LenientText, LenientText(None));
}

/// `origin.kind`, or a bare string origin
#[derive(Debug, Default)]
pub struct Origin(pub Option<String>);

impl<'de> Deserialize<'de> for Origin {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(OriginVisitor)
    }
}

struct OriginVisitor;

impl<'de> Visitor<'de> for OriginVisitor {
    type Value = Origin;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an origin")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Origin, E> {
        Ok(Origin(Some(v.to_owned())))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Origin, A::Error> {
        let mut kind = None;
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            if key == "kind" {
                kind = map.next_value::<LenientText>()?.0;
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Origin(kind))
    }

    ignore_seq!(Origin, Origin(None));
    ignore_other!(Origin, Origin(None));
}

/// A `tool_use` block: its id, tool name and searchable input fields
#[derive(Debug, Default)]
pub struct ToolUse {
    pub id: String,
    pub name: String,
    pub fields: String,
}

/// A `tool_result` block: the start of its text (up to [`TOOL_RESULT_CHARS`])
#[derive(Debug, Default)]
pub struct ToolResult {
    pub tool_use_id: String,
    pub text: String,
    pub is_error: bool,
}

/// Extracted from `message.content`, which is either a plain string or an
/// array of blocks. `text` joins the `text` blocks.
#[derive(Debug, Default)]
pub struct Content {
    pub text: String,
    pub tool_uses: Vec<ToolUse>,
    pub tool_results: Vec<ToolResult>,
}

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
        Ok(Content {
            text: v.to_owned(),
            ..Content::default()
        })
    }

    fn visit_string<E: de::Error>(self, v: String) -> Result<Content, E> {
        Ok(Content {
            text: v,
            ..Content::default()
        })
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Content, A::Error> {
        let mut out = Content::default();
        while let Some(block) = seq.next_element::<Block>()? {
            match block {
                Block::Text(text) => push_joined(&mut out.text, &text),
                Block::ToolUse(t) => out.tool_uses.push(t),
                Block::ToolResult(r) => out.tool_results.push(r),
                Block::Other => {}
            }
        }
        Ok(out)
    }

    ignore_map!(Content, Content::default());
    ignore_other!(Content, Content::default());
}

enum Block {
    Text(String),
    ToolUse(ToolUse),
    ToolResult(ToolResult),
    Other,
}

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
        // Keys come in any order: collect, then decide by `type` at the end
        let mut kind: Option<Cow<'de, str>> = None;
        let mut text: Option<String> = None;
        let mut id = String::new();
        let mut name = String::new();
        let mut fields = String::new();
        let mut result = String::new();
        let mut is_error = false;
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            match key.as_ref() {
                "type" => kind = Some(map.next_value()?),
                "text" => text = map.next_value::<LenientText>()?.0,
                "id" | "tool_use_id" => id = map.next_value::<LenientText>()?.0.unwrap_or_default(),
                "name" => name = map.next_value::<LenientText>()?.0.unwrap_or_default(),
                "input" => fields = map.next_value::<ToolInput>()?.0,
                "content" => result = map.next_value::<Capped<TOOL_RESULT_CHARS>>()?.0,
                "is_error" => is_error = map.next_value::<Option<bool>>()? == Some(true),
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(match kind.as_deref() {
            Some("text") => text.map_or(Block::Other, Block::Text),
            Some("tool_use") => Block::ToolUse(ToolUse { id, name, fields }),
            Some("tool_result") => Block::ToolResult(ToolResult {
                tool_use_id: id,
                text: result,
                is_error,
            }),
            _ => Block::Other,
        })
    }

    ignore_str!(Block, Block::Other);
    ignore_seq!(Block, Block::Other);
    ignore_other!(Block, Block::Other);
}

/// The searchable fields of a `tool_use` input, each capped, joined.
struct ToolInput(String);

impl<'de> Deserialize<'de> for ToolInput {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(ToolInputVisitor)
    }
}

struct ToolInputVisitor;

impl<'de> Visitor<'de> for ToolInputVisitor {
    type Value = ToolInput;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a tool input object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<ToolInput, A::Error> {
        let mut out = String::new();
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            if TOOL_INPUT_KEYS.contains(&key.as_ref()) {
                push_joined(&mut out, &map.next_value::<Capped<TOOL_FIELD_CHARS>>()?.0);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(ToolInput(out))
    }

    ignore_str!(ToolInput, ToolInput(String::new()));
    ignore_seq!(ToolInput, ToolInput(String::new()));
    ignore_other!(ToolInput, ToolInput(String::new()));
}

/// The first `N` characters of a string, or of the `text` blocks of an array
/// (as in a tool result's `content`). Only the kept part is copied.
pub struct Capped<const N: usize>(pub String);

impl<'de, const N: usize> Deserialize<'de> for Capped<N> {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(CappedVisitor::<N>)
    }
}

struct CappedVisitor<const N: usize>;

impl<'de, const N: usize> Visitor<'de> for CappedVisitor<N> {
    type Value = Capped<N>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a string or an array of text blocks")
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Capped<N>, E> {
        Ok(Capped(capped(v, N).to_owned()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Capped<N>, A::Error> {
        let mut out = String::new();
        let mut kept = 0;
        while let Some(block) = seq.next_element::<Block>()? {
            if let Block::Text(text) = block
                && kept < N
            {
                let part = capped(&text, N - kept);
                kept += part.chars().count();
                push_joined(&mut out, part);
            }
        }
        Ok(Capped(out))
    }

    ignore_map!(Capped<N>, Capped(String::new()));
    ignore_other!(Capped<N>, Capped(String::new()));
}

/// An `attachment` record: the parts that carry searchable text
#[derive(Debug, Default)]
pub struct Attachment {
    pub kind: String,
    /// `queued_command`: a message sent while Claude was working
    pub prompt: Option<Content>,
    pub origin: Option<Origin>,
    pub is_meta: bool,
    /// `plan_file_reference`
    pub plan: Option<String>,
}

impl<'de> Deserialize<'de> for Attachment {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(AttachmentVisitor)
    }
}

struct AttachmentVisitor;

impl<'de> Visitor<'de> for AttachmentVisitor {
    type Value = Attachment;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an attachment")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Attachment, A::Error> {
        let mut out = Attachment::default();
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            match key.as_ref() {
                "type" => out.kind = map.next_value::<LenientText>()?.0.unwrap_or_default(),
                "prompt" => out.prompt = Some(map.next_value()?),
                "origin" => out.origin = Some(map.next_value()?),
                "isMeta" => out.is_meta = map.next_value::<Option<bool>>()? == Some(true),
                "planContent" => out.plan = Some(map.next_value::<Capped<PLAN_CHARS>>()?.0),
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        Ok(out)
    }

    ignore_str!(Attachment, Attachment::default());
    ignore_seq!(Attachment, Attachment::default());
    ignore_other!(Attachment, Attachment::default());
}

/// AskUserQuestion's `toolUseResult.answers` (question → chosen answer),
/// joined; empty for every other tool's result.
#[derive(Debug, Default)]
pub struct Answers(pub String);

impl<'de> Deserialize<'de> for Answers {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        d.deserialize_any(AnswersVisitor { inner: false })
    }
}

/// At the top (`inner: false`) looks for the `answers` key; inside it
/// (`inner: true`) collects every `question: answer` pair.
struct AnswersVisitor {
    inner: bool,
}

struct InnerAnswers(String);

impl<'de> Deserialize<'de> for InnerAnswers {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(InnerAnswers(
            d.deserialize_any(AnswersVisitor { inner: true })?.0,
        ))
    }
}

impl<'de> Visitor<'de> for AnswersVisitor {
    type Value = Answers;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a tool result")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Answers, A::Error> {
        let mut out = String::new();
        while let Some(key) = map.next_key::<Cow<'de, str>>()? {
            if self.inner {
                let answer = map.next_value::<Capped<TOOL_FIELD_CHARS>>()?.0;
                push_joined(&mut out, capped(&key, TOOL_FIELD_CHARS));
                push_joined(&mut out, &answer);
            } else if key == "answers" {
                out = map.next_value::<InnerAnswers>()?.0;
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Answers(out))
    }

    ignore_str!(Answers, Answers::default());
    ignore_seq!(Answers, Answers::default());
    ignore_other!(Answers, Answers::default());
}
