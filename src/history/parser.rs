use super::Conversation;
use crate::claude::{Content, LogEntry};
use crate::error::Result;
use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use std::time::SystemTime;

/// Everything gathered from a conversation file so far. It is built line by
/// line, so a file that has only grown can be resumed from where parsing
/// stopped instead of being read again from the start.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct ParseState {
    /// Whitespace-normalized text of all messages (summary not included)
    body: String,
    has_parts: bool,
    preview_parts: Vec<String>,
    /// User messages that could be `/clear` markers
    clear_markers: Vec<String>,
    saw_other_user_message: bool,
    seen_real_user_message: bool,
    skip_next_assistant: bool,
    cwd: Option<PathBuf>,
    message_count: usize,
    summary: Option<String>,
    first_ts_ms: Option<i64>,
    last_ts_ms: Option<i64>,
}

impl ParseState {
    fn note_timestamp(&mut self, timestamp: &str) {
        if let Ok(ts) = DateTime::parse_from_rfc3339(timestamp) {
            let ms = ts.timestamp_millis();
            if self.first_ts_ms.is_none() {
                self.first_ts_ms = Some(ms);
            }
            self.last_ts_ms = Some(ms);
        }
    }

    fn push_part(&mut self, text: &str) {
        self.has_parts = true;
        push_normalized(&mut self.body, text);
    }

    fn push_preview(&mut self, text: &str) {
        if self.preview_parts.len() < 3 {
            self.preview_parts.push(text.to_owned());
        }
    }

    /// Feed one JSONL line; malformed or irrelevant lines are ignored.
    pub fn feed_line(&mut self, line: &[u8]) {
        if line.trim_ascii().is_empty() {
            return;
        }
        let Ok(entry) = serde_json::from_slice::<LogEntry>(line) else {
            return;
        };

        match entry.kind.as_ref() {
            "user" => {
                let (Some(message), Some(timestamp)) = (entry.message, entry.timestamp) else {
                    return;
                };
                let Some(Content(text)) = message.content else {
                    return;
                };

                self.note_timestamp(&timestamp);

                if self.cwd.is_none()
                    && let Some(cwd) = entry.cwd
                {
                    self.cwd = Some(PathBuf::from(cwd.as_ref()));
                }

                if text.is_empty() {
                    return;
                }

                if is_clear_only_candidate(&text) {
                    self.clear_markers.push(text.clone());
                } else {
                    self.saw_other_user_message = true;
                }

                if is_clear_metadata_message(&text) {
                    return;
                }

                self.push_part(&text);

                let is_warmup = !self.seen_real_user_message && text.trim() == "Warmup";
                if is_warmup {
                    self.skip_next_assistant = true;
                } else {
                    self.message_count += 1;
                    self.push_preview(&text);
                    self.seen_real_user_message = true;
                }
            }
            "assistant" => {
                let (Some(message), Some(timestamp)) = (entry.message, entry.timestamp) else {
                    return;
                };
                let Some(Content(text)) = message.content else {
                    return;
                };

                self.note_timestamp(&timestamp);

                if !text.is_empty() {
                    self.push_part(&text);

                    if self.skip_next_assistant {
                        self.skip_next_assistant = false;
                    } else if self.seen_real_user_message {
                        self.message_count += 1;
                        self.push_preview(&text);
                    }
                }
            }
            "summary" => {
                if self.summary.is_none()
                    && let Some(summary) = entry.summary
                {
                    self.summary = Some(summary.into_owned());
                }
            }
            _ => {}
        }
    }

    /// Build the conversation, or `None` if it has nothing worth listing.
    pub fn finish(&self, path: PathBuf, modified: Option<SystemTime>) -> Option<Conversation> {
        let clear_only =
            !self.saw_other_user_message && is_clear_only_conversation(&self.clear_markers);
        if clear_only || !self.has_parts || self.preview_parts.is_empty() {
            return None;
        }

        let timestamp = modified
            .map(DateTime::<Local>::from)
            .unwrap_or_else(Local::now);

        let preview = normalize_whitespace(&self.preview_parts.join(" ... "));

        let mut full_text = String::with_capacity(self.body.len() + 64);
        if let Some(summary) = &self.summary {
            push_normalized(&mut full_text, summary);
        }
        if !self.body.is_empty() {
            if !full_text.is_empty() {
                full_text.push(' ');
            }
            full_text.push_str(&self.body);
        }

        let duration_minutes = match (self.first_ts_ms, self.last_ts_ms) {
            (Some(first), Some(last)) => {
                let minutes = (last - first) / 60_000;
                (minutes > 0).then_some(minutes as u64)
            }
            _ => None,
        };

        Some(Conversation {
            path,
            index: 0,
            timestamp,
            preview,
            full_text,
            project_name: None,
            project_path: None,
            cwd: self.cwd.clone(),
            message_count: self.message_count,
            summary: self.summary.clone(),
            duration_minutes,
        })
    }
}

