use anyhow::Result;
use console::style;
use dialoguer::Confirm;

use rumux_core::config::{
    detect_current_worktree, find_repo_root, sanitize_branch_name, worktree_path,
};
use rumux_core::errors::RumuxError;
use rumux_core::git_ops::{delete_branch, is_branch_merged, list_worktrees, remove_worktree};

pub fn run(branch: Option<&str>, all: bool, force: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_root = find_repo_root(&cwd)?;

    if all {
        return remove_all(&repo_root, force);
    }

    let sanitized = match branch {
        Some(b) => sanitize_branch_name(b),
        None => {
            // Detect current worktree
            detect_current_worktree(&cwd, &repo_root).ok_or(RumuxError::NotInWorktree)?
        }
    };

    let wt_path = worktree_path(&repo_root, &sanitized);
    if !wt_path.exists() {
        return Err(RumuxError::WorktreeNotFound(sanitized).into());
    }

    remove_single(&repo_root, &sanitized, &wt_path, force)?;

    Ok(())
}

fn remove_single(
    repo_root: &std::path::Path,
    name: &str,
    wt_path: &std::path::Path,
    force: bool,
) -> Result<()> {
    // Warn if user is inside the worktree
    if let Ok(cwd) = std::env::current_dir()
        && let Ok(canonical_wt) = std::fs::canonicalize(wt_path)
        && let Ok(canonical_cwd) = std::fs::canonicalize(&cwd)
        && canonical_cwd.starts_with(&canonical_wt)
    {
        eprintln!(
            "{}",
            style("Warning: You are inside the worktree being removed. Your shell's cwd will become invalid.")
                .yellow()
        );
    }

    // Remove worktree directory
    remove_worktree(repo_root, wt_path, name)?;
    eprintln!("{}", style(format!("Removed worktree '{name}'.")).green());

    // Delete branch
    let merged = is_branch_merged(repo_root, name)?;
    if merged || force {
        match delete_branch(repo_root, name) {
            Ok(()) => eprintln!("{}", style(format!("Deleted branch '{name}'.")).green()),
            Err(e) => eprintln!(
                "{}",
                style(format!("Note: Could not delete branch '{name}': {e}")).dim()
            ),
        }
    } else {
        eprintln!(
            "{}",
            style(format!(
                "Branch '{name}' has not been merged. Use --force to delete it anyway."
            ))
            .yellow()
        );
    }

    Ok(())
}

fn remove_all(repo_root: &std::path::Path, force: bool) -> Result<()> {
    let worktrees = list_worktrees(repo_root)?;

    if worktrees.is_empty() {
        eprintln!("{}", style("No worktrees to remove.").yellow());
        return Ok(());
    }

    let count = worktrees.len();
    let confirmed = Confirm::new()
        .with_prompt(format!("Remove all {count} worktrees?"))
        .default(false)
        .interact()?;

    if !confirmed {
        eprintln!("{}", style("Cancelled.").dim());
        return Ok(());
    }

    for wt in &worktrees {
        if let Err(e) = remove_single(repo_root, &wt.name, &wt.path, force) {
            eprintln!(
                "{}",
                style(format!("Failed to remove '{}': {e}", wt.name)).red()
            );
        }
    }

    Ok(())
}
