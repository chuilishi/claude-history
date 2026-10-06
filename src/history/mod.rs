pub mod hidden;
mod loader;
mod parser;
mod path;

use crate::error::{AppError, Result};
use chrono::{DateTime, Local};
use std::path::PathBuf;
use std::time::SystemTime;

pub use loader::load_all_conversations_streaming;

#[derive(Clone)]
pub struct Conversation {
    pub path: PathBuf,
    pub index: usize,
    pub timestamp: DateTime<Local>,
    pub preview: String,
    pub full_text: String,
    pub project_name: Option<String>,
    pub project_path: Option<PathBuf>,
    pub cwd: Option<PathBuf>,
    pub message_count: usize,
    pub summary: Option<String>,
    pub duration_minutes: Option<u64>,
}

pub struct Project {
    pub name: String,
    pub modified: SystemTime,
}

pub enum LoaderMessage {
    Fatal(AppError),
    ProjectError,
    Batch(Vec<Conversation>),
    Done,
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
