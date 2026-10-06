use super::hidden;
use super::parser::process_conversation_file;
use super::path::{
    decode_project_dir_name_to_path, format_short_name_from_path,
};
use super::{Conversation, LoaderMessage, Project};
use crate::error::{AppError, Result};
use rayon::prelude::*;
use std::collections::HashSet;
use std::fs::read_dir;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::SystemTime;

pub fn load_all_conversations_streaming() -> Receiver<LoaderMessage> {
    let (tx, rx) = mpsc::channel();

    thread::spawn(move || {
        load_all_streaming_inner(tx);
    });

    rx
}

fn load_all_streaming_inner(tx: Sender<LoaderMessage>) {
    let root = match super::get_claude_projects_root() {
        Ok(r) => r,
        Err(e) => {
            let _ = tx.send(LoaderMessage::Fatal(e));
            return;
        }
    };

    if !root.exists() {
        let _ = tx.send(LoaderMessage::Fatal(AppError::ProjectsDirNotFound(
            root.display().to_string(),
        )));
        return;
    }

    let projects = match list_projects(&root) {
        Ok(p) => p,
        Err(e) => {
            let _ = tx.send(LoaderMessage::Fatal(e));
            return;
        }
    };

    let hidden = hidden::load_hidden();

    projects.par_iter().for_each(|project| {
        let project_dir = root.join(&project.name);

        match load_conversations(&project_dir, &hidden) {
            Ok(mut convs) => {
                if convs.is_empty() {
                    return;
                }

                let fallback_path = decode_project_dir_name_to_path(&project.name);

                for conv in &mut convs {
                    let project_path = conv.cwd.clone().unwrap_or_else(|| fallback_path.clone());
                    conv.project_name = Some(format_short_name_from_path(&project_path));
                    conv.project_path = Some(project_path);
                }

                let _ = tx.send(LoaderMessage::Batch(convs));
            }
            Err(_) => {
                let _ = tx.send(LoaderMessage::ProjectError);
            }
        }
    });

    let _ = tx.send(LoaderMessage::Done);
}

fn list_projects(root: &Path) -> Result<Vec<Project>> {
    let entries = read_dir(root)?;

    let mut projects: Vec<Project> = entries
        .par_bridge()
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();

            if !path.is_dir() {
                return None;
            }

            let has_conversations = read_dir(&path).ok()?.any(|e| {
                e.ok()
                    .map(|e| {
                        let path = e.path();
                        let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                        path.extension().map(|s| s == "jsonl").unwrap_or(false)
                            && !name.starts_with("agent-")
                    })
                    .unwrap_or(false)
            });

            if !has_conversations {
                return None;
            }

            let name = path.file_name()?.to_string_lossy().to_string();
            let modified = entry
                .metadata()
                .ok()?
                .modified()
                .ok()
                .unwrap_or(SystemTime::UNIX_EPOCH);

            Some(Project {
                name,
                modified,
            })
        })
        .collect();

    projects.sort_by(|a, b| b.modified.cmp(&a.modified));

    Ok(projects)
}

fn load_conversations(projects_dir: &Path, hidden: &HashSet<String>) -> Result<Vec<Conversation>> {
    let mut files_with_meta = Vec::new();

    for entry in read_dir(projects_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            if let Some(filename) = path.file_name().and_then(|f| f.to_str())
                && filename.starts_with("agent-")
            {
                continue;
            }

            if hidden::session_id(&path).is_some_and(|id| hidden.contains(id)) {
                continue;
            }

            let modified = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok());

            files_with_meta.push((path, modified));
        }
    }

    files_with_meta.sort_by_key(|(_, modified)| modified.unwrap_or(SystemTime::UNIX_EPOCH));
    files_with_meta.reverse();

    let mut conversations: Vec<Conversation> = files_with_meta
        .into_par_iter()
        .filter_map(|(path, modified)| {
            match process_conversation_file(path, modified) {
                Ok(Some(conversation)) => Some(conversation),
                _ => None,
            }
        })
        .collect();

    conversations.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    let fallback_path = projects_dir
        .file_name()
        .map(|n| decode_project_dir_name_to_path(&n.to_string_lossy()))
        .unwrap_or_default();

    for (idx, conv) in conversations.iter_mut().enumerate() {
        conv.index = idx;
        let project_path = conv.cwd.clone().unwrap_or_else(|| fallback_path.clone());
        conv.project_name = Some(format_short_name_from_path(&project_path));
        conv.project_path = Some(project_path);
    }

    Ok(conversations)
}
