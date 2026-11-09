mod cache;
pub mod hidden;
mod loader;
mod parser;
mod path;

use crate::error::{AppError, Result};
use chrono::{DateTime, Local};
use std::path::PathBuf;

pub use loader::load_all_conversations_streaming;

#[derive(Clone)]
pub struct Conversation {
    pub path: PathBuf,
    pub timestamp: DateTime<Local>,
    /// Display text of the first few messages, tags stripped
    pub preview: String,
    /// Folded summary and message text, matched by search
    pub search_text: String,
    pub project_name: String,
    pub project_path: PathBuf,
    pub message_count: usize,
    pub summary: Option<String>,
    pub duration_minutes: Option<u64>,
}

pub enum LoaderMessage {
    Fatal(AppError),
    Batch(Vec<Conversation>),
    Done,
}

/// This tool's own data directory, `~/.claude-history`
fn data_dir() -> Option<PathBuf> {
    Some(home::home_dir()?.join(".claude-history"))
}

pub fn get_claude_projects_root() -> Result<PathBuf> {
    let home_dir = home::home_dir().ok_or_else(|| {
        AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Could not determine home directory",
        ))
    })?;

    Ok(home_dir.join(".claude").join("projects"))
}
