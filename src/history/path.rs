//! Project paths: short display names, and decoding Claude's encoded project
//! directory names.

use std::path::{Path, PathBuf};

/// Short display name for a project path: the folder name, or
/// `project/worktree` for a git worktree under `project__worktrees/` or
/// `project/.worktrees/` (also `project/__worktrees/`).
pub fn format_short_name_from_path(path: &Path) -> String {
    let parts: Vec<_> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect();

    for (i, part) in parts.iter().enumerate() {
        let main = if part == ".worktrees" {
            ""
        } else if let Some(main) = part.strip_suffix("__worktrees") {
            main
        } else {
            continue;
        };
        // `project/.worktrees` and `project/__worktrees`: project is the parent
        let main = match (main.is_empty(), i.checked_sub(1)) {
            (false, _) => main,
            (true, Some(parent)) => &parts[parent],
            (true, None) => continue,
        };
        if let Some(worktree) = parts.get(i + 1) {
            return format!("{main}/{worktree}");
        }
    }

    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Decode a project directory name back to a path (heuristic fallback).
///
/// Claude's encoding replaces every non-alphanumeric character (except `-`)
/// with `-`, so `/`, `_` and `.` all become `-` and the encoding is lossy.
/// Runs of dashes are read as `/` for odd counts and `__` pairs otherwise,
/// favouring `__` (as in `project__worktrees`).
///
/// Only used for old logs whose entries carry no `cwd`.
pub fn decode_project_dir_name_to_path(encoded: &str) -> PathBuf {
    let mut result = String::with_capacity(encoded.len());
    let mut chars = encoded.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '-' {
            result.push(c);
            continue;
        }
        let mut count = 1;
        while chars.peek() == Some(&'-') {
            chars.next();
            count += 1;
        }
        if count == 2 {
            result.push_str("__");
            continue;
        }
        result.push('/');
        for _ in 0..(count - 1) / 2 {
            result.push_str("__");
        }
        if count > 1 && (count - 1) % 2 == 1 {
            result.push('/');
        }
    }

    PathBuf::from(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode(encoded: &str) -> String {
        decode_project_dir_name_to_path(encoded)
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn decodes_single_dashes_to_slashes() {
        assert_eq!(decode("-tmp-foo-Bar123"), "/tmp/foo/Bar123");
        // Lossy: dashes inside folder names become slashes too
        assert_eq!(
            decode("-Users-raine-code-claude-history"),
            "/Users/raine/code/claude/history"
        );
    }

    #[test]
    fn decodes_double_dashes_to_underscores() {
        assert_eq!(
            decode("-Users-raine-code-myproject--worktrees-feature"),
            "/Users/raine/code/myproject__worktrees/feature"
        );
        assert_eq!(
            decode("-Users-raine-code-myproject---worktrees-feature"),
            "/Users/raine/code/myproject/__worktrees/feature"
        );
    }

    #[test]
    fn short_name_is_folder_name() {
        assert_eq!(
            format_short_name_from_path(Path::new("/Users/raine/code/app")),
            "app"
        );
    }

    #[test]
    fn short_name_shows_worktrees() {
        for (path, expected) in [
            (
                "/Users/raine/code/WalkingMate__worktrees/template-engine",
                "WalkingMate/template-engine",
            ),
            (
                "/Users/raine/code/workmux/.worktrees/uncommitted",
                "workmux/uncommitted",
            ),
            ("/code/proj/__worktrees/feat/src", "proj/feat"),
        ] {
            assert_eq!(format_short_name_from_path(Path::new(path)), expected);
        }
    }

    #[cfg(windows)]
    #[test]
    fn short_name_shows_worktrees_with_backslashes() {
        assert_eq!(
            format_short_name_from_path(Path::new(r"C:\code\app__worktrees\fix")),
            "app/fix"
        );
    }
}
