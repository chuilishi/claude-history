//! Conversations hidden from the list. Hiding never touches Claude Code's own files;
//! session IDs are recorded one per line in `~/.claude-history/hidden`.
//! Remove a line from that file to bring the conversation back.

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

fn hidden_file_path() -> Option<PathBuf> {
    Some(home::home_dir()?.join(".claude-history").join("hidden"))
}

/// Session ID of a conversation file (`<session-id>.jsonl`)
pub fn session_id(path: &Path) -> Option<&str> {
    path.file_stem().and_then(|s| s.to_str())
}

pub fn load_hidden() -> HashSet<String> {
    let Some(path) = hidden_file_path() else {
        return HashSet::new();
    };
    fs::read_to_string(path)
        .map(|content| {
            content
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

pub fn hide(session_id: &str) -> io::Result<()> {
    let path = hidden_file_path().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "Could not determine home directory",
        )
    })?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{}", session_id)
}