/// Parse state together with how far into the file it got. `offset` always
/// sits right after a newline, so parsing can resume there.
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Progress {
    pub offset: u64,
    pub state: ParseState,
}

/// Continue parsing `reader` (positioned at `progress.offset`) to its end.
/// Returns the conversation as of the end of the input. A trailing line with
/// no newline yet counts toward the result but not toward `progress`, since
/// it may still be in the middle of being written.
pub(crate) fn parse_from<R: BufRead>(
    progress: &mut Progress,
    mut reader: R,
    path: PathBuf,
    modified: Option<SystemTime>,
) -> Result<Option<Conversation>> {
    let mut line: Vec<u8> = Vec::with_capacity(64 * 1024);
    loop {
        line.clear();
        let n = reader.read_until(b'\n', &mut line)?;
        if n == 0 {
            return Ok(progress.state.finish(path, modified));
        }
        if line.last() != Some(&b'\n') {
            let mut state = progress.state.clone();
            state.feed_line(&line);
            return Ok(state.finish(path, modified));
        }
        progress.state.feed_line(&line);
        progress.offset += n as u64;
    }
}

/// Parse a conversation file, resuming from `progress`.
pub fn process_conversation_file(
    path: PathBuf,
    modified: Option<SystemTime>,
    progress: &mut Progress,
) -> Result<Option<Conversation>> {
    let mut file = File::open(&path)?;
    if progress.offset > 0 {
        file.seek(SeekFrom::Start(progress.offset))?;
    }
    let reader = BufReader::with_capacity(256 * 1024, file);
    parse_from(progress, reader, path, modified)
}

#[cfg(test)]
pub(crate) fn process_conversation_reader<R: BufRead>(
    path: PathBuf,
    reader: R,
    modified: Option<SystemTime>,
) -> Result<Option<Conversation>> {
    parse_from(&mut Progress::default(), reader, path, modified)
}

/// True if the message could be one of the three markers that make up a
/// `/clear`-only conversation; other messages only need a flag.
fn is_clear_only_candidate(message: &str) -> bool {
    let trimmed = message.trim();
    trimmed.starts_with(
        "Caveat: The messages below were generated by the user while running local commands.",
    ) || trimmed.contains("<command-name>/clear</command-name>")
        || trimmed.contains("<local-command-stdout>")
}

/// Append `s` to `out` with whitespace runs collapsed to single spaces.
fn push_normalized(out: &mut String, s: &str) {
    for word in s.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
}

pub(crate) fn is_clear_metadata_message(message: &str) -> bool {
    let trimmed = message.trim();

    trimmed.is_empty()
        || trimmed.starts_with(
            "Caveat: The messages below were generated by the user while running local commands.",
        )
        || trimmed.contains("<local-command-caveat>")
        || trimmed.contains("<command-name>/clear</command-name>")
        || trimmed.contains("<command-message>clear</command-message>")
        || trimmed.contains("<local-command-stdout>")
        || trimmed.contains("<command-args>")
}

pub(crate) fn is_clear_only_conversation(user_messages: &[String]) -> bool {
    if user_messages.is_empty() {
        return false;
    }

    let mut saw_caveat = false;
    let mut saw_command = false;
    let mut saw_stdout = false;

    for msg in user_messages {
        let trimmed = msg.trim();
        if trimmed.is_empty() {
            continue;
        }

        let is_caveat = trimmed.starts_with(
            "Caveat: The messages below were generated by the user while running local commands.",
        );
        let has_command_tag = trimmed.contains("<command-name>/clear</command-name>");
        let has_stdout_tag = trimmed.contains("<local-command-stdout>");

        if is_caveat {
            saw_caveat = true;
        }
        if has_command_tag {
            saw_command = true;
        }
        if has_stdout_tag {
            saw_stdout = true;
        }

        if !(is_caveat || has_command_tag || has_stdout_tag) {
            return false;
        }
    }

    saw_caveat && saw_command && saw_stdout
}

