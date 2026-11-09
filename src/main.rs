mod claude;
mod cli;
mod error;
mod history;
mod search;
mod tui;

use clap::Parser;
use error::{AppError, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// With no subcommand, opens the TUI. `scripts/install.sh` reads the
/// `vX.Y.Z` from `--version`.
#[derive(Parser)]
#[command(
    name = "claude-history",
    about = "Resume Claude conversations",
    version = concat!("v", env!("CARGO_PKG_VERSION"))
)]
struct Args {
    #[command(subcommand)]
    command: Option<Cmd>,
}

#[derive(clap::Subcommand)]
enum Cmd {
    /// Print conversations matching words (prefix match, case-insensitive)
    Search {
        #[arg(required = true)]
        words: Vec<String>,
        /// Match any word instead of all; ranks by how many matched
        #[arg(long)]
        any: bool,
        /// Only conversations whose project path contains this
        #[arg(long, short)]
        project: Option<String>,
        #[arg(long, short = 'n', default_value_t = 15)]
        limit: usize,
        /// Context snippets per result
        #[arg(long, default_value_t = 2)]
        snippets: usize,
    },
    /// Print details of one conversation, with snippets for optional words
    Show {
        /// Session id or a unique prefix of it
        id: String,
        words: Vec<String>,
        #[arg(long, default_value_t = 6)]
        snippets: usize,
    },
}

fn main() {
    if let Err(e) = run() {
        match e {
            AppError::SelectionCancelled => std::process::exit(0),
            _ => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}

fn run() -> Result<()> {
    match Args::parse().command {
        Some(Cmd::Search {
            words,
            any,
            project,
            limit,
            snippets,
        }) => {
            return cli::search(cli::SearchOptions {
                words,
                any,
                project,
                limit,
                snippets,
            });
        }
        Some(Cmd::Show {
            id,
            words,
            snippets,
        }) => return cli::show(&id, &words, snippets),
        None => {}
    }

    let rx = history::load_all_conversations_streaming();
    match tui::run_with_loader(rx)? {
        tui::Action::Resume {
            session_id,
            project_path,
            permission_mode,
        } => resume_with_claude(&session_id, &project_path, permission_mode.as_deref()),
        tui::Action::Quit => Err(AppError::SelectionCancelled),
    }
}

fn resume_with_claude(
    session_id: &str,
    project_dir: &Path,
    permission_mode: Option<&str>,
) -> Result<()> {
    if !project_dir.is_dir() {
        return Err(AppError::ClaudeExecutionError(format!(
            "Project directory no longer exists: {}",
            project_dir.display()
        )));
    }

    let mut command = Command::new(claude_program());
    command.args([
        "--resume",
        session_id,
        "--allow-dangerously-skip-permissions",
    ]);
    if let Some(mode) = permission_mode {
        command.args(["--permission-mode", mode]);
    }
    command.current_dir(project_dir);

    run_claude_command(command)
}

#[cfg(not(windows))]
fn claude_program() -> PathBuf {
    PathBuf::from("claude")
}

/// `Command` only finds `.exe` files on PATH, but npm installs `claude.cmd`.
/// Look for both (exe first); fall back to the bare name for a clear error.
#[cfg(windows)]
fn claude_program() -> PathBuf {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .flat_map(|dir| ["claude.exe", "claude.cmd", "claude.bat"].map(|name| dir.join(name)))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from("claude"))
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
