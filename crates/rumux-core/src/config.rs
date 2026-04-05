use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// Sanitize a branch name: replace `/` with `-`, strip leading/trailing hyphens,
/// collapse consecutive hyphens.
pub fn sanitize_branch_name(name: &str) -> String {
    let s = name.replace('/', "-");
    let s = s.trim_matches('-').to_string();
    let mut result = String::with_capacity(s.len());
    let mut prev_hyphen = false;
    for c in s.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push('-');
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    result
}

/// Find the root of the main git repository (not a worktree) starting from `start`.
pub fn find_repo_root(start: &Path) -> Result<PathBuf> {
    let repo = git2::Repository::discover(start)
        .context("Not inside a git repository. Run this command from within a git repo.")?;
    let common_dir = repo.commondir().to_path_buf();
    // commondir is the .git directory (or the common .git dir for worktrees)
    // The repo root is its parent
    let root = if common_dir.ends_with(".git") {
        common_dir
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or(common_dir)
    } else {
        // For bare repos or unusual layouts
        common_dir
    };
    Ok(std::fs::canonicalize(&root).unwrap_or(root))
}

/// Compute the worktree directory path for a given sanitized branch name.
pub fn worktree_path(repo_root: &Path, sanitized_branch: &str) -> PathBuf {
    repo_root.join(".worktrees").join(sanitized_branch)
}

/// Detect if the current directory is inside a rumux worktree.
/// Returns the sanitized branch name if so.
pub fn detect_current_worktree(cwd: &Path, repo_root: &Path) -> Option<String> {
    let worktrees_dir = repo_root.join(".worktrees");
    let cwd = std::fs::canonicalize(cwd).unwrap_or_else(|_| cwd.to_path_buf());
    let worktrees_dir =
        std::fs::canonicalize(&worktrees_dir).unwrap_or_else(|_| worktrees_dir.clone());

    if let Ok(relative) = cwd.strip_prefix(&worktrees_dir) {
        // The first component is the worktree name
        relative
            .components()
            .next()
            .map(|c| c.as_os_str().to_string_lossy().to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_simple() {
        assert_eq!(sanitize_branch_name("feature-foo"), "feature-foo");
    }

    #[test]
    fn test_sanitize_slashes() {
        assert_eq!(sanitize_branch_name("feature/foo"), "feature-foo");
    }

    #[test]
    fn test_sanitize_multiple_slashes() {
        assert_eq!(sanitize_branch_name("feature/foo/bar"), "feature-foo-bar");
    }

    #[test]
    fn test_sanitize_leading_trailing_hyphens() {
        assert_eq!(sanitize_branch_name("-feature-"), "feature");
    }

    #[test]
    fn test_sanitize_double_hyphens() {
        assert_eq!(sanitize_branch_name("feature--foo"), "feature-foo");
    }

    #[test]
    fn test_sanitize_slash_and_hyphens() {
        assert_eq!(sanitize_branch_name("-feature/foo-"), "feature-foo");
    }

    #[test]
    fn test_sanitize_dots() {
        assert_eq!(sanitize_branch_name("feature.foo"), "feature.foo");
    }

    #[test]
    fn test_sanitize_empty() {
        assert_eq!(sanitize_branch_name(""), "");
    }

    #[test]
    fn test_sanitize_only_hyphens() {
        assert_eq!(sanitize_branch_name("---"), "");
    }

    #[test]
    fn test_sanitize_unicode() {
        assert_eq!(sanitize_branch_name("feature/über"), "feature-über");
    }

    #[test]
    fn test_sanitize_leading_slash() {
        assert_eq!(sanitize_branch_name("/feature"), "feature");
    }

    #[test]
    fn test_find_repo_root() {
        let tmp = tempfile::tempdir().unwrap();
        git2::Repository::init(tmp.path()).unwrap();
        let root = find_repo_root(tmp.path()).unwrap();
        assert_eq!(std::fs::canonicalize(tmp.path()).unwrap(), root);
    }

    #[test]
    fn test_find_repo_root_from_subdir() {
        let tmp = tempfile::tempdir().unwrap();
        git2::Repository::init(tmp.path()).unwrap();
        let subdir = tmp.path().join("a").join("b");
        std::fs::create_dir_all(&subdir).unwrap();
        let root = find_repo_root(&subdir).unwrap();
        assert_eq!(std::fs::canonicalize(tmp.path()).unwrap(), root);
    }

    #[test]
    fn test_worktree_path() {
        let root = PathBuf::from("/repo");
        assert_eq!(
            worktree_path(&root, "feature-foo"),
            PathBuf::from("/repo/.worktrees/feature-foo")
        );
    }

    #[test]
    fn test_detect_current_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let wt_dir = root.join(".worktrees").join("my-branch");
        std::fs::create_dir_all(&wt_dir).unwrap();
        let result = detect_current_worktree(&wt_dir, root);
        assert_eq!(result, Some("my-branch".to_string()));
    }

    #[test]
    fn test_detect_current_worktree_not_in_worktree() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let result = detect_current_worktree(root, root);
        assert_eq!(result, None);
    }
}