pub(crate) fn normalize_whitespace(s: &str) -> String {
    s.split_whitespace().collect::<Vec<&str>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn user_msg(text: &str, cwd: Option<&str>) -> String {
        let cwd_json = match cwd {
            Some(c) => format!(r#""cwd": "{}","#, c),
            None => String::new(),
        };
        format!(
            r#"{{"type": "user", "timestamp": "2024-01-01T00:00:00Z", {}  "message": {{"role": "user", "content": "{}"}}}}"#,
            cwd_json, text
        )
    }

    fn assistant_msg(text: &str) -> String {
        format!(
            r#"{{"type": "assistant", "timestamp": "2024-01-01T00:00:00Z", "message": {{"role": "assistant", "content": [{{"type": "text", "text": "{}"}}]}}}}"#,
            text
        )
    }

    fn parse_jsonl(content: &str) -> Result<Option<Conversation>> {
        let reader = Cursor::new(content);
        process_conversation_reader(PathBuf::from("test.jsonl"), reader, None)
    }

    #[test]
    fn filters_warmup_messages() {
        let content = [
            user_msg("Warmup", None),
            assistant_msg("Ready"),
            user_msg("Hello world", None),
            assistant_msg("Hi there"),
        ]
        .join("\n");

        let conv = parse_jsonl(&content).unwrap().unwrap();
        assert!(conv.full_text.contains("Warmup"));
        assert!(conv.full_text.contains("Hello world"));
    }

    #[test]
    fn filters_clear_only_conversations() {
        let content = [
            user_msg(
                "Caveat: The messages below were generated by the user while running local commands.",
                None,
            ),
            user_msg("<command-name>/clear</command-name>", None),
            user_msg("<local-command-stdout></local-command-stdout>", None),
        ]
        .join("\n");

        let result = parse_jsonl(&content).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn preserves_clear_command_in_mixed_conversation() {
        let content = [
            user_msg("Hello", None),
            assistant_msg("Hi"),
            user_msg(
                "Caveat: The messages below were generated by the user while running local commands.",
                None,
            ),
            user_msg("<command-name>/clear</command-name>", None),
            user_msg("Another question", None),
        ]
        .join("\n");

        let conv = parse_jsonl(&content).unwrap().unwrap();
        assert!(conv.full_text.contains("Hello"));
        assert!(conv.full_text.contains("Another question"));
    }

    #[test]
    fn extracts_cwd_from_first_user_message() {
        let content = [
            user_msg("Hello", Some("/home/user/project")),
            assistant_msg("Hi"),
            user_msg("More", Some("/other/path")),
        ]
        .join("\n");

        let conv = parse_jsonl(&content).unwrap().unwrap();
        assert_eq!(conv.cwd, Some(PathBuf::from("/home/user/project")));
    }

    #[test]
    fn handles_empty_conversation() {
        let result = parse_jsonl("").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn counts_messages_correctly() {
        let content = [
            user_msg("First", None),
            assistant_msg("Response 1"),
            user_msg("Second", None),
            assistant_msg("Response 2"),
        ]
        .join("\n");

        let conv = parse_jsonl(&content).unwrap().unwrap();
        assert_eq!(conv.message_count, 4);
    }

    #[test]
    fn excludes_warmup_from_message_count() {
        let content = [
            user_msg("Warmup", None),
            assistant_msg("Ready"),
            user_msg("Real question", None),
            assistant_msg("Real answer"),
        ]
        .join("\n");

        let conv = parse_jsonl(&content).unwrap().unwrap();
        assert_eq!(conv.message_count, 2);
    }

    #[test]
    fn extracts_summary_from_jsonl() {
        let content = [
            r#"{"type": "summary", "summary": "Test conversation summary", "leafUuid": "abc123"}"#
                .to_string(),
            user_msg("Hello", None),
            assistant_msg("Hi there"),
        ]
        .join("\n");

        let conv = parse_jsonl(&content).unwrap().unwrap();
        assert_eq!(conv.summary, Some("Test conversation summary".to_string()),);
    }

    #[test]
    fn summary_included_in_full_text() {
        let content = [
            r#"{"type": "summary", "summary": "Important topic discussion", "leafUuid": "abc123"}"#
                .to_string(),
            user_msg("Hello", None),
            assistant_msg("Hi there"),
        ]
        .join("\n");

        let conv = parse_jsonl(&content).unwrap().unwrap();
        assert!(conv.full_text.contains("Important topic discussion"));
    }

    #[test]
    fn skips_non_text_blocks() {
        let content = [
            user_msg("Hello", None),
            r#"{"type":"assistant","timestamp":"2024-01-01T00:00:00Z","message":{"content":[{"type":"tool_use","input":{"text":"secret"}},{"text":"shown","type":"text"}]}}"#.to_string(),
            r#"{"type":"user","timestamp":"2024-01-01T00:00:00Z","message":{"content":[{"type":"tool_result","content":[{"type":"text","text":"tool output"}]}]}}"#.to_string(),
        ]
        .join("\n");

        let conv = parse_jsonl(&content).unwrap().unwrap();
        assert_eq!(conv.full_text, "Hello shown");
    }

    #[test]
    fn resuming_matches_full_parse() {
        let lines = [
            r#"{"type": "summary", "summary": "Sum", "leafUuid": "x"}"#.to_string(),
            user_msg("Warmup", Some("/p")),
            assistant_msg("Ready"),
            user_msg("First question", None),
            assistant_msg("First answer"),
            user_msg("Second question", None),
            assistant_msg("Second answer"),
        ];
        let full = lines.join("\n") + "\n";
        let expected = parse_jsonl(&full).unwrap().unwrap();

        // Cut in the middle of a line, as if it were still being written
        let cut = full.find("First answer").unwrap();
        let mut progress = Progress::default();
        let partial = parse_from(
            &mut progress,
            Cursor::new(&full[..cut]),
            PathBuf::from("test.jsonl"),
            None,
        )
        .unwrap()
        .unwrap();
        assert!(!partial.full_text.contains("First answer"));
        assert!(full.as_bytes()[progress.offset as usize - 1] == b'\n');

        let rest = &full[progress.offset as usize..];
        let resumed = parse_from(
            &mut progress,
            Cursor::new(rest),
            PathBuf::from("test.jsonl"),
            None,
        )
        .unwrap()
        .unwrap();
        assert_eq!(progress.offset as usize, full.len());
        assert_eq!(resumed.full_text, expected.full_text);
        assert_eq!(resumed.preview, expected.preview);
        assert_eq!(resumed.message_count, expected.message_count);
        assert_eq!(resumed.cwd, expected.cwd);
        assert_eq!(resumed.summary, expected.summary);
    }

    #[test]
    fn normalize_whitespace_collapses_runs() {
        assert_eq!(normalize_whitespace("hello  world"), "hello world");
        assert_eq!(normalize_whitespace("  hello   world  "), "hello world");
        assert_eq!(normalize_whitespace("a\n\n\nb"), "a b");
    }

    #[test]
    fn is_clear_metadata_message_detects_patterns() {
        assert!(is_clear_metadata_message(""));
        assert!(is_clear_metadata_message(
            "Caveat: The messages below were generated by the user while running local commands."
        ));
        assert!(is_clear_metadata_message(
            "<command-name>/clear</command-name>"
        ));
        assert!(!is_clear_metadata_message("Hello world"));
    }

    #[test]
    fn is_clear_only_conversation_requires_all_three_markers() {
        assert!(!is_clear_only_conversation(&[]));

        assert!(is_clear_only_conversation(&[
            "Caveat: The messages below were generated by the user while running local commands."
                .to_string(),
            "<command-name>/clear</command-name>".to_string(),
            "<local-command-stdout></local-command-stdout>".to_string(),
        ]));

        assert!(!is_clear_only_conversation(&[
            "Caveat: The messages below were generated by the user while running local commands."
                .to_string(),
            "<command-name>/clear</command-name>".to_string(),
            "<local-command-stdout></local-command-stdout>".to_string(),
            "Hello world".to_string(),
        ]));
    }
}
