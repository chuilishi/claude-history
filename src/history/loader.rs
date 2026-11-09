use super::cache::{self, Entry, Lookup};
use super::hidden;
use super::parser::{Progress, process_conversation_file};
use super::path::decode_project_dir_name_to_path;
use super::{Conversation, LoaderMessage};
use crate::error::{AppError, Result};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs::read_dir;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::SystemTime;

pub fn load_all_conversations_streaming() -> Receiver<LoaderMessage> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        if let Err(e) = load_all(&tx) {
            let _ = tx.send(LoaderMessage::Fatal(e));
        }
    });
    rx
}

fn load_all(tx: &Sender<LoaderMessage>) -> Result<()> {
    let root = super::get_claude_projects_root()?;
    if !root.exists() {
        return Err(AppError::ProjectsDirNotFound(root.display().to_string()));
    }

    // Decoding the cache is the slowest setup step; overlap it with listing
    let (projects, old_cache) = rayon::join(|| list_projects(&root), cache::load);
    let projects = projects?;

    let hidden = hidden::load_hidden();
    let caches = Caches {
        old: Mutex::new(old_cache.entries),
        new: Mutex::new(HashMap::new()),
        changed: AtomicBool::new(false),
    };

    projects.par_iter().for_each(|project_dir| {
        if let Ok(convs) = load_conversations(project_dir, &hidden, &caches)
            && !convs.is_empty()
        {
            let _ = tx.send(LoaderMessage::Batch(convs));
        }
    });

    // Files that disappeared (or got hidden) since the last run are left over
    let removed = caches.old.into_inner().map_or(true, |old| !old.is_empty());
    // Saved before `Done` so it is written even if the app exits right after
    if caches.changed.into_inner() || removed {
        cache::save(caches.new.into_inner().unwrap_or_default());
    }

    let _ = tx.send(LoaderMessage::Done);
    Ok(())
}

struct Caches {
    /// Entries from the last run; taken out as files are looked up
    old: Mutex<HashMap<PathBuf, Entry>>,
    /// Entries for this run's files, saved at the end
    new: Mutex<HashMap<PathBuf, Entry>>,
    /// Set when any entry changed, i.e. the cache needs saving
    changed: AtomicBool,
}

/// Conversation logs are `<session-id>.jsonl`; `agent-*.jsonl` are sidechains
fn is_conversation_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| ext == "jsonl")
        && !path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("agent-"))
}

fn list_projects(root: &Path) -> Result<Vec<PathBuf>> {
    Ok(read_dir(root)?
        .filter_map(|entry| Some(entry.ok()?.path()))
        .filter(|path| path.is_dir())
        .collect())
}

fn load_conversations(
    project_dir: &Path,
    hidden: &HashSet<String>,
    caches: &Caches,
) -> Result<Vec<Conversation>> {
    let mut files = Vec::new();
    for entry in read_dir(project_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !is_conversation_file(&path)
            || hidden::session_id(&path).is_some_and(|id| hidden.contains(id))
        {
            continue;
        }
        let metadata = entry.metadata().ok();
        let modified = metadata.as_ref().and_then(|m| m.modified().ok());
        let size = metadata.map_or(0, |m| m.len());
        files.push((path, size, modified));
    }

    // Lossy; only used for old logs whose entries carry no `cwd`
    let fallback_dir = project_dir
        .file_name()
        .map(|n| decode_project_dir_name_to_path(&n.to_string_lossy()))
        .unwrap_or_default();

    Ok(files
        .into_par_iter()
        .filter_map(|(path, size, modified)| {
            load_cached(path, size, modified, &fallback_dir, caches)
        })
        .collect())
}

fn load_cached(
    path: PathBuf,
    size: u64,
    modified: Option<SystemTime>,
    fallback_dir: &Path,
    caches: &Caches,
) -> Option<Conversation> {
    let mtime_ns = cache::mtime_ns(modified);
    let cached = caches.old.lock().ok()?.remove(&path);
    let before = cached
        .as_ref()
        .map(|e| (e.size, e.mtime_ns, e.progress.offset));

    let (conv, progress) = match cache::lookup(cached, size, mtime_ns) {
        Lookup::Unchanged(progress) => (
            progress.state.finish(path.clone(), modified, fallback_dir),
            progress,
        ),
        lookup => {
            let mut progress = match lookup {
                Lookup::Resume(progress) => progress,
                _ => Progress::default(),
            };
            let conv =
                process_conversation_file(path.clone(), modified, fallback_dir, &mut progress)
                    .ok()?;
            (conv, progress)
        }
    };

    // A file with a half-written last line is reparsed from the same offset
    // every run; that alone is no reason to rewrite the cache
    if before != Some((size, mtime_ns, progress.offset)) {
        caches.changed.store(true, Ordering::Relaxed);
    }
    if let Ok(mut new) = caches.new.lock() {
        new.insert(
            path,
            Entry {
                size,
                mtime_ns,
                progress,
            },
        );
    }
    conv
}
