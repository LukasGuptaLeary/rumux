use anyhow::Result;
use console::style;

use rumux_core::config::{detect_current_worktree, find_repo_root, sanitize_branch_name, worktree_path};
use rumux_core::errors::RumuxError;
use rumux_core::git_ops::merge_branch;

pub fn run(branch: Option<&str>, squash: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let repo_root = find_repo_root(&cwd)?;

    let sanitized = match branch {
        Some(b) => sanitize_branch_name(b),
        None => {
            detect_current_worktree(&cwd, &repo_root)
                .ok_or(RumuxError::NotInWorktree)?
        }
    };

    // Verify worktree exists
    let wt_path = worktree_path(&repo_root, &sanitized);
    if !wt_path.exists() {
        return Err(RumuxError::WorktreeNotFound(sanitized).into());
    }

    eprintln!(
        "{}",
        style(format!("Merging '{sanitized}'...")).dim()
    );

    let result = merge_branch(&repo_root, &sanitized, squash)?;

    eprintln!("{}", style(&result).green());

    if squash {
        eprintln!(
            "\n{}",
            style("Remember to commit the staged changes.").yellow()
        );
    }

    Ok(())
}
