use super::Conversation;
use crate::claude::{LogEntry, extract_text_from_assistant, extract_text_from_user};
use crate::error::Result;
use chrono::{DateTime, Local};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::time::SystemTime;

pub fn process_conversation_file(
    path: PathBuf,
    modified: Option<SystemTime>,
) -> Result<Option<Conversation>> {
    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    process_conversation_reader(path, reader, modified)
}

pub(crate) fn process_conversation_reader<R: BufRead>(
    path: PathBuf,
    reader: R,
    modified: Option<SystemTime>,
) -> Result<Option<Conversation>> {
    let lines: Vec<String> = reader.lines().map_while(|l| l.ok()).collect();

    let mut all_parts = Vec::new();
    let mut preview_parts = Vec::new();
    let mut user_messages = Vec::new();
    let mut seen_real_user_message = false;
    let mut skip_next_assistant = false;
    let mut extracted_cwd: Option<PathBuf> = None;
    let mut message_count: usize = 0;
    let mut extracted_summary: Option<String> = None;
    let mut first_timestamp: Option<chrono::DateTime<chrono::FixedOffset>> = None;
    let mut last_timestamp: Option<chrono::DateTime<chrono::FixedOffset>> = None;

    for line in lines.iter() {
        if line.trim().is_empty() {
            continue;
        }

        let Ok(entry) = serde_json::from_str::<LogEntry>(line) else {
            continue;
        };

        match entry {
            LogEntry::User {
                message,
                cwd,
                timestamp,
                ..
            } => {
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&timestamp) {
                    if first_timestamp.is_none() {
                        first_timestamp = Some(ts);
                    }
                    last_timestamp = Some(ts);
                }

                if extracted_cwd.is_none()
                    && let Some(cwd_str) = cwd
                {
                    extracted_cwd = Some(PathBuf::from(cwd_str));
                }

                let text = extract_text_from_user(&message);
                if text.is_empty() {
                    continue;
                }

                user_messages.push(text.clone());

                if is_clear_metadata_message(&text) {
                    continue;
                }

                all_parts.push(text.clone());

                let is_warmup = !seen_real_user_message && text.trim() == "Warmup";
                if is_warmup {
                    skip_next_assistant = true;
                } else {
                    message_count += 1;
                    preview_parts.push(text);
                    seen_real_user_message = true;
                }
            }
            LogEntry::Assistant {
                message, timestamp, ..
            } => {
                if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&timestamp) {
                    if first_timestamp.is_none() {
                        first_timestamp = Some(ts);
                    }
                    last_timestamp = Some(ts);
                }

                let text = extract_text_from_assistant(&message);
                if !text.is_empty() {
                    all_parts.push(text.clone());

                    if skip_next_assistant {
                        skip_next_assistant = false;
                    } else if seen_real_user_message {
                        message_count += 1;
                        preview_parts.push(text);
                    }
                }
            }
            LogEntry::Summary { summary } => {
                if extracted_summary.is_none() {
                    extracted_summary = Some(summary.clone());
                }
            }
            _ => {}
        }
    }

    if is_clear_only_conversation(&user_messages) {
        return Ok(None);
    }

    if all_parts.is_empty() || preview_parts.is_empty() {
        return Ok(None);
    }

    let timestamp = modified
        .map(DateTime::<Local>::from)
        .unwrap_or_else(Local::now);

    let preview = preview_parts
        .iter()
        .take(3)
        .cloned()
        .collect::<Vec<_>>()
        .join(" ... ");
    let preview = normalize_whitespace(&preview);

    let mut full_text = all_parts.join(" ");
    if let Some(ref summary) = extracted_summary {
        full_text = format!("{} {}", summary, full_text);
    }
    let full_text = normalize_whitespace(&full_text);

    let duration_minutes = match (first_timestamp, last_timestamp) {
        (Some(first), Some(last)) => {
            let duration = last.signed_duration_since(first);
            let minutes = duration.num_minutes();
            if minutes > 0 {
                Some(minutes as u64)
            } else {
                None
            }
        }
        _ => None,
    };

    Ok(Some(Conversation {
        path,
        index: 0,
        timestamp,
        preview,
        full_text,
        project_name: None,
        project_path: None,
        cwd: extracted_cwd,
        message_count,
        summary: extracted_summary,
        duration_minutes,
    }))
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
        assert_eq!(
            conv.summary,
            Some("Test conversation summary".to_string()),
        );
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
