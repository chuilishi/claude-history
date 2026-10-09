mod claude;
mod cli;
mod error;
mod history;
mod tui;

use clap::Parser;
use cli::Args;
use error::{AppError, Result};
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    if let Err(e) = run() {
        match e {
            AppError::SelectionCancelled => {
                std::process::exit(0);
            }
            _ => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn run() -> Result<()> {
    let _args = Args::parse();

    let rx = history::load_all_conversations_streaming();

    match tui::run_with_loader(rx)? {
        (tui::Action::Resume(path), convs) => {
            let conv = convs.iter().find(|c| c.path == path);
            let project_path = conv.and_then(|c| c.project_path.as_ref());
            resume_with_claude(&path, project_path)?;
        }
        (tui::Action::Quit, _) => return Err(AppError::SelectionCancelled),
    }

    Ok(())
}

fn resume_with_claude(
    selected_path: &Path,
    project_path: Option<&PathBuf>,
) -> Result<()> {
    let conversation_id = selected_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or_else(|| {
            AppError::ClaudeExecutionError("Conversation filename is not valid Unicode".to_string())
        })?
        .to_owned();

    let project_dir = match project_path {
        Some(path) if path.exists() && path.is_dir() => path,
        Some(path) => {
            return Err(AppError::ClaudeExecutionError(format!(
                "Project directory no longer exists: {}",
                path.display()
            )));
        }
        None => {
            return Err(AppError::ClaudeExecutionError(
                "Cannot determine project directory for this conversation".to_string(),
            ));
        }
    };

    let mut command = Command::new("claude");
    command.args([
        "--resume",
        &conversation_id,
        "--allow-dangerously-skip-permissions",
    ]);
    command.current_dir(project_dir);

    run_claude_command(command)
}

#[cfg(unix)]
fn run_claude_command(mut command: Command) -> Result<()> {
    use std::os::unix::process::CommandExt;

    let err = command.exec();
    Err(AppError::ClaudeExecutionError(err.to_string()))
}

#[cfg(not(unix))]
fn run_claude_command(mut command: Command) -> Result<()> {
    let status = command
        .status()
        .map_err(|e| AppError::ClaudeExecutionError(e.to_string()))?;

    if !status.success() {
        return Err(AppError::ClaudeExecutionError(format!(
            "claude CLI exited with status {}",
            status
        )));
    }

    Ok(())
}
