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
    /// Folded text matched by search, in tiers of falling importance: title
    /// and recap; what the user typed; Claude's replies and pasted text; tool
    /// calls, subagent reports and plans
    pub search_text: String,
    /// Byte offsets in `search_text` where tiers 1, 2 and 3 start
    pub tier_starts: [usize; 3],
    pub project_name: String,
    pub project_path: PathBuf,
    pub message_count: usize,
    /// `/rename` title, else Claude's generated title, else an old-style summary
    pub title: Option<String>,
    pub duration_minutes: Option<u64>,
    /// The user's latest prompt
    pub last_prompt: Option<String>,
    /// Claude's "while you were away" recap, if no message came after it
    pub recap: Option<String>,
    /// Names of the tools and subagents the conversation used
    pub tools: Vec<String>,
    /// Every recap in order, with its time: what a long conversation went through
    pub recaps: Vec<(Option<DateTime<Local>>, String)>,
    /// Current git branch; `None` outside a repo or when detached
    pub git_branch: Option<String>,
    /// Short name of the model that answered last, e.g. `opus-5.5`
    pub model: Option<String>,
    pub lines_added: u64,
    pub lines_removed: u64,
    /// Last permission mode, if `claude --permission-mode` accepts it
    pub permission_mode: Option<String>,
    /// Session this one was continued in
    pub continued_in: Option<String>,
    /// Started through the SDK rather than the interactive CLI
    pub is_sdk: bool,
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
