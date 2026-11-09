//! On-disk parse cache in `~/.claude-history/cache.json`.
//!
//! For every conversation file it records the size and mtime it had and how
//! far parsing got. On the next start an unchanged file is not opened at all,
//! and a file that only grew (Claude Code appends to its logs) is parsed from
//! the recorded offset. Anything else is parsed from scratch. A missing or
//! unreadable cache just means everything is parsed again.

use super::parser::Progress;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Bump when the parser or the cached data changes meaning.
const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct Entry {
    pub size: u64,
    pub mtime_ns: u64,
    pub progress: Progress,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Cache {
    version: u32,
    pub entries: HashMap<PathBuf, Entry>,
}

fn cache_path() -> Option<PathBuf> {
    Some(home::home_dir()?.join(".claude-history").join("cache.json"))
}

pub fn mtime_ns(time: Option<SystemTime>) -> u64 {
    time.and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

pub fn load() -> Cache {
    let Some(bytes) = cache_path().and_then(|p| fs::read(p).ok()) else {
        return Cache::default();
    };
    match serde_json::from_slice::<Cache>(&bytes) {
        Ok(cache) if cache.version == VERSION => cache,
        _ => Cache::default(),
    }
}

pub fn save(entries: HashMap<PathBuf, Entry>) {
    let Some(path) = cache_path() else {
        return;
    };
    let cache = Cache {
        version: VERSION,
        entries,
    };
    let _ = write_atomic(&path, &cache);
}

fn write_atomic(path: &Path, cache: &Cache) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let bytes = serde_json::to_vec(cache).map_err(std::io::Error::other)?;
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path).inspect_err(|_| {
        let _ = fs::remove_file(&tmp);
    })
}

/// How to bring a cached entry up to date with a file of the given size and mtime.
pub fn lookup(cached: Option<Entry>, size: u64, mtime_ns: u64) -> Lookup {
    let Some(e) = cached else {
        return Lookup::Fresh;
    };
    let unchanged = e.size == size && e.mtime_ns == mtime_ns;
    if unchanged && e.progress.offset == size {
        Lookup::Unchanged(e.progress)
    } else if (unchanged || size > e.size) && e.progress.offset <= size {
        // Logs are append-only, so a file that grew keeps its parsed prefix
        Lookup::Resume(e.progress)
    } else {
        Lookup::Fresh
    }
}

pub enum Lookup {
    /// Fully parsed already; the file need not be opened
    Unchanged(Progress),
    /// Continue parsing from `Progress::offset`
    Resume(Progress),
    Fresh,
}
